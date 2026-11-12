//! Composer flutuante: `+` (anexar, mencionar MCP), agente, "modelo · effort" com o ícone do
//! provedor, barra de contexto e enviar. Popovers ancorados ao botão que os abre.

use rune::{Align, El, Side, TextEdit, Tx, auto_grow, col, el, row, text, text_input};

use super::{Dither, LABEL, SMALL, dither, friendly_model, icon, pal};
use crate::agents::{ALL, Agent};
use crate::session::{Item, Status};
use crate::{EFFORTS, Popover, State};

fn current_agent(s: &State) -> Agent {
    s.session().map(|x| x.agent).unwrap_or(s.new_agent)
}

fn current_model(s: &State) -> String {
    s.session().map(|x| x.model.clone()).unwrap_or_else(|| s.new_agent.default_model().into())
}

fn current_effort(s: &State) -> String {
    s.session().map(|x| x.thinking.clone()).unwrap_or_else(|| "medium".into())
}

fn provider_icon(model: &str) -> &'static str {
    if model.starts_with("anthropic/") { "burst" } else if model.starts_with("openai") { "knot" } else { "key" }
}

fn toggle(s: &mut State, p: Popover) {
    s.popover = if s.popover == p { Popover::None } else { p };
}

pub fn view(s: &State) -> El {
    let p = pal();
    let agent = current_agent(s);
    let model = current_model(s);

    let field = text_input(&s.composer, super::body(s)).key("composer").w_full()
        .placeholder(format!("Message {}…", agent.name()), p.faint).color(p.text)
        .on_edit(|s: &mut State| &mut s.composer)
        .on_input_key(|s: &mut State, k: &rune::KeyEvent, _tx: &mut Tx| {
            if matches!(k.key, rune::Key::Enter) && !k.mods.shift {
                s.submit();
                return true;
            }
            false
        });

    let sq = |key: &'static str, ic: &'static str, on: bool| {
        el().key(key).size(26.0, 26.0).center().border(1.0, if on { p.text } else { p.dim }).fill(if on { p.text } else { p.bg }).hover(p.field)
            .child(icon(ic, if on { p.bg } else { p.text }))
    };
    let plus = sq("plus-btn", "plus", s.popover == Popover::Plus).on_press(|s: &mut State| toggle(s, Popover::Plus));
    let agent_btn = sq("agent-btn", agent.icon(), s.popover == Popover::Agent).on_press(|s: &mut State| toggle(s, Popover::Agent));
    let model_btn = row().key("model-btn").h(26.0).px(8.0).gap(6.0).center_y().border(1.0, p.dim).hover(p.field)
        .child(icon(provider_icon(&model), p.text))
        .child(text(friendly_model(&model)).font(SMALL).color(p.text))
        .child(text("·").font(SMALL).color(p.faint))
        .child(text(current_effort(s)).font(SMALL).color(p.dim))
        .child(icon("chevron_down", p.dim))
        .on_press(|s: &mut State| toggle(s, Popover::Model));
    let working = s.session().map(|x| x.status == Status::Working).unwrap_or(false);
    let send = el().key("send").size(30.0, 26.0).center().fill(p.text)
        .child(icon(if working { "abort" } else { "send" }, p.bg))
        .on_press(move |s: &mut State| {
            if working && s.composer.text.trim().is_empty() {
                if let Some(x) = s.session_mut() {
                    x.abort();
                }
            } else {
                s.submit();
            }
        });

    let bar = row().key("bar").w_full().pt(8.0).gap(6.0).center_y()
        .child(plus).child(agent_btn)
        .child(el().grow(1.0))
        .child(model_btn).child(context_bar(s)).child(send);

    let mut box_ = col().key("composer-box").w_full().px(12.0).pt(10.0).pb(8.0).gap(6.0);
    if !s.attachments.is_empty() {
        let mut chips = row().key("chips").gap(6.0).flex_wrap();
        for (i, a) in s.attachments.iter().enumerate() {
            let name = a.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            chips = chips.child(row().key(format!("att-{i}")).px(6.0).py(2.0).gap(6.0).center_y().border(1.0, p.dim)
                .child(icon("file", p.dim)).child(text(name).font(SMALL).color(p.text))
                .child(icon("close", p.dim).on_press(move |s: &mut State| {
                    if i < s.attachments.len() {
                        s.attachments.remove(i);
                    }
                })));
        }
        box_ = box_.child(chips);
    }
    let box_ = super::hard(box_.fill(p.bg).border(2.0, p.text), 6.0, p.text)
        .child(auto_grow(field, 1, 8))
        .child(bar);
    col().key("composer-wrap").w_full().child(box_)
}

/// Partes do contexto (rótulo, padrão, tokens). Mensagens e saída de ferramentas são estimadas pelo
/// texto (4 caracteres por token); o resto do usado é system prompt + definições de ferramentas.
fn context_parts(s: &State) -> (Vec<(&'static str, Dither, u64)>, u64) {
    let Some(x) = s.session() else { return (vec![("free", Dither::Sparse, 200_000)], 200_000) };
    let (mut msgs, mut tools) = (0u64, 0u64);
    for it in &x.items {
        match it {
            Item::User(t) => msgs += t.len() as u64 / 4,
            Item::Assistant { text, thinking, .. } => msgs += (text.len() + thinking.len()) as u64 / 4,
            Item::Tools { calls, .. } => tools += calls.iter().map(|c| c.output.len() as u64 / 4).sum::<u64>(),
            _ => {}
        }
    }
    let used = x.ctx_tokens;
    let (msgs, tools) = (msgs.min(used), tools.min(used.saturating_sub(msgs.min(used))));
    let base = used - msgs - tools;
    let win = x.ctx_window.max(used).max(1);
    (vec![
        ("system + tools", Dither::Solid, base),
        ("messages", Dither::Checker, msgs),
        ("tool output", Dither::Stripes, tools),
        ("free", Dither::Sparse, win - used),
    ], win)
}

/// Medidor vertical do composer: segmentos, cheio de baixo para cima.
fn context_bar(s: &State) -> El {
    let p = pal();
    let (used, win) = s.session().map(|x| (x.ctx_tokens, x.ctx_window)).unwrap_or((0, 200_000));
    let frac = (used as f32 / win.max(1) as f32).clamp(0.0, 1.0);
    let segs = 6;
    let lit = (frac * segs as f32).ceil() as usize;
    let mut bar = col().key("ctx-cells").gap(1.0);
    for i in (0..segs).rev() {
        bar = bar.child(el().size(6.0, 3.0).fill(if i < lit { p.text } else { p.button }));
    }
    row().key("ctx-btn").h(26.0).px(6.0).gap(6.0).center_y().border(1.0, if s.popover == Popover::Context { p.text } else { p.dim }).hover(p.field)
        .child(bar)
        .child(text(format!("{}k", used / 1000)).font(SMALL).color(p.dim))
        .on_press(|s: &mut State| toggle(s, Popover::Context))
}

pub fn popover(s: &State) -> El {
    let (card, key, align) = match s.popover {
        Popover::Model => (model_card(s), "model-btn", Align::End),
        Popover::Agent => (agent_card(s), "agent-btn", Align::Start),
        Popover::Plus => (plus_card(s), "plus-btn", Align::Start),
        Popover::Context => (context_card(s), "ctx-btn", Align::End),
        Popover::None => (col(), "composer", Align::Start),
    };
    el().key("pop-scrim").absolute(0.0, 0.0).w_full().h_full()
        .on_press(|s: &mut State| s.popover = Popover::None)
        .child(super::float(card).anchored(key, Side::Top, align, 12.0).on_press(|_: &mut State| {}))
}

fn head(t: &str) -> El {
    text(t.to_uppercase()).font(SMALL).color(pal().dim).px(12.0).pt(8.0).pb(4.0)
}

fn model_card(s: &State) -> El {
    let p = pal();
    let cur = current_model(s);
    let mut c = col().key("model-card").w(300.0).pb(10.0).child(head("model"));
    let models = s.models();
    if models.is_empty() {
        c = c.child(text("No connected provider. Sign in under Settings.").font(SMALL).color(p.faint).px(12.0).py(4.0).wrap());
    }
    for (id, name) in models {
        let on = id == cur;
        c = c.child(
            row().key(format!("m-{id}")).w_full().px(12.0).h(28.0).gap(10.0).center_y().hover(p.field)
                .fill(if on { p.field } else { p.bg })
                .child(icon(provider_icon(id), p.text))
                .child(text(name).font(LABEL).color(if on { p.text } else { p.dim }).grow(1.0))
                .child(if on { icon("check", p.text) } else { el().size(16.0, 16.0) })
                .on_press(move |s: &mut State| {
                    if let Some(x) = s.session_mut() {
                        x.set_model(id);
                        crate::store::save(s);
                    }
                }),
        );
    }
    c.child(el().px(12.0).py(6.0).child(super::sep(276.0)))
        .child(head("effort"))
        .child(effort_meter(s).px(12.0))
}

/// Medidor segmentado de effort com alça: clique numa célula ou arraste.
fn effort_meter(s: &State) -> El {
    let p = pal();
    let cur = current_effort(s);
    let idx = EFFORTS.iter().position(|e| *e == cur).unwrap_or(1);
    let n = EFFORTS.len();
    let w = 276.0;
    let cell = (w - (n - 1) as f32 * 3.0) / n as f32;
    let step = cell + 3.0;
    let mut track = el().key("eff-track").size(w, 22.0);
    for i in 0..n {
        let x = step * i as f32;
        track = track.child(if i <= idx {
            el().absolute(x, 4.0).size(cell, 14.0).fill(p.text)
        } else {
            el().absolute(x, 4.0).size(cell, 14.0).border(1.0, p.dim).child(dither(Dither::Checker, cell - 2.0, 12.0, p.faint))
        });
    }
    let hx = step * idx as f32 + cell - 4.0;
    track = track.child(super::hard(el().key("eff-handle").absolute(hx, 0.0).size(8.0, 22.0).fill(p.text).border(1.0, p.bg), 3.0, p.dim)
        .motion(rune::Spring::new(0.25, 0.0)));
    let set = move |s: &mut State, x: f32| {
        let i = ((x / step) as usize).min(EFFORTS.len() - 1);
        if let Some(sess) = s.session_mut() {
            sess.set_thinking(EFFORTS[i]);
        }
    };
    let track = track.on_drag(move |s: &mut State, d: &mut rune::Drag, _tx: &mut Tx| set(s, d.x));
    let mut labels = row().w(w).gap(3.0);
    for (i, e) in EFFORTS.iter().enumerate() {
        labels = labels.child(el().w(cell).center().child(text(*e).font(SMALL).color(if i == idx { p.text } else { p.faint }))
            .on_press(move |s: &mut State| {
                if let Some(sess) = s.session_mut() {
                    sess.set_thinking(EFFORTS[i]);
                }
            }));
    }
    col().gap(4.0).child(track).child(labels)
}

fn agent_card(s: &State) -> El {
    let p = pal();
    let cur = current_agent(s);
    let mut c = col().key("agent-card").w(280.0).pb(8.0).child(head("agent"));
    for a in ALL {
        let blurb = match a {
            Agent::Chat => "Talk, research, write to the vault",
            Agent::Code => "Engineering, minimal and terse",
            Agent::Search => "Fast, light lookups",
            Agent::Test => "Testing and triage, files issues",
        };
        c = c.child(
            row().key(format!("a-{}", a.name())).w_full().px(12.0).py(6.0).gap(10.0).center_y().hover(p.field)
                .child(super::sidebar::agent_square(a.icon(), 24.0, a == cur))
                .child(col().grow(1.0)
                    .child(text(a.name()).font(LABEL).color(if a == cur { p.text } else { p.dim }))
                    .child(text(blurb).font(SMALL).color(p.faint)))
                .on_press(move |s: &mut State| {
                    if s.current.is_none() || s.session().map(|x| x.items.is_empty()).unwrap_or(false) {
                        s.new_agent = a;
                        if let Some(x) = s.session_mut() {
                            x.agent = a;
                            x.model = a.default_model().into();
                        }
                    }
                    s.popover = Popover::None;
                }),
        );
    }
    c.child(text("Changing agent applies to new sessions").font(SMALL).color(p.faint).px(12.0).pt(4.0))
}

/// Servidores MCP de `~/.raster/agent/mcp.json`.
fn mcp_servers() -> Vec<String> {
    let path = crate::omp::state_dir().join("agent/mcp.json");
    let v: serde_json::Value = std::fs::read_to_string(path).ok().and_then(|r| serde_json::from_str(&r).ok()).unwrap_or_default();
    v["mcpServers"].as_object().map(|m| m.keys().cloned().collect()).unwrap_or_default()
}

fn plus_card(_s: &State) -> El {
    let p = pal();
    let mut c = col().key("plus-card").w(240.0).pb(8.0).child(head("add"))
        .child(
            row().key("p-attach").w_full().px(12.0).h(28.0).gap(10.0).center_y().hover(p.field)
                .child(icon("attach", p.text)).child(text("Attach file").font(LABEL).color(p.text))
                .on_press_tx(|s: &mut State, tx: &mut Tx| {
                    s.popover = Popover::None;
                    let opts = rune::FileDialogOptions { title: "Attach file".into(), directory: Some(s.cwd.clone()), ..Default::default() };
                    tx.open_file(opts, |s: &mut State, r, _| {
                        if let Ok(Some(path)) = r {
                            s.attachments.push(path);
                        }
                    });
                }),
        )
        .child(el().px(12.0).py(4.0).child(super::sep(216.0)))
        .child(head("mention mcp"));
    let servers = mcp_servers();
    if servers.is_empty() {
        c = c.child(text("No MCP servers configured").font(SMALL).color(p.faint).px(12.0).py(4.0));
    }
    for name in servers {
        let n2 = name.clone();
        c = c.child(
            row().key(format!("p-mcp-{name}")).w_full().px(12.0).h(28.0).gap(10.0).center_y().hover(p.field)
                .child(icon("plug", p.text)).child(text(name).font(LABEL).color(p.text))
                .on_press(move |s: &mut State| {
                    let t = s.composer.text.clone();
                    let sep = if t.is_empty() || t.ends_with(' ') { "" } else { " " };
                    s.composer = TextEdit::new(format!("{t}{sep}@{n2} "));
                    s.composer.focused = true;
                    s.popover = Popover::None;
                }),
        );
    }
    c
}

fn context_card(s: &State) -> El {
    let p = pal();
    let (parts, win) = context_parts(s);
    let h = 200.0;
    let mut bar = col().key("ctx-vbar").size(28.0, h).border(1.0, p.text);
    let mut legend = col().gap(8.0).grow(1.0);
    // de cima para baixo: livre no topo, system na base
    for (_, d, tok) in parts.iter().rev() {
        let seg = ((h - 2.0) * *tok as f32 / win as f32).round();
        if seg >= 1.0 {
            bar = bar.child(dither(*d, 26.0, seg, p.text));
        }
    }
    for (label, d, tok) in &parts {
        legend = legend.child(row().gap(8.0).center_y()
            .child(el().size(14.0, 14.0).border(1.0, p.dim).child(dither(*d, 12.0, 12.0, p.text)))
            .child(text(*label).font(SMALL).color(p.text).grow(1.0))
            .child(text(format!("{:.1}k", *tok as f32 / 1000.0)).font(SMALL).color(p.dim)));
    }
    let used: u64 = parts.iter().filter(|x| x.0 != "free").map(|x| x.2).sum();
    col().key("ctx-card").w(260.0).pb(12.0)
        .child(head("context"))
        .child(row().px(12.0).gap(14.0).child(bar).child(legend
            .child(super::sep(180.0))
            .child(text(format!("{}k / {}k used", used / 1000, win / 1000)).font(SMALL).color(p.dim))
            .child(text("messages and tool output are estimates").font(SMALL).color(p.faint).wrap())))
}
