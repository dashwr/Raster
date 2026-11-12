//! Lista de sessões em `~/.raster/sessions.json` (metadados; o histórico vive nas sessões do omp).

use serde::{Deserialize, Serialize};

use crate::State;
use crate::agents::{ALL, Agent};
use crate::session::{AskKind, AskQ, Item, Session, Status, ToolCall};

#[derive(Serialize, Deserialize)]
struct Meta {
    id: u64,
    title: String,
    agent: String,
    model: String,
    thinking: String,
    cwd: String,
    started: bool,
}

/// Preferências do app (Settings), em `~/.raster/prefs.json`.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default)]
pub struct Prefs {
    pub sub_invoke: bool,
    pub max_parallel: u32,
    pub vault: bool,
    pub vault_nudge: bool,
    pub vault_first: bool,
    pub dcp_soft: u32,
    pub dcp_hard: u32,
    pub dcp_every: u32,
    pub dcp_dedup: bool,
    pub rtk: bool,
    pub ask_destructive: bool,
    pub ask_timeout: u32,
    pub dither: bool,
    pub fps: u32,
    pub font: u32,
    /// Modelo e effort padrão por agente (nome → (modelo, effort)); vazio usa o padrão do agente.
    pub agents: std::collections::BTreeMap<String, (String, String)>,
}

impl Default for Prefs {
    fn default() -> Self {
        Prefs {
            sub_invoke: true, max_parallel: 4, vault: true, vault_nudge: true, vault_first: true,
            dcp_soft: 50_000, dcp_hard: 100_000, dcp_every: 5, dcp_dedup: true,
            rtk: true, ask_destructive: true, ask_timeout: 120,
            dither: true, fps: 8, font: 13, agents: Default::default(),
        }
    }
}

impl Prefs {
    pub fn agent_defaults(&self, a: Agent) -> (String, String) {
        let (m, e) = self.agents.get(a.name()).cloned().unwrap_or_default();
        (if m.is_empty() { a.default_model().into() } else { m }, if e.is_empty() { "medium".into() } else { e })
    }
    fn path() -> std::path::PathBuf {
        crate::omp::state_dir().join("prefs.json")
    }
    pub fn load() -> Prefs {
        std::fs::read_to_string(Self::path()).ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default()
    }
    pub fn save(&self) {
        let _ = std::fs::create_dir_all(crate::omp::state_dir());
        if let Ok(j) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(Self::path(), j);
        }
    }
}

fn path() -> std::path::PathBuf {
    crate::omp::state_dir().join("sessions.json")
}

pub fn save(s: &State) {
    let metas: Vec<Meta> = s.sessions.iter().map(|x| Meta {
        id: x.id,
        title: x.title.clone(),
        agent: x.agent.name().into(),
        model: x.model.clone(),
        thinking: x.thinking.clone(),
        cwd: x.cwd.display().to_string(),
        started: x.started,
    }).collect();
    let _ = std::fs::create_dir_all(crate::omp::state_dir());
    if let Ok(j) = serde_json::to_string_pretty(&metas) {
        let _ = std::fs::write(path(), j);
    }
    // para o list_agents(scope: sessions) da extensão
    let peers: Vec<_> = s.sessions.iter().map(|x| serde_json::json!({
        "agent_id": format!("session-{}", x.id),
        "title": x.title,
        "agent": x.agent.name(),
        "status": format!("{:?}", x.status).to_lowercase(),
    })).collect();
    let dir = crate::omp::state_dir().join("ompdsh");
    let _ = std::fs::create_dir_all(&dir);
    let _ = std::fs::write(dir.join("raster-sessions.json"), serde_json::Value::Array(peers).to_string());
}

pub fn load(s: &mut State) {
    let Ok(raw) = std::fs::read_to_string(path()) else { return };
    let Ok(metas) = serde_json::from_str::<Vec<Meta>>(&raw) else { return };
    for m in metas {
        let agent = ALL.iter().copied().find(|a| a.name() == m.agent).unwrap_or(Agent::Code);
        let mut x = Session::new(m.id, agent, m.cwd.into());
        x.title = m.title;
        x.model = m.model;
        x.thinking = if crate::EFFORTS.contains(&m.thinking.as_str()) { m.thinking } else { "medium".into() };
        x.started = m.started;
        if x.started {
            x.items.push(Item::Notice("history kept in omp; send a message to resume".into()));
        }
        s.next_id = s.next_id.max(m.id + 1);
        s.sessions.push(x);
    }
}

/// Cena `demo` para snapshots headless (`raster --snap out.png --scene demo`).
pub fn demo(s: &mut State) {
    s.sessions.clear();
    let mut a = Session::new(1, Agent::Code, "/home/user/projects/raster".into());
    a.title = "montar o composer".into();
    a.status = Status::Working;
    a.ctx_tokens = 84_000;
    a.items = vec![
        Item::User("cria o composer com seletor de modelo e effort".into()),
        Item::Assistant {
            mid: "m1".into(),
            text: "Vou ler o layout atual e montar o composer.\n\n| peça | estado |\n|---|---|\n| modelo | ok |\n| effort | falta |\n\n```rust\nfn view(s: &State) -> El { col() }\n```".into(),
            thinking: String::new(),
            streaming: false,
        },
        Item::Tools {
            calls: vec![
                ToolCall { id: "t1".into(), name: "read".into(), summary: "src/ui/composer.rs".into(), output: String::new(), done: true, error: false },
                ToolCall { id: "t2".into(), name: "bash".into(), summary: "cargo check".into(), output: "Finished dev".into(), done: false, error: false },
            ],
            open: true,
        },
        Item::Ask {
            req: "q1".into(),
            kind: AskKind::Ask(vec![AskQ {
                id: "a".into(),
                question: "Onde fica o seletor de effort?".into(),
                options: vec![("card com slider".into(), "abre acima do composer".into()), ("menu".into(), "lista simples".into())],
                multi: false,
                picked: vec![0],
            }]),
            answer: None,
        },
    ];
    let mut b = Session::new(2, Agent::Search, "/home/user".into());
    b.title = "preço da rtx 5070".into();
    b.status = Status::Asking;
    let mut c = Session::new(3, Agent::Test, "/home/user/projects/raster".into());
    c.title = "e2e do login".into();
    c.status = Status::Idle;
    s.sessions = vec![a, b, c];
    s.current = Some(1);
    s.decode_t0 = -10.0;
}

pub fn demo_model(s: &mut State) {
    demo(s);
    s.popover = crate::Popover::Model;
}

pub fn demo_ctx(s: &mut State) {
    demo(s);
    s.popover = crate::Popover::Context;
}

pub fn demo_menu(s: &mut State) {
    demo(s);
    s.menu = Some(crate::Menu { x: 120.0, y: 220.0, target: crate::MenuTarget::Session(1) });
}

pub fn demo_settings(s: &mut State) {
    demo(s);
    s.view = crate::View::Configs;
}
