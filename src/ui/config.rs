//! Configs: provedores (login do Claude reaproveitado, login do Codex pelo OAuth do omp) e caminhos.

use rune::{El, col, el, row, text};
use serde_json::json;

use super::{BODY, LABEL, SMALL, STRONG, button, pal};
use crate::session::Item;
use crate::{State, View};

pub fn view(s: &State) -> El {
    let p = pal();
    let mut c = col().key("config").w_full().max_w(720.0).px(28.0).py(20.0).gap(14.0)
        .child(row().gap(10.0).center_y()
            .child(text("configs").font(STRONG).color(p.text))
            .child(el().grow(1.0))
            .child(button("cfg-close", "voltar").on_press(|s: &mut State| s.view = View::Chat)));

    // claude: login do Claude Code da máquina
    let claude = if s.logged_in { "usando o login do Claude Code desta máquina" } else { "sem login: rode `claude` e entre na sua conta" };
    c = c.child(card("Claude (assinatura)", claude, s.logged_in).child(
        row().child(button("cfg-claude", "reler login").on_press(|s: &mut State| s.logged_in = crate::auth::claude_logged_in())),
    ));

    // demais provedores pela sessão utilitária
    let util = s.util.as_ref();
    let providers = util.map(|u| u.providers.clone()).unwrap_or_default();
    let codex = providers.iter().find(|p| p.0 == "openai-codex").map(|p| p.2).unwrap_or(false);
    c = c.child(card("ChatGPT / Codex (assinatura)", if codex { "logado" } else { "abre o login OAuth do ChatGPT no navegador" }, codex).child(
        row().gap(8.0)
            .child(button("cfg-codex", if codex { "entrar de novo" } else { "entrar" }).on_press(|s: &mut State| s.util_send(json!({"type": "login", "providerId": "openai-codex"}))))
            .child(button("cfg-refresh", "atualizar status").on_press(|s: &mut State| s.util_send(json!({"type": "get_login_providers"})))),
    ));
    let others: Vec<_> = providers.iter().filter(|p| p.2 && p.0 != "anthropic" && p.0 != "openai-codex").map(|p| p.1.clone()).collect();
    if !others.is_empty() {
        c = c.child(text(format!("outros provedores autenticados no omp: {}", others.join(", "))).font(SMALL).color(p.faint).wrap());
    }

    // mensagens do fluxo de login (URL aberta, pedidos de código)
    if let Some(u) = util {
        for (idx, it) in u.items.iter().enumerate() {
            let node = match it {
                Item::Notice(t) => text(format!("— {t}")).font(SMALL).color(p.dim).wrap().selectable(true),
                Item::Error(t) => text(format!("erro: {t}")).font(SMALL).color(p.danger).wrap(),
                Item::Ask { .. } => super::transcript::ask_block(s, u.id, idx, it),
                _ => continue,
            };
            c = c.child(node.key(format!("cfg-it-{idx}")));
        }
    }

    let st = crate::omp::state_dir();
    c = c.child(col().gap(4.0).pt(10.0)
        .child(text("caminhos").font(LABEL).color(p.dim))
        .child(text(format!("estado: {}", st.display())).font(SMALL).color(p.faint))
        .child(text(format!("runtime: {}", crate::omp::runtime_dir().display())).font(SMALL).color(p.faint))
        .child(text(format!("agentes: {}", crate::agents::agents_dir().display())).font(SMALL).color(p.faint)));
    col().key("config-scroll").grow(1.0).min_h(0.0).w_full().child(c).scroll(rune::ScrollSpec { stick_bottom: false })
}

fn card(title: &str, status: &str, ok: bool) -> El {
    let p = pal();
    col().w_full().px(14.0).py(12.0).gap(8.0).fill(p.panel).border(1.0, if ok { p.dim } else { p.line })
        .child(row().gap(8.0)
            .child(text(if ok { "●" } else { "○" }).font(LABEL).color(p.text))
            .child(text(title.to_string()).font(BODY).color(p.text)))
        .child(text(status.to_string()).font(SMALL).color(p.dim).wrap())
}
