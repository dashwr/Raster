//! Painel direito: swarm de subagentes (avatares pixelados com status) e comandos em background.

use std::cell::RefCell;
use std::collections::HashMap;

use rune::{El, Pixels, col, el, row, text};

use super::{LABEL, SMALL, pal, spinner};
use crate::State;
use crate::session::{Item, SubStatus};

pub const W: f32 = 236.0;

thread_local! {
    static AVATARS: RefCell<HashMap<String, Pixels>> = RefCell::new(HashMap::new());
}

fn avatar(id: &str) -> Pixels {
    AVATARS.with(|m| {
        m.borrow_mut().entry(id.to_string()).or_insert_with(|| {
            let seed = id.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211));
            super::dither::avatar(seed, 5)
        }).clone()
    })
}

pub fn view(s: &State) -> El {
    let p = pal();
    let Some(sess) = s.session() else {
        return col().key("panel").w(W).h_full().fill(p.panel).border_sides([0.0, 0.0, 0.0, 1.0], p.line)
            .child(section("swarm")).child(empty("nenhuma sessão aberta"));
    };

    let mut grid = row().key("swarm-grid").w_full().px(12.0).gap(10.0).flex_wrap();
    let live: Vec<_> = sess.subs.iter().collect();
    for sub in &live {
        let mark = match sub.status {
            SubStatus::Working => spinner(s.spin),
            SubStatus::Idle => "·",
        };
        grid = grid.child(
            col().key(format!("sub-{}", sub.id)).w(58.0).gap(4.0).items_center()
                .child(el().size(40.0, 40.0).border(1.0, p.line).pixels_fit(&avatar(&sub.id), rune::SurfaceFit::Fill))
                .child(text(mark).font(LABEL).color(if sub.status == SubStatus::Working { p.text } else { p.faint }))
                .child(text(sub.label.clone()).font(SMALL).color(p.dim).truncate().w(58.0)),
        );
    }

    let mut bg = col().key("bg-list").w_full().gap(2.0);
    let mut n = 0;
    for (idx, it) in sess.items.iter().enumerate().rev() {
        let Item::Tools { calls, .. } = it else { continue };
        for (ci, c) in calls.iter().enumerate().rev() {
            if c.name != "bash" || n >= 14 {
                continue;
            }
            n += 1;
            let key = format!("{idx}-{ci}");
            let open = s.preview.as_deref() == Some(key.as_str());
            let mark = if !c.done { spinner(s.spin + ci) } else if c.error { "✗" } else { "✓" };
            let k2 = key.clone();
            let mut entry = col().key(format!("bg-{key}")).w_full()
                .child(
                    row().px(12.0).py(4.0).gap(8.0).hover(p.field)
                        .child(text(mark).font(SMALL).color(if c.error { p.danger } else { p.dim }))
                        .child(text(c.summary.clone()).font(SMALL).color(p.text).grow(1.0).truncate())
                        .on_press(move |s: &mut State| {
                            s.preview = if s.preview.as_deref() == Some(k2.as_str()) { None } else { Some(k2.clone()) };
                        }),
                );
            if open {
                let out = if c.output.is_empty() { "(sem saída ainda)".to_string() } else { c.output.clone() };
                let tail: String = out.lines().rev().take(18).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                entry = entry.child(
                    el().mx(10.0).mb(6.0).px(8.0).py(6.0).fill(p.bg).border(1.0, p.line)
                        .child(text(format!("> {}\n\n{tail}", c.summary)).font(SMALL).color(p.dim).wrap_anywhere().selectable(true)),
                );
            }
            bg = bg.child(entry);
        }
    }

    let mut c = col().key("panel").w(W).h_full().fill(p.panel).border_sides([0.0, 0.0, 0.0, 1.0], p.line)
        .child(section("swarm"));
    c = if live.is_empty() { c.child(empty("sem subagentes")) } else { c.child(grid) };
    c = c.child(section("background"));
    c = if n == 0 { c.child(empty("nenhum comando")) } else { c.child(bg) };
    col().key("panel-scroll").h_full().child(c.scroll(rune::ScrollSpec { stick_bottom: false }))
}

fn section(title: &'static str) -> El {
    let p = pal();
    row().w_full().px(12.0).pt(14.0).pb(8.0).gap(8.0)
        .child(text("▚").font(SMALL).color(p.dim))
        .child(text(title).font(SMALL).color(p.dim))
}

fn empty(msg: &'static str) -> El {
    text(msg).font(SMALL).color(pal().faint).px(12.0).py(4.0)
}
