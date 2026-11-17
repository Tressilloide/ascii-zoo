//! CPU, RAM e batteria per il widget "pc" (Linux: /proc e /sys).

use std::fs;

#[derive(Default)]
pub struct Stats {
    /// tempi della lettura precedente: (inattivo, totale)
    prima: Option<(u64, u64)>,
}

/// prima riga di /proc/stat: tempi di tutte le CPU insieme
fn cpu() -> Option<(u64, u64)> {
    let s = fs::read_to_string("/proc/stat").ok()?;
    let riga = s.lines().next()?;
    let n: Vec<u64> = riga.split_whitespace().skip(1).filter_map(|x| x.parse().ok()).collect();
    if n.len() < 4 {
        return None;
    }
    // user nice system idle iowait irq softirq steal (guest è già dentro user)
    let tot: u64 = n.iter().take(8).sum();
    let idle = n[3] + n.get(4).copied().unwrap_or(0);
    Some((idle, tot))
}

/// RAM usata in percentuale (MemAvailable è quella davvero libera)
fn ram() -> Option<u64> {
    let s = fs::read_to_string("/proc/meminfo").ok()?;
    let campo = |nome: &str| -> Option<u64> {
        s.lines().find(|l| l.starts_with(nome))?.split_whitespace().nth(1)?.parse().ok()
    };
    let (tot, libera) = (campo("MemTotal:")?, campo("MemAvailable:")?);
    (tot > 0).then(|| 100 - libera.min(tot) * 100 / tot)
}

impl Stats {
    /// i valori come oggetto JSON per zooSetting('statistiche', ...)
    pub fn leggi(&mut self) -> serde_json::Value {
        let mut v = serde_json::Map::new();
        if let Some(ora) = cpu() {
            if let Some(p) = self.prima {
                let tot = ora.1.saturating_sub(p.1);
                if tot > 0 {
                    let usato = tot.saturating_sub(ora.0.saturating_sub(p.0));
                    v.insert("cpu".into(), (usato as f64 * 100.0 / tot as f64).round().into());
                }
            }
            self.prima = Some(ora);
        }
        if let Some(r) = ram() {
            v.insert("ram".into(), r.into());
        }
        if let Some(b) = crate::power::batteria() {
            v.insert("batteria".into(), b.into());
            v.insert("carica".into(), crate::power::on_ac().into());
        }
        v.into()
    }
}
