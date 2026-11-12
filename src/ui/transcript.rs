//! Coluna central: transcrição da sessão (markdown, grupos de ferramentas, perguntas inline) e a
//! tela inicial pontilhada.

use rune::{Cx, El, Tx, col, el, row, text, text_input};
use rune_kit::{Kit, Markdown, MarkdownStyle};
use serde_json::json;

use super::{BODY, DISPLAY, LABEL, SMALL, STRONG, icon, pal, spinner};
use crate::session::{AskKind, Item, Session, Status};
use crate::{Menu, MenuTarget, State};

pub fn kit() -> &'static Kit {
    static K: std::sync::OnceLock<Kit> = std::sync::OnceLock::new();
    K.get_or_init(|| {
        let h = |c: &str| rune::color::hex(c).unwrap();
        Kit::from_palette(rune_kit::PaletteSeed { bg: h("#0a0a0a"), fg: h("#f2f2f2"), accent: h("#d0d0d0") })
    })
}

pub fn item_text(i: &Item) -> String {
    match i {
        Item::User(t) | Item::Notice(t) | Item::Error(t) => t.clone(),
        Item::Assistant { text, .. } => text.clone(),
        Item::Tools { calls, .. } => calls.iter().map(|c| format!("{} {}", c.name, c.summary)).collect::<Vec<_>>().join("\n"),
        Item::Ask { kind, answer, .. } => format!("{kind:?} → {}", answer.clone().unwrap_or_default()),
    }
}

pub fn view(s: &State, sess: &Session, _cx: &mut Cx<State>) -> El {
    let p = pal();
    let sid = sess.id;
    let mut list = col().key(format!("tx-{sid}")).w_full().px(16.0).py(12.0).gap(12.0);
    let el_t = s.clock - s.decode_t0;
    let fr = super::decode::frame(s.clock);
    let dec_at = |t: &str, base: f64| super::decode::reveal(t, el_t, base, fr);
    let dec = |t: &str| dec_at(t, 0.0);
    let last_ask = sess.items.iter().rposition(|i| matches!(i, Item::Ask { answer: None, .. }));
    for (idx, it) in sess.items.iter().enumerate() {
        let base = (idx as f64 * 0.04).min(0.3);
        let dec = |t: &str| dec_at(t, base);
        let node = match it {
            Item::User(t) => row().w_full().justify_end().child(
                super::hard(el().max_w(560.0).min_w(0.0).px(12.0).py(8.0).fill(p.text), 4.0, p.dim)
                    .child(text(dec(t)).font(super::body(s)).color(p.bg).wrap_anywhere().selectable(true)),
            ),
            Item::Assistant { text: t, thinking, streaming, .. } => {
                let mut c = col().w_full().gap(6.0);
                if !thinking.is_empty() {
                    let first = thinking.lines().find(|l| !l.trim().is_empty()).unwrap_or("").chars().take(140).collect::<String>();
                    c = c.child(row().gap(6.0).child(icon("thinking", p.faint)).child(text(dec(&first)).font(SMALL).color(p.faint).truncate()));
                }
                if !t.is_empty() {
                    let style = MarkdownStyle::from_kit(kit()).diagram_max_width(640.0).body_font(super::body(s));
                    let shown = if *streaming { super::decode::streaming(t, s.clock) } else { dec(t) };
                    c = c.child(Markdown::new(shown).style(style).streaming(*streaming).build(kit()));
                }
                c
            }
            Item::Tools { calls, open } => tools(s, sid, idx, calls, *open, sess.status == Status::Working && idx + 1 == sess.items.len()),
            Item::Ask { kind, answer, .. } => ask(s, sid, idx, kind, answer.as_deref(), Some(idx) == last_ask),
            Item::Notice(t) => row().w_full().gap(6.0).child(icon("info", p.faint)).child(text(dec(t)).font(SMALL).color(p.faint).wrap_anywhere().grow(1.0).min_w(0.0)),
            Item::Error(t) => row().w_full().px(10.0).py(6.0).gap(8.0).border(1.0, p.text).child(icon("error", p.text)).child(text(dec(t)).font(LABEL).color(p.text).wrap_anywhere().grow(1.0).min_w(0.0).selectable(true)),
        };
        list = list.child(
            node.key(format!("it-{sid}-{idx}")).on_secondary_press(move |s: &mut State, at, _tx: &mut Tx| {
                s.menu = Some(Menu { x: at.x, y: at.y, target: MenuTarget::Item(sid, idx) });
            }),
        );
    }
    if sess.status == Status::Working && !matches!(sess.items.last(), Some(Item::Tools { .. })) {
        list = list.child(
            row().key("working").gap(8.0)
                .child(super::glyph(spinner(s.spin), LABEL, p.text))
                .child(text(format!("{}…", super::decode::thinking(s.clock))).font(LABEL).color(p.dim)),
        );
    }
    col().key("transcript").grow(1.0).min_h(0.0).w_full().gap(8.0)
        .child(header(sess, &dec))
        .child(super::float(el().key("tx-frame").grow(1.0).min_h(0.0).w_full())
            .child(el().key("aurora").absolute(0.0, 0.0).w_full().h_full().pixels_fit(&s.aurora, rune::SurfaceFit::Fill)
                .opacity(if sess.status == Status::Working { 0.25 } else { 0.0 }).opacity_motion(rune::Spring::new(0.8, 0.0)))
            .child(col().key(format!("scroll-{sid}")).w_full().h_full().child(list).scroll(rune::ScrollSpec { stick_bottom: true })))
}

fn header(sess: &Session, dec: &dyn Fn(&str) -> String) -> El {
    let p = pal();
    super::float(row().key("hdr").w_full().px(14.0).py(8.0).gap(10.0).center_y())
        .child(super::sidebar::agent_square(sess.agent.icon(), 22.0, true))
        .child(text(dec(&sess.title)).font(STRONG).color(p.text).grow(1.0).truncate())
        .child(icon("folder", p.dim))
        .child(text(sess.cwd.display().to_string()).font(SMALL).color(p.dim).truncate())
}

fn tools(s: &State, sid: u64, idx: usize, calls: &[crate::session::ToolCall], open: bool, live: bool) -> El {
    let p = pal();
    let lead: El = if live { super::glyph(spinner(s.spin), LABEL, p.text) } else { icon("tool", p.faint) };
    let n = calls.len();
    let label = if live { format!("Working · {n} tool{}", if n == 1 { "" } else { "s" }) } else { format!("Used {n} tool{}", if n == 1 { "" } else { "s" }) };
    let head = row().gap(8.0).center_y().hover(p.panel).px(4.0).py(2.0)
        .child(lead)
        .child(text(label).font(LABEL).color(p.dim))
        .child(icon(if open { "chevron_down" } else { "chevron_right" }, p.faint))
        .on_press(move |s: &mut State| {
            if let Some(Item::Tools { open, .. }) = s.by_id(sid).and_then(|x| x.items.get_mut(idx)) {
                *open = !*open;
            }
        });
    let mut c = col().w_full().gap(4.0).child(head);
    if open {
        let mut body = col().w_full().pl(18.0).gap(3.0).border_sides([0.0, 0.0, 0.0, 1.0], p.line);
        for (i, call) in calls.iter().enumerate() {
            let mark: El = if !call.done { super::glyph(spinner(s.spin + i), SMALL, p.text) } else if call.error { icon("error", p.text) } else { icon("check", p.faint) };
            body = body.child(
                row().key(format!("call-{i}")).gap(8.0).px(8.0)
                    .child(mark)
                    .child(text(call.name.clone()).font(SMALL).color(p.text))
                    .child(text(call.summary.clone()).font(SMALL).color(p.dim).grow(1.0).truncate()),
            );
        }
        c = c.child(body);
    }
    c
}

pub fn ask_block(s: &State, sid: u64, idx: usize, it: &Item) -> El {
    match it {
        Item::Ask { kind, answer, .. } => ask(s, sid, idx, kind, answer.as_deref(), answer.is_none()),
        _ => col(),
    }
}

fn ask(s: &State, sid: u64, idx: usize, kind: &AskKind, answer: Option<&str>, active: bool) -> El {
    let p = pal();
    let mut box_ = col().w_full().max_w(640.0).px(14.0).py(12.0).gap(10.0).fill(p.panel)
        .border(1.0, if answer.is_none() { p.text } else { p.dim });
    let title = match kind {
        AskKind::Ask(_) => "Question".to_string(),
        AskKind::Select { title, .. } | AskKind::Confirm { title, .. } | AskKind::Input { title } => title.clone(),
    };
    box_ = box_.child(row().gap(8.0)
        .child(icon(if answer.is_none() { "asking" } else { "done" }, p.text))
        .child(text(title).font(STRONG).color(p.text)));
    if let Some(a) = answer {
        return box_.child(text(format!("→ {a}")).font(LABEL).color(p.dim).wrap());
    }
    match kind {
        AskKind::Ask(qs) => {
            for (qi, q) in qs.iter().enumerate() {
                box_ = box_.child(text(q.question.clone()).font(BODY).color(p.text).wrap());
                for (oi, (label, desc)) in q.options.iter().enumerate() {
                    let on = q.picked.contains(&oi);
                    let multi = q.multi;
                    box_ = box_.child(
                        row().key(format!("opt-{qi}-{oi}")).gap(10.0).px(8.0).py(5.0).hover(p.field)
                            .fill(if on { p.field } else { p.panel }).border(1.0, if on { p.text } else { p.dim })
                            .child(text(if on { "[x]" } else { "[ ]" }).font(LABEL).color(p.text))
                            .child(col().grow(1.0)
                                .child(text(label.clone()).font(LABEL).color(p.text).wrap())
                                .child(text(desc.clone()).font(SMALL).color(p.faint).wrap()))
                            .on_press(move |s: &mut State| {
                                if let Some(Item::Ask { kind: AskKind::Ask(qs), .. }) = s.by_id(sid).and_then(|x| x.items.get_mut(idx))
                                    && let Some(q) = qs.get_mut(qi)
                                {
                                    if multi {
                                        if let Some(p) = q.picked.iter().position(|x| *x == oi) { q.picked.remove(p); } else { q.picked.push(oi); }
                                    } else {
                                        q.picked = vec![oi];
                                    }
                                }
                            }),
                    );
                }
            }
            if active {
                box_ = box_.child(free_text(s));
            }
            box_ = box_.child(row().gap(8.0).child(super::button("ask-send", "Answer").on_press(move |s: &mut State| {
                let extra = s.ask_text.text.trim().to_string();
                s.ask_text.clear();
                let Some(x) = s.by_id(sid) else { return };
                let Some(Item::Ask { kind: AskKind::Ask(qs), .. }) = x.items.get(idx) else { return };
                let answers: Vec<_> = qs.iter().map(|q| {
                    let sel: Vec<String> = q.picked.iter().filter_map(|i| q.options.get(*i).map(|o| o.0.clone())).collect();
                    let mut a = json!({"id": q.id, "selectedOptions": sel});
                    if !extra.is_empty() { a["customInput"] = json!(extra); }
                    a
                }).collect();
                let shown = qs.iter().map(|q| q.picked.iter().filter_map(|i| q.options.get(*i).map(|o| o.0.clone())).collect::<Vec<_>>().join(", ")).collect::<Vec<_>>().join(" · ");
                let shown = if extra.is_empty() { shown } else { format!("{shown} · {extra}") };
                x.answer(idx, json!({"answers": answers}), shown);
            })));
        }
        AskKind::Select { options, .. } => {
            for (oi, o) in options.iter().enumerate() {
                let o2 = o.clone();
                box_ = box_.child(
                    row().key(format!("sel-{oi}")).px(8.0).py(5.0).hover(p.field).border(1.0, p.line)
                        .child(text(o.clone()).font(LABEL).color(p.text))
                        .on_press(move |s: &mut State| {
                            if let Some(x) = s.by_id(sid) { x.answer(idx, json!({"value": o2}), o2.clone()); }
                        }),
                );
            }
        }
        AskKind::Confirm { message, .. } => {
            box_ = box_.child(text(message.clone()).font(BODY).color(p.dim).wrap()).child(
                row().gap(8.0)
                    .child(super::button("yes", "Allow").on_press(move |s: &mut State| {
                        if let Some(x) = s.by_id(sid) { x.answer(idx, json!({"confirmed": true}), "allowed".into()); }
                    }))
                    .child(super::button("no", "Deny").on_press(move |s: &mut State| {
                        if let Some(x) = s.by_id(sid) { x.answer(idx, json!({"confirmed": false}), "denied".into()); }
                    })),
            );
        }
        AskKind::Input { .. } => {
            if active {
                box_ = box_.child(free_text(s));
            }
            box_ = box_.child(super::button("in-send", "Send").on_press(move |s: &mut State| {
                let v = s.ask_text.text.trim().to_string();
                s.ask_text.clear();
                if let Some(x) = s.by_id(sid) { x.answer(idx, json!({"value": v}), v.clone()); }
            }));
        }
    }
    box_
}

fn free_text(s: &State) -> El {
    let p = pal();
    el().w_full().px(8.0).py(5.0).fill(p.field).border(1.0, p.line).child(
        text_input(&s.ask_text, LABEL).key("ask-text").w_full().single_line()
            .placeholder("Other answer (optional)", p.faint).color(p.text)
            .on_edit(|s: &mut State| &mut s.ask_text),
    )
}

/// Tela inicial: saudação em pixel sobre o campo pontilhado do fundo.
pub fn welcome(s: &State) -> El {
    let p = pal();
    let (hour, hhmm) = s.time.lock().map(|t| t.clone()).unwrap_or((20, String::new()));
    let greet = match hour {
        5..=11 => "Good morning.",
        12..=17 => "Good afternoon.",
        _ => "Good evening.",
    };
    let fr = super::decode::frame(s.clock);
    let card = super::float(col().key("greet").px(28.0).py(20.0).gap(8.0).items_center())
        .child(text(hhmm).font(LABEL).color(p.dim))
        .child(text(super::decode::reveal(greet, s.clock - s.decode_t0, 0.0, fr)).font(DISPLAY).color(p.text))
        .child(row().gap(8.0).center_y()
            .child(super::sidebar::agent_square(s.new_agent.icon(), 20.0, false))
            .child(text(format!("{} · {}", s.new_agent.name(), super::friendly_model(s.new_agent.default_model()))).font(SMALL).color(p.dim)));
    col().key("welcome").grow(1.0).min_h(0.0).w_full().center().child(card)
}

/// Hora local (hora, "HH:MM"), atualizada fora do frame da UI.
pub fn tick_clock(slot: std::sync::Arc<std::sync::Mutex<(u32, String)>>, waker: rune::Waker) {
    std::thread::spawn(move || loop {
        let out = std::process::Command::new("date").arg("+%H %H:%M").output().ok();
        let raw = out.map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string()).unwrap_or_default();
        if let Some((h, hm)) = raw.split_once(' ') {
            let v = (h.parse().unwrap_or(20), hm.to_string());
            let changed = slot.lock().map(|mut t| { let c = *t != v; *t = v; c }).unwrap_or(false);
            if changed { waker(); }
        }
        std::thread::sleep(std::time::Duration::from_secs(15));
    });
}
