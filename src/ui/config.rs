//! Settings: contas, agentes, memória, contexto, ferramentas, aparência e caminhos.

use rune::{El, col, el, row, text};
use serde_json::json;

use super::{LABEL, SMALL, STRONG, button, frame, icon, pal};
use crate::agents::ALL;
use crate::session::Item;
use crate::store::Prefs;
use crate::{State, View};

fn line(label: &str, hint: &str, control: El) -> El {
    let p = pal();
    row().w_full().py(4.0).gap(12.0).center_y()
        .child(col().grow(1.0).min_w(0.0)
            .child(text(label.to_string()).font(LABEL).color(p.text))
            .child(text(hint.to_string()).font(SMALL).color(p.faint).wrap()))
        .child(control)
}

fn set(s: &mut State, f: impl FnOnce(&mut Prefs)) {
    f(&mut s.prefs);
    s.prefs.save();
    s.sync_prefs();
}

/// Switch on · off no mesmo desenho do active · all.
fn switch(key: &'static str, on: bool, f: fn(&mut Prefs)) -> El {
    let p = pal();
    let cell = |t: &'static str, lit: bool| el().size(34.0, 20.0).center().fill(if lit { p.text } else { p.bg })
        .child(text(t).font(SMALL).color(if lit { p.bg } else { p.dim }));
    row().key(key).border(1.0, p.dim).child(cell("OFF", !on)).child(cell("ON", on))
        .on_press(move |s: &mut State| set(s, f))
}

fn stepper(key: &'static str, value: String, dec: fn(&mut Prefs), inc: fn(&mut Prefs)) -> El {
    let p = pal();
    let b = |k: String, ic: &'static str, f: fn(&mut Prefs)| el().key(k).size(22.0, 22.0).center().hover(p.field)
        .child(if ic == "minus" { el().size(10.0, 2.0).fill(p.text) } else { icon(ic, p.text) }).on_press(move |s: &mut State| set(s, f));
    row().key(key).border(1.0, p.dim).center_y()
        .child(b(format!("{key}-"), "minus", dec))
        .child(el().w(64.0).center().child(text(value).font(SMALL).color(p.text)))
        .child(b(format!("{key}+"), "plus", inc))
}

fn tok(n: u64) -> String {
    if n >= 1_000_000 && n.is_multiple_of(1_000_000) { format!("{}M", n / 1_000_000) } else { format!("{}k", n / 1000) }
}

/// Passo do limite: 25k abaixo de 200k, 100k acima.
fn step(n: u64, up: bool) -> u64 {
    let d = if n < 200_000 || (!up && n == 200_000) { 25_000 } else { 100_000 };
    if up { n + d } else { n.saturating_sub(d) }
}

/// Contexto por modelo: teto do provedor e limite do usuário (o efetivo é o menor dos dois).
fn models_frame(s: &State) -> El {
    let p = pal();
    let mut c = col().gap(2.0).child(row().gap(10.0).py(2.0)
        .child(text("MODEL").font(SMALL).color(p.faint).grow(1.0))
        .child(text("PROVIDER MAX").font(SMALL).color(p.faint).w(100.0))
        .child(text("LIMIT").font(SMALL).color(p.faint).w(150.0)));
    let models = s.models();
    if models.is_empty() {
        c = c.child(text("No connected provider").font(SMALL).color(p.faint));
    }
    for (id, name) in models {
        let max = super::ctx_max(&id);
        let eff = s.prefs.ctx_for(&id);
        let custom = s.prefs.ctx_limits.contains_key(&id);
        let (a, b, r) = (id.clone(), id.clone(), id.clone());
        let btn = |k: String, ic: &'static str, f: Box<dyn Fn(&mut State)>| el().key(k).size(22.0, 22.0).center().hover(p.field)
            .child(if ic == "minus" { el().size(10.0, 2.0).fill(p.text) } else { icon(ic, p.text) })
            .on_press(move |s: &mut State| f(s));
        let lim = row().key(format!("lim-{id}")).w(150.0).gap(6.0).center_y()
            .child(row().border(1.0, p.dim).center_y()
                .child(btn(format!("lim-{id}-"), "minus", Box::new(move |s: &mut State| {
                    let n = step(s.prefs.ctx_for(&a), false).max(25_000);
                    set(s, |p| { p.ctx_limits.insert(a.clone(), n); });
                })))
                .child(el().w(52.0).center().child(text(tok(eff)).font(SMALL).color(if custom { p.text } else { p.dim })))
                .child(btn(format!("lim-{id}+"), "plus", Box::new(move |s: &mut State| {
                    let max = super::ctx_max(&b).unwrap_or(200_000);
                    let n = step(s.prefs.ctx_for(&b), true).min(max);
                    set(s, |p| { p.ctx_limits.insert(b.clone(), n); });
                }))))
            .child(if custom {
                el().key(format!("lim-{id}-r")).hover(p.field).child(icon("refresh", p.dim))
                    .on_press(move |s: &mut State| set(s, |p| { p.ctx_limits.remove(&r); }))
            } else { el() });
        c = c.child(row().key(format!("mdl-{id}")).h(28.0).gap(10.0).center_y()
            .child(text(name).font(LABEL).color(p.text).grow(1.0).min_w(0.0).truncate())
            .child(text(max.map(tok).unwrap_or_else(|| "?".into())).font(SMALL).color(p.dim).w(100.0))
            .child(lim));
    }
    c.child(text("Effective context is your limit, capped by the provider's max. DCP soft and hard limits shrink to fit it.").font(SMALL).color(p.faint).wrap())
}

/// Botão de dropdown: abre um popover ancorado abaixo dele.
fn drop(key: String, label: String, pop: crate::Popover) -> El {
    let p = pal();
    row().key(key).h(26.0).px(8.0).gap(6.0).center_y().border(1.0, p.dim).hover(p.field)
        .child(text(label).font(SMALL).color(p.text).grow(1.0).truncate())
        .child(icon("chevron_down", p.dim))
        .on_press(move |s: &mut State| s.popover = if s.popover == pop { crate::Popover::None } else { pop })
}

fn k(n: u32) -> String {
    format!("{}k", n / 1000)
}

pub fn view(s: &State) -> El {
    let p = pal();
    let pr = &s.prefs;
    let mut c = col().key("config").w_full().max_w(760.0).px(22.0).py(18.0).gap(18.0)
        .child(row().gap(10.0).center_y()
            .child(icon("settings", p.text))
            .child(text("Settings").font(STRONG).color(p.text))
            .child(el().grow(1.0))
            .child(button("cfg-close", "Back").on_press(|s: &mut State| s.view = View::Chat)));

    // contas
    let util = s.util.as_ref();
    let providers = util.map(|u| u.providers.clone()).unwrap_or_default();
    let on = |id: &str| providers.iter().any(|p| p.0 == id && p.2);
    let status = |ok: bool| row().gap(6.0).center_y().child(icon(if ok { "check" } else { "lock" }, if ok { p.text } else { p.dim }))
        .child(text(if ok { "Connected" } else { "Not connected" }).font(SMALL).color(if ok { p.text } else { p.dim }));
    let mut acc = col().gap(6.0)
        .child(line("Claude", "Reuses this machine's Claude Code login", row().gap(8.0).center_y().child(status(s.logged_in))
            .child(button("cfg-claude", "Reload").on_press(|s: &mut State| s.logged_in = crate::auth::claude_logged_in()))))
        .child(line("ChatGPT / Codex", "OAuth sign in through omp, opens the browser", row().gap(8.0).center_y().child(status(on("openai-codex")))
            .child(button("cfg-codex", if on("openai-codex") { "Sign in again" } else { "Sign in" })
                .on_press(|s: &mut State| s.util_send(json!({"type": "login", "providerId": "openai-codex"}))))))
        .child(line("Gemini", "Google account OAuth (Gemini CLI flow), opens the browser", row().gap(8.0).center_y().child(status(on("google-gemini-cli")))
            .child(button("cfg-gemini", if on("google-gemini-cli") { "Sign in again" } else { "Sign in" })
                .on_press(|s: &mut State| s.util_send(json!({"type": "login", "providerId": "google-gemini-cli"}))))))
        .child(row().child(button("cfg-refresh", "Refresh status").on_press(|s: &mut State| s.util_send(json!({"type": "get_login_providers"})))));
    if let Some(u) = util {
        for (idx, it) in u.items.iter().enumerate() {
            let node = match it {
                Item::Notice(t) => text(t.clone()).font(SMALL).color(p.dim).wrap().selectable(true),
                Item::Error(t) => text(format!("Error: {t}")).font(SMALL).color(p.text).wrap(),
                Item::Ask { .. } => super::transcript::ask_block(s, u.id, idx, it),
                _ => continue,
            };
            acc = acc.child(node.key(format!("cfg-it-{idx}")));
        }
    }
    c = c.child(frame("f-acc", "accounts", acc));

    // agentes
    let mut ag = col().gap(2.0).child(row().gap(10.0).py(2.0)
        .child(text("AGENT").font(SMALL).color(p.faint).w(110.0))
        .child(text("MODEL").font(SMALL).color(p.faint).w(170.0))
        .child(text("EFFORT").font(SMALL).color(p.faint).w(90.0))
        .child(text("PROMPT").font(SMALL).color(p.faint)));
    for a in ALL {
        let (model, effort) = pr.agent_defaults(a);
        let name = a.name();
        ag = ag.child(row().key(format!("ag-{name}")).gap(10.0).h(30.0).center_y()
            .child(row().w(110.0).gap(8.0).center_y().child(super::sidebar::agent_square(a.icon(), 22.0, false)).child(text(name).font(LABEL).color(p.text)))
            .child(el().w(170.0).child(drop(format!("ag-m-{name}"), super::friendly_model(&model), crate::Popover::AgentModel(a))))
            .child(el().w(90.0).child(drop(format!("ag-e-{name}"), effort.clone(), crate::Popover::AgentEffort(a))))
            .child(button(format!("ag-p-{name}"), "Edit").on_press(move |_: &mut State| {
                let _ = std::process::Command::new("xdg-open").arg(a.prompt_file()).spawn();
            })));
    }
    ag = ag.child(super::sep(680.0))
        .child(line("Subagents can spawn subagents", "Lets a subagent call other agents", switch("sw-sub", pr.sub_invoke, |p| p.sub_invoke = !p.sub_invoke)))
        .child(line("Max parallel subagents", "Per session", stepper("st-par", pr.max_parallel.to_string(),
            |p| p.max_parallel = p.max_parallel.saturating_sub(1).max(1), |p| p.max_parallel = (p.max_parallel + 1).min(16))));
    c = c.child(frame("f-ag", "agents", ag));

    c = c.child(frame("f-models", "models · context", models_frame(s)));

    c = c.child(frame("f-mem", "memory", col().gap(2.0)
        .child(line("Vault", "basic-memory MCP over ~/vault", switch("sw-vault", pr.vault, |p| p.vault = !p.vault)))
        .child(line("Save reminder", "Nudges the agent to write decisions and findings", switch("sw-nudge", pr.vault_nudge, |p| p.vault_nudge = !p.vault_nudge)))
        .child(line("Vault before web", "Search notes before searching the web", switch("sw-first", pr.vault_first, |p| p.vault_first = !p.vault_first)))));

    c = c.child(frame("f-ctx", "context", col().gap(2.0)
        .child(line("Soft limit", "DCP starts pruning above this", stepper("st-soft", k(pr.dcp_soft),
            |p| p.dcp_soft = p.dcp_soft.saturating_sub(10_000).max(10_000), |p| p.dcp_soft = (p.dcp_soft + 10_000).min(p.dcp_hard))))
        .child(line("Hard limit", "DCP compresses above this", stepper("st-hard", k(pr.dcp_hard),
            |p| p.dcp_hard = p.dcp_hard.saturating_sub(10_000).max(p.dcp_soft), |p| p.dcp_hard = (p.dcp_hard + 10_000).min(400_000))))
        .child(line("Nudge every", "Requests between DCP nudges", stepper("st-every", format!("{} req", pr.dcp_every),
            |p| p.dcp_every = p.dcp_every.saturating_sub(1).max(1), |p| p.dcp_every = (p.dcp_every + 1).min(50))))
        .child(line("Deduplicate", "Drop repeated tool outputs", switch("sw-dedup", pr.dcp_dedup, |p| p.dcp_dedup = !p.dcp_dedup)))));

    c = c.child(frame("f-tools", "tools", col().gap(2.0)
        .child(line("rtk", "Condense every command's output", switch("sw-rtk", pr.rtk, |p| p.rtk = !p.rtk)))
        .child(line("Ask before destructive commands", "rm, reset, force push and the like", switch("sw-destr", pr.ask_destructive, |p| p.ask_destructive = !p.ask_destructive)))
        .child(line("Question timeout", "Unanswered questions resolve after this", stepper("st-to", format!("{} s", pr.ask_timeout),
            |p| p.ask_timeout = p.ask_timeout.saturating_sub(30).max(30), |p| p.ask_timeout = (p.ask_timeout + 30).min(1800))))));

    c = c.child(frame("f-look", "appearance", col().gap(2.0)
        .child(line("Dither field", "Animated background", switch("sw-dither", pr.dither, |p| p.dither = !p.dither)))
        .child(line("Field fps", "Background frame rate", stepper("st-fps", pr.fps.to_string(), |p| p.fps = p.fps.saturating_sub(2).max(2), |p| p.fps = (p.fps + 2).min(30))))
        .child(line("Font size", "Transcript and composer text", stepper("st-font", format!("{} px", pr.font), |p| p.font = p.font.saturating_sub(1).max(10), |p| p.font = (p.font + 1).min(18))))));

    let st = crate::omp::state_dir();
    c = c.child(frame("f-paths", "paths", col().gap(4.0)
        .child(text(format!("state    {}", st.display())).font(SMALL).color(p.dim))
        .child(text(format!("runtime  {}", crate::omp::runtime_dir().display())).font(SMALL).color(p.dim))
        .child(text(format!("agents   {}", crate::agents::agents_dir().display())).font(SMALL).color(p.dim))));
    super::float(col().key("config-scroll").grow(1.0).min_h(0.0).w_full()).child(c).scroll(rune::ScrollSpec { stick_bottom: false })
}
