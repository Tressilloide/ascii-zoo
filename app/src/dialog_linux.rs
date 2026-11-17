//! Finestrella per scrivere il nome di un animale su Linux: zenity (GNOME, incluso
//! in Ubuntu) oppure kdialog (KDE).
//!
//! Domanda e nome attuale passano come argomenti, mai attraverso una shell:
//! virgolette o altri caratteri nel nome non possono cambiarne il senso.

use std::process::Command;

/// zenity legge --text come markup Pango: & < > vanno scritti come entità
fn markup(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

/// None se si preme Annulla o se non c'è nessuno dei due programmi
pub fn chiedi_nome(titolo: &str, domanda: &str, attuale: &str, max: usize) -> Option<String> {
    let zenity = Command::new("zenity")
        .args(["--entry", "--title", titolo, "--text", &markup(domanda), "--entry-text", attuale])
        .output();
    let out = match zenity {
        Ok(o) => o,
        Err(_) => Command::new("kdialog").args(["--title", titolo, "--inputbox", domanda, attuale]).output().ok()?,
    };
    if !out.status.success() {
        return None;
    }
    let testo = String::from_utf8_lossy(&out.stdout);
    Some(testo.trim_end_matches(['\n', '\r']).chars().take(max).collect())
}
