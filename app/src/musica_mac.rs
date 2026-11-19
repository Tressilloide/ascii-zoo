//! "In riproduzione" su macOS: AppleScript a Musica e a Spotify, solo se sono già aperti
//! (non si avviano mai). Il "Now Playing" di sistema (il framework privato MediaRemote)
//! da macOS 15.4 non risponde più alle app di terze parti.
//!
//! La prima volta macOS chiede il permesso "Automazione" per controllare Musica o
//! Spotify (il testo della richiesta è NSAppleEventsUsageDescription in Info.plist): per
//! questo il widget è spento di serie e lo accende l'utente. Si legge con osascript, sul
//! thread di riproduzione.rs: la richiesta del permesso lo tiene fermo finché l'utente
//! non risponde, e il resto del programma va avanti.

use std::process::Command;
use std::time::{Duration, Instant};

use objc2_app_kit::NSRunningApplication;
use objc2_foundation::NSString;

use crate::riproduzione::{Brano, Lettura};

/// ogni quanto al massimo: ogni lettura avvia osascript, e la pagina fa avanzare da sola
/// la posizione
pub const OGNI: Duration = Duration::from_secs(4);

/// dopo un permesso negato si riprova solo ogni tanto (l'utente può darlo nelle impostazioni)
const DOPO_NO: Duration = Duration::from_secs(60);

/// (bundle id, la durata è in millisecondi?): Spotify la dà in ms, Musica in secondi
const LETTORI: [(&str, bool); 2] = [("com.apple.Music", false), ("com.spotify.client", true)];

pub struct Lettore {
    /// per ogni lettore: fino a quando non chiedere, dopo un permesso negato
    negato: [Option<Instant>; 2],
}

impl Lettore {
    pub fn new() -> Self {
        Self { negato: [None, None] }
    }

    /// il lettore che sta suonando; se nessuno suona, quello in pausa
    pub fn leggi(&mut self) -> Lettura {
        let (mut in_pausa, mut permesso) = (None, false);
        for (k, (id, ms)) in LETTORI.into_iter().enumerate() {
            if !aperto(id) {
                continue;
            }
            if self.negato[k].is_some_and(|t| t.elapsed() < DOPO_NO) {
                permesso = true;
                continue;
            }
            match chiedi(id, ms) {
                Ok(Some(b)) if b.in_corso => return Lettura::Brano(b),
                Ok(Some(b)) => {
                    in_pausa.get_or_insert(b);
                }
                Ok(None) => self.negato[k] = None,
                Err(()) => {
                    self.negato[k] = Some(Instant::now());
                    permesso = true;
                }
            }
        }
        match in_pausa {
            Some(b) => Lettura::Brano(b),
            None if permesso => Lettura::Permesso,
            None => Lettura::Niente,
        }
    }
}

/// l'app è aperta? (lo dice LaunchServices, senza mandarle niente)
fn aperto(id: &str) -> bool {
    objc2::rc::autoreleasepool(|_| {
        NSRunningApplication::runningApplicationsWithBundleIdentifier(&NSString::from_str(id)).count() > 0
    })
}

/// Chiede a un lettore aperto stato, titolo, artista, durata e posizione, separati da
/// tabulazioni. Err se macOS non dà il permesso "Automazione".
fn chiedi(id: &str, durata_ms: bool) -> Result<Option<Brano>, ()> {
    let script = format!(
        r#"tell application id "{id}"
	set s to player state as text
	if s is not "playing" and s is not "paused" then return ""
	set n to ""
	set a to ""
	set d to 0
	set p to 0
	try
		set t to current track
		set n to name of t
		set a to artist of t
		set d to duration of t
	end try
	try
		set p to player position
	end try
	return s & tab & n & tab & a & tab & d & tab & p
end tell"#
    );
    let Ok(out) = Command::new("/usr/bin/osascript").arg("-e").arg(&script).output() else { return Ok(None) };
    if !out.status.success() {
        // -1743: permesso negato; -1744: servirebbe il consenso ma non si può chiedere
        let errore = String::from_utf8_lossy(&out.stderr);
        return if errore.contains("-1743") || errore.contains("-1744") { Err(()) } else { Ok(None) };
    }
    Ok(leggi_risposta(&String::from_utf8_lossy(&out.stdout), durata_ms))
}

/// "playing\tTitolo\tArtista\t225.5\t83.2" → Brano (i numeri con il punto o, con le
/// impostazioni italiane, con la virgola)
fn leggi_risposta(s: &str, durata_ms: bool) -> Option<Brano> {
    let p: Vec<&str> = s.trim_end_matches(['\n', '\r']).split('\t').collect();
    let [stato, titolo, artista, durata, posizione] = p[..] else { return None };
    let numero = |x: &str| x.trim().replace(',', ".").parse::<f64>().ok().filter(|v| v.is_finite() && *v > 0.0);
    if titolo.trim().is_empty() && artista.trim().is_empty() {
        return None;
    }
    let durata = numero(durata).map(|d| (if durata_ms { d } else { d * 1000.0 }) as u64);
    Some(Brano {
        titolo: titolo.to_string(),
        artista: artista.to_string(),
        in_corso: stato == "playing",
        posizione: numero(posizione).map(|s| (s * 1000.0) as u64).or(durata.map(|_| 0)),
        durata,
    })
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn risposta_di_musica_e_spotify() {
        let b = leggi_risposta("playing\tNinna nanna\tKARPISA\t225,5\t83,25\n", false).unwrap();
        assert_eq!((b.in_corso, b.durata, b.posizione), (true, Some(225_500), Some(83_250)));
        let b = leggi_risposta("paused\tCanzone\tArtista\t200000\t12.5\n", true).unwrap();
        assert_eq!((b.in_corso, b.durata, b.posizione), (false, Some(200_000), Some(12_500)));
        assert!(leggi_risposta("\n", false).is_none());
    }
}
