//! La regia: tiene la scena del momento, la cambia ogni tot minuti con una
//! dissolvenza al nero e ci disegna sopra l'orologio.

use crate::config::Config;
use crate::griglia::Griglia;
use crate::orologio::{Ora, Orologio};
use crate::scene::{self, Scena};

const DISSOLVENZA: f32 = 1.2;

pub struct Regia {
    pub griglia: Griglia,
    scena: Box<dyn Scena>,
    indice: usize,
    t_scena: f32,
    durata: f32,
    rotazione: bool,
    uscita: f32, // > 0 mentre la scena sfuma via
    orologio: Option<Orologio>,
    seme: u64,
}

impl Regia {
    pub fn new(w: usize, h: usize, cfg: &Config, seme: u64) -> Self {
        let rotazione = !scene::esiste(&cfg.scena);
        let indice = if rotazione {
            (seme as usize) % scene::SCENE.len()
        } else {
            scene::SCENE.iter().position(|s| s.0 == cfg.scena).unwrap_or(0)
        };
        Regia {
            griglia: Griglia::new(w, h),
            scena: scene::crea(scene::SCENE[indice].0, w, h, seme),
            indice,
            t_scena: 0.0,
            durata: cfg.minuti.max(1) as f32 * 60.0,
            rotazione,
            uscita: 0.0,
            orologio: cfg.orologio.then(|| Orologio::new(seme)),
            seme,
        }
    }

    /// avanza di `dt` secondi e ridisegna; restituisce la luce generale (per le dissolvenze)
    pub fn passo(&mut self, dt: f32, ora: Ora) -> f32 {
        let dt = dt.clamp(0.0, 0.1);
        self.t_scena += dt;
        if self.rotazione && self.uscita <= 0.0 && self.t_scena >= self.durata {
            self.uscita = DISSOLVENZA;
        }
        if self.uscita > 0.0 {
            self.uscita -= dt;
            if self.uscita <= 0.0 {
                self.indice = (self.indice + 1) % scene::SCENE.len();
                self.seme = self.seme.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                self.scena = scene::crea(scene::SCENE[self.indice].0, self.griglia.w, self.griglia.h, self.seme);
                self.t_scena = 0.0;
            }
        }
        self.griglia.pulisci();
        self.scena.passo(&mut self.griglia, dt, self.t_scena);
        if let Some(o) = &mut self.orologio {
            o.disegna(&mut self.griglia, ora, dt);
        }
        if self.uscita > 0.0 {
            self.uscita / DISSOLVENZA
        } else {
            (self.t_scena / DISSOLVENZA).min(1.0)
        }
    }
}
