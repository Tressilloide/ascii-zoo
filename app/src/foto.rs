//! "Salva una foto dello sfondo": ogni schermo manda un PNG di come appare adesso
//! (`zooFoto()` nella pagina: animali, scenografia, orologio e widget) e lo si salva
//! nella cartella Immagini dell'utente, sottocartella "ASCII Zoo", con data e ora nel
//! nome: `ascii-zoo-2026-11-19-031500.png`, con più schermi `…-schermo-1.png`,
//! `…-schermo-2.png`. Un file che c'è già non si sovrascrive mai. Alla fine si mostra la
//! foto nel gestore dei file (Esplora risorse, Finder, il file manager di Linux).
//!
//! Il PNG arriva come data URL in base64 dentro la stringa JSON del risultato dello
//! script: qualche MB per uno schermo 4K. Decodifica, scrittura e gestore dei file
//! stanno in un thread a parte: il ciclo degli eventi non si ferma.

use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use wry::WebView;

use crate::shared::zoo_log;

/// la foto della pagina; "" se la pagina non risponde come deve (eccezioni comprese)
const JS: &str = "(function () { try { return window.zooFoto ? zooFoto() : ''; } catch (e) { return ''; } })()";

/// Uno scatto su tutti gli schermi. Ogni schermo ne tiene una copia finché non ha finito
/// (anche quando la pagina non risponderà mai: wry butta via la richiesta); quando se ne
/// va l'ultima, si mostrano i file salvati.
struct Scatto {
    cartella: PathBuf,
    salvati: Mutex<Vec<PathBuf>>,
}

impl Drop for Scatto {
    fn drop(&mut self) {
        let mut salvati = std::mem::take(self.salvati.get_mut().unwrap_or_else(|e| e.into_inner()));
        if salvati.is_empty() {
            zoo_log!("foto: nessun file salvato");
            return;
        }
        salvati.sort();
        // può capitare nel thread del ciclo degli eventi: il gestore dei file si apre a parte
        std::thread::spawn(move || mostra(&salvati));
    }
}

/// chiede la foto a ogni schermo, nell'ordine (il principale per primo)
pub fn scatta<'a>(webviews: impl Iterator<Item = &'a WebView>) {
    let webviews: Vec<&WebView> = webviews.collect();
    let base = format!("ascii-zoo-{}", ora_locale());
    let scatto = Arc::new(Scatto { cartella: cartella_immagini().join("ASCII Zoo"), salvati: Mutex::new(Vec::new()) });
    zoo_log!("foto: {} schermi, in {}", webviews.len(), scatto.cartella.display());
    for (i, wv) in webviews.iter().enumerate() {
        let nome = if webviews.len() > 1 { format!("{base}-schermo-{}", i + 1) } else { base.clone() };
        let s = scatto.clone();
        let fatto = wv.evaluate_script_with_callback(JS, move |json| {
            let (s, nome) = (s.clone(), nome.clone());
            std::thread::spawn(move || salva(&s, &nome, &json));
        });
        if let Err(e) = fatto {
            zoo_log!("foto: lo schermo {} non risponde: {e}", i + 1);
        }
    }
}

/// il risultato dello script (una stringa JSON con il data URL) diventa un file PNG
fn salva(s: &Scatto, nome: &str, json: &str) {
    // serde_json, non a mano: macOS scrive "/" come "\/"
    let url: String = serde_json::from_str(json).unwrap_or_default();
    let png = url.strip_prefix("data:image/png;base64,").and_then(base64);
    let Some(png) = png.filter(|p| p.starts_with(b"\x89PNG\r\n\x1a\n")) else {
        zoo_log!("foto {nome}: la pagina non ha mandato un'immagine ({} caratteri)", json.len());
        return;
    };
    if let Err(e) = std::fs::create_dir_all(&s.cartella) {
        zoo_log!("foto: non riesco a creare {}: {e}", s.cartella.display());
        return;
    }
    match scrivi_nuovo(&s.cartella, nome, &png) {
        Ok(p) => {
            zoo_log!("foto salvata: {} ({} byte)", p.display(), png.len());
            s.salvati.lock().unwrap_or_else(|e| e.into_inner()).push(p);
        }
        Err(e) => zoo_log!("foto {nome}: non riesco a scriverla: {e}"),
    }
}

/// Scrive senza mai sovrascrivere: `nome.png`, se c'è già `nome (2).png`, `nome (3).png`…
/// create_new fa il controllo e la creazione in un colpo solo.
fn scrivi_nuovo(cartella: &Path, nome: &str, dati: &[u8]) -> std::io::Result<PathBuf> {
    for n in 1..1000 {
        let p = cartella.join(if n == 1 { format!("{nome}.png") } else { format!("{nome} ({n}).png") });
        match std::fs::OpenOptions::new().write(true).create_new(true).open(&p) {
            Ok(mut f) => {
                if let Err(e) = f.write_all(dati).and_then(|_| f.sync_all()) {
                    drop(f);
                    let _ = std::fs::remove_file(&p); // niente file a metà
                    return Err(e);
                }
                return Ok(p);
            }
            Err(e) if e.kind() == ErrorKind::AlreadyExists => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(ErrorKind::AlreadyExists, "troppi file con lo stesso nome"))
}

/// base64 standard (quello dei data URL); None se c'è un carattere che non va
fn base64(s: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(s.len() / 4 * 3);
    let (mut acc, mut bit) = (0u32, 0u32);
    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            b'=' => break,
            b'\r' | b'\n' | b' ' => continue,
            _ => return None,
        };
        acc = (acc << 6) | v as u32;
        bit += 6;
        if bit >= 8 {
            bit -= 8;
            out.push((acc >> bit) as u8);
            acc &= (1 << bit) - 1;
        }
    }
    Some(out)
}

/// data e ora locali per il nome del file: "2026-11-19-031500"
#[cfg(unix)]
fn ora_locale() -> String {
    // SAFETY: time e localtime_r scrivono solo nelle variabili passate
    let tm = unsafe {
        let t = libc::time(std::ptr::null_mut());
        let mut tm: libc::tm = std::mem::zeroed();
        libc::localtime_r(&t, &mut tm);
        tm
    };
    format!(
        "{:04}-{:02}-{:02}-{:02}{:02}{:02}",
        tm.tm_year + 1900,
        tm.tm_mon + 1,
        tm.tm_mday,
        tm.tm_hour,
        tm.tm_min,
        tm.tm_sec
    )
}

#[cfg(windows)]
fn ora_locale() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}-{:02}{:02}{:02}", t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond)
}

fn home() -> PathBuf {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var).map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
}

/// Windows: la cartella Immagini vera (anche se spostata, per esempio in OneDrive)
#[cfg(windows)]
fn cartella_immagini() -> PathBuf {
    use windows::Win32::System::Com::CoTaskMemFree;
    use windows::Win32::UI::Shell::{FOLDERID_Pictures, SHGetKnownFolderPath, KF_FLAG_DEFAULT};
    unsafe {
        if let Ok(p) = SHGetKnownFolderPath(&FOLDERID_Pictures, KF_FLAG_DEFAULT, None) {
            let s = p.to_string();
            CoTaskMemFree(Some(p.0 as *const _));
            if let Some(s) = s.ok().filter(|s| !s.is_empty()) {
                return PathBuf::from(s);
            }
        }
    }
    home().join("Pictures")
}

/// macOS: ~/Pictures (in italiano il Finder la chiama Immagini)
#[cfg(target_os = "macos")]
fn cartella_immagini() -> PathBuf {
    home().join("Pictures")
}

/// Linux: XDG_PICTURES_DIR (variabile d'ambiente o ~/.config/user-dirs.dirs, dove di
/// solito c'è XDG_PICTURES_DIR="$HOME/Immagini"), altrimenti ~/Pictures
#[cfg(target_os = "linux")]
fn cartella_immagini() -> PathBuf {
    let home = home();
    let dal_file = || {
        let testo = std::fs::read_to_string(crate::config::linux_dir::config().join("user-dirs.dirs")).ok()?;
        let riga = testo.lines().map(str::trim).find_map(|r| r.strip_prefix("XDG_PICTURES_DIR="))?;
        let valore = riga.trim().trim_matches('"');
        match valore.strip_prefix("$HOME") {
            Some(resto) => Some(home.join(resto.trim_start_matches('/'))),
            None => Some(PathBuf::from(valore)),
        }
    };
    std::env::var_os("XDG_PICTURES_DIR")
        .map(PathBuf::from)
        .or_else(dal_file)
        // "$HOME" da solo vuol dire "nessuna cartella Immagini"
        .filter(|p| p.is_absolute() && *p != home)
        .unwrap_or_else(|| home.join("Pictures"))
}

/// Windows: Esplora risorse con la foto selezionata
#[cfg(windows)]
fn mostra(file: &[PathBuf]) {
    use std::os::windows::process::CommandExt;
    // tra virgolette dopo la virgola, scritto così com'è: con gli spazi nel percorso
    // (OneDrive, nomi utente) Esplora risorse non capirebbe l'argomento citato per intero
    let _ = std::process::Command::new("explorer.exe").raw_arg(format!("/select,\"{}\"", file[0].display())).status();
}

/// macOS: il Finder con la foto selezionata
#[cfg(target_os = "macos")]
fn mostra(file: &[PathBuf]) {
    let _ = std::process::Command::new("/usr/bin/open").arg("-R").arg(&file[0]).status();
}

/// Linux: il gestore dei file con le foto selezionate (org.freedesktop.FileManager1, che
/// hanno Nautilus, Dolphin, Nemo…); se non c'è, la cartella con xdg-open
#[cfg(target_os = "linux")]
fn mostra(file: &[PathBuf]) {
    use gtk::gio;
    use gtk::prelude::*;
    let uri: Vec<String> = file.iter().map(|p| gio::File::for_path(p).uri().to_string()).collect();
    let fatto = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).and_then(|bus| {
        bus.call_sync(
            Some("org.freedesktop.FileManager1"),
            "/org/freedesktop/FileManager1",
            "org.freedesktop.FileManager1",
            "ShowItems",
            Some(&(uri, "").to_variant()),
            None,
            gio::DBusCallFlags::NONE,
            10_000,
            gio::Cancellable::NONE,
        )
    });
    match fatto {
        Ok(_) => zoo_log!("foto: mostrata nel gestore dei file"),
        Err(e) => {
            zoo_log!("foto: FileManager1 non risponde ({e}), apro la cartella");
            if let Some(cartella) = file[0].parent() {
                let _ = std::process::Command::new("xdg-open").arg(cartella).status();
            }
        }
    }
}
