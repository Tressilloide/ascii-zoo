//! CPU, RAM e batteria per il widget "pc" (Windows).

use windows::Win32::Foundation::FILETIME;
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
use windows::Win32::System::Threading::GetSystemTimes;

#[derive(Default)]
pub struct Stats {
    /// tempi della lettura precedente: (inattivo, totale)
    prima: Option<(u64, u64)>,
}

fn n(t: FILETIME) -> u64 {
    (t.dwHighDateTime as u64) << 32 | t.dwLowDateTime as u64
}

impl Stats {
    /// i valori come oggetto JSON per zooSetting('statistiche', ...)
    pub fn leggi(&mut self) -> serde_json::Value {
        let mut v = serde_json::Map::new();
        unsafe {
            let (mut idle, mut kernel, mut user) = (FILETIME::default(), FILETIME::default(), FILETIME::default());
            if GetSystemTimes(Some(&mut idle), Some(&mut kernel), Some(&mut user)).is_ok() {
                // il tempo "kernel" comprende anche quello inattivo
                let ora = (n(idle), n(kernel) + n(user));
                if let Some(p) = self.prima {
                    let tot = ora.1.saturating_sub(p.1);
                    if tot > 0 {
                        let usato = tot.saturating_sub(ora.0.saturating_sub(p.0));
                        v.insert("cpu".into(), (usato as f64 * 100.0 / tot as f64).round().into());
                    }
                }
                self.prima = Some(ora);
            }
            let mut m = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..Default::default() };
            if GlobalMemoryStatusEx(&mut m).is_ok() {
                v.insert("ram".into(), m.dwMemoryLoad.into());
            }
            let mut p = SYSTEM_POWER_STATUS::default();
            // 255 = sconosciuto; bit 128 = nessuna batteria (PC fisso)
            if GetSystemPowerStatus(&mut p).is_ok() && p.BatteryLifePercent <= 100 && p.BatteryFlag & 128 == 0 {
                v.insert("batteria".into(), p.BatteryLifePercent.into());
                v.insert("carica".into(), (p.ACLineStatus == 1).into());
            }
        }
        v.into()
    }
}
