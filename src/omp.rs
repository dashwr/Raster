//! Processo `omp --mode rpc` com a extensão dsh: uma sessão = um processo.
//! Uma thread lê stdout linha a linha (JSON) e empurra eventos para a fila da UI.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::{Arc, Mutex};

use serde_json::Value;

pub type Waker = rune::Waker;

pub enum Ev {
    Frame(Value),
    Exited,
}

#[derive(Clone, Default)]
pub struct Inbox(Arc<Mutex<Vec<(u64, Ev)>>>);

impl Inbox {
    pub fn push(&self, sid: u64, ev: Ev) {
        self.0.lock().unwrap().push((sid, ev));
    }
    pub fn drain(&self) -> Vec<(u64, Ev)> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

pub struct Spawn<'a> {
    pub sid: u64,
    pub cwd: &'a Path,
    pub model: &'a str,
    pub thinking: &'a str,
    pub prompt_files: Vec<PathBuf>,
    pub session_dir: PathBuf,
    pub resume: bool,
}

pub struct Proc {
    child: Child,
    stdin: ChildStdin,
}

impl Proc {
    pub fn send(&mut self, v: &Value) -> std::io::Result<()> {
        let mut line = serde_json::to_string(v)?;
        line.push('\n');
        self.stdin.write_all(line.as_bytes())?;
        self.stdin.flush()
    }

    /// Encerra a sessão: fecha o stdin e mata o processo dela (só o filho que o Raster subiu).
    pub fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub fn runtime_dir() -> PathBuf {
    if let Ok(d) = std::env::var("RASTER_RUNTIME") {
        return d.into();
    }
    // ao lado do executável ou em um ancestral (target/release/raster → raiz do repo)
    let exe = std::env::current_exe().ok();
    exe.as_ref()
        .and_then(|e| e.ancestors().skip(1).map(|d| d.join("runtime")).find(|d| d.join("dsh/dsh.ts").exists()))
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("runtime"))
}

pub fn state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home).join(".raster")
}

pub fn spawn(s: Spawn, inbox: Inbox, waker: Waker) -> std::io::Result<Proc> {
    let rt = runtime_dir();
    let omp = rt.join("node_modules/.bin/omp");
    let st = state_dir();
    std::fs::create_dir_all(st.join("agent"))?;
    std::fs::create_dir_all(st.join("ompdsh"))?;
    std::fs::create_dir_all(&s.session_dir)?;
    let prompts: Vec<String> = s.prompt_files.iter().map(|p| p.display().to_string()).collect();

    let prefs = crate::store::Prefs::load();
    let flag = |b: bool| if b { "1" } else { "0" };
    let mut cmd = Command::new(&omp);
    cmd.args(["--mode", "rpc", "--no-title", "--model", s.model, "--thinking", s.thinking, "--auto-approve"])
        .args(["--no-tools", "--no-context-files", "--no-skills", "--no-rules"])
        .arg("-e")
        .arg(rt.join("dsh/dsh.ts"))
        .arg("--session-dir")
        .arg(&s.session_dir)
        .current_dir(s.cwd)
        .env("PI_CODING_AGENT_DIR", st.join("agent"))
        .env("OMPDSH_ROOT", st.join("ompdsh"))
        .env("OMPDSH_MODEL", s.model)
        .env("OMPDSH_AGENT_ID", format!("session-{}", s.sid))
        .env("RASTER_OMP", &omp)
        .env("RASTER_KEEPALIVE", "1")
        .env("RASTER_VAULT_NUDGE", flag(prefs.vault_nudge))
        .env("RASTER_DCP_MIN", prefs.dcp_soft.to_string())
        .env("RASTER_DCP_MAX", prefs.dcp_hard.to_string())
        .env("RASTER_DCP_FREQ", prefs.dcp_every.to_string())
        .env("RASTER_DCP_DEDUP", flag(prefs.dcp_dedup))
        .env("RASTER_VAULT", flag(prefs.vault))
        .env("RASTER_VAULT_FIRST", flag(prefs.vault_first))
        .env("RASTER_RTK", flag(prefs.rtk))
        .env("RASTER_ASK_DESTRUCTIVE", flag(prefs.ask_destructive))
        .env("RASTER_ASK_TIMEOUT", prefs.ask_timeout.to_string())
        .env("RASTER_MAX_PARALLEL", prefs.max_parallel.to_string())
        .env("RASTER_MAX_DEPTH", if prefs.sub_invoke { "2" } else { "1" })
        .env("RASTER_PROMPT_FILES", prompts.join(":"))
        .env("RASTER_AGENTS_DIR", crate::agents::agents_dir())
        .env("RASTER_CENTER", st.join("Center.md"))
        .env("RASTER_AGENT_MODELS", crate::agents::default_models_json())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());
    if s.resume {
        cmd.arg("--continue");
    }
    if let Some(tok) = crate::auth::claude_access_token() {
        cmd.env("ANTHROPIC_OAUTH_TOKEN", tok);
    }
    let mut child = cmd.spawn()?;
    let stdin = child.stdin.take().expect("stdin");
    let stdout = child.stdout.take().expect("stdout");
    let sid = s.sid;
    std::thread::spawn(move || {
        for line in BufReader::new(stdout).lines() {
            let Ok(line) = line else { break };
            if let Ok(v) = serde_json::from_str::<Value>(&line) {
                inbox.push(sid, Ev::Frame(v));
                waker();
            }
        }
        inbox.push(sid, Ev::Exited);
        waker();
    });
    Ok(Proc { child, stdin })
}
