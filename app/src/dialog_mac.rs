//! Finestrella per scrivere il nome di un animale su macOS: una finestra di
//! dialogo standard di AppleScript (osascript, incluso in macOS).
//!
//! Domanda e nome attuale passano come argomenti, mai dentro il testo dello
//! script: virgolette o altri caratteri nel nome non possono cambiarne il senso.

use std::process::Command;

const SCRIPT: &str = r#"on run argv
  set risposta to display dialog (item 2 of argv) default answer (item 3 of argv) with title (item 1 of argv) buttons {"Annulla", "OK"} default button "OK" cancel button "Annulla"
  return text returned of risposta
end run"#;

/// None se si preme Annulla (il nome resta com'era)
pub fn chiedi_nome(titolo: &str, domanda: &str, attuale: &str, max: usize) -> Option<String> {
    let out = Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(SCRIPT)
        .arg(titolo)
        .arg(domanda)
        .arg(attuale)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let testo = String::from_utf8_lossy(&out.stdout);
    Some(testo.trim_end_matches(['\n', '\r']).chars().take(max).collect())
}
