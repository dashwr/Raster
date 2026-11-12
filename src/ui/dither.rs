//! Campo pontilhado preto e branco (Bayer 8×8 sobre manchas de luz que derivam devagar), gerado numa
//! thread e entregue por `Pixels::set`.

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

/// Anima o campo numa thread própria a `fps` quadros por segundo; 0 pausa.
pub fn animate(px: Pixels, w: u32, h: u32, fps: std::sync::Arc<std::sync::atomic::AtomicU32>) {
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut buf = Vec::new();
        loop {
            let f = fps.load(std::sync::atomic::Ordering::Relaxed);
            if f > 0 || buf.is_empty() {
                render(w, h, 3, start.elapsed().as_secs_f32(), &mut buf);
                px.set(w, h, buf.clone());
            }
            std::thread::sleep(std::time::Duration::from_millis(if f > 0 { 1000 / f as u64 } else { 250 }));
        }
    });
}

/// Aurora: cortinas que ondulam na horizontal, com raios verticais, em blocos de cinza com alfa
/// em poucos degraus (pixelada, sem dither). Fraca de propósito: fica atrás do texto.
pub fn aurora(w: u32, h: u32, cell: u32, t: f32, out: &mut Vec<u8>) {
    out.clear();
    out.resize((w * h * 4) as usize, 0);
    let (gw, gh) = (w.div_ceil(cell), h.div_ceil(cell));
    for gx in 0..gw {
        let u = gx as f32 / gw as f32;
        let curtain = |phase: f32, center: f32, amp: f32| {
            let c = center + amp * (u * 5.0 + t * 0.35 + phase).sin() + amp * 0.5 * (u * 11.0 - t * 0.5 + phase).sin();
            let rays = 0.55 + 0.45 * (u * 47.0 + t * 0.9 + phase * 3.0).sin() * (u * 23.0 - t * 0.4).cos();
            (c, rays)
        };
        let bands = [curtain(0.0, 0.30, 0.08), curtain(2.1, 0.48, 0.06), curtain(4.2, 0.20, 0.05)];
        for gy in 0..gh {
            let v = gy as f32 / gh as f32;
            let mut l = 0.0f32;
            for (i, (c, rays)) in bands.iter().enumerate() {
                // cortina: borda de baixo nítida, cauda longa para cima
                let d = v - c;
                let k = if d > 0.0 { (-(d * d) / 0.002).exp() } else { (d / 0.18).exp() };
                l += k * rays * [0.55, 0.4, 0.3][i];
            }
            let l = (l * (0.85 + 0.15 * (t * 0.7).sin())).clamp(0.0, 1.0);
            let a = ((l * 6.0).floor() / 6.0 * 200.0) as u8;
            if a == 0 {
                continue;
            }
            for y in gy * cell..((gy + 1) * cell).min(h) {
                for x in gx * cell..((gx + 1) * cell).min(w) {
                    let i = ((y * w + x) * 4) as usize;
                    out[i..i + 4].copy_from_slice(&[170, 170, 170, a]);
                }
            }
        }
    }
}

/// Anima a aurora a ~10 fps enquanto `on` estiver ligado; parada não gasta CPU.
pub fn animate_aurora(px: Pixels, w: u32, h: u32, on: std::sync::Arc<std::sync::atomic::AtomicBool>) {
    std::thread::spawn(move || {
        let start = std::time::Instant::now();
        let mut buf = Vec::new();
        loop {
            if on.load(std::sync::atomic::Ordering::Relaxed) {
                aurora(w, h, 2, start.elapsed().as_secs_f32(), &mut buf);
                px.set(w, h, buf.clone());
                std::thread::sleep(std::time::Duration::from_millis(100));
            } else {
                std::thread::sleep(std::time::Duration::from_millis(200));
            }
        }
    });
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

    #[test]
    fn aurora_is_gray_steps_on_transparent() {
        let mut buf = Vec::new();
        super::aurora(200, 120, 4, 3.0, &mut buf);
        let lit = buf.chunks(4).filter(|p| p[3] > 0).count();
        assert!(lit > 0);
        assert!(buf.chunks(4).all(|p| p[3] == 0 || p[0] == 170));
        assert!(buf.chunks(4).all(|p| p[3] <= 200));
    }
}
