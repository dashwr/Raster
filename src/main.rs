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

/// Versão completa: pacote, commit e data do build.
pub const VERSION: &str = concat!("raster ", env!("CARGO_PKG_VERSION"), " (", env!("RASTER_REV"), ", ", env!("RASTER_BUILD_DATE"), ")");

/// Máximo de anexos por mensagem.
pub const MAX_ATTACHMENTS: usize = 10;

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
    /// Settings: modelo ou effort padrão de um agente.
    AgentModel(Agent),
    AgentEffort(Agent),
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
    /// Miniaturas dos anexos de imagem, decodificadas fora da thread da UI.
    pub previews: std::collections::HashMap<PathBuf, rune::Image>,
    pub images: Option<rune::ImageCache>,
    pub prefs: store::Prefs,
    /// Quadros por segundo do campo de fundo (0 = parado), lido pela thread do dither.
    pub fps: std::sync::Arc<std::sync::atomic::AtomicU32>,
    /// Aurora do fundo do chat: quadro atual e se a thread deve animar (sessão atual pensando).
    pub aurora: Pixels,
    pub aurora_on: std::sync::Arc<std::sync::atomic::AtomicBool>,
    /// Modelo e effort escolhidos na tela inicial, antes de a sessão existir.
    pub new_model: Option<String>,
    pub new_effort: Option<String>,
    pub catalog_at: f64,
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
            previews: Default::default(),
            images: None,
            prefs: store::Prefs::load(),
            fps: Default::default(),
            aurora: Pixels::new(),
            aurora_on: Default::default(),
            new_model: None,
            new_effort: None,
            catalog_at: -100.0,
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
        let chat = self.view == View::Chat && self.current.is_some();
        let f = if self.prefs.dither && !chat { self.prefs.fps } else { 0 };
        self.fps.store(f, std::sync::atomic::Ordering::Relaxed);
    }

    pub fn attachments_from(&mut self, paths: &[PathBuf]) {
        self.attach(paths.iter().cloned());
    }

    /// Anexa arquivos até o limite; imagens ganham miniatura.
    pub fn attach(&mut self, paths: impl IntoIterator<Item = PathBuf>) {
        for p in paths {
            if self.attachments.len() >= MAX_ATTACHMENTS {
                break;
            }
            if self.attachments.contains(&p) {
                continue;
            }
            if session::image_mime(&p).is_some()
                && let (Some(c), Ok(bytes)) = (&self.images, std::fs::read(&p))
            {
                self.previews.insert(p.clone(), c.load(bytes));
            }
            self.attachments.push(p);
        }
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

    /// Modelos com credencial válida, do catálogo que a extensão grava (`ompdsh/models.json`);
    /// sem ele, a lista curada dos provedores conectados.
    pub fn models(&self) -> Vec<(String, String)> {
        let cat = ui::catalog();
        if !cat.is_empty() {
            return cat;
        }
        let providers = self.util.as_ref().map(|u| u.providers.clone()).unwrap_or_default();
        let on = |prov: &str| (prov == "anthropic" && self.logged_in) || providers.iter().any(|p| p.0 == prov && p.2);
        MODELS.iter().filter(|(id, _)| on(id.split('/').next().unwrap_or("")))
            .map(|(a, b)| (a.to_string(), b.to_string())).collect()
    }

    pub fn cur_model(&self) -> String {
        match self.session() {
            Some(x) => x.model.clone(),
            None => self.new_model.clone().unwrap_or_else(|| self.prefs.agent_defaults(self.new_agent).0),
        }
    }

    pub fn cur_effort(&self) -> String {
        match self.session() {
            Some(x) => x.thinking.clone(),
            None => self.new_effort.clone().unwrap_or_else(|| self.prefs.agent_defaults(self.new_agent).1),
        }
    }

    pub fn set_model(&mut self, id: &str) {
        match self.session_mut() {
            Some(x) => x.set_model(id),
            None => self.new_model = Some(id.into()),
        }
        store::save(self);
    }

    pub fn set_effort(&mut self, e: &str) {
        match self.session_mut() {
            Some(x) => x.set_thinking(e),
            None => self.new_effort = Some(e.into()),
        }
        store::save(self);
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
        let mut images = Vec::new();
        self.previews.clear();
        for a in self.attachments.drain(..) {
            match (session::image_mime(&a), std::fs::read(&a)) {
                (Some(mime), Ok(bytes)) => {
                    text.push_str(&format!("\n[image: {}]", a.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default()));
                    images.push((mime.to_string(), session::base64(&bytes)));
                }
                _ => text.push_str(&format!("\n@{}", a.display())),
            }
        }
        self.composer.clear();
        if self.current.is_none() {
            let id = self.next_id;
            self.next_id += 1;
            let mut x = Session::new(id, self.new_agent, self.cwd.clone());
            (x.model, x.thinking) = (self.cur_model(), self.cur_effort());
            (self.new_model, self.new_effort) = (None, None);
            self.sessions.push(x);
            self.current = Some(id);
        }
        let (inbox, waker) = (self.inbox.clone(), self.waker.clone().expect("waker"));
        if let Some(s) = self.session_mut() {
            s.prompt(text, images, &inbox, &waker);
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
    s.sync_prefs();
    if clock - s.catalog_at > 5.0 {
        s.catalog_at = clock;
        ui::reload_catalog();
    }
    let thinking = s.session().is_some_and(|x| x.status == Status::Working);
    s.aurora_on.store(thinking, std::sync::atomic::Ordering::Relaxed);
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
        (View::Chat, Some(sess)) => col().key("main").grow(1.0).h_full().min_w(0.0).gap(8.0)
            .child(ui::transcript::view(s, sess, cx)).child(ui::composer::view(s)),
        (View::Chat, None) => col().key("main").grow(1.0).h_full().min_w(0.0).gap(8.0)
            .child(ui::transcript::welcome(s)).child(ui::composer::view(s)),
    };
    let body = row().key("body").absolute(0.0, 0.0).w_full().h_full().pl(6.0).pr(10.0).pt(6.0).pb(10.0).gap(10.0)
        .child(ui::sidebar::view(s))
        .child(main)
        .child(ui::panel::view(s));
    let mut root = el().key("root").w_full().h_full().fill(ui::pal().bg);
    // campo animado só na tela inicial e no Settings; durante o chat o fundo é preto
    if s.prefs.dither && !(s.view == View::Chat && s.current.is_some()) {
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
    s.images = Some(rune::ImageCache::new(waker.clone()));
    s.waker = Some(waker);
    s.sync_prefs();
    ui::dither::animate(s.bg.clone(), 1280, 800, s.fps.clone());
    ui::dither::animate_aurora(s.aurora.clone(), 800, 600, s.aurora_on.clone());
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
    sess.prompt(msg, Vec::new(), &inbox, &waker);
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
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{}", VERSION);
        return;
    }
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
        scenes: &[("demo", store::demo), ("demo-model", store::demo_model), ("demo-ctx", store::demo_ctx), ("demo-menu", store::demo_menu), ("demo-settings", store::demo_settings), ("demo-decode", store::demo_decode), ("demo-agent-model", store::demo_agent_model), ("demo-attach", store::demo_attach)],
        min_size: Some((820.0, 520.0)),
        ..AppSpec::new("Raster", (1280.0, 800.0), State::new, view)
    });
}
