//! Volo tra le stelle: le stelle arrivano dal centro, crescono e lasciano una scia.

use super::Scena;
use crate::grid::{mescola, Caso, Griglia};

struct Stella {
    x: f32,
    y: f32,
    z: f32,
    col: [u8; 3],
}

pub struct Stelle {
    stelle: Vec<Stella>,
    caso: Caso,
    cadente: Option<(f32, f32, f32, f32, f32)>, // x, y, vx, vy, vita
}

const COLORI: [[u8; 3]; 4] = [[220, 230, 255], [180, 200, 255], [255, 236, 200], [255, 210, 170]];

impl Stelle {
    pub fn new(w: usize, h: usize, seme: u64) -> Self {
        let mut caso = Caso::new(seme);
        let n = (w * h / 10).clamp(60, 1600);
        // all'inizio le stelle sono già sparse a tutte le distanze
        let stelle = (0..n)
            .map(|_| {
                let z = caso.tra(0.05, 1.0);
                nuova(&mut caso, z)
            })
            .collect();
        Stelle { stelle, caso, cadente: None }
    }
}

fn nuova(c: &mut Caso, z: f32) -> Stella {
    let col = if c.moneta(0.8) { COLORI[c.intero(2)] } else { COLORI[2 + c.intero(2)] };
    Stella { x: c.tra(-1.0, 1.0), y: c.tra(-1.0, 1.0), z, col }
}

impl Scena for Stelle {
    fn passo(&mut self, g: &mut Griglia, dt: f32, t: f32) {
        let (w, h) = (g.w as f32, g.h as f32);
        let (cx, cy) = (w / 2.0, h / 2.0);
        let s = w * 0.32;
        let vel = 0.10 * (1.0 + 0.5 * (t * 0.07).sin());
        let ang = t * 0.015;
        let (sa, ca) = (ang.sin(), ang.cos());
        let proietta = |x: f32, y: f32, z: f32| {
            let (rx, ry) = (x * ca - y * sa, x * sa + y * ca);
            (cx + rx / z * s, cy + ry / z * s * 0.5)
        };
        for st in self.stelle.iter_mut() {
            st.z -= vel * dt;
            let (sx, sy) = proietta(st.x, st.y, st.z.max(0.01));
            if st.z <= 0.02 || sx < -1.0 || sy < -1.0 || sx > w + 1.0 || sy > h + 1.0 {
                *st = nuova(&mut self.caso, 1.0);
                continue;
            }
            let vicino = 1.0 - st.z;
            let luce = vicino.powf(1.6).max(0.08);
            let ch = if st.z > 0.7 { b'.' } else if st.z > 0.45 { b'+' } else if st.z > 0.22 { b'*' } else { b'@' };
            // scia: dalla posizione di poco fa a quella di adesso
            if st.z < 0.5 {
                let (px, py) = proietta(st.x, st.y, st.z + vel * 0.35);
                let passi = ((sx - px).abs().max((sy - py).abs() * 2.0)).ceil().min(12.0) as i32;
                for k in 1..=passi {
                    let f = k as f32 / (passi + 1) as f32;
                    let (lx, ly) = (px + (sx - px) * f, py + (sy - py) * f);
                    g.metti_sopra(lx as i32, ly as i32, b'.', st.col, luce * f * 0.5, 0.0);
                }
            }
            g.metti_sopra(sx as i32, sy as i32, ch, st.col, luce, luce * 0.9);
        }
        // ogni tanto una stella cadente
        match &mut self.cadente {
            None if self.caso.moneta(dt / 9.0) => {
                let da_sinistra = self.caso.moneta(0.5);
                let x = if da_sinistra { self.caso.tra(0.0, w * 0.5) } else { self.caso.tra(w * 0.5, w) };
                let vx = if da_sinistra { w * 0.6 } else { -w * 0.6 };
                self.cadente = Some((x, self.caso.tra(0.0, h * 0.3), vx, h * 0.35, 1.2));
            }
            Some((x, y, vx, vy, vita)) => {
                *x += *vx * dt;
                *y += *vy * dt;
                *vita -= dt;
                let ch = if (*vx > 0.0) == (*vy > 0.0) { b'\\' } else { b'/' };
                for k in 0..10 {
                    let f = k as f32 / 10.0;
                    let lx = *x - *vx * f * 0.12;
                    let ly = *y - *vy * f * 0.12;
                    let col = mescola([255, 255, 255], [120, 160, 255], f);
                    g.metti_sopra(lx as i32, ly as i32, if k == 0 { b'*' } else { ch }, col, (1.0 - f) * vita.min(1.0), 0.8);
                }
                if *vita <= 0.0 {
                    self.cadente = None;
                }
            }
            _ => {}
        }
    }
}
