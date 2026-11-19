//! "In riproduzione" su Linux: i lettori che parlano MPRIS sul bus di sessione
//! (org.mpris.MediaPlayer2.*: Spotify, Rhythmbox, VLC, Firefox, Chromium…).

use std::time::{Duration, Instant};

use gtk::glib::ToVariant;
use gtk::{gio, glib};

use crate::riproduzione::{Brano, Lettura};

/// ogni quanto al massimo si chiede ai lettori (una chiamata D-Bus ciascuno: costa poco)
pub const OGNI: Duration = Duration::from_millis(1500);

/// un lettore bloccato non deve fermare tutto: il tempo massimo di una chiamata
const ATTESA_MS: i32 = 300;

pub struct Lettore {
    bus: Option<gio::DBusConnection>,
    /// Spotify su Linux dice sempre posizione 0: da quando un brano in corso è fermo a 0
    zero: Option<(String, Instant)>,
}

impl Lettore {
    pub fn new() -> Self {
        // la connessione di sessione è la stessa del programma e si può usare da ogni thread
        Self { bus: gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok(), zero: None }
    }

    /// il lettore che sta suonando; se nessuno suona, il primo in pausa
    pub fn leggi(&mut self) -> Lettura {
        let Some(bus) = &self.bus else { return Lettura::Niente };
        let mut in_pausa = None;
        for nome in lettori(bus) {
            let Some(b) = brano(bus, &nome) else { continue };
            if b.in_corso {
                return Lettura::Brano(self.posizione_vera(b));
            }
            in_pausa.get_or_insert(b);
        }
        self.zero = None;
        in_pausa.map_or(Lettura::Niente, Lettura::Brano)
    }

    /// Una posizione che resta a 0 mentre il brano suona non è vera (Spotify): meglio
    /// non mostrarla che vedere la barra tornare indietro a ogni lettura.
    fn posizione_vera(&mut self, mut b: Brano) -> Brano {
        if b.posizione != Some(0) {
            self.zero = None;
            return b;
        }
        match &self.zero {
            Some((titolo, da)) if *titolo == b.titolo => {
                if da.elapsed() > Duration::from_millis(2500) {
                    b.posizione = None;
                }
            }
            _ => self.zero = Some((b.titolo.clone(), Instant::now())),
        }
        b
    }
}

/// i nomi dei lettori MPRIS sul bus
fn lettori(bus: &gio::DBusConnection) -> Vec<String> {
    let risposta = bus.call_sync(
        Some("org.freedesktop.DBus"),
        "/org/freedesktop/DBus",
        "org.freedesktop.DBus",
        "ListNames",
        None,
        glib::VariantTy::new("(as)").ok(),
        gio::DBusCallFlags::NONE,
        ATTESA_MS,
        gio::Cancellable::NONE,
    );
    let Some((nomi,)) = risposta.ok().and_then(|v| v.get::<(Vec<String>,)>()) else { return Vec::new() };
    let mut nomi: Vec<String> = nomi.into_iter().filter(|n| n.starts_with("org.mpris.MediaPlayer2.")).collect();
    nomi.sort(); // sempre lo stesso ordine: con due lettori in pausa non si salta dall'uno all'altro
    nomi
}

/// stato, titolo, artista, posizione e durata di un lettore (None se è fermo o non risponde)
fn brano(bus: &gio::DBusConnection, nome: &str) -> Option<Brano> {
    let risposta = bus
        .call_sync(
            Some(nome),
            "/org/mpris/MediaPlayer2",
            "org.freedesktop.DBus.Properties",
            "GetAll",
            Some(&("org.mpris.MediaPlayer2.Player",).to_variant()),
            glib::VariantTy::new("(a{sv})").ok(),
            gio::DBusCallFlags::NO_AUTO_START,
            ATTESA_MS,
            gio::Cancellable::NONE,
        )
        .ok()?;
    let p = glib::VariantDict::new(Some(&risposta.child_value(0)));
    let in_corso = match p.lookup::<String>("PlaybackStatus").ok()??.as_str() {
        "Playing" => true,
        "Paused" => false,
        _ => return None, // "Stopped"
    };
    let meta = glib::VariantDict::new(p.lookup_value("Metadata", glib::VariantTy::new("a{sv}").ok()).as_ref());
    let titolo = meta.lookup_value("xesam:title", None).and_then(|v| v.str().map(str::to_string)).unwrap_or_default();
    // xesam:artist è una lista (as), ma qualche lettore manda una stringa sola
    let artista = meta
        .lookup_value("xesam:artist", None)
        .and_then(|v| v.get::<Vec<String>>().map(|a| a.join(", ")).or_else(|| v.str().map(str::to_string)))
        .unwrap_or_default();
    if titolo.trim().is_empty() && artista.trim().is_empty() {
        return None;
    }
    // mpris:length e Position sono in microsecondi
    let durata = meta.lookup_value("mpris:length", None).as_ref().and_then(numero).filter(|&d| d > 0);
    let posizione = p.lookup_value("Position", None).as_ref().and_then(numero);
    Some(Brano {
        titolo,
        artista,
        in_corso,
        posizione: posizione.map(|us| us / 1000),
        durata: durata.map(|us| us / 1000),
    })
}

/// un numero intero non negativo, qualunque tipo abbia scelto il lettore (x, t, i, u, d)
fn numero(v: &glib::Variant) -> Option<u64> {
    let n = v
        .get::<i64>()
        .or_else(|| v.get::<u64>().map(|x| x.min(i64::MAX as u64) as i64))
        .or_else(|| v.get::<i32>().map(i64::from))
        .or_else(|| v.get::<u32>().map(i64::from))
        .or_else(|| v.get::<f64>().filter(|x| x.is_finite()).map(|x| x as i64))?;
    u64::try_from(n).ok()
}
