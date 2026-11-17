//! Tutto quello che è uguale su Windows e su macOS: lo sfondo incorporato, le
//! specie, il menu dell'icona e cosa fare quando si sceglie una voce.

use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::Icon;

use crate::config::{autostart, pulisci_nome, Config, NOME_MAX};
use crate::dialog;

pub static HTML: &str = include_str!("../assets/ascii-zoo-sfondo.html");

/// indirizzo della pagina: wry su Windows trasforma "zoo://" in http://zoo.localhost/
#[cfg(windows)]
pub const URL: &str = "http://zoo.localhost/";
#[cfg(not(windows))]
pub const URL: &str = "zoo://localhost/";

/// le specie, nello stesso ordine della pagina: (id, nome nel menu)
pub const SPECIE: [(&str, &str); 7] = [
    ("farfalle", "Farfalle"),
    ("meduse", "Meduse"),
    ("libellule", "Libellule"),
    ("koi", "Koi"),
    ("rane", "Rane"),
    ("aironi", "Aironi"),
    ("granchi", "Granchi"),
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

pub struct Menus {
    pub animali: Vec<CheckMenuItem>,
    pub rot_tempi: Vec<CheckMenuItem>,
    pub rot_specie: Vec<CheckMenuItem>,
    pub nomi: Vec<MenuItem>,
    pub koi: CheckMenuItem,
    pub batteria: CheckMenuItem,
    pub schermo: CheckMenuItem,
    pub avvio: CheckMenuItem,
    pub ricarica: MenuItem,
    pub esci: MenuItem,
}

/// cosa deve fare il programma dopo una voce del menu
pub enum Esito {
    /// impostazione cambiata (e salvata): manda lo script alle pagine, se c'è, e ricontrolla le pause
    Cambiato(Option<String>),
    Ricarica,
    Esci,
    Niente,
}

/// impostazioni + menu: la logica del menu, senza finestre
pub struct Core {
    pub cfg: Config,
    pub menus: Menus,
}

impl Core {
    /// `schermo_intero`: se mostrare la voce "Pausa con app a schermo intero"
    pub fn new(schermo_intero: bool) -> (Self, Menu) {
        let cfg = Config::load();
        let (menu, menus) = build_menu(&cfg, schermo_intero);
        (Self { cfg, menus }, menu)
    }

    /// i nomi come oggetto JSON: serde_json mette gli escape, quindi apostrofi e
    /// virgolette nei nomi non rompono lo script
    pub fn js_nomi(&self) -> String {
        let nomi = serde_json::to_string(&self.cfg.nomi).unwrap_or_else(|_| "{}".into());
        format!("window.zooSetting && zooSetting('nomi', {nomi})")
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

    /// tutte le impostazioni, da mandare quando una pagina ha finito di caricare
    pub fn js_tutto(&self, ac: bool) -> [String; 3] {
        [
            self.js_nomi(),
            self.js_rotazione(),
            format!(
                "if (window.zooSetting) {{ zooSetting('inCarica', {ac}); \
                 zooSetting('koiInCarica', {}); zooSetting('animale', {}); }}",
                self.cfg.koi_in_carica, self.cfg.animale
            ),
        ]
    }

    pub fn on_menu(&mut self, id: &str) -> Esito {
        let m = &self.menus;
        let js = if let Some(i) = m.animali.iter().position(|x| x.id().0 == id) {
            self.cfg.animale = i as u32;
            for (k, item) in self.menus.animali.iter().enumerate() {
                item.set_checked(k == i);
            }
            Some(format!("window.zooSetting && zooSetting('animale', {i})"))
        } else if let Some(i) = m.rot_tempi.iter().position(|x| x.id().0 == id) {
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
        } else if id == m.koi.id().0 {
            self.cfg.koi_in_carica = m.koi.is_checked();
            Some(format!("window.zooSetting && zooSetting('koiInCarica', {})", self.cfg.koi_in_carica))
        } else if id == m.batteria.id().0 {
            self.cfg.pausa_batteria = m.batteria.is_checked();
            None
        } else if id == m.schermo.id().0 {
            self.cfg.pausa_schermo_intero = m.schermo.is_checked();
            None
        } else if id == m.avvio.id().0 {
            autostart::set(m.avvio.is_checked());
            m.avvio.set_checked(autostart::enabled());
            return Esito::Niente;
        } else if id == m.ricarica.id().0 {
            return Esito::Ricarica;
        } else if id == m.esci.id().0 {
            return Esito::Esci;
        } else {
            return Esito::Niente;
        };
        self.cfg.save();
        Esito::Cambiato(js)
    }
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
    // nei menu di Windows "&" sottolinea la lettera dopo: va raddoppiata
    let nome = if nome.is_empty() {
        "(nessuno)".to_string()
    } else if cfg!(windows) {
        nome.replace('&', "&&")
    } else {
        nome.to_string()
    };
    format!("{label}: {nome}…")
}

fn build_menu(cfg: &Config, schermo_intero: bool) -> (Menu, Menus) {
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

    let avvio_label = if cfg!(target_os = "macos") { "Apri al login" } else { "Avvia con Windows" };
    let m = Menus {
        animali,
        rot_tempi,
        rot_specie,
        nomi,
        koi: CheckMenuItem::new("Koi nello stagno in carica (con \"A rotazione\")", true, cfg.koi_in_carica, None),
        batteria: CheckMenuItem::new("Pausa a batteria", true, cfg.pausa_batteria, None),
        schermo: CheckMenuItem::new("Pausa con app a schermo intero", true, cfg.pausa_schermo_intero, None),
        avvio: CheckMenuItem::new(avvio_label, true, autostart::enabled(), None),
        ricarica: MenuItem::new("Ricarica sfondo", true, None),
        esci: MenuItem::new("Esci", true, None),
    };
    let _ = menu.append(&sub);
    let _ = menu.append(&rot);
    let _ = menu.append(&nomi_sub);
    let _ = menu.append(&m.koi);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&m.batteria);
    if schermo_intero {
        let _ = menu.append(&m.schermo);
    }
    let _ = menu.append(&m.avvio);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&m.ricarica);
    let _ = menu.append(&m.esci);
    (menu, m)
}
