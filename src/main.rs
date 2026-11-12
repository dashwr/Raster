//! Solwired Raster: app desktop para o omp-dsh.

mod agents;
mod auth;
mod omp;
mod session;
mod store;
mod swarm;
mod ui;

use std::path::PathBuf;

use rune::{AppSpec, Cx, El, Pixels, TextEdit, Tx, col, el, row};

use agents::Agent;
use session::{Session, Status};

pub const MODELS: &[(&str, &str)] = &[
    ("anthropic/claude-opus-5-5", "Opus 5.5"),
    ("anthropic/claude-sonnet-5-5", "Sonnet 5.5"),
    ("anthropic/claude-haiku-5-5", "Haiku 5.5"),
    ("anthropic/claude-fable-5-1", "Fable 5.1"),
    ("openai-codex/gpt-6.1-sol", "GPT-6.1 Sol"),
    ("openai-codex/gpt-6-luna", "GPT-6 Luna"),
];

pub const EFFORTS: &[&str] = &["off", "minimal", "low", "medium", "high", "xhigh"];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum MenuTarget {
    Session(u64),
    Item(u64, usize),
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct Menu {
    pub x: f32,
    pub y: f32,
    pub target: MenuTarget,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum View {
    Chat,
    Configs,
}

/// Id da sessão utilitária (login e status de provedores), fora da lista.
pub const UTIL_ID: u64 = u64::MAX;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Popover {
    None,
    Model,
    Effort,
    Agent,
}

pub struct State {
    pub sessions: Vec<Session>,
    pub current: Option<u64>,
    pub next_id: u64,
    pub composer: TextEdit,
    pub search: TextEdit,
    pub ask_text: TextEdit,
    pub inbox: omp::Inbox,
    pub waker: Option<omp::Waker>,
    pub show_all: bool,
    pub new_agent: Agent,
    pub cwd: PathBuf,
    pub spin: usize,
    pub last_spin: f64,
    pub menu: Option<Menu>,
    pub popover: Popover,
    pub bg: Pixels,
    pub ripple: Option<(f32, f32, f64)>,
    pub clock: f64,
    pub logged_in: bool,
    pub preview: Option<String>,
    pub swarm: swarm::Swarm,
    pub view: View,
    pub util: Option<Session>,
    pub time: std::sync::Arc<std::sync::Mutex<(u32, String)>>,
}

impl State {
    fn new() -> State {
        let cwd = launch_dir();
        let mut s = State {
            sessions: Vec::new(),
            current: None,
            next_id: 1,
            composer: TextEdit::new(""),
            search: TextEdit::new(""),
            ask_text: TextEdit::new(""),
            inbox: omp::Inbox::default(),
            waker: None,
            show_all: true,
            new_agent: Agent::Code,
            cwd,
            spin: 0,
            last_spin: 0.0,
            menu: None,
            popover: Popover::None,
            bg: Pixels::new(),
            ripple: None,
            clock: 0.0,
            logged_in: auth::claude_logged_in(),
            preview: None,
            swarm: Default::default(),
            view: View::Chat,
            util: None,
            time: Default::default(),
        };
        store::load(&mut s);
        s.composer.focused = true;
        s
    }

    pub fn session(&self) -> Option<&Session> {
        self.sessions.iter().find(|x| Some(x.id) == self.current)
    }

    pub fn session_mut(&mut self) -> Option<&mut Session> {
        let id = self.current?;
        self.sessions.iter_mut().find(|x| x.id == id)
    }

    pub fn by_id(&mut self, id: u64) -> Option<&mut Session> {
        if id == UTIL_ID {
            return self.util.as_mut();
        }
        self.sessions.iter_mut().find(|x| x.id == id)
    }

    /// Manda um comando à sessão utilitária, subindo-a se preciso (modelo pequeno, sem prompt).
    pub fn util_send(&mut self, v: serde_json::Value) {
        let (inbox, waker) = (self.inbox.clone(), self.waker.clone().expect("waker"));
        let u = self.util.get_or_insert_with(|| Session::new(UTIL_ID, Agent::Search, self.cwd.clone()));
        if let Err(e) = u.ensure_proc(&inbox, &waker) {
            u.items.push(session::Item::Error(e));
            return;
        }
        let _ = u.send(v);
    }

    pub fn open_configs(&mut self) {
        self.view = View::Configs;
        self.util_send(serde_json::json!({"type": "get_login_providers"}));
    }

    /// Seleciona uma sessão; se já tinha histórico e está encerrada, reabre para redesenhar.
    pub fn select(&mut self, id: u64) {
        self.current = Some(id);
        self.view = View::Chat;
        self.menu = None;
        let (inbox, waker) = (self.inbox.clone(), self.waker.clone());
        if let (Some(x), Some(w)) = (self.by_id(id), waker)
            && x.items.len() <= 1
        {
            x.reopen(&inbox, &w);
        }
    }

    pub fn new_session(&mut self) {
        self.current = None;
        self.view = View::Chat;
        self.composer.clear();
        self.composer.focused = true;
    }

    /// Envia o texto do composer: cria a sessão se for a tela inicial; reabre se estiver encerrada.
    pub fn submit(&mut self) {
        let text = self.composer.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        self.composer.clear();
        if self.current.is_none() {
            let id = self.next_id;
            self.next_id += 1;
            self.sessions.push(Session::new(id, self.new_agent, self.cwd.clone()));
            self.current = Some(id);
        }
        let (inbox, waker) = (self.inbox.clone(), self.waker.clone().expect("waker"));
        if let Some(s) = self.session_mut() {
            s.prompt(text, &inbox, &waker);
        }
        store::save(self);
    }

    pub fn working(&self) -> bool {
        self.sessions.iter().any(|s| s.status == Status::Working)
    }
}

fn poll(s: &mut State, clock: f64, tx: &mut Tx) -> bool {
    s.clock = clock;
    let mut changed = false;
    for (sid, ev) in s.inbox.drain() {
        changed = true;
        if let Some(sess) = s.by_id(sid) {
            match ev {
                omp::Ev::Frame(v) => sess.on_frame(v),
                omp::Ev::Exited => sess.on_exit(),
            }
        }
    }
    let busy = s.sessions.iter().any(|x| matches!(x.status, Status::Working | Status::Asking));
    if busy {
        if clock - s.last_spin >= 0.08 {
            s.spin = s.spin.wrapping_add(1);
            s.last_spin = clock;
            changed = true;
        }
        tx.wake_after(std::time::Duration::from_millis(80));
    }
    if let Some((_, _, t0)) = s.ripple {
        if clock - t0 > 0.6 {
            s.ripple = None;
        }
        changed = true;
        tx.wake_after(std::time::Duration::from_millis(16));
    }
    changed
}

fn on_key(s: &mut State, k: &rune::KeyEvent, _tx: &mut Tx) -> bool {
    if k.is_command('n') {
        s.new_session();
        return true;
    }
    if matches!(k.key, rune::Key::Escape) {
        if s.menu.is_some() || s.popover != Popover::None {
            s.menu = None;
            s.popover = Popover::None;
            return true;
        }
        if let Some(x) = s.session_mut()
            && x.status == Status::Working
        {
            x.abort();
            return true;
        }
    }
    false
}

fn view(s: &State, cx: &mut Cx<State>) -> El {
    let p = ui::pal();
    let main = match (s.view, s.session()) {
        (View::Configs, _) => col().key("main").grow(1.0).h_full().min_w(0.0).fill(p.bg).child(ui::config::view(s)),
        (View::Chat, Some(sess)) => col().key("main").grow(1.0).h_full().min_w(0.0).fill(p.bg)
            .child(ui::transcript::view(s, sess, cx)).child(ui::composer::view(s)),
        (View::Chat, None) => col().key("main").grow(1.0).h_full().min_w(0.0).fill(p.bg)
            .child(ui::transcript::welcome(s)).child(ui::composer::view(s)),
    };
    let mut root = row().key("root").w_full().h_full().fill(p.frame)
        .child(ui::sidebar::view(s))
        .child(main)
        .child(ui::panel::view(s));
    if let Some(m) = s.menu {
        root = root.child(ui::sidebar::context_menu(m));
    }
    if s.popover != Popover::None {
        root = root.child(ui::composer::popover(s));
    }
    if let Some((x, y, t0)) = s.ripple {
        let k = ((s.clock - t0) / 0.6).clamp(0.0, 1.0) as f32;
        let r = 8.0 + 60.0 * k;
        root = root.child(
            el().key("ripple").absolute(x - r, y - r).size(r * 2.0, r * 2.0).radius(r)
                .border(1.0, p.text.alpha(0.5 * (1.0 - k))),
        );
    }
    root
}

fn setup(s: &mut State, waker: rune::Waker) {
    ui::transcript::tick_clock(s.time.clone(), waker.clone());
    swarm::watch(s.swarm.clone(), waker.clone());
    s.waker = Some(waker);
    ui::dither::animate(s.bg.clone(), 960, 600);
}

/// Pasta do projeto: o último argumento que é um diretório existente, senão a pasta atual.
fn launch_dir() -> PathBuf {
    let arg = std::env::args().skip(1).filter(|a| !a.starts_with('-')).map(PathBuf::from).rfind(|p| p.is_dir());
    let dir = arg.or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| PathBuf::from("."));
    dir.canonicalize().unwrap_or(dir)
}

/// `raster --probe "mensagem" [--agent search]`: sobe uma sessão sem UI, manda a mensagem e imprime a
/// transcrição quando ela termina. Teste ponta a ponta do runtime.
fn probe(msg: String) -> ! {
    let args: Vec<String> = std::env::args().collect();
    let agent = args.windows(2).find(|w| w[0] == "--agent").and_then(|w| agents::ALL.iter().copied().find(|a| a.name() == w[1])).unwrap_or(Agent::Search);
    let inbox = omp::Inbox::default();
    let waker: omp::Waker = std::sync::Arc::new(|| {});
    let mut sess = Session::new(0, agent, launch_dir());
    sess.thinking = "low".into();
    sess.prompt(msg, &inbox, &waker);
    let t0 = std::time::Instant::now();
    let mut saw_work = false;
    while t0.elapsed().as_secs() < 300 {
        for (_, ev) in inbox.drain() {
            match ev {
                omp::Ev::Frame(v) => sess.on_frame(v),
                omp::Ev::Exited => sess.on_exit(),
            }
        }
        saw_work |= sess.status == Status::Working;
        // RASTER_PROBE_ANSWER=1: responde perguntas com a primeira opção (testa a volta do ask)
        if sess.status == Status::Asking && std::env::var("RASTER_PROBE_ANSWER").is_ok_and(|v| v == "1") {
            let open = sess.items.iter().rposition(|i| matches!(i, session::Item::Ask { answer: None, .. }));
            if let Some(idx) = open {
                let resp = match &sess.items[idx] {
                    session::Item::Ask { kind: session::AskKind::Ask(qs), .. } => serde_json::json!({"answers": qs.iter().map(|q| serde_json::json!({"id": q.id, "selectedOptions": [q.options.first().map(|o| o.0.clone()).unwrap_or_default()]})).collect::<Vec<_>>()}),
                    session::Item::Ask { kind: session::AskKind::Select { options, .. }, .. } => serde_json::json!({"value": options.first().cloned().unwrap_or_default()}),
                    session::Item::Ask { kind: session::AskKind::Confirm { .. }, .. } => serde_json::json!({"confirmed": true}),
                    _ => serde_json::json!({"value": "ok"}),
                };
                sess.answer(idx, resp, "(probe: primeira opção)".into());
            }
            continue;
        }
        let subs_busy = sess.subs.iter().any(|x| x.status == session::SubStatus::Working);
        if sess.status == Status::Stopped || saw_work && (matches!(sess.status, Status::Stopped | Status::Asking) || (sess.status == Status::Idle && !subs_busy)) {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(100));
    }
    for it in &sess.items {
        println!("{}", ui::transcript::item_text(it));
    }
    println!("[status {:?}, subs {}, ctx {}]", sess.status, sess.subs.len(), sess.ctx_tokens);
    let ok = saw_work && sess.status == Status::Idle;
    sess.stop();
    std::process::exit(if ok { 0 } else { 1 });
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--probe") {
        probe(args.get(i + 1).cloned().unwrap_or_default());
    }
    rune::run(AppSpec::<State> {
        fonts: &[
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
            include_bytes!("../assets/fonts/JetBrainsMono-Bold.ttf"),
            include_bytes!("../assets/fonts/PixelifySans.ttf"),
        ],
        families: ("JetBrains Mono", "JetBrains Mono"),
        setup: Some(setup),
        poll: Some(poll),
        on_key: Some(on_key),
        scenes: &[("demo", store::demo)],
        min_size: Some((820.0, 520.0)),
        ..AppSpec::new("Raster", (1280.0, 800.0), State::new, view)
    });
}
