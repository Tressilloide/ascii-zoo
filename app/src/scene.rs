//! Scene dell'utente: file `.js` nella cartella `scene` accanto alle impostazioni
//! (Windows `%APPDATA%\AsciiZoo\scene`, macOS `~/Library/Application Support/AsciiZoo/scene`,
//! Linux `~/.config/AsciiZoo/scene`).
//!
//! Ogni file è una scena senza animali (pioggia, un paesaggio…): chiama `scena({...})`
//! con i colori e le funzioni che disegnano. L'id è il nome del file, il nome nel menu
//! la prima riga `// nome: …` (se manca, il nome del file). Come scriverne una: README.
//!
//! Si leggono una volta all'avvio, nell'ordine dei nomi dei file, e vanno nel menu
//! "Animale" dopo gli animali di serie: per vederne una nuova si riavvia l'app.
//!
//! Sono codice scritto da altri, quindi la pagina dello sfondo, nell'app, non può
//! usare la rete né andare su altre pagine (vedi `shared::pagina` e `navigazione_permessa`),
//! e una scena che va in errore smette di disegnare senza fermare il resto.

use std::path::PathBuf;
use std::sync::OnceLock;

/// quante scene al massimo, e quanto può pesare ognuna
const MAX_SCENE: usize = 40;
const MAX_BYTE: u64 = 256 * 1024;
const NOME_MAX: usize = 40;

pub struct Scena {
    pub id: String,
    pub nome: String,
    codice: String,
}

/// la cartella delle scene (può non esistere)
pub fn cartella() -> PathBuf {
    crate::config::cartella().join("scene")
}

/// le scene trovate all'avvio (sempre le stesse finché l'app gira)
pub fn tutte() -> &'static [Scena] {
    static SCENE: OnceLock<Vec<Scena>> = OnceLock::new();
    SCENE.get_or_init(carica)
}

/// "campo-di-grano.js" → "campo-di-grano"; solo lettere minuscole, cifre e trattini
fn id_da_file(stem: &str) -> Option<String> {
    let id: String = stem
        .trim()
        .to_lowercase()
        .chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '-' })
        .collect();
    let id = id.trim_matches('-').to_string();
    (!id.is_empty() && id.len() <= 40).then_some(id)
}

/// "// nome: Campo di grano" sulla prima riga, se no il nome del file
fn nome_scena(codice: &str, stem: &str) -> String {
    let prima = codice.trim_start_matches('\u{feff}').lines().next().unwrap_or("").trim();
    let dal_file = prima
        .strip_prefix("//")
        .map(str::trim)
        .and_then(|r| r.strip_prefix("nome:").or_else(|| r.strip_prefix("name:")))
        .map(str::trim)
        .filter(|n| !n.is_empty());
    let nome = match dal_file {
        Some(n) => n.to_string(),
        None => {
            let n = stem.replace(['-', '_'], " ");
            let mut c = n.chars();
            c.next().map(|p| p.to_uppercase().collect::<String>() + c.as_str()).unwrap_or_default()
        }
    };
    let nome: String = nome.chars().filter(|c| !c.is_control()).take(NOME_MAX).collect();
    nome.trim().to_string()
}

fn carica() -> Vec<Scena> {
    let Ok(dir) = std::fs::read_dir(cartella()) else {
        return Vec::new();
    };
    let mut file: Vec<PathBuf> = dir
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_file() && p.extension().is_some_and(|e| e.eq_ignore_ascii_case("js")))
        .collect();
    file.sort();
    let mut scene: Vec<Scena> = Vec::new();
    for p in file {
        if scene.len() >= MAX_SCENE {
            break;
        }
        let Some(stem) = p.file_stem().and_then(|s| s.to_str()) else { continue };
        let Some(id) = id_da_file(stem) else { continue };
        // un id già usato (da un animale o da un'altra scena) si salta
        if crate::shared::SPECIE.iter().any(|s| s.0 == id) || scene.iter().any(|s| s.id == id) {
            crate::shared::zoo_log!("scena {id}: id già usato, saltata");
            continue;
        }
        if std::fs::metadata(&p).map(|m| m.len() > MAX_BYTE).unwrap_or(true) {
            crate::shared::zoo_log!("scena {id}: file troppo grande, saltata");
            continue;
        }
        let Ok(codice) = std::fs::read_to_string(&p) else { continue };
        let nome = nome_scena(&codice, stem);
        if nome.is_empty() {
            continue;
        }
        crate::shared::zoo_log!("scena {id}: \"{nome}\"");
        scene.push(Scena { id, nome, codice: codice.trim_start_matches('\u{feff}').to_string() });
    }
    scene
}

/// i `<script>` da mettere nella pagina prima del motore: uno per scena, così un
/// errore di sintassi in un file non ferma le altre
pub fn script() -> String {
    let mut out = String::new();
    for s in tutte() {
        // "</script" dentro il codice chiuderebbe il tag
        let codice = s.codice.replace("</script", "<\\/script").replace("</SCRIPT", "<\\/SCRIPT");
        let id = serde_json::to_string(&s.id).unwrap_or_default();
        let nome = serde_json::to_string(&s.nome).unwrap_or_default();
        out.push_str(&format!(
            "<script>(window.ZOO_SCENE = window.ZOO_SCENE || []).push({{ id: {id}, nome: {nome}, \
             f: function (scena, hash2, clamp, lerp, TAU, celle) {{\n{codice}\n}} }});</script>\n"
        ));
    }
    out
}

/// apre la cartella delle scene nel file manager (la crea se manca, con due righe di aiuto)
pub fn apri_cartella() {
    let dir = cartella();
    let _ = std::fs::create_dir_all(&dir);
    let leggimi = dir.join("LEGGIMI.txt");
    if !leggimi.exists() {
        let _ = std::fs::write(
            &leggimi,
            "Scene di ASCII Zoo\r\n\r\n\
             Ogni file .js in questa cartella è uno sfondo in più nel menu \"Animale\".\r\n\
             Per vederne uno nuovo esci da ASCII Zoo e riaprilo.\r\n\
             Come si scrive una scena, con un esempio:\r\n\
             https://github.com/ascii-things/ascii-zoo#your-own-scenes\r\n",
        );
    }
    #[cfg(windows)]
    let prog = "explorer";
    #[cfg(target_os = "macos")]
    let prog = "open";
    #[cfg(target_os = "linux")]
    let prog = "xdg-open";
    let _ = std::process::Command::new(prog).arg(&dir).spawn();
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn id_e_nome() {
        assert_eq!(id_da_file("Campo di Grano").as_deref(), Some("campo-di-grano"));
        assert_eq!(id_da_file("--").as_deref(), None);
        assert_eq!(nome_scena("// nome: Campo di grano\nscena({})", "x"), "Campo di grano");
        assert_eq!(nome_scena("scena({})", "lucciole-blu"), "Lucciole blu");
    }
}
