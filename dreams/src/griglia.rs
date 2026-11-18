//! La griglia di caratteri che le scene disegnano, più due attrezzi comuni:
//! un generatore di numeri casuali e un rumore morbido per le cose che ondeggiano.

#[derive(Clone, Copy, Default)]
pub struct Cella {
    /// carattere ASCII (0 o b' ' = vuota)
    pub ch: u8,
    /// colore pieno, poi moltiplicato per la luce
    pub col: [u8; 3],
    /// luce da 0 a 1
    pub luce: f32,
    /// quanto bagliore intorno al carattere (0 = niente)
    pub alone: f32,
}

pub struct Griglia {
    pub w: usize,
    pub h: usize,
    pub celle: Vec<Cella>,
}

impl Griglia {
    pub fn new(w: usize, h: usize) -> Self {
        Griglia { w, h, celle: vec![Cella::default(); w * h] }
    }

    pub fn pulisci(&mut self) {
        self.celle.fill(Cella::default());
    }

    /// scrive un carattere; fuori dai bordi non fa niente
    pub fn metti(&mut self, x: i32, y: i32, ch: u8, col: [u8; 3], luce: f32, alone: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let c = &mut self.celle[y as usize * self.w + x as usize];
        c.ch = ch;
        c.col = col;
        c.luce = luce.clamp(0.0, 1.0);
        c.alone = alone;
    }

    /// come `metti`, ma solo se più luminoso di quello che c'è già
    pub fn metti_sopra(&mut self, x: i32, y: i32, ch: u8, col: [u8; 3], luce: f32, alone: f32) {
        if x < 0 || y < 0 || x as usize >= self.w || y as usize >= self.h {
            return;
        }
        let c = &self.celle[y as usize * self.w + x as usize];
        if c.ch <= b' ' || luce >= c.luce {
            self.metti(x, y, ch, col, luce, alone);
        }
    }

    pub fn testo(&mut self, x: i32, y: i32, s: &str, col: [u8; 3], luce: f32, alone: f32) {
        for (i, b) in s.bytes().enumerate() {
            if b != b' ' {
                self.metti(x + i as i32, y, b, col, luce, alone);
            }
        }
    }
}

/// Rampa di densità: dal più leggero al più pieno.
pub const RAMPA: &[u8] = b" .,:;-=+*oxO#%@";

/// carattere della rampa per un valore 0..1
pub fn da_rampa(v: f32) -> u8 {
    let v = v.clamp(0.0, 0.999);
    RAMPA[(v * RAMPA.len() as f32) as usize]
}

/// xorshift64*: veloce, abbastanza casuale per la grafica
pub struct Caso(u64);

impl Caso {
    pub fn new(seme: u64) -> Self {
        Caso(seme.max(1).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    pub fn u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    /// 0..1
    pub fn f(&mut self) -> f32 {
        (self.u64() >> 40) as f32 / (1u64 << 24) as f32
    }
    pub fn tra(&mut self, a: f32, b: f32) -> f32 {
        a + (b - a) * self.f()
    }
    pub fn intero(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { (self.u64() % n as u64) as usize }
    }
    pub fn moneta(&mut self, p: f32) -> bool {
        self.f() < p
    }
}

fn hash(i: i32, j: i32) -> f32 {
    let mut h = (i as u32).wrapping_mul(374_761_393) ^ (j as u32).wrapping_mul(668_265_263);
    h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
    h ^= h >> 16;
    (h & 0xFFFF) as f32 / 65535.0
}

fn liscia(t: f32) -> f32 {
    t * t * (3.0 - 2.0 * t)
}

/// rumore morbido 2D, valori 0..1
pub fn rumore(x: f32, y: f32) -> f32 {
    let (xi, yi) = (x.floor() as i32, y.floor() as i32);
    let (xf, yf) = (liscia(x - xi as f32), liscia(y - yi as f32));
    let a = hash(xi, yi);
    let b = hash(xi + 1, yi);
    let c = hash(xi, yi + 1);
    let d = hash(xi + 1, yi + 1);
    let ab = a + (b - a) * xf;
    let cd = c + (d - c) * xf;
    ab + (cd - ab) * yf
}

/// rumore a più ottave, valori circa 0..1
pub fn frattale(x: f32, y: f32, ottave: u32) -> f32 {
    let (mut tot, mut amp, mut freq, mut norm) = (0.0, 1.0, 1.0, 0.0);
    for _ in 0..ottave {
        tot += rumore(x * freq, y * freq) * amp;
        norm += amp;
        amp *= 0.5;
        freq *= 2.0;
    }
    tot / norm
}

/// mescola due colori
pub fn mescola(a: [u8; 3], b: [u8; 3], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0);
    [
        (a[0] as f32 + (b[0] as f32 - a[0] as f32) * t) as u8,
        (a[1] as f32 + (b[1] as f32 - a[1] as f32) * t) as u8,
        (a[2] as f32 + (b[2] as f32 - a[2] as f32) * t) as u8,
    ]
}

/// colore lungo una scala di tappe (t da 0 a 1)
pub fn scala(tappe: &[[u8; 3]], t: f32) -> [u8; 3] {
    let t = t.clamp(0.0, 1.0) * (tappe.len() - 1) as f32;
    let i = (t.floor() as usize).min(tappe.len() - 2);
    mescola(tappe[i], tappe[i + 1], t - i as f32)
}
