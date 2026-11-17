//! macOS.
//!
//! - per ogni schermo una finestra senza bordi con dentro WKWebView, al livello del
//!   desktop: sopra l'immagine di sfondo, sotto le icone del Finder e tutte le finestre;
//!   presente in tutti gli Spazi, non riceve clic e non compare in Mission Control
//! - icona nella barra dei menu, niente icona nel Dock (app "accessory")
//! - ogni 2 secondi: pausa a batteria e controllo degli schermi collegati

use std::time::{Duration, Instant};

use objc2_app_kit::{NSWindow, NSWindowCollectionBehavior};
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
use crate::shared::{self, Core, Esito, JS_START, JS_STOP, URL};

/// ogni quanto si controllano batteria e schermi
const CONTROLLO: Duration = Duration::from_secs(2);

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
    _window: Window,
    pos: PhysicalPosition<i32>,
    size: PhysicalSize<u32>,
    paused: bool,
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
        for code in self.core.js_tutto(ac) {
            self.js(&code);
        }
        for s in &self.screens {
            if s.paused {
                let _ = s.webview.evaluate_script(JS_STOP);
            }
        }
    }

    fn check_power(&mut self) {
        let ac = power::on_ac();
        if self.last_ac != Some(ac) {
            self.last_ac = Some(ac);
            self.js(&shared::js_in_carica(ac));
        }
        let pause = self.core.cfg.pausa_batteria && !ac;
        for s in &mut self.screens {
            if pause != s.paused {
                s.paused = pause;
                let _ = s.webview.evaluate_script(if pause { JS_STOP } else { JS_START });
            }
        }
        // widget "pc": nuovi valori ogni 2 secondi, solo se si vede
        if self.core.cfg.widget_statistiche && self.screens.iter().any(|s| !s.paused) {
            let v = self.stats.leggi();
            self.js(&shared::js_statistiche(&v));
        }
    }

    /// se gli schermi sono cambiati (collegati, scollegati, risoluzione) rifà gli sfondi
    fn check_screens(&mut self, target: &EventLoopWindowTarget<Evento>, proxy: &EventLoopProxy<Evento>) {
        let now = schermi(target);
        let same = now.len() == self.screens.len()
            && self.screens.iter().all(|s| now.contains(&(s.pos, s.size)));
        if !same {
            self.screens.clear();
            self.screens = now.iter().filter_map(|&(pos, size)| make_screen(target, proxy, pos, size)).collect();
        }
    }
}

/// posizione e dimensione (pixel fisici) di ogni schermo collegato
fn schermi(target: &EventLoopWindowTarget<Evento>) -> Vec<(PhysicalPosition<i32>, PhysicalSize<u32>)> {
    target.available_monitors().map(|m| (m.position(), m.size())).collect()
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
    Some(Screen { webview, _window: window, pos, size, paused: false })
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
            *control_flow = ControlFlow::WaitUntil(Instant::now() + CONTROLLO);
        }
        Event::NewEvents(StartCause::ResumeTimeReached { .. }) => {
            app.check_screens(target, &proxy);
            app.check_power();
            *control_flow = ControlFlow::WaitUntil(Instant::now() + CONTROLLO);
        }
        Event::UserEvent(Evento::PaginaPronta) => app.push_settings(),
        Event::UserEvent(Evento::Menu(id)) => match app.core.on_menu(&id) {
            Esito::Cambiato(js) => {
                if let Some(code) = js {
                    app.js(&code);
                }
                app.check_power();
            }
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
