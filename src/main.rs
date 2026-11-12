//! Solwired Raster: app desktop para o omp-dsh.

mod agents;
mod auth;
mod omp;
mod session;
mod store;
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
    pub time: std::sync::Arc<std::sync::Mutex<(u32, String)>>,
}

impl State {
    fn new() -> State {
        let cwd = std::env::args().skip(1).find(|a| !a.starts_with('-')).map(PathBuf::from)
            .or_else(|| std::env::current_dir().ok()).unwrap_or_else(|| PathBuf::from("."));
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
        self.sessions.iter_mut().find(|x| x.id == id)
    }

    pub fn new_session(&mut self) {
        self.current = None;
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
    let center = match s.session() {
        Some(sess) => ui::transcript::view(s, sess, cx),
        None => ui::transcript::welcome(s),
    };
    let main = col().key("main").grow(1.0).h_full().min_w(0.0).fill(p.bg)
        .child(center)
        .child(ui::composer::view(s));
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
    s.waker = Some(waker);
    ui::dither::animate(s.bg.clone(), 960, 600);
}

fn main() {
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
