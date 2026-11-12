//! Composer: pasta e "modelo · effort" acima; `+` e agente à esquerda; contexto e provedor à direita.
//! Popovers de modelo, effort (slider) e agente.

use rune::{El, Tx, auto_grow, col, el, row, text, text_input};

use super::{BODY, LABEL, SMALL, STRONG, friendly_model, pal};
use crate::agents::{ALL, Agent};
use crate::session::Status;
use crate::{EFFORTS, MODELS, Popover, State};

fn current_agent(s: &State) -> Agent {
    s.session().map(|x| x.agent).unwrap_or(s.new_agent)
}

fn current_model(s: &State) -> String {
    s.session().map(|x| x.model.clone()).unwrap_or_else(|| s.new_agent.default_model().into())
}

fn current_effort(s: &State) -> String {
    s.session().map(|x| x.thinking.clone()).unwrap_or_else(|| "medium".into())
}

pub fn view(s: &State) -> El {
    let p = pal();
    let agent = current_agent(s);
    let model = current_model(s);
    let cwd = s.session().map(|x| x.cwd.clone()).unwrap_or_else(|| s.cwd.clone());
    let folder = cwd.file_name().map(|f| f.to_string_lossy().to_string()).unwrap_or_else(|| "/".into());

    let above = row().key("above").w_full().px(4.0).pb(6.0).center_y()
        .child(row().gap(6.0).child(text("▤").font(SMALL).color(p.faint)).child(text(folder).font(SMALL).color(p.dim)))
        .child(el().grow(1.0))
        .child(
            row().key("model-btn").gap(6.0).px(6.0).py(2.0).hover(p.field)
                .child(text(friendly_model(&model)).font(SMALL).color(p.text))
                .child(text("·").font(SMALL).color(p.faint))
                .child(text(current_effort(s)).font(SMALL).color(p.dim))
                .on_press(|s: &mut State| s.popover = if s.popover == Popover::Model { Popover::None } else { Popover::Model }),
        );

    let field = text_input(&s.composer, BODY).key("composer").w_full()
        .placeholder(format!("mensagem para {}…", agent.name()), p.faint).color(p.text)
        .on_edit(|s: &mut State| &mut s.composer)
        .on_input_key(|s: &mut State, k: &rune::KeyEvent, _tx: &mut Tx| {
            if matches!(k.key, rune::Key::Enter) && !k.mods.shift {
                s.submit();
                return true;
            }
            false
        });

    let ctx = context_bar(s);
    let agent_btn = el().key("agent-btn").size(28.0, 24.0).center().border(1.0, agent.color().alpha(0.6)).hover(p.field)
        .child(text(agent.glyph()).font(LABEL).color(agent.color()))
        .on_press(|s: &mut State| s.popover = if s.popover == Popover::Agent { Popover::None } else { Popover::Agent });
    let plus = el().key("plus").size(24.0, 24.0).center().hover(p.field).child(text("+").font(STRONG).color(p.dim));
    let working = s.session().map(|x| x.status == Status::Working).unwrap_or(false);
    let send = el().key("send").size(28.0, 24.0).center().fill(if working { p.button } else { p.text })
        .child(text(if working { "■" } else { "↵" }).font(LABEL).color(if working { p.text } else { p.bg }))
        .on_press(move |s: &mut State| {
            if working && s.composer.text.trim().is_empty() {
                if let Some(x) = s.session_mut() { x.abort(); }
            } else {
                s.submit();
            }
        });
    let provider = text(if model.starts_with("anthropic/") { "✳" } else { "◎" }).font(LABEL).color(p.dim);

    let bar = row().key("bar").w_full().pt(8.0).gap(8.0).center_y()
        .child(plus).child(agent_btn)
        .child(el().grow(1.0))
        .child(ctx).child(provider).child(send);

    let box_ = col().key("composer-box").w_full().px(12.0).pt(10.0).pb(8.0).fill(p.panel).border(1.0, p.line)
        .child(auto_grow(field, 1, 8))
        .child(bar);
    col().key("composer-wrap").w_full().px(28.0).pb(18.0).pt(6.0).child(above).child(box_)
}

/// Barra vertical do contexto usado, com o número ao lado.
fn context_bar(s: &State) -> El {
    let p = pal();
    let (used, win) = s.session().map(|x| (x.ctx_tokens, x.ctx_window)).unwrap_or((0, 200_000));
    let frac = (used as f32 / win.max(1) as f32).clamp(0.0, 1.0);
    let h = 18.0;
    let bar = el().key("ctx-track").size(4.0, h).fill(p.button).child(
        el().key("ctx-fill").absolute(0.0, h * (1.0 - frac)).size(4.0, (h * frac).max(1.0)).fill(if frac > 0.8 { p.danger } else { p.text })
            .motion(rune::Spring::new(0.4, 0.0)),
    );
    row().key("ctx").gap(6.0).center_y().child(bar)
        .child(text(format!("{}k", used / 1000)).font(SMALL).color(p.faint))
}

pub fn popover(s: &State) -> El {
    let p = pal();
    let card = match s.popover {
        Popover::Model => model_card(s),
        Popover::Effort => effort_card(s),
        Popover::Agent => agent_card(s),
        Popover::None => col(),
    };
    el().key("pop-scrim").absolute(0.0, 0.0).w_full().h_full()
        .on_press(|s: &mut State| s.popover = Popover::None)
        .child(
            col().key("pop-pos").w_full().h_full().justify_end().items_center().pb(120.0)
                .child(card.fill(p.field).border(1.0, p.line).on_press(|_: &mut State| {})),
        )
}

fn model_card(s: &State) -> El {
    let p = pal();
    let cur = current_model(s);
    let mut c = col().key("model-card").w(300.0).py(6.0)
        .child(text("modelo").font(SMALL).color(p.faint).px(12.0).py(4.0));
    for (id, name) in MODELS {
        let on = *id == cur;
        c = c.child(
            row().key(format!("m-{id}")).w_full().px(12.0).py(6.0).gap(10.0).hover(p.button)
                .child(text(if on { "●" } else { "○" }).font(LABEL).color(p.text))
                .child(text(*name).font(LABEL).color(p.text).grow(1.0))
                .child(text(id.split('/').next().unwrap_or("")).font(SMALL).color(p.faint))
                .on_press(move |s: &mut State| {
                    if let Some(x) = s.session_mut() { x.set_model(id); crate::store::save(s); }
                    s.popover = Popover::Effort;
                }),
        );
    }
    c.child(effort_slider(s).px(12.0).pt(8.0))
}

fn effort_card(s: &State) -> El {
    let p = pal();
    col().key("effort-card").w(300.0).py(10.0).gap(8.0)
        .child(text("effort").font(SMALL).color(p.faint).px(12.0))
        .child(effort_slider(s).px(12.0))
}

/// Slider de effort: seis degraus, clique ou arraste.
fn effort_slider(s: &State) -> El {
    let p = pal();
    let cur = current_effort(s);
    let idx = EFFORTS.iter().position(|e| *e == cur).unwrap_or(3);
    let w = 276.0;
    let step = w / (EFFORTS.len() - 1) as f32;
    let mut track = el().key("eff-track").size(w, 22.0);
    track = track.child(el().absolute(0.0, 10.0).size(w, 2.0).fill(p.button));
    track = track.child(el().key("eff-fill").absolute(0.0, 10.0).size(step * idx as f32, 2.0).fill(p.text).motion(rune::Spring::new(0.3, 0.0)));
    for i in 0..EFFORTS.len() {
        let on = i <= idx;
        track = track.child(el().key(format!("eff-{i}")).absolute(step * i as f32 - 4.0, 7.0).size(8.0, 8.0).fill(if on { p.text } else { p.button }).border(1.0, p.line));
    }
    let set = move |s: &mut State, x: f32| {
        let i = ((x / step).round() as usize).min(EFFORTS.len() - 1);
        if let Some(sess) = s.session_mut() { sess.set_thinking(EFFORTS[i]); }
    };
    let track = track
        .on_drag(move |s: &mut State, d: &mut rune::Drag, _tx: &mut Tx| set(s, d.x));
    let mut labels = row().w(w + 8.0).justify_between();
    for (i, e) in EFFORTS.iter().enumerate() {
        labels = labels.child(text(*e).font(SMALL).color(if i == idx { p.text } else { p.faint }).on_press(move |s: &mut State| {
            if let Some(sess) = s.session_mut() { sess.set_thinking(EFFORTS[i]); }
        }));
    }
    col().gap(4.0).child(track).child(labels)
}

fn agent_card(s: &State) -> El {
    let p = pal();
    let cur = current_agent(s);
    let mut c = col().key("agent-card").w(300.0).py(6.0)
        .child(text("agente").font(SMALL).color(p.faint).px(12.0).py(4.0));
    for a in ALL {
        let blurb = match a {
            Agent::Chat => "conversa, pesquisa, anota no vault",
            Agent::Code => "engenharia, mínimo e conciso",
            Agent::Search => "busca leve e rápida",
            Agent::Test => "teste e triagem, abre issues",
        };
        c = c.child(
            row().key(format!("a-{}", a.name())).w_full().px(12.0).py(6.0).gap(10.0).hover(p.button)
                .child(text(a.glyph()).font(LABEL).color(a.color()).w(18.0))
                .child(col().grow(1.0)
                    .child(text(a.name()).font(LABEL).color(if a == cur { p.text } else { p.dim }))
                    .child(text(blurb).font(SMALL).color(p.faint)))
                .on_press(move |s: &mut State| {
                    if s.current.is_none() || s.session().map(|x| x.items.is_empty()).unwrap_or(false) {
                        s.new_agent = a;
                        if let Some(x) = s.session_mut() { x.agent = a; x.model = a.default_model().into(); }
                    }
                    s.popover = Popover::None;
                }),
        );
    }
    c.child(text("trocar o agente vale para sessões novas").font(SMALL).color(p.faint).px(12.0).pt(6.0))
        .child(el().h(2.0))
        .child(text("").font(BODY))
}
