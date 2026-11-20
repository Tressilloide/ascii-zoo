//! Scene dell'utente: file `.js` nella cartella `scene` accanto alle impostazioni
//! (Windows `%APPDATA%\AsciiZoo\scene`, macOS `~/Library/Application Support/AsciiZoo/scene`,
//! Linux `~/.config/AsciiZoo/scene`).
//!
//! Ogni file è una scena senza animali (pioggia, un paesaggio…), che chiama `scena({...})`
//! con i colori e le funzioni che disegnano, oppure un animale scritto come quelli di
//! serie. L'id è il nome del file, il nome nel menu la prima riga `// nome: …` (se
//! manca, il nome del file). Come scriverne uno: README.
//!
//! Gli animali di serie sono anche file: l'app li copia dalla pagina (i pezzi tra le
//! righe `// ==== animale: id ====` e `// ==== fine id ====`) nella sottocartella
//! `di-serie`, riscritta a ogni avvio. Da lì si copiano nella cartella delle scene:
//! con lo stesso nome prendono il posto dell'animale di serie, con un nome nuovo sono
//! un animale in più.
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
    /// ha l'id di un animale di serie e ne prende il posto (non è una voce in più)
    pub sostituisce: bool,
    codice: String,
}

/// i nomi del motore che un file può usare (la pagina li mette nell'oggetto `api`)
const NOMI: [&str; 21] = [
    "scena", "SPECIES", "SCENES", "byId", "TAU", "FLOW", "clamp", "lerp", "angDiff", "hash2", "steer", "segD",
    "segT", "advance", "goTo", "defaultPoke", "place", "drawCells", "celle", "rockCells", "spring",
];

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
    if cartella().is_dir() {
        scrivi_di_serie();
    }
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
        // lo stesso id di un animale di serie: lo sostituisce; di un'altra scena: si salta
        let sostituisce = crate::shared::SPECIE.iter().any(|s| s.0 == id);
        if scene.iter().any(|s| s.id == id) {
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
        scene.push(Scena { id, nome, sostituisce, codice:codice.trim_start_matches('\u{feff}').to_string() });
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
        let nomi: Vec<String> = NOMI.iter().map(|n| format!("{n} = api.{n}")).collect();
        let nomi = nomi.join(", ");
        out.push_str(&format!(
            "<script>(window.ZOO_SCENE = window.ZOO_SCENE || []).push({{ id: {id}, nome: {nome}, \
             f: function (api) {{ var {nomi};\n{codice}\n}} }});</script>\n"
        ));
    }
    out
}

/// gli animali di serie come file, (id, codice): i pezzi della pagina tra le righe
/// `// ==== animale: id ====` e `// ==== fine id ====`, uno dopo l'altro
fn di_serie() -> Vec<(&'static str, String)> {
    crate::shared::SPECIE
        .iter()
        .map(|&(id, nome)| {
            let inizio = format!("  // ==== animale: {id} ====");
            let fine = format!("  // ==== fine {id} ====");
            let mut pezzi: Vec<String> = Vec::new();
            let mut dentro: Option<Vec<&str>> = None;
            for riga in crate::shared::HTML.lines() {
                let riga = riga.trim_end_matches('\r');
                if riga == inizio {
                    dentro = Some(Vec::new());
                } else if riga == fine {
                    if let Some(p) = dentro.take() {
                        pezzi.push(p.join("\n"));
                    }
                } else if let Some(p) = dentro.as_mut() {
                    p.push(riga.strip_prefix("  ").unwrap_or(riga));
                }
            }
            let codice = format!(
                "// nome: {nome}\n\
                 // {nome} di ASCII Zoo. La cartella di-serie viene riscritta a ogni avvio: per\n\
                 // cambiare questo animale copia il file nella cartella scene. Con lo stesso nome\n\
                 // ({id}.js) prende il posto di quello di serie; con un nome nuovo (cambia anche\n\
                 // la prima riga) è un animale in più nel menu. Come funziona: README, \"Your own scenes\".\n\n{}\n",
                pezzi.join("\n\n")
            );
            (id, codice)
        })
        .collect()
}

/// scrive (o aggiorna) gli animali di serie nella sottocartella di-serie
fn scrivi_di_serie() {
    let dir = cartella().join("di-serie");
    if std::fs::create_dir_all(&dir).is_err() {
        return;
    }
    for (id, codice) in di_serie() {
        let p = dir.join(format!("{id}.js"));
        if std::fs::read_to_string(&p).ok().as_deref() != Some(codice.as_str()) {
            let _ = std::fs::write(&p, codice);
        }
    }
}

/// apre la cartella delle scene nel file manager (la crea se manca, con due righe di aiuto)
pub fn apri_cartella() {
    let dir = cartella();
    let _ = std::fs::create_dir_all(&dir);
    scrivi_di_serie();
    let leggimi = dir.join("LEGGIMI.txt");
    let testo = "Scene di ASCII Zoo\r\n\r\n\
                 Ogni file .js in questa cartella è uno sfondo in più nel menu \"Animale\".\r\n\
                 Per vederne uno nuovo esci da ASCII Zoo e riaprilo.\r\n\r\n\
                 In di-serie ci sono gli animali di ASCII Zoo come file (quella cartella viene\r\n\
                 riscritta a ogni avvio). Copiane uno qui per cambiarlo: con lo stesso nome\r\n\
                 prende il posto dell'animale di serie, con un nome nuovo è un animale in più.\r\n\r\n\
                 Come si scrive una scena, con un esempio:\r\n\
                 https://github.com/ascii-things/ascii-zoo#your-own-scenes\r\n";
    if std::fs::read_to_string(&leggimi).ok().as_deref() != Some(testo) {
        let _ = std::fs::write(&leggimi, testo);
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

    #[test]
    fn animali_di_serie() {
        let tutti = di_serie();
        assert_eq!(tutti.len(), crate::shared::SPECIE.len());
        for (id, codice) in tutti {
            // la forma dell'animale c'è, e nessuna riga "====" è rimasta dentro
            assert!(codice.contains(&format!("id: '{id}'")), "{id}");
            assert!(!codice.contains("// ===="), "{id}");
            assert_eq!(nome_scena(&codice, id), crate::shared::SPECIE.iter().find(|s| s.0 == id).unwrap().1);
        }
    }
}
