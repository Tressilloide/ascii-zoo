//! Aurora boreale: tende di luce verde e viola che ondeggiano sopra le montagne,
//! stelle che brillano e un lago che riflette tutto.

use super::Scena;
use crate::grid::{frattale, rumore, scala, Caso, Griglia};

const TENDA: [[u8; 3]; 4] = [[70, 255, 150], [40, 220, 190], [90, 140, 255], [190, 90, 230]];

pub struct Aurora {
    stelle: Vec<(i32, i32, f32, f32)>, // x, y, fase, velocità
}

impl Aurora {
    pub fn new(w: usize, h: usize, seme: u64) -> Self {
        let mut c = Caso::new(seme);
        let stelle = (0..(w * h / 45))
            .map(|_| (c.intero(w) as i32, c.intero(h * 6 / 10) as i32, c.tra(0.0, 6.28), c.tra(0.5, 2.5)))
            .collect();
        Aurora { stelle }
    }
}

impl Scena for Aurora {
    fn passo(&mut self, g: &mut Griglia, _dt: f32, t: f32) {
        let (w, h) = (g.w as f32, g.h as f32);
        let lago = (h * 0.78) as i32; // dove comincia l'acqua

        for &(x, y, fase, vel) in &self.stelle {
            let l = 0.25 + 0.25 * (t * vel + fase).sin();
            g.metti(x, y, if l > 0.42 { b'+' } else { b'.' }, [200, 210, 255], l, 0.2);
        }

        // le tende: per ogni colonna un bordo in basso che ondeggia e una luce che sale
        let mut cielo = vec![(0u8, [0u8; 3], 0f32); g.w * g.h];
        for x in 0..g.w {
            let xf = x as f32;
            let forza = ((frattale(xf * 0.018 + t * 0.025, t * 0.04, 3) - 0.35) * 2.4).clamp(0.0, 1.0);
            if forza <= 0.0 {
                continue;
            }
            let bordo = h * 0.42 + (frattale(xf * 0.012, t * 0.05 + 3.0, 3) - 0.5) * h * 0.35;
            let lungo = h * (0.18 + 0.3 * forza);
            let raggio = 0.55 + 0.45 * rumore(xf * 0.45, t * 0.8);
            for k in 0..lungo as i32 {
                let y = bordo as i32 - k;
                if y < 0 || y >= lago {
                    continue;
                }
                let f = 1.0 - k as f32 / lungo;
                let luce = forza * f.powf(1.3) * raggio;
                if luce < 0.05 {
                    continue;
                }
                let ch = if luce > 0.55 { b'|' } else if luce > 0.3 { b'!' } else if luce > 0.15 { b':' } else { b'.' };
                let col = scala(&TENDA, (1.0 - f) * 0.9 + rumore(xf * 0.05, t * 0.1) * 0.15);
                g.metti(x as i32, y, ch, col, luce, luce * 0.9);
                cielo[y as usize * g.w + x] = (ch, col, luce);
            }
        }

        // montagne: profilo con le creste, riempite di buio
        let monte = |x: f32| h * 0.56 + (frattale(x * 0.035, 11.0, 4) - 0.5) * h * 0.4;
        for x in 0..g.w as i32 {
            let cima = monte(x as f32).min(lago as f32 - 1.0) as i32;
            let prima = monte(x as f32 - 1.0) as i32;
            let dopo = monte(x as f32 + 1.0) as i32;
            let ch = if prima > cima && dopo > cima { b'^' } else if prima > dopo { b'/' } else if dopo > prima { b'\\' } else { b'_' };
            g.metti(x, cima, ch, [120, 140, 180], 0.45, 0.0);
            // un po' di neve sulle cime più alte
            for y in cima + 1..lago {
                let neve = cima < (h * 0.42) as i32 && y < cima + 2;
                if neve {
                    g.metti(x, y, b'.', [210, 220, 240], 0.35, 0.0);
                } else {
                    g.metti(x, y, b' ', [0, 0, 0], 0.0, 0.0);
                }
            }
        }

        // il lago riflette il cielo capovolto, increspato
        for y in lago..g.h as i32 {
            let d = (y - lago) as f32;
            let src = lago - 1 - (d * 1.0) as i32;
            for x in 0..g.w as i32 {
                let onda = (rumore(x as f32 * 0.2, y as f32 * 0.9 + t * 0.7) - 0.5) * 3.0;
                let sx = (x as f32 + onda) as i32;
                if src >= 0 && sx >= 0 && sx < w as i32 {
                    let (ch, col, luce) = cielo[src as usize * g.w + sx as usize];
                    if luce > 0.05 {
                        let ch = if ch == b'|' { b'=' } else { b'-' };
                        g.metti(x, y, ch, col, luce * 0.45, 0.2);
                        continue;
                    }
                }
                if rumore(x as f32 * 0.3, y as f32 * 1.7 + t * 0.5) > 0.7 {
                    g.metti(x, y, b'~', [60, 80, 120], 0.2, 0.0);
                }
            }
        }
    }
}
