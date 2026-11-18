//! Disegna la griglia in un buffer di pixel (0x00RRGGBB), con il carattere
//! JetBrains Mono rasterizzato una volta sola e un alone morbido intorno
//! ai caratteri luminosi. Il lavoro è diviso a fasce di righe tra i core.

use crate::griglia::Griglia;

static FONT: &[u8] = include_bytes!("../assets/JetBrainsMono-Regular.ttf");

pub const SFONDO: [u8; 3] = [6, 8, 12];

#[derive(Default)]
struct Glifo {
    /// pixel accesi del carattere: (x, y, intensità) dentro la cella
    punti: Vec<(u16, u16, u8)>,
    /// pixel dell'alone: (x, y, intensità) a partire da (-pad, -pad)
    alone: Vec<(u16, u16, u8)>,
}

pub struct Pittore {
    pub cw: usize,
    pub ch: usize,
    pad: usize,
    glifi: Vec<Glifo>,
}

impl Pittore {
    /// `alto` = altezza di una cella in pixel
    pub fn new(alto: usize) -> Self {
        let alto = alto.max(6);
        let font = fontdue::Font::from_bytes(FONT, fontdue::FontSettings::default()).expect("font");
        let px = alto as f32 * 0.80;
        let largo = font.metrics('M', px).advance_width.round().max(3.0) as usize;
        let linea = font.horizontal_line_metrics(px).expect("metriche");
        let alt_testo = linea.ascent - linea.descent;
        let base = ((alto as f32 - alt_testo) / 2.0 + linea.ascent).round() as i32;
        let pad = (alto / 3).max(2);

        let mut glifi: Vec<Glifo> = (0..128).map(|_| Glifo::default()).collect();
        for c in 33u8..127 {
            let (m, bmp) = font.rasterize(c as char, px);
            let mut maschera = vec![0u8; largo * alto];
            for by in 0..m.height {
                for bx in 0..m.width {
                    let x = m.xmin + bx as i32;
                    let y = base - (m.ymin + m.height as i32) + by as i32;
                    if x >= 0 && y >= 0 && (x as usize) < largo && (y as usize) < alto {
                        let v = bmp[by * m.width + bx];
                        let p = &mut maschera[y as usize * largo + x as usize];
                        *p = (*p).max(v);
                    }
                }
            }
            let g = &mut glifi[c as usize];
            for y in 0..alto {
                for x in 0..largo {
                    let v = maschera[y * largo + x];
                    if v > 8 {
                        g.punti.push((x as u16, y as u16, v));
                    }
                }
            }
            // alone: la maschera sfocata due volte su un riquadro più grande
            let (aw, ah) = (largo + 2 * pad, alto + 2 * pad);
            let mut a = vec![0f32; aw * ah];
            for y in 0..alto {
                for x in 0..largo {
                    a[(y + pad) * aw + x + pad] = maschera[y * largo + x] as f32;
                }
            }
            let r = (pad / 2).max(1);
            for _ in 0..2 {
                a = sfoca(&a, aw, ah, r);
            }
            let max = a.iter().cloned().fold(0.0, f32::max).max(1.0);
            for y in 0..ah {
                for x in 0..aw {
                    let v = (a[y * aw + x] / max * 255.0) as u8;
                    if v > 6 {
                        g.alone.push((x as u16, y as u16, v));
                    }
                }
            }
        }
        Pittore { cw: largo, ch: alto, pad, glifi }
    }

    /// quante celle stanno in uno schermo
    pub fn misura(&self, w: usize, h: usize) -> (usize, usize) {
        ((w / self.cw).max(1), (h / self.ch).max(1))
    }

    /// disegna la griglia centrata nel buffer; `luce` (0..1) scurisce tutto, per le dissolvenze
    pub fn dipingi(&self, g: &Griglia, buf: &mut [u32], w: usize, h: usize, luce: f32) {
        let ox = (w as i32 - (g.w * self.cw) as i32) / 2;
        let oy = (h as i32 - (g.h * self.ch) as i32) / 2;
        let fondo = ((SFONDO[0] as u32) << 16) | ((SFONDO[1] as u32) << 8) | SFONDO[2] as u32;
        let fili = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(4).clamp(1, 8);
        let fascia = h.div_ceil(fili).max(1);
        let luce = luce.clamp(0.0, 1.0);
        std::thread::scope(|s| {
            for (i, pezzo) in buf.chunks_mut(w * fascia).enumerate() {
                let y0 = (i * fascia) as i32;
                let y1 = y0 + (pezzo.len() / w) as i32;
                s.spawn(move || {
                    pezzo.fill(fondo);
                    self.fascia(g, pezzo, w, y0, y1, ox, oy, luce);
                });
            }
        });
    }

    #[allow(clippy::too_many_arguments)]
    fn fascia(&self, g: &Griglia, pezzo: &mut [u32], w: usize, y0: i32, y1: i32, ox: i32, oy: i32, luce: f32) {
        let (cw, ch, pad) = (self.cw as i32, self.ch as i32, self.pad as i32);
        // righe di celle che possono toccare questa fascia (alone compreso)
        let r0 = ((y0 - oy - ch - pad) / ch).max(0) as usize;
        let r1 = (((y1 - oy + pad) / ch + 1).max(0) as usize).min(g.h);
        for r in r0..r1 {
            let py = oy + r as i32 * ch;
            for c in 0..g.w {
                let cella = &g.celle[r * g.w + c];
                if cella.ch <= b' ' || cella.ch >= 128 || cella.luce <= 0.01 {
                    continue;
                }
                let gl = &self.glifi[cella.ch as usize];
                let px = ox + c as i32 * cw;
                let k = cella.luce * luce;
                let col = [cella.col[0] as f32 * k, cella.col[1] as f32 * k, cella.col[2] as f32 * k];
                if cella.alone > 0.0 {
                    let a = cella.alone.min(1.5) * 0.55;
                    let ac = [(col[0] * a) as u32, (col[1] * a) as u32, (col[2] * a) as u32];
                    if ac != [0, 0, 0] {
                        for &(x, y, v) in &gl.alone {
                            let (xx, yy) = (px - pad + x as i32, py - pad + y as i32);
                            if yy >= y0 && yy < y1 && xx >= 0 && xx < w as i32 {
                                somma(&mut pezzo[(yy - y0) as usize * w + xx as usize], ac, v as u32);
                            }
                        }
                    }
                }
                let cc = [col[0] as u32, col[1] as u32, col[2] as u32];
                for &(x, y, v) in &gl.punti {
                    let (xx, yy) = (px + x as i32, py + y as i32);
                    if yy >= y0 && yy < y1 && xx >= 0 && xx < w as i32 {
                        somma(&mut pezzo[(yy - y0) as usize * w + xx as usize], cc, v as u32);
                    }
                }
            }
        }
    }
}

#[inline]
fn somma(p: &mut u32, c: [u32; 3], v: u32) {
    let r = ((*p >> 16) & 255) + ((c[0] * v) >> 8);
    let g = ((*p >> 8) & 255) + ((c[1] * v) >> 8);
    let b = (*p & 255) + ((c[2] * v) >> 8);
    *p = (r.min(255) << 16) | (g.min(255) << 8) | b.min(255);
}

/// sfocatura a scatola separabile, raggio r
fn sfoca(src: &[f32], w: usize, h: usize, r: usize) -> Vec<f32> {
    let mut tmp = vec![0f32; w * h];
    let mut out = vec![0f32; w * h];
    let n = (2 * r + 1) as f32;
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for k in x.saturating_sub(r)..(x + r + 1).min(w) {
                s += src[y * w + k];
            }
            tmp[y * w + x] = s / n;
        }
    }
    for y in 0..h {
        for x in 0..w {
            let mut s = 0.0;
            for k in y.saturating_sub(r)..(y + r + 1).min(h) {
                s += tmp[k * w + x];
            }
            out[y * w + x] = s / n;
        }
    }
    out
}
