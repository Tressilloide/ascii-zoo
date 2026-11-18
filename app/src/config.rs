//! Impostazioni salvate in %APPDATA%\AsciiZoo\config.json

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// lunghezza massima del nome di un animale (lettere)
pub const NOME_MAX: usize = 16;

#[derive(Serialize, Deserialize, Clone, Debug)]
#[serde(default)]
pub struct Config {
    /// 0 = a rotazione, 1..=7 = un animale fisso (stesso ordine del menu)
    pub animale: u32,
    /// con "a rotazione": koi nello stagno quando il PC è in carica
    pub koi_in_carica: bool,
    /// ferma l'animazione quando il PC va a batteria
    pub pausa_batteria: bool,
    /// ferma l'animazione quando un'app (gioco, video) è a schermo intero
    pub pausa_schermo_intero: bool,
    /// ferma l'animazione di uno schermo quando le finestre ne coprono tutto il desktop (solo Windows)
    pub pausa_coperto: bool,
    /// nome dell'animale più grande, per specie (id della specie → nome; "" = nessuno)
    pub nomi: BTreeMap<String, String>,
    /// ogni quanti secondi cambia animale "a rotazione"
    pub rotazione_secondi: u32,
    /// specie tolte dalla rotazione
    pub rotazione_escluse: Vec<String>,
    /// widget: nota sul desktop ("" = nessuna)
    pub nota: String,
    /// widget: conto alla rovescia, data "aaaa-mm-gg" ("" = spento)
    pub conto_data: String,
    pub conto_etichetta: String,
    /// widget: CPU, RAM e batteria
    pub widget_statistiche: bool,
    /// angolo dei widget: "basso-destra", "basso-sinistra", "alto-destra", "alto-sinistra"
    pub widget_angolo: String,
    /// su quale schermo stanno i widget: "" = il principale, "*" = tutti, se no l'id dello schermo
    pub widget_schermo: String,
    /// animale per schermo (id dello schermo → come "animale"); chi non c'è segue "animale"
    pub animale_schermi: BTreeMap<String, u32>,
    /// un clic su una zona vuota del desktop arriva agli animali (solo Windows)
    pub clic_desktop: bool,
    /// l'orologio con la data al centro dello schermo (spento: gli animali e la scenografia
    /// usano anche il centro)
    pub orologio: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            animale: 0,
            koi_in_carica: true,
            pausa_batteria: true,
            pausa_schermo_intero: true,
            pausa_coperto: true,
            nomi: BTreeMap::from([("koi".to_string(), "KARPISA".to_string())]),
            rotazione_secondi: 120,
            rotazione_escluse: Vec::new(),
            nota: String::new(),
            conto_data: String::new(),
            conto_etichetta: String::new(),
            widget_statistiche: false,
            widget_angolo: "basso-destra".into(),
            widget_schermo: String::new(),
            animale_schermi: BTreeMap::new(),
            clic_desktop: true,
            orologio: true,
        }
    }
}

/// pulisce un nome scritto dall'utente: niente caratteri di controllo, spazi ai lati tolti, al massimo NOME_MAX lettere
pub fn pulisci_nome(s: &str) -> String {
    pulisci_testo(s, NOME_MAX)
}

/// lunghezza massima della nota e dell'etichetta del conto alla rovescia
pub const NOTA_MAX: usize = 200;
pub const ETICHETTA_MAX: usize = 40;

/// toglie i caratteri di controllo e taglia a `max` lettere
pub fn pulisci_testo(s: &str, max: usize) -> String {
    s.chars().filter(|c| !c.is_control()).collect::<String>().trim().chars().take(max).collect()
}

/// Legge una data scritta dall'utente: "25/12/2026", "25-12-2026", "25.12.2026"
/// oppure "2026-12-25". Restituisce "aaaa-mm-gg"; None se non è una data valida.
pub fn leggi_data(s: &str) -> Option<String> {
    let parti: Vec<u32> = s.trim().split(['/', '-', '.']).map(|p| p.trim().parse().ok()).collect::<Option<_>>()?;
    let [a, b, c] = parti[..] else { return None };
    let (g, m, y) = if a > 31 { (c, b, a) } else { (a, b, c) };
    let y = if y < 100 { 2000 + y } else { y };
    let bisestile = (y % 4 == 0 && y % 100 != 0) || y % 400 == 0;
    let giorni = [31, if bisestile { 29 } else { 28 }, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    (1..=12).contains(&m).then_some(())?;
    (1..=giorni[m as usize - 1]).contains(&g).then_some(())?;
    (2000..=2200).contains(&y).then(|| format!("{y:04}-{m:02}-{g:02}"))
}

/// "2026-12-25" -> "25/12/2026", per mostrarla nel menu e nella finestrella
pub fn data_italiana(iso: &str) -> String {
    let p: Vec<&str> = iso.split('-').collect();
    if p.len() == 3 { format!("{}/{}/{}", p[2], p[1], p[0]) } else { iso.to_string() }
}

/// Un eseguibile che non resta dov'è: compilato da cargo (`target/debug`, `target/release`),
/// nella cartella temporanea o, su macOS, aperto dal DMG (`/Volumes`) o "traslocato" da
/// Gatekeeper (`AppTranslocation`). L'avvio automatico non lo si sposta lì da solo.
pub fn provvisorio(exe: &std::path::Path) -> bool {
    let nomi: Vec<String> = exe.components().map(|c| c.as_os_str().to_string_lossy().to_lowercase()).collect();
    let build = |n: &String| n == "debug" || n == "release";
    // target/release/ascii-zoo oppure target/<piattaforma>/release/ascii-zoo
    let cargo = nomi.windows(2).any(|w| w[0] == "target" && build(&w[1]))
        || nomi.windows(3).any(|w| w[0] == "target" && build(&w[2]));
    cargo || exe.starts_with(std::env::temp_dir()) || mac_volatile(exe)
}

/// macOS: l'app aperta dal DMG o traslocata da Gatekeeper sparisce o cambia percorso
fn mac_volatile(exe: &std::path::Path) -> bool {
    cfg!(target_os = "macos")
        && (exe.starts_with("/Volumes") || exe.components().any(|c| c.as_os_str() == "AppTranslocation"))
}

/// Windows: %APPDATA%\AsciiZoo\config.json
#[cfg(windows)]
fn path() -> PathBuf {
    let base = std::env::var_os("APPDATA").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    base.join("AsciiZoo").join("config.json")
}

/// macOS: ~/Library/Application Support/AsciiZoo/config.json
#[cfg(target_os = "macos")]
fn path() -> PathBuf {
    let home = std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."));
    home.join("Library").join("Application Support").join("AsciiZoo").join("config.json")
}

/// Linux: $XDG_CONFIG_HOME/AsciiZoo/config.json (di solito ~/.config/AsciiZoo/config.json)
#[cfg(target_os = "linux")]
fn path() -> PathBuf {
    linux_dir::config().join("AsciiZoo").join("config.json")
}

/// cartelle dell'utente su Linux, secondo XDG
#[cfg(target_os = "linux")]
pub mod linux_dir {
    use std::path::PathBuf;

    fn home() -> PathBuf {
        std::env::var_os("HOME").map(PathBuf::from).unwrap_or_else(|| PathBuf::from("."))
    }

    /// $XDG_CONFIG_HOME, se no ~/.config
    pub fn config() -> PathBuf {
        std::env::var_os("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .filter(|p| p.is_absolute())
            .unwrap_or_else(|| home().join(".config"))
    }
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(path())
            .ok()
            // un file salvato con il BOM (es. da Blocco note) non deve azzerare tutto
            .and_then(|s| serde_json::from_str(s.trim_start_matches('\u{feff}')).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let p = path();
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(p, s);
        }
    }
}

/// Avvio con Windows: chiave Run dell'utente corrente.
#[cfg(windows)]
pub mod autostart {
    use winreg::enums::{HKEY_CURRENT_USER, KEY_READ, KEY_WRITE};
    use winreg::RegKey;

    const RUN: &str = r"Software\Microsoft\Windows\CurrentVersion\Run";
    const NAME: &str = "AsciiZoo";

    pub fn enabled() -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey_with_flags(RUN, KEY_READ)
            .and_then(|k| k.get_value::<String, _>(NAME))
            .is_ok()
    }

    pub fn set(on: bool) {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, KEY_WRITE) else { return };
        if on {
            if let Ok(exe) = std::env::current_exe() {
                let _ = key.set_value(NAME, &format!("\"{}\"", exe.display()));
            }
        } else {
            let _ = key.delete_value(NAME);
        }
    }

    /// L'exe non si installa: una versione nuova può stare in un'altra cartella. Se l'avvio
    /// con Windows è attivo ma lancia un altro exe, lo si fa puntare a questo, la copia
    /// appena avviata. Spento resta spento; si tocca solo il valore "AsciiZoo".
    pub fn aggiorna() {
        let Ok(key) = RegKey::predef(HKEY_CURRENT_USER).open_subkey_with_flags(RUN, KEY_READ | KEY_WRITE) else { return };
        let Ok(attuale) = key.get_value::<String, _>(NAME) else { return };
        let Ok(exe) = std::env::current_exe() else { return };
        let nuovo = format!("\"{}\"", exe.display());
        if !attuale.eq_ignore_ascii_case(&nuovo) && !super::provvisorio(&exe) {
            let _ = key.set_value(NAME, &nuovo);
        }
    }
}

/// Apri al login su macOS: un LaunchAgent dell'utente,
/// ~/Library/LaunchAgents/io.github.tressilloide.asciizoo.plist
#[cfg(target_os = "macos")]
pub mod autostart {
    use std::path::PathBuf;

    const LABEL: &str = "io.github.tressilloide.asciizoo";

    fn plist() -> Option<PathBuf> {
        let home = std::env::var_os("HOME")?;
        Some(PathBuf::from(home).join("Library").join("LaunchAgents").join(format!("{LABEL}.plist")))
    }

    /// i caratteri speciali dell'XML nel percorso dell'app
    fn xml(s: &str) -> String {
        s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
    }

    pub fn enabled() -> bool {
        plist().is_some_and(|p| p.exists())
    }

    pub fn set(on: bool) {
        let Some(p) = plist() else { return };
        if !on {
            let _ = std::fs::remove_file(p);
            return;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        // dal DMG o traslocata: al prossimo accesso quel percorso non c'è più
        if super::mac_volatile(&exe) {
            return;
        }
        let body = format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>Label</key>
  <string>{LABEL}</string>
  <key>ProgramArguments</key>
  <array>
    <string>{}</string>
  </array>
  <key>RunAtLoad</key>
  <true/>
  <key>ProcessType</key>
  <string>Interactive</string>
</dict>
</plist>
"#,
            xml(&exe.to_string_lossy())
        );
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, body);
    }

    /// Una versione nuova può stare in un'altra .app: se "Apri al login" è attivo ma il
    /// LaunchAgent lancia un altro programma, lo si fa puntare a questo, la copia appena
    /// avviata. Spento resta spento.
    pub fn aggiorna() {
        let Some(p) = plist() else { return };
        let Ok(attuale) = std::fs::read_to_string(&p) else { return };
        let Ok(exe) = std::env::current_exe() else { return };
        let riga = format!("<string>{}</string>", xml(&exe.to_string_lossy()));
        if !attuale.contains(&riga) && !super::provvisorio(&exe) {
            set(true);
        }
    }
}

/// Apri al login su Linux: ~/.config/autostart/ascii-zoo.desktop (standard XDG,
/// vale per GNOME, KDE, XFCE…). Lo stesso file lo scrive anche linux/install.sh.
#[cfg(target_os = "linux")]
pub mod autostart {
    use std::path::PathBuf;

    fn file() -> PathBuf {
        super::linux_dir::config().join("autostart").join("ascii-zoo.desktop")
    }

    /// argomento di Exec= secondo la specifica Desktop Entry: tra virgolette, con
    /// \ " ` $ preceduti da \ (e il \ va raddoppiato perché il file è un "string")
    fn exec(p: &str) -> String {
        let mut s = String::from("\"");
        for c in p.chars() {
            match c {
                '"' | '`' | '$' => {
                    s.push_str("\\\\");
                    s.push(c);
                }
                '\\' => s.push_str("\\\\\\\\"),
                '%' => s.push_str("%%"), // in Exec= "%" introduce i codici di campo
                '\n' | '\r' => {}
                _ => s.push(c),
            }
        }
        s.push('"');
        s
    }

    pub fn enabled() -> bool {
        file().exists()
    }

    pub fn set(on: bool) {
        let p = file();
        if !on {
            let _ = std::fs::remove_file(p);
            return;
        }
        let Ok(exe) = std::env::current_exe() else { return };
        let body = format!(
            "[Desktop Entry]\nType=Application\nName=ASCII Zoo\nComment=Sfondo animato con animali ASCII\nExec={}\nIcon=ascii-zoo\nTerminal=false\nX-GNOME-Autostart-enabled=true\n",
            exec(&exe.to_string_lossy())
        );
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, body);
    }

    /// Se "Apri al login" è attivo ma lancia un altro programma (una copia in un'altra
    /// cartella), lo si fa puntare a questo, la copia appena avviata. Spento resta spento.
    pub fn aggiorna() {
        let Ok(attuale) = std::fs::read_to_string(file()) else { return };
        let Ok(exe) = std::env::current_exe() else { return };
        let riga = format!("Exec={}", exec(&exe.to_string_lossy()));
        if !attuale.lines().any(|l| l.trim_end() == riga) && !super::provvisorio(&exe) {
            set(true);
        }
    }
}
