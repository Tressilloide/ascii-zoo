//! Tutto quello che è uguale su Windows, macOS e Linux: lo sfondo incorporato, le
//! specie, il menu dell'icona e cosa fare quando si sceglie una voce.

use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::Icon;

use crate::config::{
    autostart, data_italiana, leggi_data, pulisci_nome, pulisci_testo, Config, ETICHETTA_MAX, NOME_MAX, NOTA_MAX,
};
use crate::{dialog, tema};
use crate::nerd::{self, Nerd};
use crate::riproduzione::Riproduzione;
use crate::stats::Stats;

pub static HTML: &str = include_str!("../assets/ascii-zoo-wallpaper.html");

/// indirizzo della pagina: wry su Windows trasforma "zoo://" in http://zoo.localhost/
#[cfg(windows)]
pub const URL: &str = "http://zoo.localhost/";
#[cfg(not(windows))]
pub const URL: &str = "zoo://localhost/";

/// le specie, nello stesso ordine della pagina: (id, nome nel menu)
pub const SPECIE: [(&str, &str); 9] = [
    ("farfalle", "Farfalle"),
    ("meduse", "Meduse"),
    ("libellule", "Libellule"),
    ("koi", "Koi"),
    ("rane", "Rane"),
    ("aironi", "Aironi"),
    ("granchi", "Granchi"),
    ("cavalli", "Cavalli"),
    ("anatre", "Anatre"),
];
/// ogni quanto cambia animale "a rotazione"
pub const INTERVALLI: [(u32, &str); 7] = [
    (30, "Ogni 30 secondi"),
    (60, "Ogni minuto"),
    (120, "Ogni 2 minuti"),
    (300, "Ogni 5 minuti"),
    (600, "Ogni 10 minuti"),
    (1800, "Ogni 30 minuti"),
    (3600, "Ogni ora"),
];

/// dove stanno i widget: (valore per la pagina, nome nel menu)
pub const ANGOLI: [(&str, &str); 4] = [
    ("basso-destra", "In basso a destra"),
    ("basso-sinistra", "In basso a sinistra"),
    ("alto-destra", "In alto a destra"),
    ("alto-sinistra", "In alto a sinistra"),
];

/// il tema dello sfondo: (valore in Config, nome nel menu)
pub const TEMI: [(&str, &str); 3] = [("scuro", "Scuro"), ("chiaro", "Chiaro"), ("sistema", "Come il sistema")];

/// aspetto dei widget: (valore per la pagina, nome nel menu)
pub const DIMENSIONI: [(&str, &str); 3] = [("piccoli", "Piccoli"), ("normali", "Normali"), ("grandi", "Grandi")];
pub const CORNICI: [(&str, &str); 3] = [
    ("ascii", "+-- ASCII --+"),
    ("linee", "┌── Linee ──┐"),
    ("arrotondata", "╭── Arrotondata ──╮"),
];

/// ASCII_ZOO_LOG=1: scrive su stderr cosa succede (pause, schermi, clic), per le prove e la CI
pub fn log_attivo() -> bool {
    static ATTIVO: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ATTIVO.get_or_init(|| std::env::var_os("ASCII_ZOO_LOG").is_some_and(|v| !v.is_empty() && v != "0"))
}

/// una riga di log, solo con ASCII_ZOO_LOG=1
macro_rules! zoo_log {
    ($($a:tt)*) => {
        if $crate::shared::log_attivo() {
            eprintln!("ascii-zoo: {}", format_args!($($a)*));
        }
    };
}
pub(crate) use zoo_log;

pub const JS_STOP: &str = "window.asciiZoo && window.asciiZoo.stop()";
pub const JS_START: &str = "window.asciiZoo && window.asciiZoo.start()";

/// risposta della pagina: la richiesta del protocollo "zoo" restituisce sempre lo sfondo
pub fn pagina() -> wry::http::Response<std::borrow::Cow<'static, [u8]>> {
    wry::http::Response::builder()
        .header("Content-Type", "text/html; charset=utf-8")
        .body(std::borrow::Cow::Borrowed(HTML.as_bytes()))
        .unwrap()
}

pub fn js_in_carica(ac: bool) -> String {
    format!("window.zooSetting && zooSetting('inCarica', {ac})")
}

/// valori di CPU, RAM e batteria per il widget "pc" e delle statistiche per nerd
pub fn js_statistiche(v: &serde_json::Value) -> String {
    format!("window.zooSetting && zooSetting('statistiche', {v})")
}

/// uno schermo collegato: id stabile (nome del dispositivo) e dimensioni
#[derive(Clone, PartialEq, Debug)]
pub struct Schermo {
    pub id: String,
    pub larghezza: u32,
    pub altezza: u32,
}

/// valori speciali di "Widget > Schermo"
pub const PRINCIPALE: &str = "";
pub const TUTTI: &str = "*";

/// "Schermo 1, principale (2560×1600)"
fn nome_schermo(n: usize, s: &Schermo) -> String {
    let principale = if n == 0 { ", principale" } else { "" };
    format!("Schermo {}{principale} ({}×{})", n + 1, s.larghezza, s.altezza)
}

pub struct Menus {
    pub animali: Vec<CheckMenuItem>,
    pub rot_tempi: Vec<CheckMenuItem>,
    pub rot_specie: Vec<CheckMenuItem>,
    pub nomi: Vec<MenuItem>,
    pub nota: MenuItem,
    pub conto: MenuItem,
    pub statistiche: CheckMenuItem,
    /// "Statistiche per nerd": (id della voce, voce)
    pub nerd: Vec<(&'static str, CheckMenuItem)>,
    pub musica: CheckMenuItem,
    /// "Aspetto": dimensione, cornice, sfondo pieno
    pub dimensioni: Vec<CheckMenuItem>,
    pub cornici: Vec<CheckMenuItem>,
    pub sfondo_pieno: CheckMenuItem,
    pub angoli: Vec<CheckMenuItem>,
    pub temi: Vec<CheckMenuItem>,
    pub widget_sub: Submenu,
    /// "Widget > Schermo": (valore, voce); i primi due sono principale e tutti
    pub widget_schermi: Vec<(String, CheckMenuItem)>,
    widget_schermi_fissi: Vec<(String, CheckMenuItem)>,
    /// "Animale per schermo": un sottomenu per schermo
    pub animali_sub: Submenu,
    pub animali_schermi: Vec<(String, Submenu, Vec<CheckMenuItem>)>,
    animali_un_solo: MenuItem,
    pub clic: CheckMenuItem,
    pub orologio: CheckMenuItem,
    pub koi: CheckMenuItem,
    pub batteria: CheckMenuItem,
    pub schermo: CheckMenuItem,
    pub coperto: CheckMenuItem,
    pub avvio: CheckMenuItem,
    /// "Usa come screensaver…" (solo Windows)
    pub salvaschermo: MenuItem,
    pub foto: MenuItem,
    pub ricarica: MenuItem,
    pub esci: MenuItem,
}

/// cosa deve fare il programma dopo una voce del menu
pub enum Esito {
    /// impostazione cambiata (e salvata): manda lo script alle pagine, se c'è, e ricontrolla le pause
    Cambiato(Option<String>),
    /// cambiato qualcosa che dipende dallo schermo: a ogni pagina va mandato js_schermo
    Schermi,
    Ricarica,
    /// installare lo screensaver e aprire le sue impostazioni di Windows
    Salvaschermo,
    /// salvare una foto di ogni schermo (vedi `foto::scatta`)
    Foto,
    Esci,
    Niente,
}

/// impostazioni + menu: la logica del menu, senza finestre
pub struct Core {
    pub cfg: Config,
    pub menus: Menus,
    /// gli schermi collegati, il principale per primo
    pub schermi: Vec<Schermo>,
    /// il tema mandato alle pagine: "scuro", "chiaro" o "auto" (sistema non riconosciuto:
    /// decide la pagina con prefers-color-scheme)
    tema_pagina: &'static str,
    /// le statistiche per nerd (sysinfo), lette solo quando servono
    nerd: Nerd,
    /// widget "In riproduzione": il thread che chiede ai lettori cosa suona
    musica: Riproduzione,
}

impl Core {
    /// `solo_windows`: se mostrare le voci che esistono solo su Windows (lo screensaver)
    pub fn new(solo_windows: bool) -> (Self, Menu) {
        let cfg = Config::load();
        let (menu, menus) = build_menu(&cfg, solo_windows);
        let mut core = Self { cfg, menus, schermi: Vec::new(), tema_pagina: "scuro", nerd: Nerd::default(), musica: Riproduzione::default() };
        core.tema_pagina = core.tema_ora();
        zoo_log!("tema: {} (alla pagina: {})", core.cfg.tema, core.tema_pagina);
        (core, menu)
    }

    /// Il tema da mandare alle pagine adesso. Con "Come il sistema" lo si chiede al sistema
    /// (registro di Windows, aspetto di macOS, portale o impostazioni di GNOME su Linux).
    fn tema_ora(&self) -> &'static str {
        match self.cfg.tema.as_str() {
            "chiaro" => "chiaro",
            "sistema" => match tema::scuro() {
                Some(true) => "scuro",
                Some(false) => "chiaro",
                None => "auto",
            },
            _ => "scuro",
        }
    }

    pub fn js_tema(&self) -> String {
        format!("window.zooSetting && zooSetting('tema', '{}')", self.tema_pagina)
    }

    /// Con "Come il sistema", da chiamare ogni pochi secondi: lo script per le pagine se il
    /// tema del sistema è cambiato dall'ultima volta, se no None.
    pub fn tema_cambiato(&mut self) -> Option<String> {
        if self.cfg.tema != "sistema" {
            return None;
        }
        let ora = self.tema_ora();
        if ora == self.tema_pagina {
            return None;
        }
        zoo_log!("tema del sistema: {ora}");
        self.tema_pagina = ora;
        Some(self.js_tema())
    }

    /// i nomi come oggetto JSON: serde_json mette gli escape, quindi apostrofi e
    /// virgolette nei nomi non rompono lo script
    pub fn js_nomi(&self) -> String {
        let nomi = serde_json::to_string(&self.cfg.nomi).unwrap_or_else(|_| "{}".into());
        format!("window.zooSetting && zooSetting('nomi', {nomi})")
    }

    pub fn js_orologio(&self) -> String {
        format!("window.zooSetting && zooSetting('orologio', {})", self.cfg.orologio)
    }

    pub fn js_rotazione(&self) -> String {
        let specie: Vec<&str> = SPECIE
            .iter()
            .map(|s| s.0)
            .filter(|id| !self.cfg.rotazione_escluse.iter().any(|e| e == id))
            .collect();
        let rot = serde_json::json!({ "secondi": self.cfg.rotazione_secondi, "specie": specie });
        format!("window.zooSetting && zooSetting('rotazione', {rot})")
    }

    /// i widget si vedono su questo schermo?
    fn widget_qui(&self, schermo: &str) -> bool {
        let scelta = self.cfg.widget_schermo.as_str();
        if scelta == TUTTI {
            return true;
        }
        // schermo scelto non collegato (o "principale"): vale il principale
        let scelta_c = self.schermi.iter().any(|s| s.id == scelta);
        if scelta_c {
            scelta == schermo
        } else {
            self.schermi.first().is_none_or(|s| s.id == schermo)
        }
    }

    /// i widget per uno schermo (spenti dove non devono vedersi)
    pub fn js_widget(&self, schermo: &str) -> String {
        let c = &self.cfg;
        let qui = self.widget_qui(schermo);
        let conto = (qui && !c.conto_data.is_empty())
            .then(|| serde_json::json!({ "data": c.conto_data, "etichetta": c.conto_etichetta }));
        let nerd: Vec<&str> =
            if qui { c.widget_nerd.iter().map(String::as_str).filter(|id| nerd::disponibile(id)).collect() } else { Vec::new() };
        let w = serde_json::json!({
            "qui": qui,
            "nota": if qui { c.nota.as_str() } else { "" },
            "conto": conto,
            "statistiche": qui && c.widget_statistiche,
            "nerd": nerd,
            "musica": qui && c.widget_musica,
            "angolo": c.widget_angolo,
            "dimensione": c.widget_dimensione,
            "cornice": c.widget_cornice,
            "sfondo": if c.widget_sfondo_pieno { "pieno" } else { "trasparente" },
        });
        format!("window.zooSetting && zooSetting('widget', {w})")
    }

    /// Il giro periodico dei widget (ogni ~2 secondi, dal controllo delle pause di ogni
    /// piattaforma): restituisce gli script da mandare a tutte le pagine. `visibile`: c'è
    /// almeno uno schermo non in pausa; se no non si legge niente.
    pub fn giro(&mut self, stats: &mut Stats, visibile: bool) -> Vec<String> {
        let mut js = Vec::new();
        let c = &self.cfg;
        let nerd_su = c.widget_nerd.iter().any(|id| nerd::disponibile(id));
        if visibile && (c.widget_statistiche || nerd_su) {
            let mut v = serde_json::Map::new();
            if c.widget_statistiche {
                if let serde_json::Value::Object(m) = stats.leggi() {
                    v = m;
                }
            }
            self.nerd.leggi(&c.widget_nerd, &mut v);
            js.push(js_statistiche(&v.into()));
        } else if !nerd_su {
            // spente: sysinfo libera quello che aveva
            self.nerd.leggi(&[], &mut serde_json::Map::new());
        }
        js.extend(self.musica.giro(self.cfg.widget_musica, visibile));
        js
    }

    /// l'animale di uno schermo: il suo, se scelto, se no quello di "Animale"
    pub fn animale_di(&self, schermo: &str) -> u32 {
        self.cfg.animale_schermi.get(schermo).copied().unwrap_or(self.cfg.animale)
    }

    /// le impostazioni che cambiano da schermo a schermo
    pub fn js_schermo(&self, schermo: &str) -> [String; 2] {
        [
            self.js_widget(schermo),
            format!("window.zooSetting && zooSetting('animale', {})", self.animale_di(schermo)),
        ]
    }

    /// tutte le impostazioni di uno schermo, da mandare quando la sua pagina ha finito di caricare
    pub fn js_tutto(&self, ac: bool, schermo: &str) -> Vec<String> {
        let mut v = vec![
            self.js_tema(),
            self.js_nomi(),
            self.js_rotazione(),
            self.js_orologio(),
            format!(
                "if (window.zooSetting) {{ zooSetting('inCarica', {ac}); zooSetting('koiInCarica', {}); }}",
                self.cfg.koi_in_carica
            ),
        ];
        v.extend(self.js_schermo(schermo));
        // quello che vive nel programma e non nella pagina: torna dopo "Ricarica sfondo"
        v.push(self.musica.js_attuale());
        v
    }

    /// Gli schermi collegati, il principale per primo. Rifà le voci del menu che
    /// dipendono dagli schermi (solo se sono cambiati).
    pub fn set_schermi(&mut self, schermi: Vec<Schermo>) {
        if schermi == self.schermi {
            return;
        }
        let m = &mut self.menus;
        for (id, item) in m.widget_schermi.drain(..) {
            if id == PRINCIPALE || id == TUTTI {
                m.widget_schermi_fissi.push((id, item));
            } else {
                let _ = m.widget_sub.remove(&item);
            }
        }
        m.widget_schermi.append(&mut m.widget_schermi_fissi);
        for (_, sub, _) in m.animali_schermi.drain(..) {
            let _ = m.animali_sub.remove(&sub);
        }
        let _ = m.animali_sub.remove(&m.animali_un_solo);

        for (n, s) in schermi.iter().enumerate() {
            let nome = nome_schermo(n, s);
            let item = CheckMenuItem::new(&nome, true, false, None);
            let _ = m.widget_sub.append(&item);
            m.widget_schermi.push((s.id.clone(), item));

            let sub = Submenu::new(&nome, true);
            let attuale = self.cfg.animale_schermi.get(&s.id).map(|a| *a as usize + 1).unwrap_or(0);
            let voci: Vec<CheckMenuItem> = std::iter::once("Come in \"Animale\"")
                .chain(std::iter::once("A rotazione"))
                .chain(SPECIE.iter().map(|s| s.1))
                .enumerate()
                .map(|(k, name)| CheckMenuItem::new(name, true, k == attuale, None))
                .collect();
            for (k, v) in voci.iter().enumerate() {
                let _ = sub.append(v);
                if k == 0 {
                    let _ = sub.append(&PredefinedMenuItem::separator());
                }
            }
            let _ = m.animali_sub.append(&sub);
            m.animali_schermi.push((s.id.clone(), sub, voci));
        }
        if schermi.len() < 2 {
            let _ = m.animali_sub.append(&m.animali_un_solo);
        }
        self.schermi = schermi;
        self.segna_widget_schermo();
    }

    /// mette la spunta sulla voce giusta di "Widget > Schermo"
    fn segna_widget_schermo(&self) {
        let scelta = &self.cfg.widget_schermo;
        let esiste = self.schermi.iter().any(|s| &s.id == scelta) || scelta == TUTTI;
        for (id, item) in &self.menus.widget_schermi {
            item.set_checked(if esiste { id == scelta } else { id == PRINCIPALE });
        }
    }

    pub fn on_menu(&mut self, id: &str) -> Esito {
        let m = &self.menus;
        if let Some(i) = m.animali.iter().position(|x| x.id().0 == id) {
            self.cfg.animale = i as u32;
            for (k, item) in self.menus.animali.iter().enumerate() {
                item.set_checked(k == i);
            }
            self.cfg.save();
            return Esito::Schermi;
        }
        if let Some(i) = m.widget_schermi.iter().position(|x| x.1.id().0 == id) {
            self.cfg.widget_schermo = m.widget_schermi[i].0.clone();
            self.segna_widget_schermo();
            self.cfg.save();
            return Esito::Schermi;
        }
        if let Some((sid, k, voci)) = m.animali_schermi.iter().find_map(|(sid, _, voci)| {
            voci.iter().position(|x| x.id().0 == id).map(|k| (sid.clone(), k, voci))
        }) {
            for (j, v) in voci.iter().enumerate() {
                v.set_checked(j == k);
            }
            if k == 0 {
                self.cfg.animale_schermi.remove(&sid);
            } else {
                self.cfg.animale_schermi.insert(sid, k as u32 - 1);
            }
            self.cfg.save();
            return Esito::Schermi;
        }
        // i widget cambiano da schermo a schermo: si rimandano a ognuno i suoi
        let mut widget = false;
        let js = if let Some(i) = m.rot_tempi.iter().position(|x| x.id().0 == id) {
            self.cfg.rotazione_secondi = INTERVALLI[i].0;
            for (k, item) in self.menus.rot_tempi.iter().enumerate() {
                item.set_checked(k == i);
            }
            Some(self.js_rotazione())
        } else if let Some(i) = m.rot_specie.iter().position(|x| x.id().0 == id) {
            // almeno un animale deve restare in rotazione
            if !m.rot_specie.iter().any(|x| x.is_checked()) {
                m.rot_specie[i].set_checked(true);
            }
            self.cfg.rotazione_escluse = SPECIE
                .iter()
                .zip(&self.menus.rot_specie)
                .filter(|(_, item)| !item.is_checked())
                .map(|(s, _)| s.0.to_string())
                .collect();
            Some(self.js_rotazione())
        } else if let Some(i) = m.nomi.iter().position(|x| x.id().0 == id) {
            let (sid, label) = SPECIE[i];
            let attuale = self.cfg.nomi.get(sid).cloned().unwrap_or_default();
            let domanda = format!("Nome per {} (vuoto = nessun nome):", label.to_lowercase());
            let Some(nuovo) = dialog::chiedi_nome("ASCII Zoo", &domanda, &attuale, NOME_MAX) else {
                return Esito::Niente; // annullato: non cambia niente
            };
            let nuovo = pulisci_nome(&nuovo);
            self.menus.nomi[i].set_text(etichetta_nome(label, &nuovo));
            self.cfg.nomi.insert(sid.to_string(), nuovo);
            Some(self.js_nomi())
        } else if id == m.nota.id().0 {
            let Some(nuova) = dialog::chiedi_nome(
                "ASCII Zoo",
                "Nota da mostrare sul desktop (vuoto = nessuna nota):",
                &self.cfg.nota,
                NOTA_MAX,
            ) else {
                return Esito::Niente;
            };
            self.cfg.nota = pulisci_testo(&nuova, NOTA_MAX);
            self.menus.nota.set_text(etichetta_nota(&self.cfg.nota));
            {
                widget = true;
                None
            }
        } else if id == m.conto.id().0 {
            if !self.chiedi_conto() {
                return Esito::Niente;
            }
            self.menus.conto.set_text(etichetta_conto(&self.cfg));
            {
                widget = true;
                None
            }
        } else if id == m.statistiche.id().0 {
            self.cfg.widget_statistiche = m.statistiche.is_checked();
            {
                widget = true;
                None
            }
        } else if id == m.musica.id().0 {
            self.cfg.widget_musica = m.musica.is_checked();
            {
                widget = true;
                None
            }
        } else if m.nerd.iter().any(|(_, item)| item.id().0 == id) {
            self.cfg.widget_nerd =
                m.nerd.iter().filter(|(_, item)| item.is_checked()).map(|(nid, _)| nid.to_string()).collect();
            {
                widget = true;
                None
            }
        } else if let Some(i) = m.dimensioni.iter().position(|x| x.id().0 == id) {
            self.cfg.widget_dimensione = DIMENSIONI[i].0.to_string();
            for (k, item) in self.menus.dimensioni.iter().enumerate() {
                item.set_checked(k == i);
            }
            {
                widget = true;
                None
            }
        } else if let Some(i) = m.cornici.iter().position(|x| x.id().0 == id) {
            self.cfg.widget_cornice = CORNICI[i].0.to_string();
            for (k, item) in self.menus.cornici.iter().enumerate() {
                item.set_checked(k == i);
            }
            {
                widget = true;
                None
            }
        } else if id == m.sfondo_pieno.id().0 {
            self.cfg.widget_sfondo_pieno = m.sfondo_pieno.is_checked();
            {
                widget = true;
                None
            }
        } else if let Some(i) = m.angoli.iter().position(|x| x.id().0 == id) {
            self.cfg.widget_angolo = ANGOLI[i].0.to_string();
            for (k, item) in self.menus.angoli.iter().enumerate() {
                item.set_checked(k == i);
            }
            {
                widget = true;
                None
            }
        } else if let Some(i) = m.temi.iter().position(|x| x.id().0 == id) {
            self.cfg.tema = TEMI[i].0.to_string();
            for (k, item) in self.menus.temi.iter().enumerate() {
                item.set_checked(k == i);
            }
            self.tema_pagina = self.tema_ora();
            Some(self.js_tema())
        } else if id == m.clic.id().0 {
            self.cfg.clic_desktop = m.clic.is_checked();
            None
        } else if id == m.orologio.id().0 {
            self.cfg.orologio = m.orologio.is_checked();
            Some(self.js_orologio())
        } else if id == m.koi.id().0 {
            self.cfg.koi_in_carica = m.koi.is_checked();
            Some(format!("window.zooSetting && zooSetting('koiInCarica', {})", self.cfg.koi_in_carica))
        } else if id == m.batteria.id().0 {
            self.cfg.pausa_batteria = m.batteria.is_checked();
            None
        } else if id == m.schermo.id().0 {
            self.cfg.pausa_schermo_intero = m.schermo.is_checked();
            None
        } else if id == m.coperto.id().0 {
            self.cfg.pausa_coperto = m.coperto.is_checked();
            None
        } else if id == m.avvio.id().0 {
            autostart::set(m.avvio.is_checked());
            m.avvio.set_checked(autostart::enabled());
            return Esito::Niente;
        } else if id == m.salvaschermo.id().0 {
            return Esito::Salvaschermo;
        } else if id == m.foto.id().0 {
            return Esito::Foto;
        } else if id == m.ricarica.id().0 {
            return Esito::Ricarica;
        } else if id == m.esci.id().0 {
            return Esito::Esci;
        } else {
            return Esito::Niente;
        };
        self.cfg.save();
        if widget {
            return Esito::Schermi;
        }
        Esito::Cambiato(js)
    }
}

impl Core {
    /// chiede etichetta e data del conto alla rovescia; false se si annulla
    fn chiedi_conto(&mut self) -> bool {
        let attuale = if self.cfg.conto_etichetta.is_empty() { "esame" } else { &self.cfg.conto_etichetta };
        let Some(etichetta) = dialog::chiedi_nome("ASCII Zoo", "Conto alla rovescia: per cosa?", attuale, ETICHETTA_MAX)
        else {
            return false;
        };
        let mut domanda = "Che giorno? (es. 25/12/2026; vuoto = spegni il conto alla rovescia)";
        let mut data = data_italiana(&self.cfg.conto_data);
        loop {
            let Some(scritta) = dialog::chiedi_nome("ASCII Zoo", domanda, &data, 10) else {
                return false;
            };
            if scritta.trim().is_empty() {
                self.cfg.conto_data.clear();
                break;
            }
            if let Some(iso) = leggi_data(&scritta) {
                self.cfg.conto_data = iso;
                break;
            }
            domanda = "Data non valida. Scrivila come giorno/mese/anno, es. 25/12/2026:";
            data = scritta;
        }
        self.cfg.conto_etichetta = pulisci_testo(&etichetta, ETICHETTA_MAX);
        true
    }
}

/// testo scelto dall'utente dentro una voce di menu
fn nel_menu(s: &str) -> String {
    // nei menu di Windows "&" sottolinea la lettera dopo: va raddoppiata
    if cfg!(windows) {
        s.replace('&', "&&")
    } else {
        s.to_string()
    }
}

fn etichetta_nota(nota: &str) -> String {
    if nota.is_empty() {
        return "Nota: (nessuna)…".into();
    }
    let corta: String = nota.chars().take(24).collect();
    let altro = if nota.chars().count() > 24 { "…" } else { "" };
    format!("Nota: {}{altro}…", nel_menu(&corta))
}

fn etichetta_conto(cfg: &Config) -> String {
    if cfg.conto_data.is_empty() {
        return "Conto alla rovescia: (spento)…".into();
    }
    format!("Conto alla rovescia: {} {}…", nel_menu(&cfg.conto_etichetta), data_italiana(&cfg.conto_data))
}

/// icona dell'area di notifica / barra dei menu: una piccola koi arancione e bianca
pub fn tray_icon_image() -> Icon {
    const N: u32 = 32;
    let mut px = vec![0u8; (N * N * 4) as usize];
    for y in 0..N {
        for x in 0..N {
            let (fx, fy) = (x as f32 - 15.5, y as f32 - 15.5);
            // corpo ellittico + coda a sinistra
            let body = (fx / 12.0).powi(2) + (fy / 7.5).powi(2) <= 1.0 && fx > -9.0;
            let tail = fx <= -8.0 && fx >= -15.0 && fy.abs() <= (-8.0 - fx) * 0.9 + 1.0;
            if !(body || tail) {
                continue;
            }
            let spot = ((fx - 2.0) / 5.0).powi(2) + ((fy + 1.5) / 3.5).powi(2) <= 1.0;
            let eye = (fx - 8.0).powi(2) + (fy + 2.0).powi(2) <= 1.6;
            let (r, g, b) = if eye { (20, 20, 30) } else if spot { (246, 242, 233) } else { (255, 143, 46) };
            let i = ((y * N + x) * 4) as usize;
            px[i..i + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    Icon::from_rgba(px, N, N).expect("icona")
}

/// voce del menu dei nomi: "Koi: KARPISA…" oppure "Farfalle: (nessuno)…"
pub fn etichetta_nome(label: &str, nome: &str) -> String {
    let nome = if nome.is_empty() { "(nessuno)".to_string() } else { nel_menu(nome) };
    format!("{label}: {nome}…")
}

fn build_menu(cfg: &Config, solo_windows: bool) -> (Menu, Menus) {
    let menu = Menu::new();

    let sub = Submenu::new("Animale", true);
    let animali: Vec<CheckMenuItem> = std::iter::once("A rotazione")
        .chain(SPECIE.iter().map(|s| s.1))
        .enumerate()
        .map(|(i, name)| CheckMenuItem::new(name, true, i as u32 == cfg.animale, None))
        .collect();
    for item in &animali {
        let _ = sub.append(item);
    }

    let rot = Submenu::new("Rotazione", true);
    let rot_tempi: Vec<CheckMenuItem> = INTERVALLI
        .iter()
        .map(|(s, name)| CheckMenuItem::new(*name, true, *s == cfg.rotazione_secondi, None))
        .collect();
    for item in &rot_tempi {
        let _ = rot.append(item);
    }
    let _ = rot.append(&PredefinedMenuItem::separator());
    let _ = rot.append(&MenuItem::new("Animali in rotazione:", false, None));
    let rot_specie: Vec<CheckMenuItem> = SPECIE
        .iter()
        .map(|(id, name)| CheckMenuItem::new(*name, true, !cfg.rotazione_escluse.iter().any(|e| e == id), None))
        .collect();
    for item in &rot_specie {
        let _ = rot.append(item);
    }

    let nomi_sub = Submenu::new("Nomi degli animali", true);
    let nomi: Vec<MenuItem> = SPECIE
        .iter()
        .map(|(id, name)| MenuItem::new(etichetta_nome(name, cfg.nomi.get(*id).map(String::as_str).unwrap_or("")), true, None))
        .collect();
    for item in &nomi {
        let _ = nomi_sub.append(item);
    }

    let widget = Submenu::new("Widget", true);
    let nota = MenuItem::new(etichetta_nota(&cfg.nota), true, None);
    let conto = MenuItem::new(etichetta_conto(cfg), true, None);
    let statistiche = CheckMenuItem::new("CPU, RAM e batteria", true, cfg.widget_statistiche, None);
    let nerd_sub = Submenu::new("Statistiche per nerd", true);
    let nerd: Vec<(&'static str, CheckMenuItem)> = nerd::VOCI
        .iter()
        .filter(|(id, _)| nerd::disponibile(id))
        .map(|(id, name)| (*id, CheckMenuItem::new(*name, true, cfg.widget_nerd.iter().any(|x| x == id), None)))
        .collect();
    for (_, item) in &nerd {
        let _ = nerd_sub.append(item);
    }
    let aspetto = Submenu::new("Aspetto", true);
    let _ = aspetto.append(&MenuItem::new("Dimensione:", false, None));
    let dimensioni: Vec<CheckMenuItem> = DIMENSIONI
        .iter()
        .map(|(id, name)| CheckMenuItem::new(*name, true, cfg.widget_dimensione == *id, None))
        .collect();
    for item in &dimensioni {
        let _ = aspetto.append(item);
    }
    let _ = aspetto.append(&PredefinedMenuItem::separator());
    let _ = aspetto.append(&MenuItem::new("Cornice:", false, None));
    let cornici: Vec<CheckMenuItem> = CORNICI
        .iter()
        .map(|(id, name)| CheckMenuItem::new(*name, true, cfg.widget_cornice == *id, None))
        .collect();
    for item in &cornici {
        let _ = aspetto.append(item);
    }
    let _ = aspetto.append(&PredefinedMenuItem::separator());
    let sfondo_pieno = CheckMenuItem::new("Sfondo pieno dietro i riquadri", true, cfg.widget_sfondo_pieno, None);
    let _ = aspetto.append(&sfondo_pieno);
    let angoli: Vec<CheckMenuItem> = ANGOLI
        .iter()
        .map(|(id, name)| CheckMenuItem::new(*name, true, cfg.widget_angolo == *id, None))
        .collect();
    let _ = widget.append(&nota);
    let _ = widget.append(&conto);
    let _ = widget.append(&statistiche);
    let _ = widget.append(&nerd_sub);
    let musica = CheckMenuItem::new("In riproduzione", true, cfg.widget_musica, None);
    let _ = widget.append(&musica);
    let _ = widget.append(&PredefinedMenuItem::separator());
    let _ = widget.append(&aspetto);
    let _ = widget.append(&PredefinedMenuItem::separator());
    let _ = widget.append(&MenuItem::new("Posizione:", false, None));
    for item in &angoli {
        let _ = widget.append(item);
    }
    let _ = widget.append(&PredefinedMenuItem::separator());
    let _ = widget.append(&MenuItem::new("Schermo:", false, None));
    let widget_schermi = vec![
        (PRINCIPALE.to_string(), CheckMenuItem::new("Schermo principale", true, true, None)),
        (TUTTI.to_string(), CheckMenuItem::new("Tutti gli schermi", true, false, None)),
    ];
    for (_, item) in &widget_schermi {
        let _ = widget.append(item);
    }
    // gli schermi veri si aggiungono con set_schermi
    let tema_sub = Submenu::new("Tema", true);
    // un valore sconosciuto nella config vale come "scuro"
    let tema_cfg = if TEMI.iter().any(|t| t.0 == cfg.tema) { cfg.tema.as_str() } else { "scuro" };
    let temi: Vec<CheckMenuItem> =
        TEMI.iter().map(|(id, name)| CheckMenuItem::new(*name, true, tema_cfg == *id, None)).collect();
    for item in &temi {
        let _ = tema_sub.append(item);
    }
    let animali_sub = Submenu::new("Animale per schermo", true);
    let animali_un_solo = MenuItem::new("(c'è un solo schermo)", false, None);
    let _ = animali_sub.append(&animali_un_solo);

    let avvio_label = if cfg!(windows) { "Avvia con Windows" } else { "Apri al login" };
    let m = Menus {
        animali,
        rot_tempi,
        rot_specie,
        nomi,
        nota,
        conto,
        statistiche,
        nerd,
        musica,
        dimensioni,
        cornici,
        sfondo_pieno,
        angoli,
        temi,
        widget_sub: widget,
        widget_schermi,
        widget_schermi_fissi: Vec::new(),
        animali_sub,
        animali_schermi: Vec::new(),
        animali_un_solo,
        clic: CheckMenuItem::new("Clic sul desktop agli animali", true, cfg.clic_desktop, None),
        orologio: CheckMenuItem::new("Orologio al centro", true, cfg.orologio, None),
        koi: CheckMenuItem::new("Koi nello stagno in carica (con \"A rotazione\")", true, cfg.koi_in_carica, None),
        batteria: CheckMenuItem::new("Pausa a batteria", true, cfg.pausa_batteria, None),
        schermo: CheckMenuItem::new("Pausa con app a schermo intero", true, cfg.pausa_schermo_intero, None),
        coperto: CheckMenuItem::new("Pausa quando le finestre coprono il desktop", true, cfg.pausa_coperto, None),
        avvio: CheckMenuItem::new(avvio_label, true, autostart::enabled(), None),
        salvaschermo: MenuItem::new("Usa come screensaver…", true, None),
        foto: MenuItem::new("Salva una foto dello sfondo", true, None),
        ricarica: MenuItem::new("Ricarica sfondo", true, None),
        esci: MenuItem::new("Esci", true, None),
    };
    let _ = menu.append(&sub);
    let _ = menu.append(&m.animali_sub);
    let _ = menu.append(&rot);
    let _ = menu.append(&nomi_sub);
    let _ = menu.append(&m.widget_sub);
    let _ = menu.append(&tema_sub);
    let _ = menu.append(&m.orologio);
    let _ = menu.append(&m.koi);
    let _ = menu.append(&m.clic);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&m.batteria);
    let _ = menu.append(&m.schermo);
    let _ = menu.append(&m.coperto);
    let _ = menu.append(&m.avvio);
    if solo_windows {
        // lo screensaver per ora c'è solo su Windows
        let _ = menu.append(&m.salvaschermo);
    }
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&m.foto);
    let _ = menu.append(&m.ricarica);
    let _ = menu.append(&m.esci);
    (menu, m)
}
