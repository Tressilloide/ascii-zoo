//! Batteria su Linux: /sys/class/power_supply.

use std::fs;
use std::path::Path;

fn leggi(p: &Path, nome: &str) -> String {
    fs::read_to_string(p.join(nome)).map(|s| s.trim().to_string()).unwrap_or_default()
}

/// true se l'alimentatore è collegato (o se è un computer senza batteria)
pub fn on_ac() -> bool {
    let Ok(dir) = fs::read_dir("/sys/class/power_supply") else { return true };
    let mut batteria = false;
    for e in dir.flatten() {
        let p = e.path();
        match leggi(&p, "type").as_str() {
            "Mains" | "USB" if leggi(&p, "online") == "1" => return true,
            // le batterie di mouse e tastiere hanno scope=Device: non contano
            "Battery" if leggi(&p, "scope") != "Device" => batteria = true,
            _ => {}
        }
    }
    !batteria
}

/// true se il computer ha una batteria (un fisso no: per lui on_ac è sempre vero, e "in
/// carica" non ha senso). Non cambia mentre l'app gira: si guarda una volta
pub fn ha_batteria() -> bool {
    static HA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HA.get_or_init(|| batteria().is_some())
}

/// carica della batteria del computer in percentuale, se ce n'è una
pub fn batteria() -> Option<u8> {
    let dir = fs::read_dir("/sys/class/power_supply").ok()?;
    for e in dir.flatten() {
        let p = e.path();
        if leggi(&p, "type") == "Battery" && leggi(&p, "scope") != "Device" {
            if let Ok(v) = leggi(&p, "capacity").parse::<u8>() {
                return Some(v.min(100));
            }
        }
    }
    None
}
