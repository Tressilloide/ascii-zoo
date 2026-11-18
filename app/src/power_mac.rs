//! Batteria su macOS (IOKit).

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    /// secondi di batteria rimasti; -2 = illimitato (alimentatore collegato), -1 = a batteria, stima in corso
    fn IOPSGetTimeRemainingEstimate() -> f64;
}

const ILLIMITATO: f64 = -2.0;

/// true se l'alimentatore è collegato (o se è un Mac senza batteria)
pub fn on_ac() -> bool {
    unsafe { IOPSGetTimeRemainingEstimate() == ILLIMITATO }
}

/// Carica della batteria del Mac in percentuale, se ce n'è una (IOPowerSources):
/// Current Capacity / Max Capacity della batteria interna.
pub fn batteria() -> Option<u8> {
    use objc2_core_foundation::{CFArray, CFDictionary, CFNumber, CFRetained, CFString, CFType};
    use objc2_io_kit::{IOPSCopyPowerSourcesInfo, IOPSCopyPowerSourcesList, IOPSGetPowerSourceDescription};

    let info = IOPSCopyPowerSourcesInfo()?;
    // SAFETY: info è quello restituito da IOPSCopyPowerSourcesInfo, e la lista contiene
    // CFTypeRef (lo dice la documentazione di IOPSCopyPowerSourcesList)
    let lista = unsafe { IOPSCopyPowerSourcesList(Some(&info)) }?;
    let lista = unsafe { CFRetained::cast_unchecked::<CFArray<CFType>>(lista) };
    let chiave = |k: &std::ffi::CStr| CFString::from_str(k.to_str().unwrap_or_default());
    let (tipo, presente) = (chiave(objc2_io_kit::kIOPSTypeKey), chiave(objc2_io_kit::kIOPSIsPresentKey));
    let (ora, massima) = (chiave(objc2_io_kit::kIOPSCurrentCapacityKey), chiave(objc2_io_kit::kIOPSMaxCapacityKey));
    let interna = chiave(objc2_io_kit::kIOPSInternalBatteryType);
    for i in 0..lista.len() {
        let Some(ps) = lista.get(i) else { continue };
        // SAFETY: ps viene dalla lista di info; il dizionario ha chiavi CFString
        let Some(d) = (unsafe { IOPSGetPowerSourceDescription(Some(&info), Some(&ps)) }) else { continue };
        let d = unsafe { CFRetained::cast_unchecked::<CFDictionary<CFString, CFType>>(d) };
        let numero = |k: &CFString| d.get(k)?.downcast::<CFNumber>().ok()?.as_i64();
        let testo = |k: &CFString| Some(d.get(k)?.downcast::<CFString>().ok()?.to_string());
        // le batterie di mouse e tastiere Bluetooth hanno un altro tipo
        if testo(&tipo).as_ref() != Some(&interna.to_string()) {
            continue;
        }
        // "Is Present" è un CFBoolean: se c'è ed è falso, la batteria è stata tolta
        if let Some(p) = d.get(&presente).and_then(|v| v.downcast::<objc2_core_foundation::CFBoolean>().ok()) {
            if !p.as_bool() {
                continue;
            }
        }
        let (Some(c), Some(m)) = (numero(&ora), numero(&massima)) else { continue };
        if m > 0 {
            return Some((c * 100 / m).clamp(0, 100) as u8);
        }
    }
    None
}
