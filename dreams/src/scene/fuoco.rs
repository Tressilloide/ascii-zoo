//! Falò: il classico fuoco a propagazione di calore, con le braci che salgono
//! e la legna in basso.

use super::Scena;
use crate::griglia::{da_rampa, rumore, scala, Caso, Griglia};

const COLORI: [[u8; 3]; 6] = [[60, 8, 4], [170, 30, 8], [235, 90, 20], [255, 160, 40], [255, 225, 120], [255, 250, 235]];

struct Brace {
    x: f32,
    y: f32,
    vx: f32,
    vita: f32,
}

pub struct Fuoco {
    w: usize,
    h: usize,
    calore: Vec<f32>,
    braci: Vec<Brace>,
    caso: Caso,
    resto: f32,
}

impl Fuoco {
    pub fn new(w: usize, h: usize, seme: u64) -> Self {
        Fuoco { w, h, calore: vec![0.0; w * (h + 2)], braci: Vec::new(), caso: Caso::new(seme), resto: 0.0 }
    }

    /// un passo della simulazione (a frequenza fissa, così non dipende dagli fps)
    fn simula(&mut self, t: f32) {
        let (w, h) = (self.w, self.h);
        let base = h * 86 / 100; // il fuoco nasce sopra la legna
        let centro = w as f32 / 2.0;
        let largo = (w as f32 * 0.17).max(6.0);
        // la fonte di calore: più forte al centro, irregolare
        for y in base..base + 2 {
            for x in 0..w {
                let d = ((x as f32 - centro) / largo).abs();
                let forza = (1.0 - d * d).max(0.0);
                let tremolio = rumore(x as f32 * 0.35, t * 3.0);
                let v = if self.caso.moneta(0.7) { forza * (0.7 + 0.6 * tremolio) } else { 0.0 };
                self.calore[y * w + x] = v.min(1.0);
            }
        }
        // il calore sale e si raffredda
        let raffredda = 1.25 / h as f32;
        let vento = (rumore(t * 0.3, 7.0) - 0.5) * 1.5;
        for y in 0..base {
            for x in 0..w {
                let sx = (x as f32 + vento * self.caso.f()).round() as isize;
                let somma = {
                    let c = &self.calore;
                    let pesca = |dx: isize, dy: usize| {
                        let xx = (sx + dx).clamp(0, w as isize - 1) as usize;
                        c[(y + dy).min(h + 1) * w + xx]
                    };
                    pesca(-1, 1) + pesca(0, 1) + pesca(1, 1) + pesca(0, 2)
                };
                let v = somma / 4.0 - raffredda * (0.6 + self.caso.f() * 0.8);
                self.calore[y * w + x] = v.max(0.0);
            }
        }
        // braci
        if self.caso.moneta(0.35) && self.braci.len() < 80 {
            let x = centro + self.caso.tra(-largo * 0.6, largo * 0.6);
            self.braci.push(Brace { x, y: base as f32 * 0.7, vx: self.caso.tra(-3.0, 3.0), vita: self.caso.tra(1.5, 4.0) });
        }
    }
}

impl Scena for Fuoco {
    fn passo(&mut self, g: &mut Griglia, dt: f32, t: f32) {
        self.resto += dt;
        let passo = 1.0 / 30.0;
        let mut n = 0;
        while self.resto >= passo && n < 4 {
            self.simula(t);
            self.resto -= passo;
            n += 1;
        }
        self.resto = self.resto.min(passo);

        let (w, h) = (self.w, self.h);
        let base = h * 86 / 100;
        for y in 0..base {
            for x in 0..w {
                let v = self.calore[y * w + x];
                if v > 0.04 {
                    let col = scala(&COLORI, v);
                    g.metti(x as i32, y as i32, da_rampa(v.powf(0.8)), col, (v * 1.6).min(1.0), v * 0.9);
                }
            }
        }
        // braci che salgono ondeggiando
        for b in self.braci.iter_mut() {
            b.y -= 6.0 * dt;
            b.x += (b.vx + (t * 2.0 + b.y * 0.3).sin() * 2.0) * dt;
            b.vita -= dt;
            let f = (b.vita / 2.0).min(1.0);
            g.metti_sopra(b.x as i32, b.y as i32, if f > 0.6 { b'*' } else { b'.' }, [255, 150, 50], f, 0.7 * f);
        }
        self.braci.retain(|b| b.vita > 0.0 && b.y > 0.0);

        // la legna: tronchi incrociati sotto il fuoco
        let centro = w as i32 / 2;
        let largo = (w as f32 * 0.13) as i32;
        let legno = [120, 70, 35];
        for (i, y) in (base..h.min(base + 4)).enumerate() {
            let y = y as i32;
            for x in centro - largo + i as i32 * 2..centro + largo - i as i32 * 2 {
                let ch = if (x + y) % 7 == 0 { b'=' } else if i == 0 { b'_' } else { b'#' };
                let brace = self.calore[(base as usize) * w + (x.clamp(0, w as i32 - 1)) as usize];
                let col = scala(&[legno, [255, 120, 30]], brace * 0.6);
                g.metti(x, y, ch, col, 0.35 + brace * 0.4, brace * 0.3);
            }
        }
        // ciottoli intorno
        for (i, x) in [centro - largo - 3, centro - largo - 1, centro + largo, centro + largo + 2].iter().enumerate() {
            g.testo(*x, base as i32 + 2 + (i % 2) as i32, "()", [110, 110, 120], 0.3, 0.0);
        }
    }
}
