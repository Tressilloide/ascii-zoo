//! Orologio grande sopra la scena: cifre 5x7 disegnate con '#', larghe il doppio
//! (le celle sono alte il doppio), che si spostano piano rimbalzando sui bordi
//! così niente resta fermo nello stesso punto dello schermo.

use crate::griglia::Griglia;

const CIFRE: [[&str; 7]; 11] = [
    ["01110", "10001", "10011", "10101", "11001", "10001", "01110"],
    ["00100", "01100", "00100", "00100", "00100", "00100", "01110"],
    ["01110", "10001", "00001", "00010", "00100", "01000", "11111"],
    ["11110", "00001", "00001", "01110", "00001", "00001", "11110"],
    ["00010", "00110", "01010", "10010", "11111", "00010", "00010"],
    ["11111", "10000", "11110", "00001", "00001", "10001", "01110"],
    ["00110", "01000", "10000", "11110", "10001", "10001", "01110"],
    ["11111", "00001", "00010", "00100", "01000", "01000", "01000"],
    ["01110", "10001", "10001", "01110", "10001", "10001", "01110"],
    ["01110", "10001", "10001", "01111", "00001", "00010", "01100"],
    ["00000", "00100", "00100", "00000", "00100", "00100", "00000"], // due punti
];

const GIORNI: [&str; 7] = ["SUNDAY", "MONDAY", "TUESDAY", "WEDNESDAY", "THURSDAY", "FRIDAY", "SATURDAY"];
const MESI: [&str; 12] = ["JAN", "FEB", "MAR", "APR", "MAY", "JUN", "JUL", "AUG", "SEP", "OCT", "NOV", "DEC"];

#[derive(Clone, Copy)]
pub struct Ora {
    pub ore: u32,
    pub minuti: u32,
    pub secondi: u32,
    pub giorno_settimana: u32,
    pub giorno: u32,
    pub mese: u32,
}

pub struct Orologio {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
}

impl Orologio {
    pub fn new(seme: u64) -> Self {
        // partenza e direzione diverse per ogni schermo
        let a = (seme % 628) as f32 / 100.0;
        let (sx, sy) = (if seme % 2 == 0 { 1.0 } else { -1.0 }, if seme % 3 == 0 { 1.0 } else { -1.0 });
        Orologio {
            x: 1.0 + (seme % 97) as f32,
            y: 1.0 + (seme / 97 % 41) as f32,
            vx: sx * a.cos().abs().max(0.4) * 0.7,
            vy: sy * a.sin().abs().max(0.4) * 0.35,
        }
    }

    pub fn disegna(&mut self, g: &mut Griglia, ora: Ora, dt: f32) {
        let testo = [ora.ore / 10, ora.ore % 10, 10, ora.minuti / 10, ora.minuti % 10];
        let largo = (5 * 2 + 2) * 5 - 2; // 5 simboli da 10 celle + spazi
        let alto = 7 + 2;
        let (mw, mh) = (g.w as i32 - largo - 2, g.h as i32 - alto - 1);
        if mw <= 1 || mh <= 1 {
            return;
        }
        // rimbalzo lento
        self.x += self.vx * dt;
        self.y += self.vy * dt;
        if self.x < 1.0 || self.x > mw as f32 {
            self.vx = -self.vx;
            self.x = self.x.clamp(1.0, mw as f32);
        }
        if self.y < 1.0 || self.y > mh as f32 {
            self.vy = -self.vy;
            self.y = self.y.clamp(1.0, mh as f32);
        }
        let (ox, oy) = (self.x as i32, self.y as i32);
        // spazio pulito dietro l'orologio, così si legge sempre
        for y in oy - 1..oy + alto + 1 {
            for x in ox - 2..ox + largo + 2 {
                g.metti(x, y, b' ', [0, 0, 0], 0.0, 0.0);
            }
        }
        let lampeggia = ora.secondi % 2 == 0;
        for (i, &c) in testo.iter().enumerate() {
            let bx = ox + i as i32 * 12;
            for (r, riga) in CIFRE[c as usize].iter().enumerate() {
                for (k, b) in riga.bytes().enumerate() {
                    let x = bx + k as i32 * 2;
                    let y = oy + r as i32;
                    if b == b'1' {
                        let luce = if c == 10 && !lampeggia { 0.35 } else { 0.92 };
                        g.metti(x, y, b'#', [235, 240, 255], luce, 0.55);
                        g.metti(x + 1, y, b'#', [235, 240, 255], luce, 0.55);
                    } else if c != 10 {
                        g.metti(x, y, b'.', [235, 240, 255], 0.07, 0.0);
                        g.metti(x + 1, y, b'.', [235, 240, 255], 0.07, 0.0);
                    }
                }
            }
        }
        let data = format!(
            "{}  {}  {}",
            GIORNI[ora.giorno_settimana as usize % 7],
            ora.giorno,
            MESI[(ora.mese as usize + 11) % 12]
        );
        let spaziata: String = data.chars().flat_map(|c| [c, ' ']).collect();
        let dx = ox + (largo - spaziata.len() as i32) / 2;
        g.testo(dx, oy + 8, spaziata.trim_end(), [170, 180, 210], 0.6, 0.2);
    }
}
