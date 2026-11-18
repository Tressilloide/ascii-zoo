//! Linux (pensato per Ubuntu/GNOME; funziona anche con altri desktop che rispettano
//! le finestre di tipo "desktop", come KDE e XFCE).
//!
//! - per ogni schermo una finestra X11 di tipo "desktop" con dentro WebKitGTK: sotto tutte
//!   le finestre, in tutti gli spazi di lavoro, fuori dalla barra delle applicazioni e
//!   trasparente ai clic (i clic vanno alle icone del desktop). Sotto Wayland passa da
//!   XWayland (GDK_BACKEND=x11): Wayland non permette a un programma normale di mettersi
//!   al posto dello sfondo.
//! - icona AppIndicator nella barra in alto (Ubuntu la mostra di serie; su GNOME senza
//!   Ubuntu serve l'estensione "AppIndicator and KStatusNotifierItem Support")
//! - ogni mezzo secondo le pause: a batteria, con lo schermo bloccato o spento, con
//!   un'app a schermo intero (vedi `Sonda`); animare quello che nessuno vede costa
//!   CPU e GPU per niente, e qui ogni fotogramma passa da XWayland
//! - ogni 2 secondi: controllo degli schermi collegati, margini della barra in alto
//!   per i widget

use std::cell::Cell;
use std::os::linux::net::SocketAddrExt;
use std::os::unix::net::{SocketAddr, UnixListener};
use std::rc::Rc;
use std::time::{Duration, Instant};

use gtk::prelude::*;
use gtk::{gio, glib};
use tao::dpi::{PhysicalPosition, PhysicalSize};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::platform::unix::WindowExtUnix;
use tao::window::{Window, WindowBuilder};
use tray_icon::menu::MenuEvent;
use tray_icon::{TrayIcon, TrayIconBuilder};
use wry::{PageLoadEvent, WebView, WebViewBuilder, WebViewBuilderExtUnix};

use crate::power;
use crate::shared::{self, Core, Esito, Schermo, JS_START, JS_STOP, URL};
use crate::stats::Stats;

/// ogni quanto si ricontrollano le pause: quando lo sfondo torna visibile riparte subito
const GIRO: Duration = Duration::from_millis(500);
/// ogni quanti giri si controllano batteria, schermi e margini (2 secondi)
const GIRI_CONTROLLO: u32 = 4;

enum Evento {
    Menu(String),
    PaginaPronta,
}

/// lo sfondo di uno schermo (prima la WebView, poi la sua finestra: i campi si
/// distruggono in quest'ordine)
struct Screen {
    webview: WebView,
    window: Window,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scala: f64,
    /// nome e posizione dello schermo: le impostazioni per schermo si legano a questo
    id: String,
    paused: bool,
    /// ultimi margini mandati alla pagina (barra in alto, dock…)
    margini: Option<[i32; 4]>,
    sonda: Sonda,
}

impl Screen {
    /// il centro dello schermo in pixel "logici", quelli di GDK (e della pagina)
    fn centro_logico(&self) -> (i32, i32) {
        let sc = if self.scala > 0.0 { self.scala } else { 1.0 };
        (
            ((self.pos.x as f64 + self.size.width as f64 / 2.0) / sc) as i32,
            ((self.pos.y as f64 + self.size.height as f64 / 2.0) / sc) as i32,
        )
    }
}

/// Capisce se uno schermo è occupato per intero da un'altra finestra (un'app a schermo
/// intero). Sotto Wayland un'app X11 non vede le finestre native che le stanno sopra,
/// ma mutter (GNOME), quando una finestra X11 è nascosta del tutto, le manda i messaggi
/// "fotogramma disegnato" (_NET_WM_FRAME_DRAWN) a un sesto della frequenza dello
/// schermo: il frame clock di GTK passa da ~60 a ~10 giri al secondo. La sonda tiene il
/// frame clock in moto e a ogni giro ridisegna un pixel (senza niente da disegnare GTK
/// non aspetta il compositor e gira sempre a 60). Con le finestre solo massimizzate non
/// scatta: per mutter la striscia sotto la barra in alto e la dock resta visibile.
/// Dove il compositor non rallenta (altri desktop) semplicemente non scatta mai, e
/// "lento" vale come "coperto" solo dopo aver visto almeno una volta il ritmo pieno:
/// con un compositor che rallentasse sempre lo sfondo non resta fermo.
#[derive(Default)]
struct Sonda {
    giri: Rc<Cell<u32>>,
    id: Option<gtk::TickCallbackId>,
    da: Option<Instant>,
    /// controlli di fila con il frame clock rallentato
    lenti: u32,
    /// il frame clock ha girato almeno una volta a ritmo pieno
    visto_veloce: bool,
}

impl Sonda {
    /// true se la finestra è nascosta del tutto da più di un controllo
    fn coperto(&mut self, gw: &gtk::ApplicationWindow, refresh_hz: f64) -> bool {
        if self.id.is_none() {
            let giri = self.giri.clone();
            self.id = Some(gw.add_tick_callback(move |w, _| {
                giri.set(giri.get() + 1);
                w.queue_draw_area(0, 0, 1, 1);
                glib::ControlFlow::Continue
            }));
            // la prima misura parte adesso
            self.giri.set(0);
            self.da = Some(Instant::now());
            return false;
        }
        let ora = Instant::now();
        let dt = self.da.map_or(0.0, |d| (ora - d).as_secs_f64());
        self.da = Some(ora);
        let giri = self.giri.replace(0);
        if dt > 0.0 && (giri as f64 / dt) < refresh_hz / 3.0 {
            self.lenti += 1;
        } else {
            self.lenti = 0;
            self.visto_veloce = true;
        }
        self.visto_veloce && self.lenti >= 2
    }

    /// quando non serve (in pausa per altri motivi) il frame clock si lascia stare
    fn spegni(&mut self) {
        if let Some(id) = self.id.take() {
            id.remove();
        }
        self.lenti = 0;
    }
}

/// Schermo bloccato o spento (salvaschermo di GNOME, o quello standard di KDE e altri):
/// lo sfondo non lo vede nessuno.
fn schermo_bloccato(bus: Option<&gio::DBusConnection>) -> bool {
    let Some(bus) = bus else { return false };
    let Ok(tipo) = glib::VariantTy::new("(b)") else { return false };
    for (nome, percorso) in
        [("org.gnome.ScreenSaver", "/org/gnome/ScreenSaver"), ("org.freedesktop.ScreenSaver", "/org/freedesktop/ScreenSaver")]
    {
        let risposta = bus.call_sync(
            Some(nome),
            percorso,
            nome,
            "GetActive",
            None,
            Some(tipo),
            gio::DBusCallFlags::NO_AUTO_START,
            200,
            gio::Cancellable::NONE,
        );
        if let Some((attivo,)) = risposta.ok().and_then(|v| v.get::<(bool,)>()) {
            return attivo;
        }
    }
    false
}

struct App {
    core: Core,
    screens: Vec<Screen>,
    tray: Option<TrayIcon>,
    last_ac: Option<bool>,
    stats: Stats,
    /// bus di sessione, per sapere se lo schermo è bloccato
    bus: Option<gio::DBusConnection>,
}

impl App {
    fn js(&self, code: &str) {
        for s in &self.screens {
            let _ = s.webview.evaluate_script(code);
        }
    }

    fn push_settings(&mut self) {
        let ac = power::on_ac();
        self.last_ac = Some(ac);
        for s in &mut self.screens {
            for code in self.core.js_tutto(ac, &s.id) {
                let _ = s.webview.evaluate_script(&code);
            }
            if s.paused {
                let _ = s.webview.evaluate_script(JS_STOP);
            }
            // una pagina (ri)caricata non sa niente dei margini
            s.margini = None;
        }
        self.check_margini();
    }

    /// manda a ogni schermo le impostazioni che dipendono dallo schermo
    fn push_schermi(&self) {
        for s in &self.screens {
            for code in self.core.js_schermo(&s.id) {
                let _ = s.webview.evaluate_script(&code);
            }
        }
    }

    /// Pausa schermo per schermo: a batteria, con lo schermo bloccato o spento, con
    /// un'app a schermo intero sopra lo sfondo.
    fn check_pause(&mut self) {
        let cfg = &self.core.cfg;
        let batteria = cfg.pausa_batteria && self.last_ac == Some(false);
        let bloccato = schermo_bloccato(self.bus.as_ref());
        let display = gtk::gdk::Display::default();
        for s in &mut self.screens {
            // la sonda serve solo se lo schermo intero è l'unico motivo che resta
            let coperto = if cfg.pausa_schermo_intero && !batteria && !bloccato {
                let (cx, cy) = s.centro_logico();
                let refresh = display
                    .as_ref()
                    .and_then(|d| d.monitor_at_point(cx, cy))
                    .map(|m| m.refresh_rate())
                    .filter(|&mhz| mhz > 0)
                    .map_or(60.0, |mhz| mhz as f64 / 1000.0);
                s.sonda.coperto(s.window.gtk_window(), refresh)
            } else {
                s.sonda.spegni();
                false
            };
            let pause = batteria || bloccato || coperto;
            if pause != s.paused {
                s.paused = pause;
                let _ = s.webview.evaluate_script(if pause { JS_STOP } else { JS_START });
            }
        }
    }

    fn check_power(&mut self) {
        let ac = power::on_ac();
        if self.last_ac != Some(ac) {
            self.last_ac = Some(ac);
            self.js(&shared::js_in_carica(ac));
        }
        self.check_pause();
        // widget "pc": nuovi valori ogni 2 secondi, solo se si vede
        if self.core.cfg.widget_statistiche && self.screens.iter().any(|s| !s.paused) {
            let v = self.stats.leggi();
            self.js(&shared::js_statistiche(&v));
        }
    }

    /// Quanto dello schermo occupano la barra in alto di GNOME, il dock e simili
    /// (area di lavoro più piccola del monitor): lo si dice alla pagina, così i widget
    /// non ci finiscono sotto. Si manda solo quando cambia.
    fn check_margini(&mut self) {
        let Some(display) = gtk::gdk::Display::default() else { return };
        for s in &mut self.screens {
            let (cx, cy) = s.centro_logico();
            let Some(mon) = display.monitor_at_point(cx, cy) else { continue };
            let (g, w) = (mon.geometry(), mon.workarea());
            let m = [
                w.y() - g.y(),
                (g.x() + g.width()) - (w.x() + w.width()),
                (g.y() + g.height()) - (w.y() + w.height()),
                w.x() - g.x(),
            ]
            .map(|v| v.max(0));
            if s.margini == Some(m) {
                continue;
            }
            s.margini = Some(m);
            let _ = s.webview.evaluate_script(&format!(
                "window.zooSetting && zooSetting('margini', {{alto: {}, destra: {}, basso: {}, sinistra: {}}})",
                m[0], m[1], m[2], m[3]
            ));
        }
    }

    /// in fondo alla pila, anche sotto la finestra delle icone del desktop (che su
    /// GNOME è anche lei di tipo "desktop")
    fn tieni_sotto(&self) {
        for s in &self.screens {
            if let Some(w) = s.window.gtk_window().window() {
                w.lower();
            }
        }
    }

    /// se gli schermi sono cambiati (collegati, scollegati, risoluzione) rifà gli sfondi
    fn check_screens(&mut self, target: &EventLoopWindowTarget<Evento>, proxy: &EventLoopProxy<Evento>) {
        let now = schermi(target);
        self.core.set_schermi(now.iter().map(|m| m.3.clone()).collect());
        let same = now.len() == self.screens.len()
            && self.screens.iter().all(|s| now.iter().any(|m| m.0 == s.pos && m.1 == s.size && m.3.id == s.id));
        if !same {
            self.screens.clear();
            self.screens = now
                .into_iter()
                .filter_map(|(pos, size, scala, sc)| make_screen(target, proxy, pos, size, scala, sc.id))
                .collect();
        }
    }
}

/// posizione e dimensione (pixel fisici) e scala di ogni schermo, il principale per primo
fn schermi(target: &EventLoopWindowTarget<Evento>) -> Vec<(PhysicalPosition<i32>, PhysicalSize<u32>, f64, Schermo)> {
    let principale = target.primary_monitor().map(|m| m.position());
    let mut v: Vec<_> = target
        .available_monitors()
        .map(|m| {
            let (pos, size) = (m.position(), m.size());
            let nome = m.name().unwrap_or_else(|| "schermo".into());
            let id = format!("{nome}@{},{}", pos.x, pos.y);
            (pos, size, m.scale_factor(), Schermo { id, larghezza: size.width, altezza: size.height })
        })
        .collect();
    v.sort_by_key(|(pos, _, _, _)| (Some(*pos) != principale, pos.x, pos.y));
    v
}

fn make_screen(
    target: &EventLoopWindowTarget<Evento>,
    proxy: &EventLoopProxy<Evento>,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    scala: f64,
    id: String,
) -> Option<Screen> {
    let window = WindowBuilder::new()
        .with_title("ASCII Zoo")
        .with_decorations(false)
        .with_resizable(false)
        .with_visible(false)
        .with_position(pos)
        .with_inner_size(size)
        .build(target)
        .ok()?;
    // va detto prima che la finestra compaia: il gestore delle finestre lo legge allora
    let gw = window.gtk_window();
    gw.set_type_hint(gtk::gdk::WindowTypeHint::Desktop);
    gw.set_skip_taskbar_hint(true);
    gw.set_skip_pager_hint(true);
    gw.set_accept_focus(false);
    gw.set_focus_on_map(false);
    gw.set_keep_below(true);
    gw.stick();

    let pronta = proxy.clone();
    let webview = WebViewBuilder::new()
        .with_custom_protocol("zoo".into(), |_id, _req| shared::pagina())
        .with_url(URL)
        .with_background_color((9, 10, 15, 255))
        .with_devtools(false)
        .with_on_page_load_handler(move |ev, _url| {
            if let PageLoadEvent::Finished = ev {
                let _ = pronta.send_event(Evento::PaginaPronta);
            }
        })
        .build_gtk(window.default_vbox()?)
        .ok()?;
    window.set_visible(true);
    // trasparente ai clic: vanno alle icone del desktop o a quello che c'è sotto
    gw.input_shape_combine_region(Some(&gtk::cairo::Region::create()));
    if let Some(w) = gw.window() {
        w.lower();
    }
    Some(Screen { webview, window, pos, size, scala, id, paused: false, margini: None, sonda: Sonda::default() })
}

/// Una sola copia per utente: un socket "astratto" (non è un file, sparisce da solo
/// quando il programma si chiude). Se il nome è già preso, c'è già un ASCII Zoo.
fn una_sola_copia() -> Option<UnixListener> {
    let utente = std::env::var("USER").unwrap_or_default();
    let addr = SocketAddr::from_abstract_name(format!("ascii-zoo-{utente}")).ok()?;
    UnixListener::bind_addr(&addr).ok()
}

pub fn run() {
    let Some(_una_sola) = una_sola_copia() else { return };

    // Wayland non permette a un programma di mettersi al posto dello sfondo: si usa
    // XWayland, dove le finestre di tipo "desktop" funzionano
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    // con i driver NVIDIA proprietari WebKitGTK a volte mostra una pagina vuota
    if std::path::Path::new("/proc/driver/nvidia").exists() && std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none() {
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }

    let event_loop = EventLoopBuilder::<Evento>::with_user_event().build();
    let proxy = event_loop.create_proxy();

    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = menu_proxy.send_event(Evento::Menu(e.id.0));
    }));

    let (core, menu) = Core::new(false);
    let mut menu = Some(menu);
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok();
    let mut app = App { core, screens: Vec::new(), tray: None, last_ac: None, stats: Stats::default(), bus };
    let mut giro: u32 = 0;

    event_loop.run(move |event, target, control_flow| match event {
        Event::NewEvents(StartCause::Init) => {
            app.check_screens(target, &proxy);
            if let Some(menu) = menu.take() {
                app.tray = TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_tooltip("ASCII Zoo")
                    .with_icon(shared::tray_icon_image())
                    .build()
                    .map_err(|e| eprintln!("ascii-zoo: icona nella barra non disponibile: {e}"))
                    .ok();
            }
            *control_flow = ControlFlow::WaitUntil(Instant::now() + GIRO);
        }
        Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
            giro = giro.wrapping_add(1);
            if giro % GIRI_CONTROLLO == 0 {
                app.check_screens(target, &proxy);
                app.check_power(); // ricontrolla anche le pause
                app.check_margini();
                app.tieni_sotto();
            } else {
                app.check_pause();
            }
            *control_flow = ControlFlow::WaitUntil(Instant::now() + GIRO);
        }
        Event::UserEvent(Evento::PaginaPronta) => app.push_settings(),
        Event::UserEvent(Evento::Menu(id)) => match app.core.on_menu(&id) {
            Esito::Cambiato(js) => {
                if let Some(code) = js {
                    app.js(&code);
                }
                app.check_power();
            }
            Esito::Schermi => app.push_schermi(),
            Esito::Ricarica => {
                for s in &mut app.screens {
                    s.paused = false;
                    let _ = s.webview.load_url(URL);
                }
            }
            Esito::Esci => {
                app.screens.clear();
                app.tray = None;
                *control_flow = ControlFlow::Exit;
            }
            Esito::Niente => {}
        },
        _ => {}
    });
}
