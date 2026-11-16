//! ASCII Zoo: sfondo animato per Windows.
//!
//! - lo sfondo è l'HTML in assets/, incorporato nell'eseguibile
//! - per ogni monitor una finestra con dentro WebView2 viene messa dietro le icone del desktop
//! - una finestra nascosta di controllo tiene il timer e si accorge dei cambi di schermo
//!   (monitor collegati/scollegati, risoluzione, riavvio di Explorer) e rifà gli sfondi
//! - un'icona nell'area di notifica ha tutte le opzioni
//! - pausa automatica a batteria e, schermo per schermo, con app a schermo intero

#![windows_subsystem = "windows"]

mod config;
mod desktop;
mod dialog;
mod power;

use std::borrow::Cow;
use std::cell::RefCell;
use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use tray_icon::menu::{CheckMenuItem, Menu, MenuEvent, MenuItem, PredefinedMenuItem, Submenu};
use tray_icon::{Icon, TrayIcon, TrayIconBuilder};
use windows::core::{w, BOOL};
use windows::Win32::Foundation::{GetLastError, ERROR_ALREADY_EXISTS, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::CreateMutexW;
use windows::Win32::UI::HiDpi::{SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::WindowsAndMessaging::*;
use wry::http::Response;
use wry::{PageLoadEvent, WebView, WebViewBuilder};

use config::{autostart, pulisci_nome, Config, NOME_MAX};

static HTML: &str = include_str!("../assets/ascii-zoo-sfondo.html");
const URL: &str = "http://zoo.localhost/";
/// le specie, nello stesso ordine della pagina: (id, nome nel menu)
const SPECIE: [(&str, &str); 7] = [
    ("farfalle", "Farfalle"),
    ("meduse", "Meduse"),
    ("libellule", "Libellule"),
    ("koi", "Koi"),
    ("rane", "Rane"),
    ("aironi", "Aironi"),
    ("granchi", "Granchi"),
];
/// ogni quanto cambia animale "a rotazione"
const INTERVALLI: [(u32, &str); 7] = [
    (30, "Ogni 30 secondi"),
    (60, "Ogni minuto"),
    (120, "Ogni 2 minuti"),
    (300, "Ogni 5 minuti"),
    (600, "Ogni 10 minuti"),
    (1800, "Ogni 30 minuti"),
    (3600, "Ogni ora"),
];
const TIMER: usize = 1;
/// tick (da 250 ms) da aspettare dopo un cambio di schermo prima di rifare gli sfondi
const REBUILD_DELAY: u32 = 4;

static PAGE_READY: AtomicBool = AtomicBool::new(false);
/// un monitor è cambiato o Explorer è ripartito: gli sfondi vanno rifatti
static REBUILD: AtomicBool = AtomicBool::new(false);
/// messaggio "TaskbarCreated", mandato da Explorer quando riparte
static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);

/// HWND che wry può usare come finestra ospite
struct Host(HWND);
impl HasWindowHandle for Host {
    fn window_handle(&self) -> Result<WindowHandle<'_>, HandleError> {
        let raw = Win32WindowHandle::new(NonZeroIsize::new(self.0 .0 as isize).ok_or(HandleError::Unavailable)?);
        Ok(unsafe { WindowHandle::borrow_raw(RawWindowHandle::Win32(raw)) })
    }
}

struct Menus {
    animali: Vec<CheckMenuItem>,
    rot_tempi: Vec<CheckMenuItem>,
    rot_specie: Vec<CheckMenuItem>,
    nomi: Vec<MenuItem>,
    koi: CheckMenuItem,
    batteria: CheckMenuItem,
    schermo: CheckMenuItem,
    avvio: CheckMenuItem,
    ricarica: MenuItem,
    esci: MenuItem,
}

/// lo sfondo di un monitor
struct Screen {
    webview: WebView,
    hwnd: HWND,
    parent: HWND,
    rect: RECT,
    paused: bool,
}

impl Screen {
    fn js(&self, code: &str) {
        let _ = self.webview.evaluate_script(code);
    }
}

struct App {
    hinst: HINSTANCE,
    ctl: HWND,
    screens: Vec<Screen>,
    _tray: TrayIcon,
    menus: Menus,
    cfg: Config,
    last_ac: Option<bool>,
    ticks: u32,
    rebuild_at: Option<u32>,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

impl App {
    fn js(&self, code: &str) {
        for s in &self.screens {
            s.js(code);
        }
    }

    /// i nomi come oggetto JSON: serde_json mette gli escape, quindi apostrofi e
    /// virgolette nei nomi non rompono lo script
    fn js_nomi(&self) -> String {
        let nomi = serde_json::to_string(&self.cfg.nomi).unwrap_or_else(|_| "{}".into());
        format!("window.zooSetting && zooSetting('nomi', {nomi})")
    }

    fn js_rotazione(&self) -> String {
        let specie: Vec<&str> = SPECIE
            .iter()
            .map(|s| s.0)
            .filter(|id| !self.cfg.rotazione_escluse.iter().any(|e| e == id))
            .collect();
        let rot = serde_json::json!({ "secondi": self.cfg.rotazione_secondi, "specie": specie });
        format!("window.zooSetting && zooSetting('rotazione', {rot})")
    }

    /// manda allo sfondo tutte le impostazioni (funzione zooSetting della pagina)
    fn push_settings(&mut self) {
        let ac = power::on_ac();
        self.last_ac = Some(ac);
        self.js(&self.js_nomi());
        self.js(&self.js_rotazione());
        self.js(&format!(
            "if (window.zooSetting) {{ zooSetting('inCarica', {ac}); \
             zooSetting('koiInCarica', {}); zooSetting('animale', {}); }}",
            self.cfg.koi_in_carica, self.cfg.animale
        ));
        for s in &self.screens {
            if s.paused {
                s.js("window.asciiZoo && window.asciiZoo.stop()");
            }
        }
    }

    fn check_power(&mut self) {
        let ac = power::on_ac();
        if self.last_ac != Some(ac) {
            self.last_ac = Some(ac);
            self.js(&format!("window.zooSetting && zooSetting('inCarica', {ac})"));
        }
        let battery = self.cfg.pausa_batteria && !ac;
        let full = if self.cfg.pausa_schermo_intero { power::fullscreen_monitor() } else { None };
        for s in &mut self.screens {
            let pause = battery || full == Some(s.rect);
            if pause != s.paused {
                s.paused = pause;
                s.js(if pause { "window.asciiZoo && window.asciiZoo.stop()" } else { "window.asciiZoo && window.asciiZoo.start()" });
            }
        }
    }

    /// rifà gli sfondi se i monitor sono cambiati (o se Explorer ha distrutto le finestre)
    fn rebuild(&mut self) {
        let rects = monitors();
        let same = rects.len() == self.screens.len()
            && self.screens.iter().all(|s| unsafe { IsWindow(Some(s.hwnd)).as_bool() } && rects.contains(&s.rect));
        if same {
            for s in &self.screens {
                unsafe { desktop::fit(s.hwnd, s.parent, s.rect) };
            }
            return;
        }
        drop_screens(&mut self.screens);
        self.screens = make_screens(self.hinst, &rects);
        // le impostazioni arrivano quando le pagine hanno finito di caricare (PAGE_READY)
    }

    fn on_menu(&mut self, id: &str) {
        let m = &self.menus;
        if let Some(i) = m.animali.iter().position(|x| x.id().0 == id) {
            self.cfg.animale = i as u32;
            for (k, item) in self.menus.animali.iter().enumerate() {
                item.set_checked(k == i);
            }
            self.js(&format!("window.zooSetting && zooSetting('animale', {i})"));
        } else if let Some(i) = m.rot_tempi.iter().position(|x| x.id().0 == id) {
            self.cfg.rotazione_secondi = INTERVALLI[i].0;
            for (k, item) in self.menus.rot_tempi.iter().enumerate() {
                item.set_checked(k == i);
            }
            self.js(&self.js_rotazione());
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
            self.js(&self.js_rotazione());
        } else if let Some(i) = m.nomi.iter().position(|x| x.id().0 == id) {
            let (sid, label) = SPECIE[i];
            let attuale = self.cfg.nomi.get(sid).cloned().unwrap_or_default();
            let domanda = format!("Nome per {} (vuoto = nessun nome):", label.to_lowercase());
            match dialog::chiedi_nome("ASCII Zoo", &domanda, &attuale, NOME_MAX) {
                Some(nuovo) => {
                    let nuovo = pulisci_nome(&nuovo);
                    self.menus.nomi[i].set_text(etichetta_nome(label, &nuovo));
                    self.cfg.nomi.insert(sid.to_string(), nuovo);
                    self.js(&self.js_nomi());
                }
                None => return, // annullato: non cambia niente
            }
        } else if id == m.koi.id().0 {
            self.cfg.koi_in_carica = m.koi.is_checked();
            self.js(&format!("window.zooSetting && zooSetting('koiInCarica', {})", self.cfg.koi_in_carica));
        } else if id == m.batteria.id().0 {
            self.cfg.pausa_batteria = m.batteria.is_checked();
        } else if id == m.schermo.id().0 {
            self.cfg.pausa_schermo_intero = m.schermo.is_checked();
        } else if id == m.avvio.id().0 {
            autostart::set(m.avvio.is_checked());
            m.avvio.set_checked(autostart::enabled());
            return;
        } else if id == m.ricarica.id().0 {
            for s in &mut self.screens {
                s.paused = false;
                let _ = s.webview.load_url(URL);
            }
            return;
        } else if id == m.esci.id().0 {
            unsafe { let _ = DestroyWindow(self.ctl); }
            return;
        }
        self.cfg.save();
        self.check_power();
    }

    fn tick(&mut self) {
        while let Ok(ev) = MenuEvent::receiver().try_recv() {
            self.on_menu(&ev.id.0);
        }
        self.ticks = self.ticks.wrapping_add(1);
        if REBUILD.swap(false, Ordering::SeqCst) {
            // altri cambi in arrivo spostano l'attesa: si rifà tutto una volta sola
            self.rebuild_at = Some(self.ticks.wrapping_add(REBUILD_DELAY));
        }
        if self.rebuild_at == Some(self.ticks) {
            self.rebuild_at = None;
            self.rebuild();
        }
        if PAGE_READY.swap(false, Ordering::SeqCst) {
            self.push_settings();
        }
        if self.ticks % 8 == 0 {
            self.check_power();
        }
    }
}

fn primary_monitor() -> RECT {
    unsafe {
        let mon = MonitorFromPoint(POINT { x: 0, y: 0 }, MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        mi.rcMonitor
    }
}

/// rettangoli di tutti i monitor collegati (pixel fisici)
fn monitors() -> Vec<RECT> {
    unsafe extern "system" fn add(mon: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        let list = &mut *(out.0 as *mut Vec<RECT>);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(mon, &mut mi).as_bool() {
            list.push(mi.rcMonitor);
        }
        BOOL(1)
    }
    let mut list: Vec<RECT> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(add), LPARAM(&mut list as *mut Vec<RECT> as isize));
    }
    if list.is_empty() {
        list.push(primary_monitor());
    }
    list
}

/// finestra + WebView dietro le icone, sopra il monitor `rect`
unsafe fn make_screen(hinst: HINSTANCE, rect: RECT) -> Option<Screen> {
    let hwnd = CreateWindowExW(
        WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
        w!("AsciiZooWallpaper"),
        w!("ASCII Zoo"),
        WS_POPUP,
        rect.left,
        rect.top,
        rect.right - rect.left,
        rect.bottom - rect.top,
        None,
        None,
        Some(hinst),
        None,
    )
    .ok()?;
    let Some(embed) = desktop::attach(hwnd) else {
        let _ = DestroyWindow(hwnd);
        return None;
    };
    desktop::fit(hwnd, embed.parent, rect);

    let built = WebViewBuilder::new()
        .with_custom_protocol("zoo".into(), |_id, _req| {
            Response::builder()
                .header("Content-Type", "text/html; charset=utf-8")
                .body(Cow::Borrowed(HTML.as_bytes()))
                .unwrap()
        })
        .with_url(URL)
        .with_background_color((9, 10, 15, 255))
        .with_devtools(false)
        .with_on_page_load_handler(|ev, _url| {
            if let PageLoadEvent::Finished = ev {
                PAGE_READY.store(true, Ordering::SeqCst);
            }
        })
        .build(&Host(hwnd));
    match built {
        Ok(webview) => Some(Screen { webview, hwnd, parent: embed.parent, rect, paused: false }),
        Err(_) => {
            let _ = DestroyWindow(hwnd);
            None
        }
    }
}

fn make_screens(hinst: HINSTANCE, rects: &[RECT]) -> Vec<Screen> {
    rects.iter().filter_map(|r| unsafe { make_screen(hinst, *r) }).collect()
}

fn drop_screens(screens: &mut Vec<Screen>) {
    for s in screens.drain(..) {
        let hwnd = s.hwnd;
        drop(s); // prima la WebView, poi la sua finestra
        unsafe {
            let _ = DestroyWindow(hwnd);
        }
    }
}

/// la finestra nascosta di controllo: timer, cambi di schermo, uscita
unsafe extern "system" fn control_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            APP.with(|a| {
                if let Ok(mut a) = a.try_borrow_mut() {
                    if let Some(app) = a.as_mut() {
                        app.tick();
                    }
                }
            });
            LRESULT(0)
        }
        WM_DISPLAYCHANGE => {
            REBUILD.store(true, Ordering::SeqCst);
            LRESULT(0)
        }
        WM_DESTROY => {
            PostQuitMessage(0);
            LRESULT(0)
        }
        m if m != 0 && m == TASKBAR_CREATED.load(Ordering::SeqCst) => {
            // Explorer è ripartito: il desktop è nuovo, gli sfondi vanno riagganciati
            REBUILD.store(true, Ordering::SeqCst);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// le finestre degli sfondi non fanno niente di speciale
unsafe extern "system" fn screen_proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    DefWindowProcW(hwnd, msg, wp, lp)
}

/// icona dell'area di notifica: una piccola koi arancione e bianca
fn tray_icon_image() -> Icon {
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
fn etichetta_nome(label: &str, nome: &str) -> String {
    // nei menu di Windows "&" sottolinea la lettera dopo: va raddoppiata
    let nome = if nome.is_empty() { "(nessuno)".to_string() } else { nome.replace('&', "&&") };
    format!("{label}: {nome}…")
}

fn build_menu(cfg: &Config) -> (Menu, Menus) {
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

    let m = Menus {
        animali,
        rot_tempi,
        rot_specie,
        nomi,
        koi: CheckMenuItem::new("Koi nello stagno in carica (con \"A rotazione\")", true, cfg.koi_in_carica, None),
        batteria: CheckMenuItem::new("Pausa a batteria", true, cfg.pausa_batteria, None),
        schermo: CheckMenuItem::new("Pausa con app a schermo intero", true, cfg.pausa_schermo_intero, None),
        avvio: CheckMenuItem::new("Avvia con Windows", true, autostart::enabled(), None),
        ricarica: MenuItem::new("Ricarica sfondo", true, None),
        esci: MenuItem::new("Esci", true, None),
    };
    let _ = menu.append(&sub);
    let _ = menu.append(&rot);
    let _ = menu.append(&nomi_sub);
    let _ = menu.append(&m.koi);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&m.batteria);
    let _ = menu.append(&m.schermo);
    let _ = menu.append(&m.avvio);
    let _ = menu.append(&PredefinedMenuItem::separator());
    let _ = menu.append(&m.ricarica);
    let _ = menu.append(&m.esci);
    (menu, m)
}

/// ridisegna lo sfondo normale di Windows quando il programma si chiude
fn restore_wallpaper() {
    unsafe {
        let mut buf = [0u16; 520];
        if SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr().cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)).is_ok() {
            let _ = SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(buf.as_mut_ptr().cast()), SPIF_UPDATEINIFILE);
        }
    }
}

fn main() {
    unsafe {
        // una sola copia alla volta
        let _mutex = CreateMutexW(None, true, w!("AsciiZoo.SingleInstance"));
        if GetLastError() == ERROR_ALREADY_EXISTS {
            return;
        }
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let hinst: HINSTANCE = GetModuleHandleW(None).expect("modulo").into();
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(control_proc),
            hInstance: hinst,
            lpszClassName: w!("AsciiZooControl"),
            ..Default::default()
        });
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(screen_proc),
            hInstance: hinst,
            lpszClassName: w!("AsciiZooWallpaper"),
            hCursor: cursor,
            ..Default::default()
        });

        // finestra di primo livello mai mostrata: riceve WM_DISPLAYCHANGE e TaskbarCreated,
        // che le finestre figlie del desktop non ricevono
        let ctl = CreateWindowExW(
            WS_EX_TOOLWINDOW,
            w!("AsciiZooControl"),
            w!("ASCII Zoo"),
            WS_POPUP,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(hinst),
            None,
        )
        .expect("finestra");
        TASKBAR_CREATED.store(RegisterWindowMessageW(w!("TaskbarCreated")), Ordering::SeqCst);

        let screens = make_screens(hinst, &monitors());
        if screens.is_empty() {
            MessageBoxW(None, w!("Non riesco a trovare il desktop di Windows."), w!("ASCII Zoo"), MB_ICONERROR);
            return;
        }

        let cfg = Config::load();
        let (menu, menus) = build_menu(&cfg);
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ASCII Zoo")
            .with_icon(tray_icon_image())
            .build()
            .expect("icona di notifica");

        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                hinst,
                ctl,
                screens,
                _tray: tray,
                menus,
                cfg,
                last_ac: None,
                ticks: 0,
                rebuild_at: None,
            })
        });

        SetTimer(Some(ctl), TIMER, 250, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if let Some(mut app) = APP.with(|a| a.borrow_mut().take()) {
            drop_screens(&mut app.screens);
        }
        restore_wallpaper();
    }
}
