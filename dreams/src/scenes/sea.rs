//! Mare di notte: la luna con i crateri, nuvole che le passano davanti,
//! il riflesso che tremola sulle onde e una barchetta a vela che attraversa.

use super::Scena;
use crate::grid::{frattale, rumore, Caso, Griglia};

const BARCA: [&str; 4] = ["    |\\    ", "    | \\   ", "  __|__\\_ ", "  \\_____/ "];

pub struct Mare {
    stelle: Vec<(i32, i32, f32)>,
    barca_x: f32,
    caso: Caso,
}

impl Mare {
    pub fn new(w: usize, h: usize, seme: u64) -> Self {
        let mut c = Caso::new(seme);
        let stelle = (0..(w * h / 60)).map(|_| (c.intero(w) as i32, c.intero(h / 2) as i32, c.tra(0.0, 6.28))).collect();
        let barca_x = c.tra(-10.0, w as f32 * 0.5);
        Mare { stelle, barca_x, caso: c }
    }
}

impl Scena for Mare {
    fn passo(&mut self, g: &mut Griglia, dt: f32, t: f32) {
        let (w, h) = (g.w as f32, g.h as f32);
        let orizzonte = (h * 0.55) as i32;
        let (lx, ly) = (w * 0.7, h * 0.24);
        let r = (h * 0.11).max(3.0);

        for &(x, y, fase) in &self.stelle {
            if y < orizzonte - 1 {
                let l = 0.2 + 0.2 * (t * 1.3 + fase).sin();
                g.metti(x, y, b'.', [200, 210, 255], l, 0.0);
            }
        }

        // nuvole che passano lente (anche davanti alla luna)
        for y in 0..orizzonte - 2 {
            for x in 0..g.w as i32 {
                let n = frattale(x as f32 * 0.03 - t * 0.04, y as f32 * 0.12, 4);
                if n > 0.58 {
                    let dens = ((n - 0.58) * 4.0).min(1.0);
                    let ch = if dens > 0.6 { b'%' } else if dens > 0.3 { b':' } else { b'.' };
                    g.metti(x, y, ch, [120, 125, 150], 0.18 + dens * 0.22, 0.0);
                }
            }
        }

        // luna: un disco con crateri (le celle sono alte il doppio che larghe)
        for y in (ly - r) as i32..=(ly + r) as i32 {
            for x in (lx - r * 2.0) as i32..=(lx + r * 2.0) as i32 {
                let dx = (x as f32 - lx) / 2.0;
                let dy = y as f32 - ly;
                let d = (dx * dx + dy * dy).sqrt() / r;
                if d <= 1.0 {
                    let cratere = frattale(x as f32 * 0.25, y as f32 * 0.5, 3);
                    let bordo = 1.0 - d.powf(6.0);
                    let ch = if cratere > 0.62 { b'%' } else if cratere > 0.5 { b'#' } else { b'@' };
                    let col = if cratere > 0.62 { [200, 196, 170] } else { [255, 250, 225] };
                    g.metti(x, y, ch, col, 0.95 * bordo + 0.05, 0.9);
                }
            }
        }

        // orizzonte e onde, più grandi e più chiare man mano che si avvicinano
        for y in orizzonte..g.h as i32 {
            let d = (y - orizzonte) as f32 / (h - orizzonte as f32).max(1.0);
            let freq = 0.5 - d * 0.35;
            for x in 0..g.w as i32 {
                let v = (x as f32 * freq + t * (0.8 + d) + y as f32 * 1.7).sin() * 0.6
                    + (rumore(x as f32 * 0.08, y as f32 * 0.5 + t * 0.3) - 0.5) * 1.2;
                // riflesso della luna: una striscia che si allarga verso chi guarda
                let largo = r * (1.0 + d * 2.5);
                let nel_riflesso = (x as f32 - lx).abs() < largo;
                let tremolio = rumore(x as f32 * 0.6, y as f32 * 1.1 + t * 2.2);
                if nel_riflesso && tremolio > 0.42 + (x as f32 - lx).abs() / largo * 0.4 {
                    let ch = if tremolio > 0.7 { b'=' } else { b'-' };
                    g.metti(x, y, ch, [255, 240, 190], 0.5 + tremolio * 0.5, 0.6);
                } else if v > 0.55 {
                    g.metti(x, y, b'~', [70, 120, 170], 0.18 + 0.32 * d, 0.0);
                } else if v > 0.1 {
                    g.metti(x, y, b'-', [50, 90, 140], 0.12 + 0.22 * d, 0.0);
                }
            }
        }
        for x in 0..g.w as i32 {
            if (x as f32 - lx).abs() > r * 1.2 {
                g.metti(x, orizzonte, b'_', [90, 110, 150], 0.35, 0.0);
            }
        }

        // la barchetta: scura, contro il riflesso
        self.barca_x += dt * 1.2;
        if self.barca_x > w + 12.0 {
            self.barca_x = -12.0 - self.caso.tra(0.0, w * 0.5);
        }
        let by = orizzonte + ((h - orizzonte as f32) * 0.3) as i32 - BARCA.len() as i32;
        let dondolo = ((t * 1.1).sin() * 0.6).round() as i32;
        for (i, riga) in BARCA.iter().enumerate() {
            for (j, c) in riga.bytes().enumerate() {
                let x = self.barca_x as i32 + j as i32;
                let y = by + i as i32 + if i >= 2 { 0 } else { dondolo.min(0) };
                if c != b' ' {
                    g.metti(x, y, c, [170, 150, 120], 0.55, 0.0);
                } else if i >= 2 && j > 1 && j < 9 {
                    g.metti(x, y, b' ', [0, 0, 0], 0.0, 0.0);
                }
            }
        }
    }
}
