//! Windows.
//!
//! - per ogni monitor una finestra con dentro WebView2 viene messa dietro le icone del desktop
//! - una finestra nascosta di controllo tiene il timer e si accorge dei cambi di schermo
//!   (monitor collegati/scollegati, risoluzione, riavvio di Explorer) e rifà gli sfondi
//! - un'icona nell'area di notifica ha tutte le opzioni
//! - pausa automatica a batteria e, schermo per schermo, con app a schermo intero

use std::cell::RefCell;
use std::num::NonZeroIsize;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use raw_window_handle::{HandleError, HasWindowHandle, RawWindowHandle, Win32WindowHandle, WindowHandle};
use tray_icon::menu::MenuEvent;
use tray_icon::{TrayIcon, TrayIconBuilder};
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
use wry::{PageLoadEvent, WebView, WebViewBuilder};

use crate::shared::{self, Core, Esito, JS_START, JS_STOP, URL};
use crate::{desktop, power};

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
    core: Core,
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

    /// manda allo sfondo tutte le impostazioni (funzione zooSetting della pagina)
    fn push_settings(&mut self) {
        let ac = power::on_ac();
        self.last_ac = Some(ac);
        for code in self.core.js_tutto(ac) {
            self.js(&code);
        }
        for s in &self.screens {
            if s.paused {
                s.js(JS_STOP);
            }
        }
    }

    fn check_power(&mut self) {
        let ac = power::on_ac();
        if self.last_ac != Some(ac) {
            self.last_ac = Some(ac);
            self.js(&shared::js_in_carica(ac));
        }
        let cfg = &self.core.cfg;
        let battery = cfg.pausa_batteria && !ac;
        let full = if cfg.pausa_schermo_intero { power::fullscreen_monitor() } else { None };
        for s in &mut self.screens {
            let pause = battery || full == Some(s.rect);
            if pause != s.paused {
                s.paused = pause;
                s.js(if pause { JS_STOP } else { JS_START });
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
        match self.core.on_menu(id) {
            Esito::Cambiato(js) => {
                if let Some(code) = js {
                    self.js(&code);
                }
                self.check_power();
            }
            Esito::Ricarica => {
                for s in &mut self.screens {
                    s.paused = false;
                    let _ = s.webview.load_url(URL);
                }
            }
            Esito::Esci => unsafe {
                let _ = DestroyWindow(self.ctl);
            },
            Esito::Niente => {}
        }
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
        .with_custom_protocol("zoo".into(), |_id, _req| shared::pagina())
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

/// ridisegna lo sfondo normale di Windows quando il programma si chiude
fn restore_wallpaper() {
    unsafe {
        let mut buf = [0u16; 520];
        if SystemParametersInfoW(SPI_GETDESKWALLPAPER, buf.len() as u32, Some(buf.as_mut_ptr().cast()), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0)).is_ok() {
            let _ = SystemParametersInfoW(SPI_SETDESKWALLPAPER, 0, Some(buf.as_mut_ptr().cast()), SPIF_UPDATEINIFILE);
        }
    }
}

pub fn run() {
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

        let (core, menu) = Core::new(true);
        let tray = TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("ASCII Zoo")
            .with_icon(shared::tray_icon_image())
            .build()
            .expect("icona di notifica");

        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                hinst,
                ctl,
                screens,
                _tray: tray,
                core,
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
