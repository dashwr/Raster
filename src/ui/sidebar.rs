//! Sidebar esquerda: marca, nova sessão, busca, switch active · all, árvore pasta → sessões, settings.

use rune::{El, Tx, col, el, row, text, text_input};

use super::{BRAND, Dither, LABEL, SMALL, dither, icon, icon_sized, pal, sep, spinner};
use crate::session::Status;
use crate::{Menu, MenuTarget, State};

pub const W: f32 = 236.0;
const INNER: f32 = W - 2.0;

fn status_mark(s: &State, st: Status) -> El {
    let p = pal();
    match st {
        Status::Working => super::glyph(spinner(s.spin), LABEL, p.text),
        Status::Asking => icon("asking", p.text),
        Status::Idle => icon("idle", p.faint),
        Status::Stopped => el().size(16.0, 16.0),
    }
}

/// Ícone de agente dentro de um quadrado.
pub fn agent_square(name: &str, size: f32, on: bool) -> El {
    let p = pal();
    el().size(size, size).center().border(1.0, if on { p.text } else { p.dim }).fill(if on { p.text } else { p.bg })
        .child(icon(name, if on { p.bg } else { p.text }))
}

pub fn view(s: &State) -> El {
    let p = pal();
    let brand = row().key("brand").px(14.0).pt(16.0).pb(10.0).gap(8.0).center_y()
        .child(text("RASTER").font(BRAND).color(p.text))
        .child(text(concat!("v", env!("CARGO_PKG_VERSION"))).font(SMALL).color(p.dim))
        .help(crate::VERSION);

    let new = row().key("new").mx(12.0).px(8.0).h(28.0).gap(8.0).center_y().fill(p.text).border(1.0, p.text)
        .child(icon("plus", p.bg))
        .child(text("New session").font(LABEL).color(p.bg).grow(1.0))
        .child(text("Ctrl N").font(SMALL).color(p.button))
        .on_press(|s: &mut State| s.new_session());

    let search = row().key("search-row").mx(12.0).px(8.0).h(28.0).gap(8.0).center_y().border(1.0, p.dim).hover(p.field)
        .child(icon("search", p.dim))
        .child(
            text_input(&s.search, LABEL).key("search").grow(1.0).single_line()
                .placeholder("Search sessions", p.faint).color(p.text)
                .on_edit(|s: &mut State| &mut s.search),
        );

    let sw_w = W - 24.0;
    let half = (sw_w - 2.0) / 2.0;
    let thumb_x = if s.show_all { half } else { 0.0 };
    let lbl = |key: &'static str, t: &'static str, on: bool, all: bool| {
        el().key(key).size(half, 22.0).center()
            .child(text(t).font(SMALL).color(if on { p.bg } else { p.dim }))
            .on_press(move |s: &mut State| s.show_all = all)
    };
    let switch = el().key("switch").mx(12.0).size(sw_w, 24.0).border(1.0, p.dim)
        .child(dither(Dither::Checker, sw_w - 2.0, 22.0, p.faint).absolute(0.0, 0.0))
        .child(el().key("sw-thumb").absolute(thumb_x, 0.0).size(half, 22.0).fill(p.text).motion(rune::Spring::new(0.25, 0.0)))
        .child(row().absolute(0.0, 0.0).child(lbl("sw-act", "ACTIVE", !s.show_all, false)).child(lbl("sw-all", "ALL", s.show_all, true)));

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
            let label: El = if s.renaming == Some(id) {
                text_input(&s.rename, LABEL).key("rename").grow(1.0).single_line().color(p.text)
                    .on_edit(|s: &mut State| &mut s.rename)
                    .on_input_key(|s: &mut State, k: &rune::KeyEvent, _tx: &mut Tx| {
                        if matches!(k.key, rune::Key::Enter) {
                            s.commit_rename();
                            return true;
                        }
                        false
                    })
            } else {
                text(x.title.clone()).font(LABEL).color(if sel { p.text } else { p.dim }).grow(1.0).truncate()
            };
            let mut r = el().key(format!("s-{id}")).size(INNER, 28.0).hover(p.field);
            if sel {
                r = r.child(dither(Dither::Checker, INNER, 28.0, p.button).absolute(0.0, 0.0));
            }
            rows.push(
                r.child(
                    row().absolute(0.0, 0.0).size(INNER, 28.0).pl(22.0).pr(10.0).gap(8.0).center_y()
                        .child(agent_square(x.agent.icon(), 20.0, false))
                        .child(label)
                        .child(status_mark(s, x.status)),
                )
                .on_press(move |s: &mut State| s.select(id))
                .on_secondary_press(move |s: &mut State, at, _tx: &mut Tx| {
                    s.menu = Some(Menu { x: at.x, y: at.y, target: MenuTarget::Session(id) });
                }),
            );
        }
        if rows.is_empty() {
            continue;
        }
        tree = tree.child(
            row().key(format!("dir-{f}")).px(14.0).pt(10.0).pb(4.0).gap(6.0).center_y()
                .child(icon("folder", p.dim))
                .child(text(name).font(SMALL).color(p.dim).truncate()),
        );
        for r in rows {
            tree = tree.child(r);
        }
    }

    let foot = col().key("foot").w_full().gap(0.0)
        .child(sep(INNER))
        .child(row().key("cfg-open").px(14.0).h(34.0).gap(8.0).center_y().hover(p.field)
            .child(icon("settings", p.dim)).child(text("Settings").font(LABEL).color(p.dim))
            .on_press(|s: &mut State| s.open_configs()));

    super::float(col().key("sidebar").w(W).h_full().gap(8.0))
        .child(brand).child(new).child(search).child(switch)
        .child(col().key("tree-scroll").grow(1.0).min_h(0.0).child(tree).scroll(rune::ScrollSpec { stick_bottom: false }))
        .child(foot)
}

type MenuAction = Box<dyn Fn(&mut State, &mut Tx)>;

/// Menu de contexto desenhado no app (o Linux não tem menu nativo no rune), com ícone por opção.
pub fn context_menu(m: Menu) -> El {
    let p = pal();
    let item = |key: &'static str, ic: &'static str, label: &'static str, f: MenuAction| {
        row().key(key).px(10.0).h(28.0).gap(10.0).w_full().center_y().hover(p.field)
            .child(icon_sized(ic, 16.0, p.text))
            .child(text(label).font(LABEL).color(p.text))
            .on_press_tx(move |s: &mut State, tx: &mut Tx| {
                f(s, tx);
                s.menu = None;
            })
    };
    let mut menu = super::float(col().key("ctx").absolute(m.x, m.y).w(196.0).px(1.0).py(4.0));
    match m.target {
        MenuTarget::Session(id) => {
            menu = menu
                .child(item("m-open", "open", "Open", Box::new(move |s, _| s.select(id))))
                .child(item("m-rename", "rename", "Rename", Box::new(move |s, _| s.start_rename(id))))
                .child(item("m-copy", "copy", "Copy title", Box::new(move |s, tx| {
                    if let Some(x) = s.by_id(id) {
                        tx.copy(x.title.clone());
                    }
                })))
                .child(item("m-reveal", "folder", "Reveal folder", Box::new(move |s, _| {
                    if let Some(x) = s.by_id(id) {
                        let _ = std::process::Command::new("xdg-open").arg(&x.cwd).spawn();
                    }
                })))
                .child(el().px(10.0).py(3.0).child(sep(174.0)))
                .child(item("m-stop", "stop", "End session", Box::new(move |s, _| {
                    if let Some(x) = s.by_id(id) {
                        x.stop();
                    }
                })))
                .child(item("m-del", "trash", "Remove from list", Box::new(move |s, _| {
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
            menu = menu.child(item("m-copy", "copy", "Copy text", Box::new(move |s, tx| {
                let t = s.by_id(id).and_then(|x| x.items.get(idx)).map(crate::ui::transcript::item_text).unwrap_or_default();
                tx.copy(t);
            })));
        }
    }
    el().key("ctx-scrim").absolute(0.0, 0.0).w_full().h_full()
        .on_press(|s: &mut State| s.menu = None)
        .child(menu)
}
