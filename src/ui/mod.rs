//! Tema float (preto e branco, raio zero, mono, sombras duras) e peças comuns da UI.

pub mod composer;
pub mod config;
pub mod decode;
pub mod dither;
pub mod panel;
pub mod sidebar;
pub mod transcript;

use rune::tok::{Family, TextStyle};
use rune::{Color, El, Pixels, el, row, text};

pub const PIXEL: Family = Family::Named("Pixelify Sans");

pub struct Pal {
    pub bg: Color,
    pub panel: Color,
    pub field: Color,
    pub button: Color,
    pub line: Color,
    pub text: Color,
    pub dim: Color,
    pub faint: Color,
}

pub fn pal() -> &'static Pal {
    static P: std::sync::OnceLock<Pal> = std::sync::OnceLock::new();
    P.get_or_init(|| {
        let h = |s: &str| rune::color::hex(s).unwrap();
        Pal {
            bg: h("#000000"),
            panel: h("#000000"),
            field: h("#161616"),
            button: h("#242424"),
            line: h("#8a8a8a"),
            text: h("#ececec"),
            dim: h("#8a8a8a"),
            faint: h("#5a5a5a"),
        }
    })
}

/// Texto do corpo no tamanho escolhido em Settings.
pub fn body(s: &crate::State) -> TextStyle {
    let f = s.prefs.font as f32;
    TextStyle::new(f, 400.0).lh(f + 6.0)
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
    el().key(key.into()).px(10.0).py(5.0).fill(p.bg).hover(p.field).border(1.0, p.text)
        .child(text(label.into()).font(LABEL).color(p.text))
}

static CATALOG: std::sync::RwLock<Vec<(String, String)>> = std::sync::RwLock::new(Vec::new());

/// Catálogo de modelos com credencial (provedor/id, nome).
pub fn catalog() -> Vec<(String, String)> {
    CATALOG.read().map(|c| c.clone()).unwrap_or_default()
}

pub fn reload_catalog() {
    let path = crate::omp::state_dir().join("ompdsh/models.json");
    let Ok(raw) = std::fs::read_to_string(path) else { return };
    let Ok(v) = serde_json::from_str::<Vec<serde_json::Value>>(&raw) else { return };
    let list: Vec<(String, String)> = v.iter().filter_map(|m| {
        let (p, id) = (m["provider"].as_str()?, m["id"].as_str()?);
        if p == "openrouter" {
            return None;
        }
        Some((format!("{p}/{id}"), m["name"].as_str().unwrap_or(id).to_string()))
    }).collect();
    if let Ok(mut c) = CATALOG.write() {
        *c = list;
    }
}

pub fn friendly_model(id: &str) -> String {
    if let Some((_, n)) = catalog().into_iter().find(|m| m.0 == id) {
        return n;
    }
    for (m, name) in crate::MODELS {
        if *m == id {
            return name.to_string();
        }
    }
    id.rsplit('/').next().unwrap_or(id).to_string()
}

fn rgba8(c: Color) -> [u8; 4] {
    [c.r, c.g, c.b, c.a].map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8)
}

fn px_from(w: u32, h: u32, on: impl Fn(u32, u32) -> bool, c: [u8; 4]) -> Pixels {
    let mut rgba = vec![0u8; (w * h * 4) as usize];
    for y in 0..h {
        for x in 0..w {
            if on(x, y) {
                let o = ((y * w + x) * 4) as usize;
                rgba[o..o + 4].copy_from_slice(&c);
            }
        }
    }
    let p = Pixels::new();
    p.set(w, h, rgba);
    p
}

fn icon_rows(name: &str) -> Option<&'static [&'static str]> {
    static SET: std::sync::OnceLock<std::collections::HashMap<&'static str, Vec<&'static str>>> = std::sync::OnceLock::new();
    let set = SET.get_or_init(|| {
        let mut m = std::collections::HashMap::new();
        let mut lines = include_str!("../../assets/icons/icons.txt").lines();
        while let Some(head) = lines.next() {
            let Some(n) = head.split_whitespace().next() else { continue };
            m.insert(n, lines.by_ref().take(16).collect());
        }
        m
    });
    set.get(name).map(|v| v.as_slice())
}

thread_local! {
    static PX: std::cell::RefCell<std::collections::HashMap<(String, [u8; 4]), Pixels>> = Default::default();
}

fn cached(key: String, c: Color, make: impl FnOnce([u8; 4]) -> Pixels) -> Pixels {
    let c = rgba8(c);
    PX.with(|m| m.borrow_mut().entry((key, c)).or_insert_with(|| make(c)).clone())
}

/// Ícone pixel 16x16 de `assets/icons/icons.txt`, desenhado em `size` pontos.
pub fn icon_sized(name: &str, size: f32, color: Color) -> El {
    let px = cached(format!("i:{name}"), color, |c| {
        let rows = icon_rows(name).unwrap_or(&[]);
        px_from(16, 16, |x, y| rows.get(y as usize).and_then(|r| r.as_bytes().get(x as usize)) == Some(&b'#'), c)
    });
    el().size(size, size).pixels_fit(&px, rune::SurfaceFit::Fill)
}

pub fn icon(name: &str, color: Color) -> El {
    icon_sized(name, 16.0, color)
}

/// Padrões de preenchimento: sólido, xadrez, listras, pontos, grade esparsa e pontilhado de linha.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Dither {
    Solid,
    Checker,
    Stripes,
    Sparse,
}

pub fn dither_px(kind: Dither, w: u32, h: u32, color: Color) -> Pixels {
    cached(format!("d:{kind:?}:{w}x{h}"), color, |c| {
        px_from(w, h, |x, y| match kind {
            Dither::Solid => true,
            Dither::Checker => (x + y) % 2 == 0,
            Dither::Stripes => x % 3 == 0,
            Dither::Sparse => x % 4 == 0 && y % 2 == 0 && (x / 4 + y / 2) % 2 == 0,
        }, c)
    })
}

/// Bloco de `w`x`h` pontos com um padrão de dither.
pub fn dither(kind: Dither, w: f32, h: f32, color: Color) -> El {
    el().size(w, h).pixels_fit(&dither_px(kind, w.max(1.0) as u32, h.max(1.0) as u32, color), rune::SurfaceFit::Fill)
}

/// Separador pontilhado horizontal.
pub fn sep(w: f32) -> El {
    let px = cached(format!("sep:{w}"), pal().dim, |c| px_from(w as u32, 1, |x, _| x % 2 == 0, c));
    el().size(w, 1.0).pixels_fit(&px, rune::SurfaceFit::Fill)
}

/// Sombra dura deslocada, sem blur.
pub fn hard(e: El, d: f32, color: Color) -> El {
    e.box_shadow(rune::Shadow { x: d, y: d, blur: 0.0, spread: 0.0, color, inset: false })
}

/// Painel solto do float: fundo preto, borda clara e sombra branca dura.
pub fn float(e: El) -> El {
    let p = pal();
    hard(e.fill(p.bg).border(1.0, p.text), 4.0, p.text)
}

/// Caixa com título na borda (estilo box).
pub fn frame(key: impl Into<String>, title: &str, body: El) -> El {
    let p = pal();
    el().key(key.into()).w_full().pt(6.0)
        .child(body.w_full().border(1.0, p.dim).px(10.0).pt(14.0).pb(10.0))
        .child(el().absolute(10.0, 0.0).px(5.0).fill(p.bg).child(text(title.to_uppercase()).font(SMALL).color(p.dim)))
}
