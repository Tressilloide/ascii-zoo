//! Lo screensaver su macOS: il modulo `ASCII Zoo.saver` (compilato da dreams/macos)
//! viaggia dentro l'app, in `Contents/Resources`. La voce "Usa come screensaver…" lo copia
//! in `~/Library/Screen Savers` e apre le impostazioni del salvaschermo, dove lo si sceglie;
//! all'avvio l'app aggiorna la copia installata se è diversa dalla sua, come su Windows.
//!
//! Si tocca solo `~/Library/Screen Savers/ASCII Zoo.saver`, e solo se è nostro: lo dice il
//! CFBundleIdentifier del suo Info.plist. Un modulo di altri con lo stesso nome resta com'è.

use std::fs;
use std::io;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::Command;

use objc2::rc::autoreleasepool;
use objc2::runtime::AnyObject;
use objc2_foundation::{ns_string, NSDictionary, NSProcessInfo, NSString};

use crate::shared::zoo_log;

const NOME: &str = "ASCII Zoo.saver";
/// lo stesso di dreams/macos/Info.plist
const ID: &str = "io.github.tressilloide.asciizoo.saver";

/// il modulo dentro l'app: ASCII Zoo.app/Contents/Resources/ASCII Zoo.saver
/// (None se il programma non gira da una .app, per esempio con cargo run)
pub fn nell_app() -> Option<PathBuf> {
    let exe = std::env::current_exe().ok()?;
    let p = exe.parent()?.parent()?.join("Resources").join(NOME);
    p.join("Contents/Info.plist").is_file().then_some(p)
}

/// dove macOS cerca i salvaschermo di questo utente
fn installato() -> Option<PathBuf> {
    Some(PathBuf::from(std::env::var_os("HOME")?).join("Library/Screen Savers").join(NOME))
}

/// il CFBundleIdentifier di un bundle (Info.plist in XML o binario)
fn identificativo(bundle: &Path) -> Option<String> {
    let plist = bundle.join("Contents/Info.plist");
    let plist = NSString::from_str(plist.to_str()?);
    // anche fuori dal ciclo degli eventi (--installa-salvaschermo, all'avvio)
    autoreleasepool(|_| {
        #[allow(deprecated)]
        let d = unsafe { NSDictionary::<NSString, AnyObject>::dictionaryWithContentsOfFile(&plist) }?;
        let v = d.objectForKey(ns_string!("CFBundleIdentifier"))?;
        Some(v.downcast_ref::<NSString>()?.to_string())
    })
}

/// true se in `p` c'è il nostro modulo (una cartella, non un collegamento, con il nostro identificativo)
fn nostro(p: &Path) -> bool {
    fs::symlink_metadata(p).map(|m| m.is_dir()).unwrap_or(false) && identificativo(p).as_deref() == Some(ID)
}

/// i file di un bundle, con il percorso relativo, in ordine
fn file(radice: &Path) -> io::Result<Vec<PathBuf>> {
    fn giro(radice: &Path, cartella: &Path, out: &mut Vec<PathBuf>) -> io::Result<()> {
        for voce in fs::read_dir(cartella)? {
            let voce = voce?;
            let tipo = voce.file_type()?;
            if tipo.is_dir() {
                giro(radice, &voce.path(), out)?;
            } else {
                out.push(voce.path().strip_prefix(radice).map_err(io::Error::other)?.to_path_buf());
            }
        }
        Ok(())
    }
    let mut out = Vec::new();
    giro(radice, radice, &mut out)?;
    out.sort();
    Ok(out)
}

/// true se i due bundle hanno gli stessi file con gli stessi byte
fn uguali(a: &Path, b: &Path) -> bool {
    let (Ok(fa), Ok(fb)) = (file(a), file(b)) else { return false };
    fa == fb && fa.iter().all(|f| matches!((fs::read(a.join(f)), fs::read(b.join(f))), (Ok(x), Ok(y)) if x == y))
}

/// Copia il bundle scrivendo i file uno per uno. Non fs::copy: su macOS copia anche gli
/// attributi estesi, compresa la quarantena di un'app scaricata, e macOS rifiuterebbe il
/// modulo. Si tengono solo i permessi (l'eseguibile resta eseguibile).
fn copia(da: &Path, a: &Path) -> io::Result<()> {
    fs::create_dir(a)?;
    for voce in fs::read_dir(da)? {
        let voce = voce?;
        let tipo = voce.file_type()?;
        let (sorgente, dest) = (voce.path(), a.join(voce.file_name()));
        if tipo.is_dir() {
            copia(&sorgente, &dest)?;
        } else if tipo.is_file() {
            fs::write(&dest, fs::read(&sorgente)?)?;
            let modo = voce.metadata()?.permissions().mode() & 0o755;
            fs::set_permissions(&dest, fs::Permissions::from_mode(modo))?;
        }
        // collegamenti simbolici: nel nostro modulo non ce ne sono
    }
    Ok(())
}

/// Mette (o rimette) il modulo in ~/Library/Screen Savers. Prima lo copia accanto con un
/// nome nascosto, poi lo scambia con quello vecchio: macOS non vede mai un modulo a metà.
fn metti() -> Result<PathBuf, String> {
    let da = nell_app().ok_or("il modulo non c'è: avvia ASCII Zoo dalla sua app (ASCII Zoo.app)")?;
    let dest = installato().ok_or("manca la cartella dell'utente (HOME)")?;
    let cartella = dest.parent().ok_or("percorso strano")?.to_path_buf();
    if fs::symlink_metadata(&dest).is_ok() && !nostro(&dest) {
        return Err(format!("in {} c'è già un \"{NOME}\" che non è di ASCII Zoo: non lo tocco", cartella.display()));
    }
    fs::create_dir_all(&cartella).map_err(|e| format!("{}: {e}", cartella.display()))?;
    let pid = std::process::id();
    let nuovo = cartella.join(format!(".{NOME}.nuovo-{pid}"));
    let vecchio = cartella.join(format!(".{NOME}.vecchio-{pid}"));
    if let Err(e) = copia(&da, &nuovo) {
        let _ = fs::remove_dir_all(&nuovo);
        return Err(format!("copia in {}: {e}", cartella.display()));
    }
    let c_era = dest.exists();
    if c_era {
        if let Err(e) = fs::rename(&dest, &vecchio) {
            let _ = fs::remove_dir_all(&nuovo);
            return Err(format!("{}: {e}", dest.display()));
        }
    }
    if let Err(e) = fs::rename(&nuovo, &dest) {
        // si rimette com'era
        if c_era {
            let _ = fs::rename(&vecchio, &dest);
        }
        let _ = fs::remove_dir_all(&nuovo);
        return Err(format!("{}: {e}", dest.display()));
    }
    if c_era {
        // era nostro (controllato sopra): la versione vecchia si può cancellare
        let _ = fs::remove_dir_all(&vecchio);
    }
    Ok(dest)
}

/// Se il modulo è installato ma è diverso da quello dentro l'app (un'altra versione), lo
/// sostituisce: così dopo un aggiornamento dell'app non resta indietro. Se non c'è non fa niente.
pub fn aggiorna_se_installato() {
    let (Some(da), Some(dest)) = (nell_app(), installato()) else { return };
    if !nostro(&dest) || uguali(&da, &dest) {
        return;
    }
    match metti() {
        Ok(_) => zoo_log!("screensaver aggiornato in {}", dest.display()),
        Err(e) => zoo_log!("screensaver non aggiornato: {e}"),
    }
}

/// "Usa come screensaver…": installa il modulo e apre le impostazioni del salvaschermo
pub fn installa() {
    match metti() {
        Ok(dest) => {
            zoo_log!("screensaver installato in {}", dest.display());
            apri_impostazioni();
        }
        Err(e) => avviso("Lo screensaver non si è installato", &e),
    }
}

/// "ascii-zoo --installa-salvaschermo [--apri-impostazioni]" (per la prova in CI): installa
/// il modulo, apre le impostazioni solo se richiesto ed esce; con 1 se non ci riesce
pub fn installa_da_riga_di_comando(apri: bool) -> ! {
    match metti() {
        Ok(dest) => {
            println!("screensaver installato in {}", dest.display());
            if apri {
                apri_impostazioni();
            }
            std::process::exit(0)
        }
        Err(e) => {
            eprintln!("screensaver non installato: {e}");
            std::process::exit(1)
        }
    }
}

/// Apre le impostazioni del salvaschermo. Dove sono cambia con le versioni di macOS:
/// - 11 e 12: Preferenze di Sistema, "Scrivania e salvaschermo"
/// - da 13 a 15: Impostazioni di Sistema, "Salvaschermo"
/// - da 26: il salvaschermo è dentro "Sfondo" (il pannello a parte non c'è più)
fn apri_impostazioni() {
    // macOS 26 dice "16" ai programmi compilati con un SDK più vecchio: dal 16 in su è "Sfondo"
    let versione = NSProcessInfo::processInfo().operatingSystemVersion().majorVersion;
    let mut c = Command::new("/usr/bin/open");
    if versione >= 16 {
        c.arg("x-apple.systempreferences:com.apple.Wallpaper-Settings.extension?ScreenSaver");
    } else if versione >= 13 {
        c.arg("x-apple.systempreferences:com.apple.ScreenSaver-Settings.extension");
    } else {
        c.args(["-b", "com.apple.systempreferences", "/System/Library/PreferencePanes/DesktopScreenEffectsPref.prefPane"]);
    }
    if !c.status().map(|s| s.success()).unwrap_or(false) {
        // riserva: le impostazioni di sistema e basta
        let _ = Command::new("/usr/bin/open").args(["-b", "com.apple.systempreferences"]).status();
    }
}

/// un avviso di macOS (osascript); titolo e testo passano come argomenti, mai dentro lo script
fn avviso(titolo: &str, testo: &str) {
    let _ = Command::new("/usr/bin/osascript")
        .args(["-e", "on run argv\n  display alert (item 1 of argv) message (item 2 of argv) as warning\nend run"])
        .args([titolo, testo])
        .spawn();
}
