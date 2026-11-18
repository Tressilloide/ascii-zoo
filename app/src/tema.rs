//! Tema del sistema su Windows, per "Tema > Come il sistema": quello delle app
//! (Impostazioni > Personalizzazione > Colori > Modalità app).

use winreg::enums::{HKEY_CURRENT_USER, KEY_READ};
use winreg::RegKey;

const CHIAVE: &str = r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize";

/// Some(true) se il sistema è scuro, Some(false) se è chiaro, None se non si capisce
pub fn scuro() -> Option<bool> {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(CHIAVE, KEY_READ)
        .and_then(|k| k.get_value::<u32, _>("AppsUseLightTheme"))
        .ok()
        .map(|chiaro| chiaro == 0)
}
