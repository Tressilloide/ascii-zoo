//! Tema del sistema su macOS, per "Tema > Come il sistema": l'aspetto chiaro o scuro
//! (Impostazioni di Sistema > Aspetto, anche con "Automatico"). L'app non ne sceglie uno
//! suo, quindi il suo aspetto effettivo è quello del sistema e cambia insieme a lui.

use objc2::MainThreadMarker;
use objc2_app_kit::{NSAppearanceNameAqua, NSAppearanceNameDarkAqua, NSApplication};
use objc2_foundation::NSArray;

/// Some(true) se il sistema è scuro, Some(false) se è chiaro, None se non si capisce
pub fn scuro() -> Option<bool> {
    let mtm = MainThreadMarker::new()?;
    let aspetto = NSApplication::sharedApplication(mtm).effectiveAppearance();
    let (chiaro, scuro) = unsafe { (NSAppearanceNameAqua, NSAppearanceNameDarkAqua) };
    let scelto = aspetto.bestMatchFromAppearancesWithNames(&NSArray::from_slice(&[chiaro, scuro]))?;
    Some(scelto.isEqualToString(scuro))
}
