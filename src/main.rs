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

pub const EFFORTS: &[&str] = &["low", "medium", "high", "xhigh", "max"];

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
    Agent,
    Plus,
    Context,
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
    pub clock: f64,
    pub logged_in: bool,
    pub preview: Option<String>,
    pub swarm: swarm::Swarm,
    pub view: View,
    pub util: Option<Session>,
    pub time: std::sync::Arc<std::sync::Mutex<(u32, String)>>,
    pub renaming: Option<u64>,
    pub rename: TextEdit,
    /// Início da decodificação da tela (troca, abertura ou nova conversa).
    pub decode_t0: f64,
    pub attachments: Vec<PathBuf>,
    pub prefs: store::Prefs,
    /// Quadros por segundo do campo de fundo (0 = parado), lido pela thread do dither.
    pub fps: std::sync::Arc<std::sync::atomic::AtomicU32>,
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
            clock: 0.0,
            logged_in: auth::claude_logged_in(),
            preview: None,
            swarm: Default::default(),
            view: View::Chat,
            util: None,
            time: Default::default(),
            renaming: None,
            rename: TextEdit::new(""),
            decode_t0: 0.0,
            attachments: Vec::new(),
            prefs: store::Prefs::load(),
            fps: Default::default(),
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
        if self.current != Some(id) {
            self.decode_t0 = self.clock;
        }
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

    /// Aplica as preferências que vivem na UI (as do runtime vão por env no spawn).
    pub fn sync_prefs(&mut self) {
        let f = if self.prefs.dither { self.prefs.fps } else { 0 };
        self.fps.store(f, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn start_rename(&mut self, id: u64) {
        let t = self.by_id(id).map(|x| x.title.clone()).unwrap_or_default();
        self.rename = TextEdit::new(&t);
        self.rename.focused = true;
        self.renaming = Some(id);
    }

    pub fn commit_rename(&mut self) {
        let t = self.rename.text.trim().to_string();
        if let Some(id) = self.renaming.take()
            && !t.is_empty()
            && let Some(x) = self.by_id(id)
        {
            x.title = t;
            store::save(self);
        }
    }

    /// Modelos de provedores com conexão ativa: Claude pelo login local, o resto pelo omp.
    pub fn models(&self) -> Vec<(&'static str, &'static str)> {
        let providers = self.util.as_ref().map(|u| u.providers.clone()).unwrap_or_default();
        MODELS.iter().copied().filter(|(id, _)| {
            let prov = id.split('/').next().unwrap_or("");
            (prov == "anthropic" && self.logged_in) || providers.iter().any(|p| p.0 == prov && p.2)
        }).collect()
    }

    pub fn new_session(&mut self) {
        self.decode_t0 = self.clock;
        self.current = None;
        self.view = View::Chat;
        self.composer.clear();
        self.composer.focused = true;
    }

    /// Envia o texto do composer: cria a sessão se for a tela inicial; reabre se estiver encerrada.
    pub fn submit(&mut self) {
        let mut text = self.composer.text.trim().to_string();
        if text.is_empty() {
            return;
        }
        for a in self.attachments.drain(..) {
            text.push_str(&format!("\n@{}", a.display()));
        }
        self.composer.clear();
        if self.current.is_none() {
            let id = self.next_id;
            self.next_id += 1;
            let mut x = Session::new(id, self.new_agent, self.cwd.clone());
            (x.model, x.thinking) = self.prefs.agent_defaults(self.new_agent);
            self.sessions.push(x);
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
    if clock - s.decode_t0 < ui::decode::SPAN + 0.1 || s.sessions.iter().any(|x| x.status == Status::Working) {
        changed = true;
        tx.wake_after(std::time::Duration::from_millis(33));
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
    let main = match (s.view, s.session()) {
        (View::Configs, _) => col().key("main").grow(1.0).h_full().min_w(0.0).child(ui::config::view(s)),
        (View::Chat, Some(sess)) => col().key("main").grow(1.0).h_full().min_w(0.0).gap(16.0)
            .child(ui::transcript::view(s, sess, cx)).child(ui::composer::view(s)),
        (View::Chat, None) => col().key("main").grow(1.0).h_full().min_w(0.0).gap(16.0)
            .child(ui::transcript::welcome(s)).child(ui::composer::view(s)),
    };
    let body = row().key("body").absolute(0.0, 0.0).w_full().h_full().px(18.0).pt(18.0).pb(24.0).gap(22.0)
        .child(ui::sidebar::view(s))
        .child(main)
        .child(ui::panel::view(s));
    let mut root = el().key("root").w_full().h_full().fill(ui::pal().bg);
    if s.prefs.dither {
        root = root.child(el().key("field").absolute(0.0, 0.0).w_full().h_full().pixels_fit(&s.bg, rune::SurfaceFit::Fill).opacity(0.5));
    }
    let mut root = root.child(body);
    if s.popover != Popover::None {
        root = root.child(ui::composer::popover(s));
    }
    if let Some(m) = s.menu {
        root = root.child(ui::sidebar::context_menu(m));
    }
    root
}

fn setup(s: &mut State, waker: rune::Waker) {
    ui::transcript::tick_clock(s.time.clone(), waker.clone());
    swarm::watch(s.swarm.clone(), waker.clone());
    s.waker = Some(waker);
    s.sync_prefs();
    ui::dither::animate(s.bg.clone(), 640, 400, s.fps.clone());
    s.util_send(serde_json::json!({"type": "get_login_providers"}));
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
        scenes: &[("demo", store::demo), ("demo-model", store::demo_model), ("demo-ctx", store::demo_ctx), ("demo-menu", store::demo_menu), ("demo-settings", store::demo_settings)],
        min_size: Some((820.0, 520.0)),
        ..AppSpec::new("Raster", (1280.0, 800.0), State::new, view)
    });
}
