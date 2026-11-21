//! Batteria e app a schermo intero.

use windows::Win32::Foundation::RECT;
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTONULL};
use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
use windows::Win32::UI::WindowsAndMessaging::{GetClassNameW, GetForegroundWindow, GetWindowRect};

/// true se il caricatore è collegato (o se è un PC fisso senza batteria)
pub fn on_ac() -> bool {
    let mut s = SYSTEM_POWER_STATUS::default();
    unsafe {
        if GetSystemPowerStatus(&mut s).is_err() {
            return true;
        }
    }
    // 0 = batteria, 1 = corrente, 255 = sconosciuto
    s.ACLineStatus != 0
}

/// true se il PC ha una batteria (un PC fisso no: per lui on_ac è sempre vero, e "in carica"
/// non ha senso). Non cambia mentre l'app gira: si guarda una volta
pub fn ha_batteria() -> bool {
    static HA: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *HA.get_or_init(|| {
        let mut s = SYSTEM_POWER_STATUS::default();
        // BatteryFlag: 128 = nessuna batteria, 255 = non si sa (ha anche il bit 128).
        // Stessa regola del widget della batteria (stats.rs)
        let letto = unsafe { GetSystemPowerStatus(&mut s).is_ok() };
        letto && s.BatteryLifePercent <= 100 && s.BatteryFlag & 128 == 0
    })
}

/// Se la finestra in primo piano copre tutto il suo monitor (gioco, video…),
/// restituisce il rettangolo di quel monitor: si mette in pausa solo lo sfondo
/// di quello schermo.
pub fn fullscreen_monitor() -> Option<RECT> {
    unsafe {
        let fg = GetForegroundWindow();
        if fg.is_invalid() {
            return None;
        }
        let mut buf = [0u16; 64];
        let n = GetClassNameW(fg, &mut buf) as usize;
        let class = String::from_utf16_lossy(&buf[..n]);
        // il desktop stesso e la barra delle applicazioni non contano
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd" | "Shell_SecondaryTrayWnd") {
            return None;
        }
        let mon = MonitorFromWindow(fg, MONITOR_DEFAULTTONULL);
        if mon.is_invalid() {
            return None;
        }
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return None;
        }
        let mut r = RECT::default();
        if GetWindowRect(fg, &mut r).is_err() {
            return None;
        }
        let m = mi.rcMonitor;
        (r.left <= m.left && r.top <= m.top && r.right >= m.right && r.bottom >= m.bottom).then_some(m)
    }
}
