//! Composer flutuante: `+` (anexar, mencionar MCP), agente, "modelo · effort" com o ícone do
//! provedor, barra de contexto e enviar. Popovers ancorados ao botão que os abre.

use rune::{Align, El, Side, TextEdit, Tx, auto_grow, col, el, row, text, text_input};

use super::{Dither, LABEL, SMALL, dither, friendly_model, icon, icon_sized, pal};
use crate::agents::{ALL, Agent};
use crate::session::{Item, Status};
use crate::{EFFORTS, Popover, State};

fn current_agent(s: &State) -> Agent {
    s.session().map(|x| x.agent).unwrap_or(s.new_agent)
}

fn current_model(s: &State) -> String {
    s.cur_model()
}

fn current_effort(s: &State) -> String {
    s.cur_effort()
}

fn provider_icon(model: &str) -> &'static str {
    if model.starts_with("anthropic/") { "burst" } else if model.starts_with("openai") { "knot" } else if model.starts_with("google") { "bolt" } else { "key" }
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
        .on_input_paste(|s: &mut State, c: &rune::ClipboardContent, _tx: &mut Tx| {
            if !c.files().is_empty() {
                s.attachments_from(c.files());
                return true;
            }
            let Some(img) = c.image() else { return false };
            let dir = crate::omp::state_dir().join("pastes");
            let path = dir.join(format!("paste-{}.png", std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_millis()).unwrap_or(0)));
            if std::fs::create_dir_all(&dir).is_ok() && std::fs::write(&path, img.png()).is_ok() {
                s.attach([path]);
            }
            true
        })
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
        let mut chips = row().key("chips").w_full().gap(8.0).flex_wrap().items_end();
        for (i, a) in s.attachments.iter().enumerate() {
            let name = a.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_default();
            let close = el().key(format!("att-x-{i}")).size(16.0, 16.0).center().fill(p.bg).border(1.0, p.dim).hover(p.field)
                .child(icon_sized("close", 12.0, p.text))
                .on_press(move |s: &mut State| {
                    if i < s.attachments.len() {
                        let a = s.attachments.remove(i);
                        s.previews.remove(&a);
                    }
                });
            let chip = match s.previews.get(a) {
                Some(img) => col().key(format!("att-{i}")).w(64.0).gap(2.0)
                    .child(el().size(64.0, 64.0).border(1.0, p.dim).pixels_fit(img.pixels(), rune::SurfaceFit::Fill)
                        .child(close.absolute(47.0, 1.0)))
                    .child(text(name).font(SMALL).color(p.dim).truncate().w(64.0)),
                None => row().key(format!("att-{i}")).h(26.0).px(6.0).gap(6.0).center_y().border(1.0, p.dim)
                    .child(icon("file", p.dim)).child(text(name).font(SMALL).color(p.text).max_w(180.0).truncate()).child(close),
            };
            chips = chips.child(chip);
        }
        let full = s.attachments.len() >= crate::MAX_ATTACHMENTS;
        chips = chips.child(text(format!("{}/{}", s.attachments.len(), crate::MAX_ATTACHMENTS)).font(SMALL).color(if full { p.text } else { p.faint }));
        box_ = box_.child(chips);
    }
    let box_ = super::hard(box_.fill(p.bg).border(2.0, p.text), 6.0, p.text)
        .child(auto_grow(field, 1, 8))
        .child(bar);
    col().key("composer-wrap").w_full().child(box_)
        .on_drop_files(|s: &mut State, files: &[std::path::PathBuf], _tx: &mut Tx| s.attachments_from(files))
}

/// Partes do contexto (rótulo, padrão, tokens). Mensagens e saída de ferramentas são estimadas pelo
/// texto (4 caracteres por token); o resto do usado é system prompt + definições de ferramentas.
fn context_parts(s: &State) -> (Vec<(&'static str, Dither, u64)>, u64) {
    let Some(x) = s.session() else {
        let w = s.prefs.ctx_for(&s.cur_model());
        return (vec![("free", Dither::Sparse, w)], w);
    };
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
    let win = s.prefs.ctx_for(&x.model).max(used).max(1);
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
    let (used, win) = s.session().map(|x| (x.ctx_tokens, s.prefs.ctx_for(&x.model))).unwrap_or((0, s.prefs.ctx_for(&s.cur_model())));
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
    let (card, key, align): (El, String, Align) = match s.popover {
        Popover::Model => (model_card(s), "model-btn".into(), Align::End),
        Popover::Agent => (agent_card(s), "agent-btn".into(), Align::Start),
        Popover::Plus => (plus_card(s), "plus-btn".into(), Align::Start),
        Popover::Context => (context_card(s), "ctx-btn".into(), Align::End),
        Popover::AgentModel(a) => (agent_model_card(s, a), format!("ag-m-{}", a.name()), Align::Start),
        Popover::AgentEffort(a) => (agent_effort_card(s, a), format!("ag-e-{}", a.name()), Align::Start),
        Popover::None => (col(), "composer".into(), Align::Start),
    };
    let side = if matches!(s.popover, Popover::AgentModel(_) | Popover::AgentEffort(_)) { Side::Bottom } else { Side::Top };
    // 1 pt de recuo: o hover das linhas não cobre a borda nem a sombra
    el().key("pop-scrim").absolute(0.0, 0.0).w_full().h_full()
        .on_press(|s: &mut State| s.popover = Popover::None)
        .child(super::float(el().px(1.0).py(1.0).child(card)).anchored_fit(key, side, align, 8.0).on_press(|_: &mut State| {}))
}

/// Linha de lista em popover: marca, rótulo e check quando ativa.
fn pick_row(key: String, ic: Option<&'static str>, label: String, on: bool, f: impl Fn(&mut State) + 'static) -> El {
    let p = pal();
    let mut r = row().key(key).w_full().px(11.0).h(28.0).gap(10.0).center_y().hover(p.field).fill(if on { p.field } else { p.bg });
    if let Some(ic) = ic {
        r = r.child(icon(ic, p.text));
    }
    r.child(text(label).font(LABEL).color(if on { p.text } else { p.dim }).grow(1.0).truncate())
        .child(if on { icon("check", p.text) } else { el().size(16.0, 16.0) })
        .on_press(f)
}

fn agent_model_card(s: &State, a: Agent) -> El {
    let (cur, _) = s.prefs.agent_defaults(a);
    let mut list = col().key("am-list").w_full();
    let models = s.models();
    if models.is_empty() {
        list = list.child(text("No connected provider").font(SMALL).color(pal().faint).px(12.0).py(4.0));
    }
    for (id, name) in models {
        let id2 = id.clone();
        list = list.child(pick_row(format!("am-{id}"), Some(provider_icon(&id)), name, id == cur, move |s: &mut State| {
            let v = id2.clone();
            s.prefs.agents.entry(a.name().into()).or_default().0 = v;
            s.prefs.save();
            s.popover = Popover::None;
        }));
    }
    col().key("am-card").w(260.0).pb(6.0).child(head(&format!("{} model", a.name())))
        .child(list.max_h(320.0).scroll(rune::ScrollSpec { stick_bottom: false }))
}

fn agent_effort_card(s: &State, a: Agent) -> El {
    let (_, cur) = s.prefs.agent_defaults(a);
    let mut c = col().key("ae-card").w(160.0).pb(6.0).child(head("effort"));
    for e in EFFORTS {
        c = c.child(pick_row(format!("ae-{e}"), None, e.to_string(), *e == cur, move |s: &mut State| {
            s.prefs.agents.entry(a.name().into()).or_default().1 = e.to_string();
            s.prefs.save();
            s.popover = Popover::None;
        }));
    }
    c
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
    let mut list = col().key("m-list").w_full();
    for (id, name) in models {
        let id2 = id.clone();
        list = list.child(pick_row(format!("m-{id}"), Some(provider_icon(&id)), name, id == cur, move |s: &mut State| s.set_model(&id2)));
    }
    c = c.child(list.max_h(260.0).scroll(rune::ScrollSpec { stick_bottom: false }));
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
    for (i, eff) in EFFORTS.iter().enumerate() {
        let x = step * i as f32;
        let c = if i <= idx {
            el().absolute(x, 4.0).size(cell, 14.0).fill(p.text)
        } else {
            el().absolute(x, 4.0).size(cell, 14.0).border(1.0, p.dim).child(dither(Dither::Checker, cell - 2.0, 12.0, p.faint))
        };
        track = track.child(c.key(format!("eff-c{i}")).on_press(move |s: &mut State| s.set_effort(eff)));
    }
    let hx = step * idx as f32 + cell - 4.0;
    track = track.child(super::hard(el().key("eff-handle").absolute(hx, 0.0).size(8.0, 22.0).fill(p.text).border(1.0, p.bg), 3.0, p.dim)
        .motion(rune::Spring::new(0.25, 0.0)));
    let set = move |s: &mut State, x: f32| {
        let i = ((x.max(0.0) / step) as usize).min(EFFORTS.len() - 1);
        s.set_effort(EFFORTS[i]);
    };
    let track = track.on_drag(move |s: &mut State, d: &mut rune::Drag, _tx: &mut Tx| set(s, d.x));
    let mut labels = row().w(w).gap(3.0);
    for (i, e) in EFFORTS.iter().enumerate() {
        labels = labels.child(el().w(cell).center().child(text(*e).font(SMALL).color(if i == idx { p.text } else { p.faint }))
            .on_press(move |s: &mut State| s.set_effort(EFFORTS[i])));
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
                        (s.new_model, s.new_effort) = (None, None);
                        let s2 = s.prefs.agent_defaults(a);
                        if let Some(x) = s.session_mut() {
                            x.agent = a;
                            (x.model, x.thinking) = s2;
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
                            s.attach([path]);
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
