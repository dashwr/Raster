//! Sidebar esquerda: marca, nova sessão, busca, árvore pasta → sessões, configs.

use rune::{El, Tx, col, el, row, text, text_input};

use super::{BODY, BRAND, LABEL, SMALL, pal, spinner};
use crate::session::Status;
use crate::{Menu, MenuTarget, State};

pub const W: f32 = 232.0;

fn status_mark(st: Status, spin: usize) -> &'static str {
    match st {
        Status::Working => spinner(spin),
        Status::Asking => "⠿?",
        Status::Idle => "·",
        Status::Stopped => " ",
    }
}

pub fn view(s: &State) -> El {
    let p = pal();
    let brand = row().key("brand").px(14.0).py(14.0).gap(8.0).center_y()
        .child(text("RASTER").font(BRAND).color(p.text))
        .child(text(concat!("v", env!("CARGO_PKG_VERSION"))).font(SMALL).color(p.faint));

    let new = row().key("new").px(14.0).py(7.0).gap(10.0).center_y().hover(p.field)
        .child(text("+").font(LABEL).color(p.text))
        .child(text("nova sessão").font(LABEL).color(p.text))
        .on_press(|s: &mut State| s.new_session());

    let search = row().key("search-row").mx(10.0).px(8.0).py(5.0).gap(8.0).center_y().fill(p.field).border(1.0, p.line)
        .child(text("⌕").font(LABEL).color(p.dim))
        .child(
            text_input(&s.search, LABEL).key("search").grow(1.0).single_line()
                .placeholder("buscar", p.faint).color(p.text)
                .on_edit(|s: &mut State| &mut s.search),
        );

    let filter = row().key("filter").px(14.0).pt(12.0).pb(4.0).gap(10.0)
        .child(toggle("f-act", "ativas", !s.show_all, |s: &mut State| s.show_all = false))
        .child(toggle("f-all", "todas", s.show_all, |s: &mut State| s.show_all = true));

    let q = s.search.text.to_lowercase();
    let mut folders: Vec<String> = Vec::new();
    for x in &s.sessions {
        let f = x.cwd.display().to_string();
        if !folders.contains(&f) {
            folders.push(f);
        }
    }
    let mut tree = col().key("tree").gap(1.0);
    for f in &folders {
        let name = f.rsplit('/').find(|p| !p.is_empty()).unwrap_or("/").to_string();
        let mut rows = Vec::new();
        for x in s.sessions.iter().rev().filter(|x| x.cwd.display().to_string() == *f) {
            if !s.show_all && x.status == Status::Stopped {
                continue;
            }
            if !q.is_empty() && !x.title.to_lowercase().contains(&q) {
                continue;
            }
            let id = x.id;
            let sel = s.current == Some(id);
            let mark = status_mark(x.status, s.spin);
            let mark_color = if x.status == Status::Asking { p.text } else { p.dim };
            rows.push(
                row().key(format!("s-{id}")).pl(26.0).pr(10.0).py(5.0).gap(8.0).center_y()
                    .fill(if sel { p.field } else { p.panel }).hover(p.field)
                    .child(text(x.agent.glyph()).font(SMALL).color(x.agent.color()).w(16.0))
                    .child(text(x.title.clone()).font(LABEL).color(if sel { p.text } else { p.dim }).grow(1.0).truncate())
                    .child(text(mark).font(LABEL).color(mark_color))
                    .on_press(move |s: &mut State| {
                        s.current = Some(id);
                        s.menu = None;
                    })
                    .on_secondary_press(move |s: &mut State, at, _tx: &mut Tx| {
                        s.menu = Some(Menu { x: at.x, y: at.y, target: MenuTarget::Session(id) });
                    }),
            );
        }
        if rows.is_empty() {
            continue;
        }
        tree = tree.child(
            row().key(format!("dir-{f}")).px(14.0).pt(10.0).pb(4.0).gap(8.0)
                .child(text("▤").font(SMALL).color(p.faint))
                .child(text(name).font(SMALL).color(p.faint).truncate()),
        );
        for r in rows {
            tree = tree.child(r);
        }
    }

    let login = if s.logged_in { "claude: login do Claude Code" } else { "claude: sem login" };
    let foot = col().key("foot").px(14.0).py(10.0).gap(6.0).border_sides([1.0, 0.0, 0.0, 0.0], p.line)
        .child(text(login).font(SMALL).color(p.faint).truncate())
        .child(row().gap(8.0).child(text("⚙").font(LABEL).color(p.dim)).child(text("configs").font(LABEL).color(p.dim)));

    col().key("sidebar").w(W).h_full().fill(p.panel).border_sides([0.0, 1.0, 0.0, 0.0], p.line)
        .child(brand).child(new).child(search).child(filter)
        .child(col().key("tree-scroll").grow(1.0).min_h(0.0).child(tree).scroll(rune::ScrollSpec { stick_bottom: false }))
        .child(foot)
}

fn toggle(key: &'static str, label: &'static str, on: bool, f: fn(&mut State)) -> El {
    let p = pal();
    el().key(key).px(6.0).py(2.0).fill(if on { p.text } else { p.panel }).border(1.0, p.line)
        .child(text(label).font(SMALL).color(if on { p.bg } else { p.dim }))
        .on_press(f)
}

/// Menu de contexto desenhado no app (o Linux não tem menu nativo no rune).
pub fn context_menu(m: Menu) -> El {
    let p = pal();
    let item = |key: &'static str, label: &'static str, f: Box<dyn Fn(&mut State, &mut Tx)>| {
        row().key(key).px(12.0).py(6.0).w_full().hover(p.button)
            .child(text(label).font(BODY).color(p.text))
            .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                f(s, tx);
                s.menu = None;
            })
    };
    let mut menu = col().key("ctx").absolute(m.x, m.y).w(200.0).fill(p.field).border(1.0, p.line).py(4.0);
    match m.target {
        MenuTarget::Session(id) => {
            menu = menu
                .child(item("m-open", "abrir", Box::new(move |s, _| s.current = Some(id))))
                .child(item("m-stop", "encerrar sessão", Box::new(move |s, _| {
                    if let Some(x) = s.by_id(id) {
                        x.stop();
                    }
                })))
                .child(item("m-del", "remover da lista", Box::new(move |s, _| {
                    if let Some(x) = s.by_id(id) {
                        x.stop();
                    }
                    s.sessions.retain(|x| x.id != id);
                    if s.current == Some(id) {
                        s.current = None;
                    }
                    crate::store::save(s);
                })));
        }
        MenuTarget::Item(id, idx) => {
            menu = menu.child(item("m-copy", "copiar texto", Box::new(move |s, tx| {
                let t = s.by_id(id).and_then(|x| x.items.get(idx)).map(crate::ui::transcript::item_text).unwrap_or_default();
                tx.copy(t);
            })));
        }
    }
    el().key("ctx-scrim").absolute(0.0, 0.0).w_full().h_full()
        .on_press(|s: &mut State| s.menu = None)
        .child(menu)
}
