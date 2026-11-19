//! Widget "In riproduzione": titolo, artista, stato e avanzamento di quello che sta
//! suonando. Lo legge il modulo `musica` di ogni sistema (MPRIS su Linux, i controlli
//! multimediali di Windows, AppleScript a Musica e Spotify su macOS) su un thread a
//! parte: le chiamate possono aspettare (un lettore bloccato, la richiesta del permesso
//! "Automazione" su macOS) e il ciclo dei messaggi non deve fermarsi mai.
//!
//! Il giro periodico chiede una lettura ogni 2 secondi, solo con il widget acceso e uno
//! schermo non in pausa; alla pagina va la lettura con l'istante in cui è stata fatta,
//! e la pagina fa avanzare la posizione da sola tra una lettura e l'altra.

use std::sync::mpsc::{sync_channel, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use crate::musica::{Lettore, OGNI};

/// quello che sta suonando
#[derive(Clone, Debug, PartialEq)]
pub struct Brano {
    pub titolo: String,
    pub artista: String,
    /// true: suona (▶); false: in pausa (❚❚)
    pub in_corso: bool,
    /// posizione e durata in millisecondi, se il lettore le dice
    pub posizione: Option<u64>,
    pub durata: Option<u64>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Lettura {
    /// nessun lettore, o nessuno che suona o è in pausa
    Niente,
    Brano(Brano),
    /// macOS: l'utente non ha dato il permesso "Automazione" per Musica o Spotify
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    Permesso,
}

/// l'ultima lettura del thread: numero progressivo, lettura, quando (ms dall'epoch)
#[derive(Default)]
struct Ultima {
    n: u64,
    lettura: Option<(Lettura, u64)>,
}

#[derive(Default)]
pub struct Riproduzione {
    /// per chiedere una lettura al thread; None = thread spento
    richieste: Option<SyncSender<()>>,
    ultima: Arc<Mutex<Ultima>>,
    /// numero dell'ultima lettura mandata alle pagine
    mandata: u64,
}

impl Riproduzione {
    /// Il giro periodico: chiede una lettura nuova se serve e restituisce lo script per
    /// le pagine se ce n'è una da mandare. `acceso`: widget acceso; `visibile`: almeno uno
    /// schermo non in pausa.
    pub fn giro(&mut self, acceso: bool, visibile: bool) -> Option<String> {
        if !acceso {
            // senza chi chiede il thread finisce da solo
            self.richieste = None;
            let ce_ne_era = self.ultima.lock().ok()?.lettura.take().is_some();
            return ce_ne_era.then(|| js(None));
        }
        if visibile {
            let ultima = &self.ultima;
            let tx = self.richieste.get_or_insert_with(|| avvia(ultima.clone()));
            // se il thread sta ancora leggendo, questa richiesta si salta
            let _ = tx.try_send(());
        }
        let u = self.ultima.lock().ok()?;
        if u.n == self.mandata {
            return None;
        }
        self.mandata = u.n;
        Some(js(u.lettura.as_ref()))
    }

    /// lo stato da mandare a una pagina appena (ri)caricata
    pub fn js_attuale(&self) -> String {
        js(self.ultima.lock().ok().as_ref().and_then(|u| u.lettura.as_ref()))
    }
}

fn adesso_ms() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_millis() as u64).unwrap_or(0)
}

/// il thread che legge, una volta per richiesta e al massimo ogni OGNI
fn avvia(ultima: Arc<Mutex<Ultima>>) -> SyncSender<()> {
    let (tx, rx) = sync_channel::<()>(1);
    let _ = std::thread::Builder::new().name("in-riproduzione".into()).spawn(move || {
        let mut lettore = Lettore::new();
        let mut prima: Option<Instant> = None;
        while rx.recv().is_ok() {
            if prima.is_some_and(|p| p.elapsed() < OGNI) {
                continue;
            }
            prima = Some(Instant::now());
            let lettura = lettore.leggi();
            let quando = adesso_ms();
            let Ok(mut u) = ultima.lock() else { break };
            u.n += 1;
            u.lettura = Some((lettura, quando));
        }
    });
    tx
}

/// zooSetting('musica', …): null se non suona niente
fn js(l: Option<&(Lettura, u64)>) -> String {
    let v = match l {
        Some((Lettura::Brano(b), quando)) => {
            let corto = |s: &str| -> String { s.chars().filter(|c| !c.is_control()).take(200).collect() };
            serde_json::json!({
                "titolo": corto(&b.titolo),
                "artista": corto(&b.artista),
                "stato": if b.in_corso { "play" } else { "pausa" },
                "pos": b.posizione,
                "durata": b.durata,
                "quando": quando,
            })
        }
        Some((Lettura::Permesso, _)) => serde_json::json!({ "permesso": true }),
        _ => serde_json::Value::Null,
    };
    format!("window.zooSetting && zooSetting('musica', {v})")
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn script_per_la_pagina() {
        let b = Brano { titolo: "Ninna\nnanna".into(), artista: "KARPISA".into(), in_corso: true, posizione: Some(83_000), durata: None };
        let s = js(Some(&(Lettura::Brano(b), 1000)));
        assert!(s.contains(r#""titolo":"Ninnananna""#) && s.contains(r#""pos":83000"#) && s.contains(r#""durata":null"#));
        assert!(s.contains(r#""stato":"play""#) && s.contains(r#""quando":1000"#));
        assert!(js(None).ends_with("zooSetting('musica', null)"));
        assert!(js(Some(&(Lettura::Permesso, 0))).contains(r#"{"permesso":true}"#));
    }

    #[test]
    fn spento_manda_null_una_volta() {
        let mut r = Riproduzione::default();
        assert_eq!(r.giro(false, true), None);
        r.ultima.lock().unwrap().lettura = Some((Lettura::Niente, 0));
        assert!(r.giro(false, true).unwrap().ends_with("null)"));
        assert_eq!(r.giro(false, true), None);
    }
}
