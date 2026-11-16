//! Impostazioni salvate in %APPDATA%\AsciiZoo\config.json

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// lunghezza massima del nome di un animale (lettere)
pub const NOME_MAX: usize = 16;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// 0 = a rotazione, 1..=7 = un animale fisso (stesso ordine del menu)
    pub animale: u32,
    /// con "a rotazione": koi nello stagno quando il PC è in carica
    pub koi_in_carica: bool,
    /// ferma l'animazione quando il PC va a batteria
    pub pausa_batteria: bool,
    /// ferma l'animazione quando un'app (gioco, video) è a schermo intero
    pub pausa_schermo_intero: bool,
    /// nome dell'animale più grande, per specie (id della specie → nome; "" = nessuno)
    pub nomi: BTreeMap<String, String>,
    /// ogni quanti secondi cambia animale "a rotazione"
    pub rotazione_secondi: u32,
    /// specie tolte dalla rotazione
    pub rotazione_escluse: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            animale: 0,
            koi_in_carica: true,
            pausa_batteria: true,
            pausa_schermo_intero: true,
            nomi: BTreeMap::from([("koi".to_string(), "KARPISA".to_string())]),
            rotazione_secondi: 120,
            rotazione_escluse: Vec::new(),
        }
    }
}

/// pulisce un nome scritto dall'utente: niente caratteri di controllo, spazi ai lati tolti, al massimo NOME_MAX lettere
pub fn pulisci_nome(s: &str) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(NOME_MAX).collect()
}

fn path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("AsciiZoo").join("config.json")
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            // un file salvato con il BOM (es. da Blocco note) non deve azzerare tutto
            .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}

/// Avvio con Windows: chiave Run dell'utente corrente.
pub mod autostart {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "AsciiZoo";

    pub fn enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(NAME))
            .is_ok()
    }

    pub fn set(on: bool) {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, KEY_WRITE) else { return };
        if on {
            if let Ok(exe) = std::env::current_exe() {
                let _ = key.set_value(NAME, &format!("\"{}\"", exe.display()));
            }
        } else {
            let _ = key.delete_value(NAME);
        }
    }
}
