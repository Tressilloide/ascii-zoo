//! Le scene. Ognuna riceve la griglia già pulita a ogni fotogramma e ci disegna sopra.

use crate::griglia::Griglia;

mod aurora;
mod fuoco;
mod mare;
mod pioggia;
mod stelle;
mod vita;

pub trait Scena {
    /// `dt` = secondi dall'ultimo fotogramma, `t` = secondi dall'inizio della scena
    fn passo(&mut self, g: &mut Griglia, dt: f32, t: f32);
}

/// (id, nome mostrato)
pub const SCENE: &[(&str, &str)] = &[
    ("stelle", "Starfield"),
    ("pioggia", "Rain & lightning"),
    ("fuoco", "Campfire"),
    ("aurora", "Aurora"),
    ("vita", "Game of Life"),
    ("mare", "Night sea"),
];

pub fn esiste(id: &str) -> bool {
    SCENE.iter().any(|s| s.0 == id)
}

pub fn crea(id: &str, w: usize, h: usize, seme: u64) -> Box<dyn Scena> {
    match id {
        "pioggia" => Box::new(pioggia::Pioggia::new(w, h, seme)),
        "fuoco" => Box::new(fuoco::Fuoco::new(w, h, seme)),
        "aurora" => Box::new(aurora::Aurora::new(w, h, seme)),
        "vita" => Box::new(vita::Vita::new(w, h, seme)),
        "mare" => Box::new(mare::Mare::new(w, h, seme)),
        _ => Box::new(stelle::Stelle::new(w, h, seme)),
    }
}
