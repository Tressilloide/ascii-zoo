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
