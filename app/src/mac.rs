//! macOS.
//!
//! - per ogni schermo una finestra senza bordi con dentro WKWebView, al livello del
//!   desktop: sopra l'immagine di sfondo, sotto le icone del Finder e tutte le finestre;
//!   presente in tutti gli Spazi, non riceve clic e non compare in Mission Control
//! - icona nella barra dei menu, niente icona nel Dock (app "accessory")
//! - ogni mezzo secondo le pause: a batteria e quando lo sfondo non si vede (finestre
//!   che coprono il desktop, app a schermo intero, schermo bloccato o spento: lo dice
//!   macOS con lo stato di occlusione della finestra); animare quello che nessuno vede
//!   costa CPU e GPU per niente
//! - ogni 2 secondi: controllo degli schermi collegati

use std::time::{Duration, Instant};

use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior, NSWindowOcclusionState};
use tao::dpi::{PhysicalPosition, PhysicalSize};
use tao::event::{Event, StartCause};
use tao::event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy, EventLoopWindowTarget};
use tao::platform::macos::{ActivationPolicy, EventLoopExtMacOS, WindowExtMacOS};
use tao::window::{Window, WindowBuilder};
use tray_icon::menu::MenuEvent;
use tray_icon::{TrayIcon, TrayIconBuilder};
use wry::{PageLoadEvent, WebView, WebViewBuilder};

use crate::power;
use crate::stats::Stats;
use crate::shared::{self, Core, Esito, Schermo, JS_START, JS_STOP, URL};

/// ogni quanto si ricontrollano le pause: quando lo sfondo torna visibile riparte subito
const GIRO: Duration = Duration::from_millis(500);
/// ogni quanti giri si controllano batteria e schermi (2 secondi)
const GIRI_CONTROLLO: u32 = 4;

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGWindowLevelForKey(key: i32) -> i32;
}
/// kCGDesktopWindowLevelKey: il livello dell'immagine di sfondo del desktop
const K_CG_DESKTOP_WINDOW_LEVEL_KEY: i32 = 2;

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
    /// nome e posizione dello schermo: le impostazioni per schermo si legano a questo
    id: String,
    paused: bool,
    /// macOS ha detto almeno una volta che la finestra si vede: solo dopo "non si vede"
    /// vale come pausa (se per qualche motivo risultasse sempre coperta, per esempio
    /// dalla finestra trasparente delle icone del Finder, lo sfondo non resta fermo)
    visto: bool,
}

impl Screen {
    /// qualche pezzo della finestra si vede (lo calcola il window server)
    fn visibile(&self) -> bool {
        let ns = self.window.ns_window() as *const NSWindow;
        if ns.is_null() {
            return true;
        }
        unsafe { &*ns }.occlusionState().contains(NSWindowOcclusionState::Visible)
    }
}

struct App {
    core: Core,
    screens: Vec<Screen>,
    tray: Option<TrayIcon>,
    last_ac: Option<bool>,
    stats: Stats,
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
        for s in &self.screens {
            for code in self.core.js_tutto(ac, &s.id) {
                let _ = s.webview.evaluate_script(&code);
            }
            if s.paused {
                let _ = s.webview.evaluate_script(JS_STOP);
            }
        }
    }

    /// manda a ogni schermo le impostazioni che dipendono dallo schermo
    fn push_schermi(&self) {
        for s in &self.screens {
            for code in self.core.js_schermo(&s.id) {
                let _ = s.webview.evaluate_script(&code);
            }
        }
    }

    /// Pausa schermo per schermo: a batteria, oppure quando lo sfondo non si vede.
    /// L'occlusione non distingue le finestre che coprono il desktop da un'app a schermo
    /// intero: basta una delle due opzioni del menu.
    fn check_pause(&mut self) {
        let cfg = &self.core.cfg;
        let batteria = cfg.pausa_batteria && self.last_ac == Some(false);
        let nascosto_conta = cfg.pausa_coperto || cfg.pausa_schermo_intero;
        for s in &mut self.screens {
            let visibile = s.visibile();
            s.visto |= visibile;
            let pause = batteria || (nascosto_conta && s.visto && !visibile);
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

    /// se gli schermi sono cambiati (collegati, scollegati, risoluzione) rifà gli sfondi
    fn check_screens(&mut self, target: &EventLoopWindowTarget<Evento>, proxy: &EventLoopProxy<Evento>) {
        let now = schermi(target);
        self.core.set_schermi(now.iter().map(|m| m.2.clone()).collect());
        let same = now.len() == self.screens.len()
            && self.screens.iter().all(|s| now.iter().any(|m| m.0 == s.pos && m.1 == s.size && m.2.id == s.id));
        if !same {
            self.screens.clear();
            self.screens =
                now.into_iter().filter_map(|(pos, size, sc)| make_screen(target, proxy, pos, size, sc.id)).collect();
        }
    }
}

/// posizione e dimensione (pixel fisici) di ogni schermo collegato, il principale per primo
fn schermi(target: &EventLoopWindowTarget<Evento>) -> Vec<(PhysicalPosition<i32>, PhysicalSize<u32>, Schermo)> {
    let principale = target.primary_monitor().map(|m| m.position());
    let mut v: Vec<_> = target
        .available_monitors()
        .map(|m| {
            let (pos, size) = (m.position(), m.size());
            let nome = m.name().unwrap_or_else(|| "schermo".into());
            let id = format!("{nome}@{},{}", pos.x, pos.y);
            (pos, size, Schermo { id, larghezza: size.width, altezza: size.height })
        })
        .collect();
    v.sort_by_key(|(pos, _, _)| (Some(*pos) != principale, pos.x, pos.y));
    v
}

/// porta la finestra al livello del desktop, in tutti gli Spazi, trasparente ai clic
fn al_livello_desktop(window: &Window) {
    let ns = window.ns_window() as *const NSWindow;
    if ns.is_null() {
        return;
    }
    unsafe {
        let ns = &*ns;
        // un gradino sopra l'immagine di sfondo, molto sotto le icone del Finder
        ns.setLevel(CGWindowLevelForKey(K_CG_DESKTOP_WINDOW_LEVEL_KEY) as isize + 1);
        ns.setCollectionBehavior(
            NSWindowCollectionBehavior::CanJoinAllSpaces
                | NSWindowCollectionBehavior::Stationary
                | NSWindowCollectionBehavior::IgnoresCycle,
        );
        ns.setIgnoresMouseEvents(true);
        ns.setHasShadow(false);
    }
}

fn make_screen(
    target: &EventLoopWindowTarget<Evento>,
    proxy: &EventLoopProxy<Evento>,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
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
    al_livello_desktop(&window);

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
        .build(&window)
        .ok()?;
    window.set_visible(true);
    Some(Screen { webview, window, pos, size, id, paused: false, visto: false })
}

pub fn run() {
    let mut event_loop = EventLoopBuilder::<Evento>::with_user_event().build();
    // niente icona nel Dock né nel selettore delle app: solo la barra dei menu
    event_loop.set_activation_policy(ActivationPolicy::Accessory);
    let proxy = event_loop.create_proxy();

    let menu_proxy = proxy.clone();
    MenuEvent::set_event_handler(Some(move |e: MenuEvent| {
        let _ = menu_proxy.send_event(Evento::Menu(e.id.0));
    }));

    let (core, menu) = Core::new(false);
    let mut menu = Some(menu);
    let mut app = App { core, screens: Vec::new(), tray: None, last_ac: None, stats: Stats::default() };
    let mut giro: u32 = 0;

    event_loop.run(move |event, target, control_flow| match event {
        Event::NewEvents(StartCause::Init) => {
            app.check_screens(target, &proxy);
            // su macOS l'icona si crea quando il ciclo degli eventi è già partito
            if let Some(menu) = menu.take() {
                app.tray = TrayIconBuilder::new()
                    .with_menu(Box::new(menu))
                    .with_tooltip("ASCII Zoo")
                    .with_icon(shared::tray_icon_image())
                    .build()
                    .ok();
            }
            *control_flow = ControlFlow::WaitUntil(Instant::now() + GIRO);
        }
        Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
            giro = giro.wrapping_add(1);
            if giro % GIRI_CONTROLLO == 0 {
                app.check_screens(target, &proxy);
                app.check_power(); // ricontrolla anche le pause
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
