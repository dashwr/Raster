//! Campo pontilhado preto e branco (Bayer 8×8 sobre manchas de luz que derivam devagar), gerado numa
//! thread e entregue por `Pixels::set`; e avatares pixelados determinísticos para o swarm.

use rune::Pixels;

const BAYER: [[u8; 8]; 8] = [
    [0, 32, 8, 40, 2, 34, 10, 42],
    [48, 16, 56, 24, 50, 18, 58, 26],
    [12, 44, 4, 36, 14, 46, 6, 38],
    [60, 28, 52, 20, 62, 30, 54, 22],
    [3, 35, 11, 43, 1, 33, 9, 41],
    [51, 19, 59, 27, 49, 17, 57, 25],
    [15, 47, 7, 39, 13, 45, 5, 37],
    [63, 31, 55, 23, 61, 29, 53, 21],
];

/// Brilho do campo em (u, v) no tempo t: duas luzes que orbitam e um poço escuro.
fn field(u: f32, v: f32, t: f32) -> f32 {
    let blob = |cx: f32, cy: f32, r: f32| {
        let (dx, dy) = (u - cx, (v - cy) * 1.4);
        (-(dx * dx + dy * dy) / (r * r)).exp()
    };
    let a = blob(0.68 + 0.08 * (t * 0.11).sin(), 0.30 + 0.06 * (t * 0.07).cos(), 0.22);
    let b = blob(0.22 + 0.06 * (t * 0.05).cos(), 0.72 + 0.05 * (t * 0.09).sin(), 0.30) * 0.55;
    let well = blob(0.48, 0.78, 0.12);
    let grain = ((u * 91.0 + t * 0.3).sin() * (v * 67.0 - t * 0.2).cos()) * 0.04;
    (a + b + grain - well * 0.8).clamp(0.0, 1.0)
}

/// Rende um quadro: `cell` pixels de lado por ponto da grade, para ler como pixel art.
pub fn render(w: u32, h: u32, cell: u32, t: f32, out: &mut Vec<u8>) {
    out.clear();
    out.resize((w * h * 4) as usize, 0);
    let (gw, gh) = (w.div_ceil(cell), h.div_ceil(cell));
    for gy in 0..gh {
        for gx in 0..gw {
            let l = field(gx as f32 / gw as f32, gy as f32 / gh as f32, t);
            let th = (BAYER[(gy % 8) as usize][(gx % 8) as usize] as f32 + 0.5) / 64.0;
            let c = if l > th { 236u8 } else { 10u8 };
            for y in gy * cell..((gy + 1) * cell).min(h) {
                let row = (y * w) as usize;
                for x in gx * cell..((gx + 1) * cell).min(w) {
                    let i = (row + x as usize) * 4;
                    out[i..i + 4].copy_from_slice(&[c, c, c, 255]);
                }
            }
        }
    }
}

/// Anima o campo numa thread própria, a ~8 quadros por segundo.
pub fn animate(px: Pixels, w: u32, h: u32) {
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut buf = Vec::new();
        loop {
            render(w, h, 3, start.elapsed().as_secs_f32(), &mut buf);
            px.set(w, h, buf.clone());
            std::thread::sleep(std::time::Duration::from_millis(125));
        }
    });
}

/// Avatar 8×8 espelhado a partir de uma semente, ampliado `scale` vezes.
pub fn avatar(seed: u64, scale: u32) -> Pixels {
    let mut x = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
    let mut bits = [[false; 8]; 8];
    for row in &mut bits {
        for c in 0..4 {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            let on = x % 100 < 48;
            row[c] = on;
            row[7 - c] = on;
        }
    }
    let n = 8 * scale;
    let mut rgba = vec![0u8; (n * n * 4) as usize];
    for y in 0..n {
        for xx in 0..n {
            let on = bits[(y / scale) as usize][(xx / scale) as usize];
            let c = if on { 242u8 } else { 17u8 };
            let i = ((y * n + xx) * 4) as usize;
            rgba[i..i + 4].copy_from_slice(&[c, c, c, 255]);
        }
    }
    let px = Pixels::new();
    px.set(n, n, rgba);
    px
}

#[cfg(test)]
mod tests {
    #[test]
    fn render_fills_every_pixel_black_or_white() {
        let mut buf = Vec::new();
        super::render(30, 20, 3, 1.0, &mut buf);
        assert_eq!(buf.len(), 30 * 20 * 4);
        assert!(buf.chunks(4).all(|p| (p[0] == 236 || p[0] == 10) && p[3] == 255));
        assert!(buf.chunks(4).any(|p| p[0] == 236) && buf.chunks(4).any(|p| p[0] == 10));
    }
}
