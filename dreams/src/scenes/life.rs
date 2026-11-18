//! Il gioco della vita di Conway: ogni cellula è disegnata con due caratteri
//! (così resta quadrata), cambia colore invecchiando e lascia una scia quando muore.
//! Se il mondo si ferma, arriva una pioggia di nuove cellule.

use super::Scena;
use crate::grid::{scala, Caso, Griglia};

const ETA: [[u8; 3]; 5] = [[120, 255, 230], [90, 220, 120], [230, 230, 90], [255, 140, 80], [230, 90, 200]];

pub struct Vita {
    w: usize,
    h: usize,
    viva: Vec<bool>,
    eta: Vec<u16>,
    scia: Vec<f32>,
    caso: Caso,
    resto: f32,
    storia: Vec<usize>,
    da_seminare: f32,
}

impl Vita {
    pub fn new(w: usize, h: usize, seme: u64) -> Self {
        let (lw, lh) = ((w / 2).max(4), h.max(4));
        let mut v = Vita {
            w: lw,
            h: lh,
            viva: vec![false; lw * lh],
            eta: vec![0; lw * lh],
            scia: vec![0.0; lw * lh],
            caso: Caso::new(seme),
            resto: 0.0,
            storia: Vec::new(),
            da_seminare: 25.0,
        };
        for i in 0..v.viva.len() {
            v.viva[i] = v.caso.moneta(0.22);
        }
        v
    }

    /// una macchia di cellule casuali in un punto a caso
    fn semina(&mut self) {
        let r = (self.h / 5).max(3) as i32;
        let cx = self.caso.intero(self.w) as i32;
        let cy = self.caso.intero(self.h) as i32;
        for dy in -r..=r {
            for dx in -r..=r {
                if dx * dx + dy * dy <= r * r && self.caso.moneta(0.4) {
                    let x = (cx + dx).rem_euclid(self.w as i32) as usize;
                    let y = (cy + dy).rem_euclid(self.h as i32) as usize;
                    let i = y * self.w + x;
                    if !self.viva[i] {
                        self.viva[i] = true;
                        self.eta[i] = 0;
                    }
                }
            }
        }
    }

    fn generazione(&mut self) {
        let (w, h) = (self.w, self.h);
        let mut nuova = vec![false; w * h];
        for y in 0..h {
            let (su, giu) = ((y + h - 1) % h, (y + 1) % h);
            for x in 0..w {
                let (sx, dx) = ((x + w - 1) % w, (x + 1) % w);
                let n = [
                    su * w + sx, su * w + x, su * w + dx,
                    y * w + sx, y * w + dx,
                    giu * w + sx, giu * w + x, giu * w + dx,
                ]
                .iter()
                .filter(|&&i| self.viva[i])
                .count();
                let i = y * w + x;
                nuova[i] = n == 3 || (n == 2 && self.viva[i]);
            }
        }
        let mut quante = 0;
        for i in 0..w * h {
            if nuova[i] {
                quante += 1;
                self.eta[i] = if self.viva[i] { self.eta[i].saturating_add(1) } else { 0 };
            } else if self.viva[i] {
                self.scia[i] = 1.0;
            }
        }
        self.viva = nuova;
        // mondo fermo o quasi vuoto: si semina
        self.storia.push(quante);
        if self.storia.len() > 40 {
            self.storia.remove(0);
        }
        let fermo = self.storia.len() == 40 && self.storia.iter().all(|&q| q.abs_diff(quante) <= 6);
        if fermo || quante < w * h / 60 {
            self.semina();
            self.storia.clear();
        }
    }
}

impl Scena for Vita {
    fn passo(&mut self, g: &mut Griglia, dt: f32, _t: f32) {
        self.resto += dt;
        let passo = 1.0 / 10.0;
        while self.resto >= passo {
            self.generazione();
            self.resto -= passo;
        }
        self.da_seminare -= dt;
        if self.da_seminare <= 0.0 {
            self.semina();
            self.da_seminare = self.caso.tra(15.0, 35.0);
        }
        for s in self.scia.iter_mut() {
            *s = (*s - dt * 0.8).max(0.0);
        }
        for y in 0..self.h {
            for x in 0..self.w {
                let i = y * self.w + x;
                let (gx, gy) = ((x * 2) as i32, y as i32);
                if self.viva[i] {
                    let e = (self.eta[i] as f32 / 60.0).min(1.0);
                    let col = scala(&ETA, e);
                    let (a, b) = if self.eta[i] < 2 { (b'(', b')') } else if self.eta[i] < 20 { (b'[', b']') } else { (b'{', b'}') };
                    g.metti(gx, gy, a, col, 0.9, 0.45);
                    g.metti(gx + 1, gy, b, col, 0.9, 0.45);
                } else if self.scia[i] > 0.02 {
                    let l = self.scia[i] * 0.35;
                    g.metti(gx, gy, b'.', [90, 120, 160], l, 0.0);
                    g.metti(gx + 1, gy, b'.', [90, 120, 160], l, 0.0);
                }
            }
        }
    }
}
