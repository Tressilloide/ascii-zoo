//! Windows: finestre del salvaschermo (una per monitor), anteprima nella
//! finestrella delle impostazioni di Windows, finestra delle impostazioni,
//! installazione.

use std::cell::RefCell;
use std::time::{Duration, Instant};

use windows::core::{w, BOOL, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateFontIndirectW, DeleteObject, EnumDisplayMonitors, GetDC, GetMonitorInfoW, GetSysColorBrush, ReleaseDC,
    SetDIBitsToDevice, ValidateRect, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, COLOR_BTNFACE, DIB_RGB_COLORS, HBRUSH, HDC, HFONT,
    HMONITOR, MONITORINFO, GetStockObject, BLACK_BRUSH,
};
use windows::Win32::Media::{timeBeginPeriod, timeEndPeriod};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForSystem, SetProcessDpiAwarenessContext, DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2};
use windows::Win32::UI::Input::KeyboardAndMouse::VK_ESCAPE;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::{Config, MINUTI, RIGHE};
use crate::painter::Pittore;
use crate::director::Regia;
use crate::scenes::SCENE;

#[derive(Clone, Copy, PartialEq)]
pub enum Modo {
    SchermoIntero,
    Anteprima(isize),
    Finestra,
}

struct Schermo {
    hwnd: HWND,
    w: usize,
    h: usize,
    buf: Vec<u32>,
    pittore: Pittore,
    regia: Regia,
}

struct Stato {
    modo: Modo,
    mouse: Option<POINT>,
    inizio: Instant,
    uscire: bool,
    /// finestra ridimensionata: (hwnd, larghezza, altezza)
    nuove_misure: Option<(HWND, usize, usize)>,
}

thread_local! {
    static STATO: RefCell<Option<Stato>> = const { RefCell::new(None) };
}

fn esci() {
    STATO.with(|s| {
        if let Some(s) = s.borrow_mut().as_mut() {
            s.uscire = true;
        }
    });
}

fn con_stato<R>(f: impl FnOnce(&mut Stato) -> R) -> Option<R> {
    STATO.with(|s| s.try_borrow_mut().ok().and_then(|mut b| b.as_mut().map(f)))
}

unsafe extern "system" fn proc_schermo(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    let modo = con_stato(|s| s.modo);
    let pronto = con_stato(|s| s.inizio.elapsed() > Duration::from_millis(600)).unwrap_or(false);
    match (msg, modo) {
        (WM_PAINT, _) => {
            let _ = ValidateRect(Some(hwnd), None);
            LRESULT(0)
        }
        (WM_ERASEBKGND, _) => LRESULT(1),
        (WM_SETCURSOR, Some(Modo::SchermoIntero)) => {
            SetCursor(None);
            LRESULT(1)
        }
        (WM_MOUSEMOVE, Some(Modo::SchermoIntero)) => {
            let mut p = POINT::default();
            let _ = GetCursorPos(&mut p);
            let mossa = con_stato(|s| match s.mouse {
                None => {
                    s.mouse = Some(p);
                    false
                }
                Some(m) => (p.x - m.x).abs() + (p.y - m.y).abs() > 12,
            })
            .unwrap_or(false);
            if mossa && pronto {
                esci();
            }
            LRESULT(0)
        }
        (WM_KEYDOWN | WM_SYSKEYDOWN | WM_LBUTTONDOWN | WM_RBUTTONDOWN | WM_MBUTTONDOWN | WM_XBUTTONDOWN | WM_MOUSEWHEEL, Some(Modo::SchermoIntero)) => {
            if pronto {
                esci();
            }
            LRESULT(0)
        }
        (WM_ACTIVATEAPP, Some(Modo::SchermoIntero)) => {
            if wp.0 == 0 && pronto {
                esci();
            }
            LRESULT(0)
        }
        (WM_SYSCOMMAND, Some(Modo::SchermoIntero)) if (wp.0 & 0xFFF0) == 0xF140 || (wp.0 & 0xFFF0) == 0xF060 => LRESULT(0),
        (WM_KEYDOWN, Some(Modo::Finestra)) if wp.0 == VK_ESCAPE.0 as usize => {
            esci();
            LRESULT(0)
        }
        (WM_SIZE, Some(Modo::Finestra)) => {
            let (w, h) = ((lp.0 & 0xFFFF) as usize, ((lp.0 >> 16) & 0xFFFF) as usize);
            if w > 0 && h > 0 {
                con_stato(|s| s.nuove_misure = Some((hwnd, w, h)));
            }
            LRESULT(0)
        }
        (WM_CLOSE, _) => {
            esci();
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

/// rettangoli (in pixel fisici) di tutti i monitor
fn monitor() -> Vec<RECT> {
    unsafe extern "system" fn uno(m: HMONITOR, _: HDC, _: *mut RECT, out: LPARAM) -> BOOL {
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        if GetMonitorInfoW(m, &mut mi).as_bool() {
            (*(out.0 as *mut Vec<RECT>)).push(mi.rcMonitor);
        }
        BOOL(1)
    }
    let mut v: Vec<RECT> = Vec::new();
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(uno), LPARAM(&mut v as *mut _ as isize));
    }
    v
}

fn nuovo_schermo(hwnd: HWND, w: usize, h: usize, cfg: &Config, righe: u32, seme: u64) -> Schermo {
    let pittore = Pittore::new(h / righe.max(1) as usize);
    let (gw, gh) = pittore.misura(w, h);
    Schermo { hwnd, w, h, buf: vec![0; w * h], regia: Regia::new(gw, gh, cfg, seme), pittore }
}

fn mostra(s: &Schermo) {
    unsafe {
        let bmi = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: s.w as i32,
                biHeight: -(s.h as i32), // dall'alto in basso
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let dc = GetDC(Some(s.hwnd));
        SetDIBitsToDevice(dc, 0, 0, s.w as u32, s.h as u32, 0, 0, 0, s.h as u32, s.buf.as_ptr() as *const _, &bmi, DIB_RGB_COLORS);
        ReleaseDC(Some(s.hwnd), dc);
    }
}

pub fn avvia(modo: Modo, cfg: &Config) {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let hinst = GetModuleHandleW(None).expect("modulo");
        let classe = w!("AsciiZooSalvaschermo");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc_schermo),
            hInstance: hinst.into(),
            lpszClassName: classe,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        });
        STATO.with(|s| {
            *s.borrow_mut() = Some(Stato { modo, mouse: None, inizio: Instant::now(), uscire: false, nuove_misure: None })
        });

        let seme = crate::seme();
        let mut schermi: Vec<Schermo> = Vec::new();
        let mut padre = None;
        match modo {
            Modo::SchermoIntero => {
                for (i, r) in monitor().iter().enumerate() {
                    let (w, h) = ((r.right - r.left) as usize, (r.bottom - r.top) as usize);
                    let Ok(hwnd) = CreateWindowExW(
                        WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
                        classe,
                        w!("ASCII Zoo screensaver"),
                        WS_POPUP | WS_VISIBLE,
                        r.left,
                        r.top,
                        w as i32,
                        h as i32,
                        None,
                        None,
                        Some(hinst.into()),
                        None,
                    ) else {
                        continue;
                    };
                    schermi.push(nuovo_schermo(hwnd, w, h, cfg, cfg.righe, seme.wrapping_add(i as u64 * 7919)));
                }
                ShowCursor(false);
            }
            Modo::Anteprima(p) => {
                let p = HWND(p as *mut _);
                let mut r = RECT::default();
                let _ = GetClientRect(p, &mut r);
                let (w, h) = ((r.right - r.left).max(1) as usize, (r.bottom - r.top).max(1) as usize);
                if let Ok(hwnd) = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    classe,
                    w!("ASCII Zoo screensaver"),
                    WS_CHILD | WS_VISIBLE,
                    0,
                    0,
                    w as i32,
                    h as i32,
                    Some(p),
                    None,
                    Some(hinst.into()),
                    None,
                ) {
                    let mut c = cfg.clone();
                    c.orologio = false;
                    schermi.push(nuovo_schermo(hwnd, w, h, &c, 18, seme));
                }
                padre = Some(p);
            }
            Modo::Finestra => {
                let dpi = GetDpiForSystem().max(96) as i32;
                let (cw, ch) = (1280 * dpi / 96, 720 * dpi / 96);
                let mut r = RECT { left: 0, top: 0, right: cw, bottom: ch };
                let _ = AdjustWindowRectEx(&mut r, WS_OVERLAPPEDWINDOW, false, WINDOW_EX_STYLE(0));
                if let Ok(hwnd) = CreateWindowExW(
                    WINDOW_EX_STYLE(0),
                    classe,
                    w!("ASCII Zoo screensaver (Esc to close)"),
                    WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                    CW_USEDEFAULT,
                    CW_USEDEFAULT,
                    r.right - r.left,
                    r.bottom - r.top,
                    None,
                    None,
                    Some(hinst.into()),
                    None,
                ) {
                    let mut cr = RECT::default();
                    let _ = GetClientRect(hwnd, &mut cr);
                    let (w, h) = ((cr.right - cr.left).max(1) as usize, (cr.bottom - cr.top).max(1) as usize);
                    schermi.push(nuovo_schermo(hwnd, w, h, cfg, cfg.righe, seme));
                }
            }
        }
        if schermi.is_empty() {
            return;
        }

        // ciclo: messaggi, poi un fotogramma per schermo, a 30 fotogrammi al secondo
        // (15 a batteria: le scene sono lente e si dimezza il consumo)
        let _ = timeBeginPeriod(1);
        let mut quadro = Duration::from_secs_f64(1.0 / 30.0);
        let mut prossimo_controllo = Instant::now();
        let mut prima = Instant::now();
        let mut msg = MSG::default();
        'giro: loop {
            if Instant::now() >= prossimo_controllo {
                prossimo_controllo = Instant::now() + Duration::from_secs(2);
                quadro = Duration::from_secs_f64(if a_batteria() { 1.0 / 15.0 } else { 1.0 / 30.0 });
            }
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                if msg.message == WM_QUIT {
                    break 'giro;
                }
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            if con_stato(|s| s.uscire).unwrap_or(true) {
                break;
            }
            if let Some(p) = padre {
                if !IsWindow(Some(p)).as_bool() {
                    break;
                }
            }
            if let Some((hwnd, w, h)) = con_stato(|s| s.nuove_misure.take()).flatten() {
                if let Some(s) = schermi.iter_mut().find(|s| s.hwnd == hwnd) {
                    if (w, h) != (s.w, s.h) {
                        *s = nuovo_schermo(hwnd, w, h, cfg, cfg.righe, seme);
                    }
                }
            }
            let adesso = Instant::now();
            let dt = (adesso - prima).as_secs_f32();
            prima = adesso;
            let ora = crate::ora_adesso();
            for s in schermi.iter_mut() {
                if IsIconic(s.hwnd).as_bool() {
                    continue;
                }
                let luce = s.regia.passo(dt, ora);
                s.pittore.dipingi(&s.regia.griglia, &mut s.buf, s.w, s.h, luce);
                mostra(s);
            }
            let speso = adesso.elapsed();
            if speso < quadro {
                std::thread::sleep(quadro - speso);
            }
        }
        let _ = timeEndPeriod(1);
        for s in &schermi {
            let _ = DestroyWindow(s.hwnd);
        }
        if modo == Modo::SchermoIntero {
            ShowCursor(true);
        }
    }
}

// ---------------------------------------------------------------- impostazioni

const ID_OK: usize = 1;
const ID_ANNULLA: usize = 2;
const ID_PROVA: usize = 10;
const ID_SCENA: usize = 20;
const ID_MINUTI: usize = 21;
const ID_RIGHE: usize = 22;
const ID_OROLOGIO: usize = 23;
const CBS_DROPDOWNLIST: u32 = 0x0003;
const BS_AUTOCHECKBOX: u32 = 0x0003;
const BS_DEFPUSHBUTTON: u32 = 0x0001;
const CB_ADDSTRING: u32 = 0x0143;
const CB_SETCURSEL: u32 = 0x014E;
const CB_GETCURSEL: u32 = 0x0147;
const BM_SETCHECK: u32 = 0x00F1;
const BM_GETCHECK: u32 = 0x00F0;

thread_local! {
    static DIALOGO: RefCell<Option<(HWND, Config)>> = const { RefCell::new(None) };
}

/// legge i controlli della finestra nelle impostazioni
fn leggi(dlg: HWND, base: &Config) -> Config {
    unsafe {
        let sel = |id: usize| SendMessageW(GetDlgItem(Some(dlg), id as i32).unwrap_or_default(), CB_GETCURSEL, None, None).0.max(0) as usize;
        let mut c = base.clone();
        let s = sel(ID_SCENA);
        c.scena = if s == 0 { "rotazione".into() } else { SCENE.get(s - 1).map(|x| x.0).unwrap_or("rotazione").into() };
        c.minuti = MINUTI.get(sel(ID_MINUTI)).copied().unwrap_or(3);
        c.righe = RIGHE.get(sel(ID_RIGHE)).map(|r| r.0).unwrap_or(48);
        c.orologio = SendMessageW(GetDlgItem(Some(dlg), ID_OROLOGIO as i32).unwrap_or_default(), BM_GETCHECK, None, None).0 == 1;
        c
    }
}

unsafe extern "system" fn proc_impostazioni(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = wp.0 & 0xFFFF;
            let base = DIALOGO.with(|d| d.borrow().as_ref().map(|x| x.1.clone())).unwrap_or_default();
            match id {
                ID_OK => {
                    let c = leggi(hwnd, &base);
                    if let Err(e) = c.salva() {
                        MessageBoxW(Some(hwnd), &HSTRING::from(format!("Could not save the settings: {e}")), w!("ASCII Zoo screensaver"), MB_ICONWARNING);
                    }
                    let _ = DestroyWindow(hwnd);
                }
                ID_ANNULLA => {
                    let _ = DestroyWindow(hwnd);
                }
                ID_PROVA => {
                    // prova con le scelte non ancora salvate
                    let c = leggi(hwnd, &base);
                    if let Ok(exe) = std::env::current_exe() {
                        let _ = std::process::Command::new(exe)
                            .args(["/s", "--scena", &c.scena, "--minuti", &c.minuti.to_string(), "--righe", &c.righe.to_string()])
                            .args(if c.orologio { None } else { Some("--senza-orologio") })
                            .spawn();
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            LRESULT(0)
        }
        _ => DefWindowProcW(hwnd, msg, wp, lp),
    }
}

pub fn impostazioni(cfg: Config) {
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let Ok(hinst) = GetModuleHandleW(None) else { return };
        let classe = w!("AsciiZooSalvaschermoImpostazioni");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc_impostazioni),
            hInstance: hinst.into(),
            lpszClassName: classe,
            hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        });
        let dpi = GetDpiForSystem().max(96) as i32;
        let px = |v: i32| v * dpi / 96;
        let (cw, ch) = (px(400), px(232));
        let style = WS_POPUP | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let ex = WS_EX_DLGMODALFRAME;
        let mut r = RECT { left: 0, top: 0, right: cw, bottom: ch };
        let _ = AdjustWindowRectEx(&mut r, style, false, ex);
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        let mut area = RECT::default();
        let _ = SystemParametersInfoW(SPI_GETWORKAREA, 0, Some(&mut area as *mut RECT as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        let x = area.left + (area.right - area.left - ww) / 2;
        let y = area.top + (area.bottom - area.top - wh) / 2;
        let Ok(dlg) = CreateWindowExW(ex, classe, w!("ASCII Zoo screensaver"), style, x, y, ww, wh, None, None, Some(hinst.into()), None) else {
            return;
        };
        DIALOGO.with(|d| *d.borrow_mut() = Some((dlg, cfg.clone())));

        let mut ncm = NONCLIENTMETRICSW { cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
        let _ = SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, ncm.cbSize, Some(&mut ncm as *mut _ as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        let font: HFONT = CreateFontIndirectW(&ncm.lfMessageFont);
        #[allow(clippy::too_many_arguments)]
        let figlio = |cls: PCWSTR, testo: &str, st: u32, x: i32, y: i32, w: i32, h: i32, id: usize| -> HWND {
            let h = CreateWindowExW(
                WINDOW_EX_STYLE(0),
                cls,
                &HSTRING::from(testo),
                WS_CHILD | WS_VISIBLE | WINDOW_STYLE(st),
                x,
                y,
                w,
                h,
                Some(dlg),
                Some(HMENU(id as *mut _)),
                Some(hinst.into()),
                None,
            )
            .unwrap_or_default();
            SendMessageW(h, WM_SETFONT, Some(WPARAM(font.0 as usize)), Some(LPARAM(1)));
            h
        };
        let aggiungi = |cb: HWND, t: &str| {
            let w: Vec<u16> = t.encode_utf16().chain(Some(0)).collect();
            SendMessageW(cb, CB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
        };

        let m = px(16);
        let (lw, cx, cbw) = (px(130), m + px(134), cw - m - (m + px(134)));
        let riga = |n: i32| px(16) + n * px(36);
        figlio(w!("STATIC"), "Scene", 0, m, riga(0) + px(4), lw, px(20), 0);
        let cb = figlio(w!("COMBOBOX"), "", CBS_DROPDOWNLIST | WS_VSCROLL.0 | WS_TABSTOP.0, cx, riga(0), cbw, px(240), ID_SCENA);
        aggiungi(cb, "All, one after another");
        for s in SCENE {
            aggiungi(cb, s.1);
        }
        let sel = SCENE.iter().position(|s| s.0 == cfg.scena).map(|i| i + 1).unwrap_or(0);
        SendMessageW(cb, CB_SETCURSEL, Some(WPARAM(sel)), None);

        figlio(w!("STATIC"), "Change scene every", 0, m, riga(1) + px(4), lw, px(20), 0);
        let cb = figlio(w!("COMBOBOX"), "", CBS_DROPDOWNLIST | WS_VSCROLL.0 | WS_TABSTOP.0, cx, riga(1), cbw, px(200), ID_MINUTI);
        for n in MINUTI {
            aggiungi(cb, &if *n == 1 { "1 minute".to_string() } else { format!("{n} minutes") });
        }
        let sel = MINUTI.iter().position(|&n| n == cfg.minuti).unwrap_or(1);
        SendMessageW(cb, CB_SETCURSEL, Some(WPARAM(sel)), None);

        figlio(w!("STATIC"), "Character size", 0, m, riga(2) + px(4), lw, px(20), 0);
        let cb = figlio(w!("COMBOBOX"), "", CBS_DROPDOWNLIST | WS_VSCROLL.0 | WS_TABSTOP.0, cx, riga(2), cbw, px(200), ID_RIGHE);
        for r in RIGHE {
            aggiungi(cb, r.1);
        }
        let sel = RIGHE.iter().position(|r| r.0 == cfg.righe).unwrap_or(1);
        SendMessageW(cb, CB_SETCURSEL, Some(WPARAM(sel)), None);

        let ck = figlio(w!("BUTTON"), "Show the clock", BS_AUTOCHECKBOX | WS_TABSTOP.0, cx, riga(3), cbw, px(24), ID_OROLOGIO);
        SendMessageW(ck, BM_SETCHECK, Some(WPARAM(cfg.orologio as usize)), None);

        let (bw, bh) = (px(92), px(28));
        let by = ch - m - bh;
        figlio(w!("BUTTON"), "Preview", WS_TABSTOP.0, m, by, bw, bh, ID_PROVA);
        figlio(w!("BUTTON"), "OK", WS_TABSTOP.0 | BS_DEFPUSHBUTTON, cw - m - 2 * bw - px(8), by, bw, bh, ID_OK);
        figlio(w!("BUTTON"), "Cancel", WS_TABSTOP.0, cw - m - bw, by, bw, bh, ID_ANNULLA);

        let _ = ShowWindow(dlg, SW_SHOW);
        let _ = SetForegroundWindow(dlg);
        let mut msg = MSG::default();
        while IsWindow(Some(dlg)).as_bool() {
            let got = GetMessageW(&mut msg, None, 0, 0).0;
            if got <= 0 {
                break;
            }
            if !IsDialogMessageW(dlg, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let _ = DeleteObject(font.into());
    }
}

/// dove sta lo screensaver installato: %LOCALAPPDATA%\AsciiZoo\ascii-zoo.scr
fn file_scr() -> Option<std::path::PathBuf> {
    let base = std::env::var_os("LOCALAPPDATA")?;
    Some(std::path::PathBuf::from(base).join("AsciiZoo").join("ascii-zoo.scr"))
}

/// Se ascii-zoo.scr esiste ed è diverso da questo programma, lo sostituisce.
/// Se Windows lo sta usando proprio adesso la copia fallisce: si riprova al prossimo avvio.
pub fn aggiorna_se_installato() {
    let (Some(dest), Ok(exe)) = (file_scr(), std::env::current_exe()) else {
        return;
    };
    if exe == dest || !dest.exists() {
        return;
    }
    let uguali = match (std::fs::read(&exe), std::fs::read(&dest)) {
        (Ok(a), Ok(b)) => a == b,
        _ => true,
    };
    if !uguali {
        let _ = std::fs::copy(&exe, &dest);
    }
}

/// copia l'eseguibile come .scr nella cartella dell'utente e apre le impostazioni
/// del salvaschermo di Windows con ASCII Zoo già scelto (senza diritti di amministratore)
pub fn installa() {
    let esito = (|| -> Result<std::path::PathBuf, String> {
        let dest = file_scr().ok_or("LOCALAPPDATA mancante")?;
        if let Some(dir) = dest.parent() {
            std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        if exe != dest {
            std::fs::copy(&exe, &dest).map_err(|e| e.to_string())?;
        }
        std::process::Command::new("rundll32.exe")
            .arg(format!("desk.cpl,InstallScreenSaver {}", dest.display()))
            .spawn()
            .map_err(|e| e.to_string())?;
        Ok(dest)
    })();
    if let Err(e) = esito {
        unsafe {
            MessageBoxW(None, &HSTRING::from(format!("Installation failed: {e}")), w!("ASCII Zoo screensaver"), MB_ICONWARNING);
        }
    }
}

/// true se il PC va a batteria (senza caricatore)
fn a_batteria() -> bool {
    use windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
    let mut st = SYSTEM_POWER_STATUS::default();
    unsafe { GetSystemPowerStatus(&mut st).is_ok() && st.ACLineStatus == 0 }
}
