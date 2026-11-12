//! Tema pixelized (preto e branco, raio zero, mono) e peças comuns da UI.

pub mod composer;
pub mod config;
pub mod dither;
pub mod panel;
pub mod sidebar;
pub mod transcript;

use rune::tok::{Family, TextStyle};
use rune::{Color, El, Pixels, el, row, text};

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

thread_local! {
    static BRAILLE_PX: std::cell::RefCell<std::collections::HashMap<(u32, [u8; 4]), Pixels>> = Default::default();
}

/// Texto com braille desenhado em pixels: as fontes embutidas não têm o bloco U+2800, e o
/// fallback virava barras verticais. Os bits do codepoint são os próprios pontos.
pub fn glyph(s: &str, style: TextStyle, color: Color) -> El {
    let mut r = row().gap(1.0).center_y();
    let mut plain = String::new();
    for ch in s.chars() {
        let cp = ch as u32;
        if (0x2800..=0x28FF).contains(&cp) {
            if !plain.is_empty() {
                r = r.child(text(std::mem::take(&mut plain)).font(style).color(color));
            }
            r = r.child(el().size(7.0, 13.0).pixels_fit(&braille_px(cp - 0x2800, color), rune::SurfaceFit::Fill));
        } else {
            plain.push(ch);
        }
    }
    if !plain.is_empty() {
        r = r.child(text(plain).font(style).color(color));
    }
    r
}

fn braille_px(bits: u32, color: Color) -> Pixels {
    let c = [color.r, color.g, color.b, color.a].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    BRAILLE_PX.with(|m| {
        m.borrow_mut().entry((bits, c)).or_insert_with(|| {
            // 2 colunas x 4 linhas de pontos 2x2 com 1px de folga: 7x13 com borda de 1px
            const DOTS: [(u32, u32); 8] = [(0, 0), (0, 1), (0, 2), (1, 0), (1, 1), (1, 2), (0, 3), (1, 3)];
            let (w, h) = (7u32, 13u32);
            let mut rgba = vec![0u8; (w * h * 4) as usize];
            for (i, (dx, dy)) in DOTS.iter().enumerate() {
                if bits & (1 << i) == 0 {
                    continue;
                }
                for y in 0..2 {
                    for x in 0..2 {
                        let (px, py) = (1 + dx * 3 + x, 1 + dy * 3 + y);
                        let o = ((py * w + px) * 4) as usize;
                        rgba[o..o + 4].copy_from_slice(&c);
                    }
                }
            }
            let p = Pixels::new();
            p.set(w, h, rgba);
            p
        }).clone()
    })
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
