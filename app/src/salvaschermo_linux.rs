//! Linux: lo screensaver, dalla parte dell'app. GNOME non ha più un modo per aggiungere
//! screensaver di altri, quindi lo fa partire ASCII Zoo: dopo `salvaschermo_minuti` minuti
//! senza mouse e tastiera lancia sé stesso come `ascii-zoo --salvaschermo` (sogno_linux.rs),
//! un processo a parte come il .scr su Windows, e lo chiude al primo segno di vita.
//!
//! - Da quanto non si usa il PC: su GNOME lo dice mutter (org.gnome.Mutter.IdleMonitor):
//!   AddIdleWatch avvisa dopo N minuti senza input, AddUserActiveWatch (vale una volta
//!   sola) al primo input. Non basta che la finestra dello screensaver veda mouse e tasti:
//!   se GNOME non le dà il focus, i tasti vanno all'app di prima. Su altri desktop (KDE…)
//!   si chiede org.freedesktop.ScreenSaver.GetSessionIdleTime ogni 2 secondi (ogni mezzo
//!   secondo mentre lo screensaver gira). Senza nessuno dei due le voci dei minuti restano
//!   grigie; "Prova adesso" funziona lo stesso.
//! - Non parte se qualcuno ha chiesto di non andare in inattività (un video che va:
//!   org.gnome.SessionManager.IsInhibited, su KDE org.freedesktop.PowerManagement.Inhibit),
//!   con lo schermo bloccato o se su uno schermo c'è un'app a schermo intero (la `Sonda` di
//!   linux.rs, quando gira). Se il motivo passa mentre il PC è ancora inattivo, parte allora.
//! - Si chiude con lo schermo bloccato o spento (ActiveChanged di org.gnome.ScreenSaver o
//!   org.freedesktop.ScreenSaver): sotto la schermata di blocco non deve consumare niente.
//! - Spento di serie: un aggiornamento non accende uno screensaver a chi non l'ha chiesto.

use std::os::unix::process::CommandExt;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};
use tray_icon::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};

use crate::config::Config;
use crate::shared::zoo_log;

/// le attese nel menu, in minuti (0 = spento). Lo screensaver si vede solo se parte prima
/// che GNOME spenga lo schermo (di serie dopo 5 minuti): per questo ci sono anche 1, 2 e 3.
const MINUTI: [u32; 8] = [0, 1, 2, 3, 5, 10, 15, 30];


const MUTTER: &str = "org.gnome.Mutter.IdleMonitor";
const MUTTER_PERCORSO: &str = "/org/gnome/Mutter/IdleMonitor/Core";

/// quello che arriva dal bus di sessione (lo passa al ciclo dell'app linux.rs)
pub enum Segnale {
    /// mutter: è scattato l'osservatore con questo id (inattività o di nuovo attività)
    Mutter(u32),
    /// il monitor di inattività di mutter c'è (true) o è sparito
    MutterPresente(bool),
    /// lo schermo si è bloccato o spento (true) o si è riacceso
    Bloccato(bool),
}

/// chi dice da quanto non si usa il PC
#[derive(Clone, Copy, PartialEq, Debug)]
enum Fonte {
    Nessuna,
    Mutter,
    /// org.freedesktop.ScreenSaver.GetSessionIdleTime
    Sessione,
}

struct Figlio {
    processo: Child,
    da: Instant,
}

/// GetSessionIdleTime: la specifica dice secondi, KDE dà millisecondi. Si guarda quanto
/// cresce in un secondo.
#[derive(Default)]
struct Sessione {
    ultimo: Option<(u32, Instant)>,
    per_secondo: Option<f64>,
}

pub struct Salvaschermo {
    sub: Submenu,
    minuti: Vec<(u32, CheckMenuItem)>,
    /// quando GNOME spegne lo schermo (Impostazioni > Energia), solo su GNOME
    nota: MenuItem,
    nota_testo: String,
    scene: Vec<(String, CheckMenuItem)>,
    prova: MenuItem,
    fonte: Fonte,
    id_inattivo: Option<u32>,
    id_attivo: Option<u32>,
    /// nessuno usa il PC da abbastanza minuti (vale fino al prossimo input)
    inattivo: bool,
    /// lo screensaver si è chiuso da solo: prima di ripartire si aspetta un input (solo
    /// con GetSessionIdleTime; con mutter l'osservatore scatta solo dopo un input)
    aspetta_input: bool,
    figlio: Option<Figlio>,
    sessione: Sessione,
    /// impostazioni di GNOME per lo spegnimento dello schermo, se ci sono
    gnome: Option<gio::Settings>,
    /// l'ultimo motivo per cui non è partito (per non ripeterlo nel log ogni 2 secondi)
    fermo_per: &'static str,
    giro: u32,
}

/// una chiamata D-Bus breve, senza far partire servizi che non ci sono
fn chiama(
    bus: &gio::DBusConnection,
    nome: &str,
    percorso: &str,
    interfaccia: &str,
    metodo: &str,
    args: Option<&glib::Variant>,
    tipo: Option<&str>,
) -> Option<glib::Variant> {
    let tipo = match tipo {
        Some(t) => Some(glib::VariantTy::new(t).ok()?),
        None => None,
    };
    bus.call_sync(
        Some(nome),
        percorso,
        interfaccia,
        metodo,
        args,
        tipo,
        gio::DBusCallFlags::NO_AUTO_START,
        500,
        gio::Cancellable::NONE,
    )
    .ok()
}

fn mutter(bus: &gio::DBusConnection, metodo: &str, args: Option<&glib::Variant>, tipo: Option<&str>) -> Option<glib::Variant> {
    chiama(bus, MUTTER, MUTTER_PERCORSO, MUTTER, metodo, args, tipo)
}

/// da quanto non si usa il PC, nell'unità di chi risponde (secondi o millisecondi)
fn idle_sessione(bus: &gio::DBusConnection) -> Option<u32> {
    let s = "org.freedesktop.ScreenSaver";
    chiama(bus, s, "/org/freedesktop/ScreenSaver", s, "GetSessionIdleTime", None, Some("(u)"))?.get::<(u32,)>().map(|v| v.0)
}

/// qualcuno (un video, una presentazione…) ha chiesto di non andare in inattività
fn inibito(bus: &gio::DBusConnection) -> bool {
    let gnome = "org.gnome.SessionManager";
    // 8 = inattività
    let otto = (8u32,).to_variant();
    if let Some((true,)) =
        chiama(bus, gnome, "/org/gnome/SessionManager", gnome, "IsInhibited", Some(&otto), Some("(b)")).and_then(|v| v.get())
    {
        return true;
    }
    let pm = "org.freedesktop.PowerManagement.Inhibit";
    matches!(
        chiama(bus, pm, "/org/freedesktop/PowerManagement/Inhibit", pm, "HasInhibit", None, Some("(b)")).and_then(|v| v.get()),
        Some((true,))
    )
}

/// le impostazioni di GNOME sullo spegnimento dello schermo, se lo schema esiste
/// (gio::Settings::new con uno schema che non c'è chiuderebbe il programma)
fn impostazioni_gnome() -> Option<gio::Settings> {
    let schema = "org.gnome.desktop.session";
    gio::SettingsSchemaSource::default()?.lookup(schema, true)?;
    Some(gio::Settings::new(schema))
}

fn etichetta_minuti(n: u32) -> String {
    match n {
        0 => "Spento".into(),
        1 => "Dopo 1 minuto".into(),
        n => format!("Dopo {n} minuti"),
    }
}

impl Salvaschermo {
    /// `avvisa` porta i segnali del bus al ciclo dell'app
    pub fn new(bus: Option<&gio::DBusConnection>, avvisa: Arc<dyn Fn(Segnale) + Send + Sync>, cfg: &Config) -> Self {
        let sub = Submenu::new("Screensaver", true);
        let scelta = cfg.salvaschermo_minuti;
        let minuti: Vec<(u32, CheckMenuItem)> =
            MINUTI.iter().map(|&n| (n, CheckMenuItem::new(etichetta_minuti(n), true, n == scelta, None))).collect();
        // un valore scritto a mano nella config, che il menu non ha
        let mut minuti = minuti;
        if !MINUTI.contains(&scelta) {
            minuti.push((scelta, CheckMenuItem::new(etichetta_minuti(scelta), true, true, None)));
        }
        for (_, v) in &minuti {
            let _ = sub.append(v);
        }
        let nota = MenuItem::new("", false, None);
        let _ = sub.append(&PredefinedMenuItem::separator());
        let _ = sub.append(&MenuItem::new("Scena:", false, None));
        let scena = if ascii_dreams::scene().iter().any(|s| s.0 == cfg.salvaschermo_scena) {
            cfg.salvaschermo_scena.as_str()
        } else {
            "rotazione"
        };
        let scene: Vec<(String, CheckMenuItem)> = std::iter::once(("rotazione".to_string(), "Tutte, una dopo l'altra".to_string()))
            .chain(ascii_dreams::scene().iter().map(|(id, nome)| {
                let nome = crate::shared::nome_salvaschermo(id).unwrap_or(nome);
                (id.to_string(), nome.to_string())
            }))
            .map(|(id, nome)| {
                let v = CheckMenuItem::new(&nome, true, id == scena, None);
                (id, v)
            })
            .collect();
        for (_, v) in &scene {
            let _ = sub.append(v);
        }
        let _ = sub.append(&PredefinedMenuItem::separator());
        let prova = MenuItem::new("Prova adesso", true, None);
        let _ = sub.append(&prova);

        let mut s = Salvaschermo {
            sub,
            minuti,
            nota,
            nota_testo: String::new(),
            scene,
            prova,
            fonte: Fonte::Nessuna,
            id_inattivo: None,
            id_attivo: None,
            inattivo: false,
            aspetta_input: false,
            figlio: None,
            sessione: Sessione::default(),
            gnome: None,
            fermo_per: "",
            giro: 0,
        };
        let Some(bus) = bus else {
            s.aggiorna_menu();
            return s;
        };
        // KDE e altri: GetSessionIdleTime risponde? (con mutter si passa a lui appena c'è)
        if idle_sessione(bus).is_some() {
            s.fonte = Fonte::Sessione;
        }
        let a = avvisa.clone();
        bus.signal_subscribe(
            Some(MUTTER),
            Some(MUTTER),
            Some("WatchFired"),
            Some(MUTTER_PERCORSO),
            None,
            gio::DBusSignalFlags::NONE,
            move |_, _, _, _, _, p| {
                if let Some((id,)) = p.get::<(u32,)>() {
                    a(Segnale::Mutter(id));
                }
            },
        );
        for interfaccia in ["org.gnome.ScreenSaver", "org.freedesktop.ScreenSaver"] {
            let a = avvisa.clone();
            bus.signal_subscribe(
                None,
                Some(interfaccia),
                Some("ActiveChanged"),
                None,
                None,
                gio::DBusSignalFlags::NONE,
                move |_, _, _, _, _, p| {
                    if let Some((attivo,)) = p.get::<(bool,)>() {
                        a(Segnale::Bloccato(attivo));
                    }
                },
            );
        }
        // mutter c'è già (avviso subito) o arriva dopo; se GNOME Shell riparte gli
        // osservatori di prima spariscono e vanno rifatti (si osserva per tutta la vita del
        // programma: l'id non serve)
        let (a, b) = (avvisa.clone(), avvisa);
        let _ = gio::bus_watch_name_on_connection(
            bus,
            MUTTER,
            gio::BusNameWatcherFlags::NONE,
            move |_, _, _| a(Segnale::MutterPresente(true)),
            move |_, _| b(Segnale::MutterPresente(false)),
        );
        s.aggiorna_menu();
        s
    }

    /// mette il sottomenu nel menu dell'icona, dopo la voce con l'id `dopo`
    pub fn inserisci(&self, menu: &Menu, dopo: &str) {
        let pos = menu.items().iter().position(|v| v.id().0 == dopo).map_or(menu.items().len(), |p| p + 1);
        let _ = menu.insert(&self.sub, pos);
    }

    /// lo screensaver sta girando
    pub fn in_corso(&self) -> bool {
        self.figlio.is_some()
    }

    /// le voci dei minuti si possono scegliere solo se si sa quando il PC è inattivo
    fn aggiorna_menu(&mut self) {
        let attive = self.fonte != Fonte::Nessuna;
        for (_, v) in &self.minuti {
            v.set_enabled(attive);
        }
        let testo = match self.fonte {
            Fonte::Nessuna => "(qui non so quando il PC è inattivo)".to_string(),
            Fonte::Sessione => String::new(),
            // come fa GNOME: lo schermo si spegne dopo idle-delay secondi senza input
            Fonte::Mutter => match self.gnome.as_ref().map(|g| g.uint("idle-delay")) {
                Some(0) => "GNOME non spegne mai lo schermo".into(),
                Some(s) if s % 60 == 0 => format!("GNOME spegne lo schermo dopo {} min", s / 60),
                Some(s) => format!("GNOME spegne lo schermo dopo {s} s"),
                None => String::new(),
            },
        };
        if testo != self.nota_testo {
            // la nota sta subito dopo i minuti; vuota, non c'è
            if self.nota_testo.is_empty() {
                let _ = self.sub.insert(&self.nota, self.minuti.len());
            }
            if testo.is_empty() {
                let _ = self.sub.remove(&self.nota);
            } else {
                self.nota.set_text(&testo);
            }
            self.nota_testo = testo;
        }
    }

    /// una voce del menu: true se era di questo sottomenu
    pub fn on_menu(&mut self, id: &str, cfg: &mut Config, bus: Option<&gio::DBusConnection>) -> bool {
        if let Some(i) = self.minuti.iter().position(|(_, v)| v.id().0 == id) {
            let n = self.minuti[i].0;
            for (k, (_, v)) in self.minuti.iter().enumerate() {
                v.set_checked(k == i);
            }
            cfg.salvaschermo_minuti = n;
            cfg.save();
            zoo_log!("screensaver: {}", etichetta_minuti(n).to_lowercase());
            if let (Some(g), true) = (self.gnome.as_ref(), n > 0) {
                let spegne = g.uint("idle-delay");
                if spegne > 0 && n * 60 >= spegne {
                    zoo_log!("screensaver: GNOME spegne lo schermo prima ({spegne} s): non si vedrà");
                }
            }
            if let Some(bus) = bus {
                self.osserva(cfg, bus);
            }
            return true;
        }
        if let Some(i) = self.scene.iter().position(|(_, v)| v.id().0 == id) {
            for (k, (_, v)) in self.scene.iter().enumerate() {
                v.set_checked(k == i);
            }
            cfg.salvaschermo_scena = self.scene[i].0.clone();
            cfg.save();
            return true;
        }
        if id == self.prova.id().0 {
            self.ferma(bus, "prova");
            self.avvia(&cfg.salvaschermo_scena);
            return true;
        }
        false
    }

    /// (ri)mette l'osservatore di mutter per l'inattività, con i minuti scelti
    fn osserva(&mut self, cfg: &Config, bus: &gio::DBusConnection) {
        if let Some(id) = self.id_inattivo.take() {
            let _ = mutter(bus, "RemoveWatch", Some(&(id,).to_variant()), None);
        }
        let minuti = cfg.salvaschermo_minuti.min(24 * 60);
        if minuti == 0 {
            self.inattivo = false;
            if self.figlio.is_none() {
                self.togli_attivo(Some(bus));
            }
        }
        if self.fonte != Fonte::Mutter || minuti == 0 {
            return;
        }
        let ms = (minuti as u64 * 60_000,).to_variant();
        self.id_inattivo = mutter(bus, "AddIdleWatch", Some(&ms), Some("(u)")).and_then(|v| v.get::<(u32,)>()).map(|v| v.0);
        zoo_log!("screensaver: mutter avvisa dopo {minuti} min senza input (osservatore {:?})", self.id_inattivo);
    }

    /// l'osservatore di mutter per il prossimo input (scatta una volta sola)
    fn osserva_attivita(&mut self, bus: Option<&gio::DBusConnection>) {
        if self.fonte != Fonte::Mutter || self.id_attivo.is_some() {
            return;
        }
        if let Some(bus) = bus {
            self.id_attivo = mutter(bus, "AddUserActiveWatch", None, Some("(u)")).and_then(|v| v.get::<(u32,)>()).map(|v| v.0);
        }
    }

    fn togli_attivo(&mut self, bus: Option<&gio::DBusConnection>) {
        if let (Some(id), Some(bus)) = (self.id_attivo.take(), bus) {
            let _ = mutter(bus, "RemoveWatch", Some(&(id,).to_variant()), None);
        }
    }

    /// un segnale dal bus. `bloccato` e `coperto` come in `giro`.
    pub fn segnale(&mut self, s: Segnale, cfg: &Config, bus: Option<&gio::DBusConnection>, bloccato: bool, coperto: bool) {
        match s {
            Segnale::Mutter(id) if Some(id) == self.id_inattivo => {
                zoo_log!("screensaver: nessun input da {} min", cfg.salvaschermo_minuti);
                self.inattivo = true;
                self.osserva_attivita(bus);
                self.prova_a_partire(cfg, bus, bloccato, coperto);
            }
            Segnale::Mutter(id) if Some(id) == self.id_attivo => {
                zoo_log!("screensaver: mutter dice che qualcuno ha usato il PC");
                self.id_attivo = None;
                self.inattivo = false;
                self.ferma(bus, "mouse o tastiera");
            }
            Segnale::Mutter(_) => {}
            Segnale::MutterPresente(c) => {
                zoo_log!("screensaver: monitor di inattività di mutter {}", if c { "presente" } else { "sparito" });
                // gli osservatori vecchi, se c'erano, sono spariti con lui
                self.id_inattivo = None;
                self.id_attivo = None;
                self.inattivo = false;
                if c {
                    self.fonte = Fonte::Mutter;
                    if self.gnome.is_none() {
                        self.gnome = impostazioni_gnome();
                    }
                } else {
                    self.fonte = if bus.and_then(idle_sessione).is_some() { Fonte::Sessione } else { Fonte::Nessuna };
                }
                if let Some(bus) = bus {
                    self.osserva(cfg, bus);
                }
                if self.figlio.is_some() {
                    self.osserva_attivita(bus);
                }
                self.aggiorna_menu();
            }
            Segnale::Bloccato(true) => {
                self.inattivo = false;
                self.ferma(bus, "schermo bloccato o spento");
            }
            Segnale::Bloccato(false) => {}
        }
    }

    /// Ogni mezzo secondo: lo screensaver c'è ancora? Con GetSessionIdleTime, il PC è
    /// inattivo? E ogni 2 secondi, se il PC è inattivo ma lo screensaver non è partito
    /// (video, schermo intero), lo si riprova. `bloccato`: schermo bloccato o spento;
    /// `coperto`: un'app a schermo intero su almeno uno schermo.
    pub fn giro(&mut self, cfg: &Config, bus: Option<&gio::DBusConnection>, bloccato: bool, coperto: bool) {
        self.giro = self.giro.wrapping_add(1);
        let ogni_2s = self.giro % 4 == 0;
        if let Some(f) = self.figlio.as_mut() {
            match f.processo.try_wait() {
                Ok(None) => {
                    // l'input che ha fatto partire la prova (clic sul menu) non deve chiuderlo
                    if f.da.elapsed() > Duration::from_secs(1) {
                        self.osserva_attivita(bus);
                    }
                }
                esito => {
                    zoo_log!("screensaver: chiuso da sé ({:?})", esito.map(|e| e.map(|e| e.to_string())));
                    self.figlio = None;
                    self.inattivo = false;
                    self.aspetta_input = true;
                    self.togli_attivo(bus);
                }
            }
        }
        if bloccato && self.figlio.is_some() {
            self.inattivo = false;
            self.ferma(bus, "schermo bloccato o spento");
        }
        if self.fonte == Fonte::Sessione && (ogni_2s || self.figlio.is_some()) {
            if let Some(bus) = bus {
                self.sessione(cfg, bus);
            }
        }
        if ogni_2s {
            self.prova_a_partire(cfg, bus, bloccato, coperto);
            self.aggiorna_menu();
        }
    }

    /// GetSessionIdleTime: quando torna indietro qualcuno ha usato il PC
    fn sessione(&mut self, cfg: &Config, bus: &gio::DBusConnection) {
        let Some(v) = idle_sessione(bus) else { return };
        let ora = Instant::now();
        let s = &mut self.sessione;
        let mut attivita = false;
        if let Some((prima, quando)) = s.ultimo {
            if v < prima {
                attivita = true;
            } else if s.per_secondo.is_none() && v > prima {
                let al_secondo = (v - prima) as f64 / (ora - quando).as_secs_f64().max(0.001);
                s.per_secondo = Some(if al_secondo > 50.0 { 1000.0 } else { 1.0 });
            }
        }
        s.ultimo = Some((v, ora));
        let secondi = s.per_secondo.map(|p| v as f64 / p);
        // nel primo secondo della prova il valore è ancora quello del clic sul menu
        let giovane = self.figlio.as_ref().is_some_and(|f| f.da.elapsed() < Duration::from_secs(1));
        if attivita && !giovane {
            self.aspetta_input = false;
            self.inattivo = false;
            self.ferma(Some(bus), "mouse o tastiera");
        }
        let minuti = cfg.salvaschermo_minuti;
        if let Some(secondi) = secondi {
            if minuti > 0 && !self.inattivo && !self.aspetta_input && secondi >= (minuti * 60) as f64 {
                zoo_log!("screensaver: nessun input da {minuti} min");
                self.inattivo = true;
            }
        }
    }

    /// fa partire lo screensaver se il PC è inattivo e niente lo impedisce
    fn prova_a_partire(&mut self, cfg: &Config, bus: Option<&gio::DBusConnection>, bloccato: bool, coperto: bool) {
        if !self.inattivo || self.figlio.is_some() || cfg.salvaschermo_minuti == 0 {
            self.fermo_per = "";
            return;
        }
        let motivo = if bloccato {
            "schermo bloccato o spento"
        } else if coperto {
            "un'app è a schermo intero"
        } else if bus.is_some_and(inibito) {
            "qualcuno ha chiesto di non andare in inattività (video?)"
        } else {
            ""
        };
        if !motivo.is_empty() {
            if self.fermo_per != motivo {
                zoo_log!("screensaver: non parte: {motivo}");
                self.fermo_per = motivo;
            }
            return;
        }
        self.fermo_per = "";
        self.avvia(&cfg.salvaschermo_scena);
    }

    /// lancia `ascii-zoo --salvaschermo --scena …`
    fn avvia(&mut self, scena: &str) {
        // se l'eseguibile è stato sostituito mentre girava, /proc/self/exe è ancora lui
        let exe = std::env::current_exe().ok().filter(|p| p.exists()).unwrap_or_else(|| "/proc/self/exe".into());
        let mut cmd = Command::new(exe);
        cmd.args(["--salvaschermo", "--scena", scena]);
        // lo screensaver resta su XWayland anche quando lo sfondo è nel modo Wayland
        // (GDK_BACKEND=wayland nell'ambiente dell'app): sopra tutto e a schermo intero
        if std::env::var_os("WAYLAND_DISPLAY").is_some() {
            cmd.env("GDK_BACKEND", "x11");
        }
        // se l'app si chiude (o la chiude una copia nuova) lo screensaver non resta lì da solo
        // SAFETY: prctl si può chiamare tra fork ed exec
        unsafe {
            cmd.pre_exec(|| {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGTERM as libc::c_ulong);
                Ok(())
            });
        }
        match cmd.spawn() {
            Ok(p) => {
                zoo_log!("screensaver: parte (pid {}, scena {scena})", p.id());
                self.figlio = Some(Figlio { processo: p, da: Instant::now() });
                // la misura di GetSessionIdleTime ricomincia da qui
                self.sessione.ultimo = None;
            }
            Err(e) => eprintln!("ascii-zoo: non riesco ad avviare lo screensaver: {e}"),
        }
    }

    /// chiude lo screensaver, se gira
    pub fn ferma(&mut self, bus: Option<&gio::DBusConnection>, motivo: &str) {
        if let Some(mut f) = self.figlio.take() {
            zoo_log!("screensaver: lo chiudo ({motivo})");
            let _ = f.processo.kill();
            let _ = f.processo.wait();
        }
        if !self.inattivo {
            self.togli_attivo(bus);
        }
    }
}
