//! Painel direito: swarm de subagentes (avatares pixelados com status) e comandos em background.

use rune::{El, col, el, row, text};

use super::{LABEL, SMALL, icon, icon_sized, pal, spinner};
use crate::State;
use crate::session::{Item, SubStatus};
use crate::swarm::{Member, Phase};

pub const W: f32 = 196.0;

/// Gato fixo por subagente (8 variações), escolhido pelo id.
fn cat(id: &str) -> String {
    let h = id.bytes().fold(1469598103934665603u64, |h, b| (h ^ b as u64).wrapping_mul(1099511628211));
    format!("cat_{}", h % 8 + 1)
}

pub fn view(s: &State) -> El {
    let p = pal();
    let Some(sess) = s.session() else {
        return super::float(col().key("panel").w(W).pb(12.0))
            .child(section("swarm")).child(empty("No session open"));
    };

    let mut grid = row().key("swarm-grid").w_full().px(12.0).gap(8.0).flex_wrap();
    let watched = s.swarm.lock().ok().and_then(|m| m.get(&sess.id).cloned()).unwrap_or_default();
    let live: Vec<Member> = if watched.is_empty() {
        sess.subs.iter().map(|x| Member {
            id: x.id.clone(), label: x.label.clone(), agent: String::new(), model: String::new(),
            phase: if x.status == SubStatus::Working { Phase::Thinking } else { Phase::Idle },
        }).collect()
    } else {
        watched
    };
    for sub in &live {
        let state: El = match sub.phase {
            Phase::Thinking => super::glyph(spinner(s.spin), LABEL, p.text),
            Phase::Writing => icon("writing", p.text),
            Phase::Tool => icon("tool", p.text),
            Phase::Idle => icon("idle", p.dim),
            Phase::Dead => icon("dead", p.dim),
        };
        let tip = format!("{} · {} · {}", sub.label, if sub.agent.is_empty() { "subagent" } else { &sub.agent }, crate::ui::friendly_model(&sub.model));
        grid = grid.child(
            col().key(format!("sub-{}", sub.id)).w(36.0).gap(4.0).items_center()
                .child(el().size(36.0, 36.0).center().border(1.0, p.text).child(icon_sized(&cat(&sub.id), 32.0, p.text)))
                .child(el().size(16.0, 16.0).center().child(state))
                .help(tip),
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
            let mark: El = if !c.done { super::glyph(spinner(s.spin + ci), SMALL, p.text) } else if c.error { icon("error", p.text) } else { icon("check", p.dim) };
            let k2 = key.clone();
            let mut entry = col().key(format!("bg-{key}")).w_full()
                .child(
                    row().px(12.0).py(4.0).gap(8.0).hover(p.field)
                        .child(mark)
                        .child(text(c.summary.clone()).font(SMALL).color(p.text).grow(1.0).truncate())
                        .on_press(move |s: &mut State| {
                            s.preview = if s.preview.as_deref() == Some(k2.as_str()) { None } else { Some(k2.clone()) };
                        }),
                );
            if open {
                let out = if c.output.is_empty() { "(no output yet)".to_string() } else { c.output.clone() };
                let tail: String = out.lines().rev().take(18).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n");
                entry = entry.child(
                    el().mx(10.0).mb(6.0).px(8.0).py(6.0).fill(p.bg).border(1.0, p.dim)
                        .child(text(format!("> {}\n\n{tail}", c.summary)).font(SMALL).color(p.dim).wrap_anywhere().selectable(true)),
                );
            }
            bg = bg.child(entry);
        }
    }

    let mut c = super::float(col().key("panel").w(W).pb(12.0)).child(section("swarm"));
    c = if live.is_empty() { c.child(empty("No subagents")) } else { c.child(grid) };
    c = c.child(section("background"));
    c = if n == 0 { c.child(empty("No commands")) } else { c.child(bg) };
    col().key("panel-col").child(c)
}

fn section(title: &'static str) -> El {
    let p = pal();
    col().w_full().px(12.0).pt(12.0).pb(8.0).gap(6.0)
        .child(text(title.to_uppercase()).font(SMALL).color(p.dim))
        .child(super::sep(W - 26.0))
}

fn empty(msg: &'static str) -> El {
    text(msg).font(SMALL).color(pal().faint).px(12.0).py(4.0)
}
