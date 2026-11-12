//! Efeito de decodificação ASCII: letras e números passam por caracteres aleatórios até assentar;
//! espaço e pontuação ficam no lugar e servem de esqueleto (markdown continua válido).

/// Duração da decodificação da tela inteira, em segundos.
pub const SPAN: f64 = 0.6;
/// Caracteres finais de um texto em streaming que ainda estão decodificando.
const TAIL: usize = 24;
const NOISE: &[u8] = b"ABCDEFGHJKLMNPQRSTUVWXYZabcdefghkmnpqrstuvwxyz0123456789#$%&*+=?@<>/\\";

pub const THINKING: [&str; 10] = [
    "Thinking",
    "Reading the room",
    "Connecting the dots",
    "Weighing options",
    "Checking assumptions",
    "Tracing the code",
    "Sketching a plan",
    "Looking for edge cases",
    "Cross-checking",
    "Almost there",
];

fn noise(i: usize, frame: u64) -> char {
    let mut x = (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ frame.wrapping_mul(0xD1B5_4A32_D192_ED03);
    x ^= x >> 29;
    x = x.wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x ^= x >> 32;
    NOISE[(x % NOISE.len() as u64) as usize] as char
}

/// Texto com os caracteres a partir de `settled` trocados por ruído (só alfanuméricos).
pub fn scramble_from(s: &str, settled: usize, frame: u64) -> String {
    s.chars().enumerate().map(|(i, c)| if i >= settled && c.is_alphanumeric() { noise(i, frame) } else { c }).collect()
}

/// Progresso `k` de 0 a 1: a frente de decodificação varre o texto da esquerda para a direita.
pub fn sweep(s: &str, k: f32, frame: u64) -> String {
    if k >= 1.0 {
        return s.to_string();
    }
    let n = s.chars().count();
    scramble_from(s, (n as f32 * k.max(0.0)) as usize, frame)
}

pub fn frame(clock: f64) -> u64 {
    (clock * 30.0) as u64
}

/// Progresso da decodificação da tela desde `t0`.
pub fn screen(clock: f64, t0: f64) -> f32 {
    ((clock - t0) / SPAN) as f32
}

/// Fim de um texto em streaming ainda decodificando.
pub fn streaming(s: &str, clock: f64) -> String {
    let n = s.chars().count();
    scramble_from(s, n.saturating_sub(TAIL), frame(clock))
}

/// Frase de thinking atual: troca a cada 2,5 s e decodifica nos primeiros 0,5 s.
pub fn thinking(clock: f64) -> String {
    let i = (clock / 2.5) as usize % THINKING.len();
    let t = (clock % 2.5) as f32;
    sweep(THINKING[i], t / 0.5, frame(clock))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keeps_skeleton_and_settles() {
        let s = "## Hi, `x` - 42.";
        let z = sweep(s, 0.0, 7);
        assert_eq!(z.chars().count(), s.chars().count());
        for (a, b) in s.chars().zip(z.chars()) {
            if !a.is_alphanumeric() {
                assert_eq!(a, b);
            }
        }
        assert_eq!(sweep(s, 1.0, 7), s);
        assert_eq!(scramble_from("abc", 3, 1), "abc");
    }
}
