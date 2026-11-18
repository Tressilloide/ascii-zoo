//! Impostazioni, salvate in un file JSON nella cartella dei dati dell'utente.

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Serialize, Deserialize, Clone)]
#[serde(default)]
pub struct Config {
    /// id di una scena, oppure "rotazione"
    pub scena: String,
    /// ogni quanti minuti cambia scena (con "rotazione")
    pub minuti: u32,
    /// orologio grande sopra la scena
    pub orologio: bool,
    /// grandezza dei caratteri: righe di testo per altezza dello schermo
    pub righe: u32,
}

impl Default for Config {
    fn default() -> Self {
        Config { scena: "rotazione".into(), minuti: 3, orologio: true, righe: 48 }
    }
}

pub const RIGHE: &[(u32, &str)] = &[(64, "Small"), (48, "Medium"), (34, "Large")];
pub const MINUTI: &[u32] = &[1, 3, 5, 10, 30];

fn file() -> Option<PathBuf> {
    #[cfg(windows)]
    let base = std::env::var_os("APPDATA").map(PathBuf::from);
    #[cfg(target_os = "macos")]
    let base = std::env::var_os("HOME").map(|h| PathBuf::from(h).join("Library/Application Support"));
    #[cfg(all(unix, not(target_os = "macos")))]
    let base = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")));
    base.map(|b| b.join("AsciiZoo").join("screensaver.json"))
}

impl Config {
    pub fn carica() -> Self {
        file()
            .and_then(|f| std::fs::read_to_string(f).ok())
            .and_then(|s| serde_json::from_str::<Config>(&s).ok())
            .map(|mut c| {
                c.minuti = c.minuti.clamp(1, 120);
                c.righe = c.righe.clamp(16, 120);
                c
            })
            .unwrap_or_default()
    }

    pub fn salva(&self) -> std::io::Result<()> {
        let f = file().ok_or_else(|| std::io::Error::other("nessuna cartella utente"))?;
        if let Some(d) = f.parent() {
            std::fs::create_dir_all(d)?;
        }
        std::fs::write(f, serde_json::to_string_pretty(self).unwrap_or_default())
    }
}
