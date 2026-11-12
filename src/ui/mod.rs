//! Tema pixelized (preto e branco, raio zero, mono) e peças comuns da UI.

pub mod composer;
pub mod config;
pub mod dither;
pub mod panel;
pub mod sidebar;
pub mod transcript;

use rune::tok::{Family, TextStyle};
use rune::{Color, El, el, text};

pub const PIXEL: Family = Family::Named("Pixelify Sans");

pub struct Pal {
    pub bg: Color,
    pub frame: Color,
    pub panel: Color,
    pub field: Color,
    pub button: Color,
    pub line: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
    pub danger: Color,
}

pub fn pal() -> &'static Pal {
    static P: std::sync::OnceLock<Pal> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let h = |s: &str| rune::color::hex(s).unwrap();
        Pal {
            bg: h("#0a0a0a"),
            frame: h("#0a0a0a"),
            panel: h("#111111"),
            field: h("#181818"),
            button: h("#222222"),
            line: h("#2a2a2a"),
            text: h("#f2f2f2"),
            dim: h("#8c8c8c"),
            faint: h("#5a5a5a"),
            danger: h("#ff6b6b"),
        }
    })
}

pub const BODY: TextStyle = TextStyle::new(13.0, 400.0).lh(19.0);
pub const SMALL: TextStyle = TextStyle::new(11.0, 400.0).lh(15.0);
pub const LABEL: TextStyle = TextStyle::new(12.0, 400.0).lh(16.0);
pub const STRONG: TextStyle = TextStyle::new(12.0, 700.0).lh(16.0);
pub const DISPLAY: TextStyle = TextStyle::new(28.0, 400.0).family(PIXEL);
pub const BRAND: TextStyle = TextStyle::new(16.0, 400.0).family(PIXEL);

pub const BRAILLE: [&str; 8] = ["⣾", "⣽", "⣻", "⢿", "⡿", "⣟", "⣯", "⣷"];

pub fn spinner(frame: usize) -> &'static str {
    BRAILLE[frame % BRAILLE.len()]
}

/// Botão quadrado, sem raio, com hover em cinza.
pub fn button(key: impl Into<String>, label: impl Into<String>) -> El {
    let p = pal();
    el().key(key.into()).px(10.0).py(5.0).fill(p.button).hover(p.line).border(1.0, p.line)
        .child(text(label.into()).font(LABEL).color(p.text))
}

pub fn friendly_model(id: &str) -> String {
    for (m, name) in crate::MODELS {
        if *m == id {
            return name.to_string();
        }
    }
    id.rsplit('/').next().unwrap_or(id).to_string()
}
