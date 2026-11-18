//! Pioggia di notte: gocce col vento, schizzi e cerchi nelle pozzanghere, ogni tanto un fulmine.

use super::Scena;
use crate::grid::{rumore, Caso, Griglia};

struct Goccia {
    x: f32,
    y: f32,
    v: f32,
    lungo: i32,
}

struct Cerchio {
    x: f32,
    eta: f32,
}

struct Fulmine {
    punti: Vec<(i32, i32, u8)>,
    vita: f32,
}

pub struct Pioggia {
    gocce: Vec<Goccia>,
    cerchi: Vec<Cerchio>,
    fulmine: Option<Fulmine>,
    lampo: f32,
    prossimo_fulmine: f32,
    caso: Caso,
}

const ACQUA: [u8; 3] = [150, 185, 235];

impl Pioggia {
    pub fn new(w: usize, h: usize, seme: u64) -> Self {
        let mut caso = Caso::new(seme);
        let mut gocce = Vec::new();
        for _ in 0..(w * h / 18) {
            let v = caso.tra(0.9, 1.4);
            gocce.push(Goccia { x: caso.tra(0.0, w as f32), y: caso.tra(0.0, h as f32), v, lungo: if v > 1.2 { 3 } else { 2 } });
        }
        let prossimo_fulmine = caso.tra(6.0, 14.0);
        Pioggia { gocce, cerchi: Vec::new(), fulmine: None, lampo: 0.0, prossimo_fulmine, caso }
    }

    fn crea_fulmine(&mut self, w: usize, terra: i32) -> Fulmine {
        let mut punti = Vec::new();
        let mut x = self.caso.tra(w as f32 * 0.15, w as f32 * 0.85) as i32;
        let mut rami: Vec<(i32, i32, i32)> = Vec::new();
        for y in 0..terra {
            let passo = self.caso.intero(3) as i32 - 1;
            let ch = match passo {
                -1 => b'/',
                1 => b'\\',
                _ => b'|',
            };
            if passo == 1 {
                punti.push((x, y, ch));
                x += 1;
            } else {
                x += passo;
                punti.push((x, y, ch));
            }
            if self.caso.moneta(0.06) && y < terra - 4 {
                rami.push((x, y, if self.caso.moneta(0.5) { -1 } else { 1 }));
            }
        }
        for (mut x, y0, dir) in rami {
            let lungo = 3 + self.caso.intero(6) as i32;
            for y in y0 + 1..(y0 + lungo).min(terra) {
                x += dir;
                punti.push((x, y, if dir > 0 { b'\\' } else { b'/' }));
            }
        }
        Fulmine { punti, vita: 0.45 }
    }
}

impl Scena for Pioggia {
    fn passo(&mut self, g: &mut Griglia, dt: f32, t: f32) {
        let (w, h) = (g.w as f32, g.h as f32);
        let terra = (h * 0.86) as i32;
        let vento = (t * 0.05).sin() * 0.35 + (t * 0.31).sin() * 0.08;
        let caduta = h * 0.9;

        // lampo: lo schermo si illumina di puntini
        self.lampo = (self.lampo - dt * 2.2).max(0.0);
        if self.lampo > 0.02 {
            for y in 0..terra {
                for x in 0..g.w {
                    if (x * 7 + y as usize * 13) % 5 == 0 {
                        g.metti(x as i32, y, b'.', [190, 200, 255], self.lampo * 0.35, 0.0);
                    }
                }
            }
        }

        // gocce
        let ch_goccia = if vento.abs() < 0.12 { b'|' } else if vento > 0.0 { b'\\' } else { b'/' };
        let luce_base = 0.8 + self.lampo * 0.2;
        let mut nuovi = Vec::new();
        for gc in self.gocce.iter_mut() {
            gc.y += caduta * gc.v * dt;
            gc.x += vento * caduta * gc.v * dt * 0.5;
            if gc.y as i32 >= terra {
                nuovi.push(gc.x);
                gc.y = self.caso.tra(-h * 0.3, 0.0);
                gc.x = self.caso.tra(-w * 0.2, w * 1.2);
                continue;
            }
            for k in 0..gc.lungo {
                let y = gc.y as i32 - k;
                let x = (gc.x - vento * k as f32 * 0.5) as i32;
                let f = 1.0 - k as f32 / (gc.lungo as f32 + 0.5);
                let ch = if k == 0 { ch_goccia } else if k == 1 { b':' } else { b'.' };
                g.metti_sopra(x, y, ch, ACQUA, luce_base * f * (gc.v - 0.4), 0.25 * f);
            }
        }
        for x in nuovi {
            if self.cerchi.len() < 120 && self.caso.moneta(0.2) {
                self.cerchi.push(Cerchio { x, eta: 0.0 });
            }
        }

        // superficie dell'acqua e terreno
        for x in 0..g.w {
            let n = rumore(x as f32 * 0.15, t * 0.6);
            let ch = if n > 0.62 { b'~' } else if n > 0.4 { b'-' } else { b'_' };
            g.metti(x as i32, terra, ch, [80, 110, 150], 0.35 + self.lampo * 0.4, 0.0);
            for y in terra + 1..g.h as i32 {
                let d = (y - terra) as f32 / (h - terra as f32);
                if rumore(x as f32 * 0.4, y as f32 * 1.3 + t * 0.4) > 0.55 + d * 0.2 {
                    g.metti(x as i32, y, b'.', [60, 85, 120], 0.25 + self.lampo * 0.3, 0.0);
                }
            }
        }

        // schizzi e cerchi
        self.cerchi.retain_mut(|c| {
            c.eta += dt;
            c.eta < 0.9
        });
        for c in &self.cerchi {
            let x = c.x as i32;
            if c.eta < 0.12 {
                g.metti_sopra(x, terra - 1, b'o', ACQUA, 0.8, 0.4);
                g.metti_sopra(x - 1, terra - 2, b'\'', ACQUA, 0.5, 0.0);
                g.metti_sopra(x + 1, terra - 2, b'`', ACQUA, 0.5, 0.0);
            } else {
                let r = (c.eta * 5.0) as i32;
                let f = 1.0 - c.eta / 0.9;
                g.metti_sopra(x - r - 1, terra, b'(', ACQUA, 0.6 * f, 0.15);
                g.metti_sopra(x + r + 1, terra, b')', ACQUA, 0.6 * f, 0.15);
            }
        }

        // fulmine
        self.prossimo_fulmine -= dt;
        if self.prossimo_fulmine <= 0.0 {
            let f = self.crea_fulmine(g.w, terra);
            self.fulmine = Some(f);
            self.lampo = 1.0;
            self.prossimo_fulmine = self.caso.tra(9.0, 25.0);
        }
        if let Some(f) = &mut self.fulmine {
            f.vita -= dt;
            // tremola: acceso, spento, acceso
            let acceso = f.vita > 0.3 || (f.vita > 0.12 && f.vita < 0.22);
            if acceso {
                for &(x, y, ch) in &f.punti {
                    g.metti(x, y, ch, [235, 240, 255], 1.0, 1.2);
                }
            }
            if f.vita <= 0.0 {
                self.fulmine = None;
            }
        }
    }
}
