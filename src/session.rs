//! Estado de uma sessão e tradução dos frames do RPC do omp em itens da transcrição.

use std::path::PathBuf;

use serde_json::{Value, json};

use crate::agents::Agent;
use crate::omp::{self, Inbox, Proc, Waker};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Idle,
    Working,
    Asking,
    Stopped,
}

#[derive(Clone, Debug)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub summary: String,
    pub output: String,
    pub done: bool,
    pub error: bool,
}

#[derive(Clone, Debug)]
pub struct AskQ {
    pub id: String,
    pub question: String,
    pub options: Vec<(String, String)>,
    pub multi: bool,
    pub picked: Vec<usize>,
}

#[derive(Clone, Debug)]
pub enum AskKind {
    Ask(Vec<AskQ>),
    Select { title: String, options: Vec<String> },
    Confirm { title: String, message: String },
    Input { title: String },
}

#[derive(Clone, Debug)]
pub enum Item {
    User(String),
    Assistant { mid: String, text: String, thinking: String, streaming: bool },
    Tools { calls: Vec<ToolCall>, open: bool },
    Ask { req: String, kind: AskKind, answer: Option<String> },
    Notice(String),
    Error(String),
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SubStatus {
    Working,
    Idle,
}

#[derive(Clone, Debug)]
pub struct Sub {
    pub id: String,
    pub label: String,
    pub status: SubStatus,
}

pub struct Session {
    pub id: u64,
    pub title: String,
    pub agent: Agent,
    pub model: String,
    pub thinking: String,
    pub cwd: PathBuf,
    pub items: Vec<Item>,
    pub status: Status,
    pub proc: Option<Proc>,
    pub ctx_tokens: u64,
    pub ctx_window: u64,
    pub started: bool,
    pub seq: u64,
    pub subs: Vec<Sub>,
    pub pending_labels: Vec<(String, String)>,
    /// Provedores de login (id, nome, autenticado), da resposta a `get_login_providers`.
    pub providers: Vec<(String, String, bool)>,
}

impl Session {
    pub fn new(id: u64, agent: Agent, cwd: PathBuf) -> Session {
        Session {
            id,
            title: format!("sessão {id}"),
            agent,
            model: agent.default_model().into(),
            thinking: "medium".into(),
            cwd,
            items: Vec::new(),
            status: Status::Stopped,
            proc: None,
            ctx_tokens: 0,
            ctx_window: 200_000,
            started: false,
            seq: 0,
            subs: Vec::new(),
            pending_labels: Vec::new(),
            providers: Vec::new(),
        }
    }

    pub fn session_dir(&self) -> PathBuf {
        omp::state_dir().join("sessions").join(self.id.to_string())
    }

    pub fn ensure_proc(&mut self, inbox: &Inbox, waker: &Waker) -> Result<(), String> {
        if self.proc.is_some() {
            return Ok(());
        }
        let prompt_files = vec![self.agent.prompt_file(), crate::agents::center_file()];
        let p = omp::spawn(
            omp::Spawn {
                sid: self.id,
                cwd: &self.cwd,
                model: &self.model,
                thinking: &self.thinking,
                prompt_files,
                session_dir: self.session_dir(),
                resume: self.started,
            },
            inbox.clone(),
            waker.clone(),
        )
        .map_err(|e| format!("não subiu o omp: {e}"))?;
        self.proc = Some(p);
        self.status = Status::Idle;
        let _ = self.send(json!({"type": "set_ask_dialog", "enabled": true}));
        Ok(())
    }

    pub fn send(&mut self, mut v: Value) -> Result<(), String> {
        self.seq += 1;
        v["id"] = json!(format!("r{}", self.seq));
        let p = self.proc.as_mut().ok_or("sessão encerrada")?;
        p.send(&v).map_err(|e| e.to_string())
    }

    pub fn prompt(&mut self, text: String, inbox: &Inbox, waker: &Waker) {
        if self.title.starts_with("sessão ") && !text.trim().is_empty() {
            self.title = text.trim().lines().next().unwrap_or("").chars().take(40).collect();
        }
        self.items.push(Item::User(text.clone()));
        if let Err(e) = self.ensure_proc(inbox, waker) {
            self.items.push(Item::Error(e));
            return;
        }
        self.started = true;
        let behaviour = if self.status == Status::Working { "steer" } else { "prompt" };
        let msg = if behaviour == "steer" {
            json!({"type": "steer", "message": text})
        } else {
            json!({"type": "prompt", "message": text})
        };
        if let Err(e) = self.send(msg) {
            self.items.push(Item::Error(e));
        } else {
            self.status = Status::Working;
        }
    }

    pub fn abort(&mut self) {
        let _ = self.send(json!({"type": "abort"}));
    }

    /// Mata o processo; a próxima mensagem re-sobe com `--continue`.
    pub fn stop(&mut self) {
        if let Some(p) = self.proc.take() {
            p.stop();
        }
        self.status = Status::Stopped;
        self.items.push(Item::Notice("sessão encerrada; mande uma mensagem para reabrir".into()));
    }

    pub fn set_model(&mut self, model: &str) {
        self.model = model.into();
        if let Some((provider, id)) = model.split_once('/') {
            let _ = self.send(json!({"type": "set_model", "provider": provider, "modelId": id}));
        }
    }

    pub fn set_thinking(&mut self, level: &str) {
        self.thinking = level.into();
        let _ = self.send(json!({"type": "set_thinking_level", "level": level}));
    }

    pub fn answer(&mut self, idx: usize, response: Value, shown: String) {
        if let Some(Item::Ask { req, answer, .. }) = self.items.get_mut(idx) {
            let mut r = response;
            r["type"] = json!("extension_ui_response");
            r["id"] = json!(req.clone());
            *answer = Some(shown);
            if let Some(p) = self.proc.as_mut() {
                let _ = p.send(&r);
            }
        }
        if !self.items.iter().any(|i| matches!(i, Item::Ask { answer: None, .. })) && self.status == Status::Asking {
            self.status = Status::Working;
        }
    }

    pub fn on_exit(&mut self) {
        if self.proc.take().is_some() {
            self.status = Status::Stopped;
            self.items.push(Item::Notice("o processo da sessão saiu".into()));
        }
    }

    pub fn on_frame(&mut self, v: Value) {
        let ty = v["type"].as_str().unwrap_or("");
        match ty {
            "agent_start" => self.status = Status::Working,
            "agent_end" | "session_settled" => {
                if self.status != Status::Asking {
                    self.status = Status::Idle;
                }
                for it in &mut self.items {
                    if let Item::Assistant { streaming, .. } = it {
                        *streaming = false;
                    }
                }
            }
            "message_start" | "message_update" | "message_end" => self.on_message(&v, ty == "message_end"),
            "tool_execution_start" => {
                if matches!(v["toolName"].as_str(), Some("subagent" | "subagent_fork")) {
                    let a = &v["args"];
                    let label = a["description"].as_str().or(a["name"].as_str()).or(a["prompt"].as_str()).unwrap_or("subagente");
                    self.pending_labels.push((v["toolCallId"].as_str().unwrap_or("").into(), label.chars().take(40).collect()));
                }
                let call = ToolCall {
                    id: v["toolCallId"].as_str().unwrap_or("").into(),
                    name: v["toolName"].as_str().unwrap_or("").into(),
                    summary: summarize(&v["args"]),
                    output: String::new(),
                    done: false,
                    error: false,
                };
                match self.items.last_mut() {
                    Some(Item::Tools { calls, .. }) => calls.push(call),
                    _ => self.items.push(Item::Tools { calls: vec![call], open: false }),
                }
            }
            "tool_execution_end" => {
                let id = v["toolCallId"].as_str().unwrap_or("");
                for it in self.items.iter_mut().rev() {
                    if let Item::Tools { calls, .. } = it
                        && let Some(c) = calls.iter_mut().find(|c| c.id == id)
                    {
                        c.done = true;
                        c.error = v["isError"].as_bool().unwrap_or(false);
                        c.output = result_text(&v["result"]);
                        if let Some(sid) = c.output.trim().strip_prefix("started subagent ") {
                            let label = self.pending_labels.iter().find(|(t, _)| t == id).map(|x| x.1.clone()).unwrap_or_default();
                            self.subs.push(Sub { id: sid.trim().into(), label, status: SubStatus::Working });
                        }
                        break;
                    }
                }
            }
            "extension_ui_request" => self.on_ui(&v),
            "prompt_result" => {
                if v["status"] == "error" {
                    let m = v["error"]["message"].as_str().unwrap_or("erro").to_string();
                    self.items.push(Item::Error(m));
                }
            }
            "response" if v["command"] == "get_login_providers" && v["success"] == true => {
                self.providers = v["data"]["providers"].as_array().into_iter().flatten()
                    .map(|p| (p["id"].as_str().unwrap_or("").into(), p["name"].as_str().unwrap_or("").into(), p["authenticated"].as_bool().unwrap_or(false)))
                    .collect();
            }
            "response" if v["command"] == "login" && v["success"] == true => {
                self.items.push(Item::Notice(format!("Signed in: {}", v["data"]["providerId"].as_str().unwrap_or(""))));
                let _ = self.send(json!({"type": "get_login_providers"}));
            }
            "response" if v["command"] == "get_messages" && v["success"] == true => self.restore(&v["data"]["messages"]),
            "response" if v["success"] == false => {
                let m = v["error"].as_str().unwrap_or("comando recusado").to_string();
                self.items.push(Item::Error(m));
            }
            _ => {}
        }
    }

    fn on_message(&mut self, v: &Value, end: bool) {
        let m = &v["message"];
        if m["role"] == "user" && end {
            let t = flat_text(&m["content"]);
            if is_injected(&t) {
                self.on_injected(&t);
            }
            return;
        }
        if m["role"] != "assistant" {
            return;
        }
        let mid = v["messageId"].as_str().unwrap_or("").to_string();
        let mut text = String::new();
        let mut thinking = String::new();
        for b in m["content"].as_array().into_iter().flatten() {
            match b["type"].as_str() {
                Some("text") => text.push_str(b["text"].as_str().unwrap_or("")),
                Some("thinking") => thinking.push_str(b["thinking"].as_str().unwrap_or("")),
                _ => {}
            }
        }
        if end {
            if let Some(e) = m["errorMessage"].as_str()
                && m["stopReason"] != "aborted"
            {
                self.items.push(Item::Error(e.into()));
            }
            let u = &m["usage"];
            let total = ["input", "cacheRead", "cacheWrite", "output"].iter().map(|k| u[*k].as_u64().unwrap_or(0)).sum::<u64>();
            if total > 0 {
                self.ctx_tokens = total;
            }
        }
        if text.is_empty() && thinking.is_empty() {
            return;
        }
        let pos = self.items.iter().rposition(|i| matches!(i, Item::Assistant { mid: x, .. } if *x == mid));
        let item = Item::Assistant { mid, text, thinking, streaming: !end };
        match pos {
            Some(i) => self.items[i] = item,
            None => self.items.push(item),
        }
    }

    fn on_ui(&mut self, v: &Value) {
        let req = v["id"].as_str().unwrap_or("").to_string();
        let s = |k: &str| v[k].as_str().unwrap_or("").to_string();
        let kind = match v["method"].as_str().unwrap_or("") {
            "ask" => AskKind::Ask(
                v["questions"].as_array().into_iter().flatten().map(|q| AskQ {
                    id: q["id"].as_str().unwrap_or("").into(),
                    question: q["question"].as_str().unwrap_or("").into(),
                    options: q["options"].as_array().into_iter().flatten()
                        .map(|o| (o["label"].as_str().unwrap_or("").into(), o["description"].as_str().unwrap_or("").into()))
                        .collect(),
                    multi: q["multi"].as_bool().unwrap_or(false),
                    picked: q["recommended"].as_u64().map(|r| vec![r as usize]).unwrap_or_default(),
                }).collect(),
            ),
            "select" => AskKind::Select {
                title: s("title"),
                options: v["options"].as_array().into_iter().flatten().filter_map(|o| o.as_str().map(String::from)).collect(),
            },
            "confirm" => AskKind::Confirm { title: s("title"), message: s("message") },
            "input" | "editor" => AskKind::Input { title: s("title") },
            "cancel" => {
                let target = s("targetId");
                for it in &mut self.items {
                    if let Item::Ask { req, answer, .. } = it
                        && *req == target
                        && answer.is_none()
                    {
                        *answer = Some("(cancelado)".into());
                    }
                }
                return;
            }
            "open_url" => {
                let url = s("url");
                let _ = std::process::Command::new("xdg-open").arg(&url).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::null()).spawn();
                self.items.push(Item::Notice(format!("abri no navegador: {url}")));
                if let Some(i) = v["instructions"].as_str() {
                    self.items.push(Item::Notice(i.to_string()));
                }
                return;
            }
            "notify" => {
                self.items.push(Item::Notice(s("message")));
                return;
            }
            _ => return,
        };
        self.items.push(Item::Ask { req, kind, answer: None });
        self.status = Status::Asking;
    }

    /// Mensagens que a extensão injeta como usuário (avisos de subagente, mensagens entre sessões).
    fn on_injected(&mut self, t: &str) {
        if t.is_empty() {
            return;
        }
        for sub in &mut self.subs {
            if t.contains(&format!("subagent {} finished", sub.id)) || t.contains(&format!("subagent {} stopped", sub.id)) {
                sub.status = SubStatus::Idle;
            }
        }
        let first = t.lines().next().unwrap_or("").chars().take(160).collect::<String>();
        self.items.push(Item::Notice(first));
    }

    /// Reconstrói a transcrição a partir de `get_messages` (sessão retomada).
    fn restore(&mut self, msgs: &Value) {
        let Some(list) = msgs.as_array() else { return };
        let mut items = Vec::new();
        for (i, m) in list.iter().enumerate() {
            match m["role"].as_str() {
                Some("user") => {
                    let t = flat_text(&m["content"]);
                    if is_injected(&t) {
                        items.push(Item::Notice(t.lines().next().unwrap_or("").chars().take(160).collect()));
                    } else if !t.is_empty() {
                        items.push(Item::User(t));
                    }
                }
                Some("assistant") => {
                    let mut text = String::new();
                    let mut calls = Vec::new();
                    for b in m["content"].as_array().into_iter().flatten() {
                        match b["type"].as_str() {
                            Some("text") => text.push_str(b["text"].as_str().unwrap_or("")),
                            Some("toolCall") => calls.push(ToolCall {
                                id: b["id"].as_str().unwrap_or("").into(),
                                name: b["name"].as_str().unwrap_or("").into(),
                                summary: summarize(&b["arguments"]),
                                output: String::new(),
                                done: true,
                                error: false,
                            }),
                            _ => {}
                        }
                    }
                    if !text.is_empty() {
                        items.push(Item::Assistant { mid: format!("r{i}"), text, thinking: String::new(), streaming: false });
                    }
                    if !calls.is_empty() {
                        match items.last_mut() {
                            Some(Item::Tools { calls: c, .. }) => c.extend(calls),
                            _ => items.push(Item::Tools { calls, open: false }),
                        }
                    }
                }
                Some("toolResult") => {
                    let id = m["toolCallId"].as_str().unwrap_or("");
                    if let Some(Item::Tools { calls, .. }) = items.iter_mut().rev().find(|x| matches!(x, Item::Tools { .. }))
                        && let Some(c) = calls.iter_mut().find(|c| c.id == id)
                    {
                        c.output = result_text(m);
                        c.error = m["isError"].as_bool().unwrap_or(false);
                    }
                }
                _ => {}
            }
        }
        if !items.is_empty() {
            self.items = items;
        }
    }

    /// Abre o processo de uma sessão já iniciada e pede o histórico para redesenhar.
    pub fn reopen(&mut self, inbox: &Inbox, waker: &Waker) {
        if self.proc.is_some() || !self.started {
            return;
        }
        if self.ensure_proc(inbox, waker).is_ok() {
            let _ = self.send(json!({"type": "get_messages"}));
        }
    }

    pub fn tool_count(&self) -> usize {
        self.items.iter().map(|i| if let Item::Tools { calls, .. } = i { calls.len() } else { 0 }).sum()
    }
}

/// Avisos que a extensão dsh entrega como mensagem de usuário (texto fixo do dsh).
fn is_injected(t: &str) -> bool {
    t.starts_with("Background subagent ") || (t.starts_with("Agent ") && t.contains(" sent a message:")) || t.starts_with("Continue working toward the active goal")
}

fn flat_text(c: &Value) -> String {
    if let Some(s) = c.as_str() {
        return s.to_string();
    }
    c.as_array().into_iter().flatten().filter_map(|b| b["text"].as_str()).collect::<Vec<_>>().join("\n")
}

fn summarize(args: &Value) -> String {
    for k in ["command", "description", "path", "file_path", "pattern", "url", "query", "prompt", "task"] {
        if let Some(s) = args[k].as_str() {
            return s.lines().next().unwrap_or("").chars().take(120).collect();
        }
    }
    String::new()
}

fn result_text(r: &Value) -> String {
    let mut out = String::new();
    for b in r["content"].as_array().into_iter().flatten() {
        if let Some(t) = b["text"].as_str() {
            out.push_str(t);
        }
    }
    if out.is_empty() && let Some(t) = r.as_str() {
        out.push_str(t);
    }
    let n = out.chars().count();
    if n > 4000 { out.chars().skip(n - 4000).collect() } else { out }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sess() -> Session {
        Session::new(1, Agent::Code, "/tmp".into())
    }

    #[test]
    fn streaming_assistant_text_updates_one_item_then_settles() {
        let mut s = sess();
        s.on_frame(json!({"type": "agent_start"}));
        let msg = |t: &str| json!({"role": "assistant", "content": [{"type": "text", "text": t}]});
        s.on_frame(json!({"type": "message_update", "messageId": "m1", "message": msg("Ol")}));
        s.on_frame(json!({"type": "message_end", "messageId": "m1", "message": {"role": "assistant", "content": [{"type": "text", "text": "Olá"}], "usage": {"input": 10, "output": 5}}}));
        s.on_frame(json!({"type": "agent_end"}));
        assert_eq!(s.items.len(), 1);
        assert!(matches!(&s.items[0], Item::Assistant { text, streaming: false, .. } if text == "Olá"));
        assert_eq!(s.status, Status::Idle);
        assert_eq!(s.ctx_tokens, 15);
    }

    #[test]
    fn tool_calls_group_and_subagent_goes_from_working_to_idle() {
        let mut s = sess();
        s.on_frame(json!({"type": "tool_execution_start", "toolCallId": "t1", "toolName": "subagent", "args": {"description": "contar arquivos"}}));
        s.on_frame(json!({"type": "tool_execution_start", "toolCallId": "t2", "toolName": "bash", "args": {"command": "ls"}}));
        s.on_frame(json!({"type": "tool_execution_end", "toolCallId": "t1", "toolName": "subagent", "result": {"content": [{"type": "text", "text": "started subagent abc-1"}]}}));
        assert!(matches!(&s.items[0], Item::Tools { calls, .. } if calls.len() == 2 && calls[0].done && !calls[1].done));
        assert_eq!(s.subs.len(), 1);
        assert_eq!(s.subs[0].label, "contar arquivos");
        assert_eq!(s.subs[0].status, SubStatus::Working);
        let user = |t: &str| json!({"type": "message_end", "message": {"role": "user", "attribution": "user", "content": [{"type": "text", "text": t}]}});
        s.on_frame(user("Agent abc-1 sent a message: 12"));
        s.on_frame(user("Background subagent abc-1 finished and will do no further work unless you send it more."));
        assert_eq!(s.subs[0].status, SubStatus::Idle);
        s.on_frame(user("pergunta normal"));
        assert_eq!(s.items.iter().filter(|i| matches!(i, Item::Notice(_))).count(), 2);
    }

    #[test]
    fn ask_request_sets_asking_and_cancel_closes_it() {
        let mut s = sess();
        s.on_frame(json!({"type": "extension_ui_request", "id": "u1", "method": "ask", "questions": [
            {"id": "q", "question": "qual?", "options": [{"label": "a"}, {"label": "b", "description": "bê"}], "recommended": 1}
        ]}));
        assert_eq!(s.status, Status::Asking);
        assert!(matches!(&s.items[0], Item::Ask { kind: AskKind::Ask(qs), answer: None, .. } if qs[0].picked == vec![1] && qs[0].options[1].1 == "bê"));
        s.on_frame(json!({"type": "extension_ui_request", "id": "u2", "method": "cancel", "targetId": "u1"}));
        assert!(matches!(&s.items[0], Item::Ask { answer: Some(_), .. }));
    }

    #[test]
    fn error_prompt_result_becomes_error_item() {
        let mut s = sess();
        s.on_frame(json!({"type": "prompt_result", "status": "error", "error": {"message": "401", "retryable": false}}));
        assert!(matches!(&s.items[0], Item::Error(m) if m == "401"));
    }
}
