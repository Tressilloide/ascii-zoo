//! Finestrella per scrivere il nome di un animale: il menu dell'area di
//! notifica non può contenere un campo di testo.
//!
//! Invio = OK, Esc o la X = annulla (il nome resta com'era).

use std::cell::{Cell, RefCell};

use windows::core::{w, HSTRING, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{CreateFontIndirectW, DeleteObject, GetSysColorBrush, COLOR_BTNFACE, HFONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::GetDpiForSystem;
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::*;

const ID_OK: usize = 1; // IDOK: anche il tasto Invio
const ID_ANNULLA: usize = 2; // IDCANCEL: anche il tasto Esc
const ES_AUTOHSCROLL: u32 = 0x0080;
const BS_DEFPUSHBUTTON: u32 = 0x0001;
const EM_LIMITTEXT: u32 = 0x00C5;
const EM_SETSEL: u32 = 0x00B1;

thread_local! {
    static EDIT: Cell<HWND> = const { Cell::new(HWND(std::ptr::null_mut())) };
    static RISPOSTA: RefCell<Option<String>> = const { RefCell::new(None) };
}

unsafe extern "system" fn proc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = wp.0 & 0xFFFF;
            if id == ID_OK {
                let edit = EDIT.with(|e| e.get());
                let n = GetWindowTextLengthW(edit).max(0) as usize;
                let mut buf = vec![0u16; n + 1];
                let len = GetWindowTextW(edit, &mut buf).max(0) as usize;
                RISPOSTA.with(|r| *r.borrow_mut() = Some(String::from_utf16_lossy(&buf[..len])));
                let _ = DestroyWindow(hwnd);
            } else if id == ID_ANNULLA {
                let _ = DestroyWindow(hwnd);
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

/// Chiede un nome. `None` se l'utente annulla (Annulla, Esc, X).
pub fn chiedi_nome(titolo: &str, domanda: &str, attuale: &str, max: usize) -> Option<String> {
    unsafe {
        let hinst = GetModuleHandleW(None).ok()?;
        let class = w!("AsciiZooNome");
        RegisterClassW(&WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: hinst.into(),
            lpszClassName: class,
            hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            ..Default::default()
        }); // la seconda volta è già registrata: va bene così
        RISPOSTA.with(|r| *r.borrow_mut() = None);

        let dpi = GetDpiForSystem().max(96) as i32;
        let px = |v: i32| v * dpi / 96;
        let (cw, ch) = (px(380), px(128));
        let style = WS_POPUP | WS_CAPTION | WS_SYSMENU;
        let ex = WS_EX_DLGMODALFRAME | WS_EX_TOPMOST;
        let mut r = RECT { left: 0, top: 0, right: cw, bottom: ch };
        let _ = AdjustWindowRectEx(&mut r, style, false, ex);
        let (ww, wh) = (r.right - r.left, r.bottom - r.top);
        // al centro dell'area di lavoro dello schermo principale
        let mut work = RECT::default();
        let _ = SystemParametersInfoW(SPI_GETWORKAREA, 0, Some(&mut work as *mut RECT as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        let x = work.left + (work.right - work.left - ww) / 2;
        let y = work.top + (work.bottom - work.top - wh) / 2;
        let dlg = CreateWindowExW(ex, class, &HSTRING::from(titolo), style, x, y, ww, wh, None, None, Some(hinst.into()), None).ok()?;

        // il carattere dei messaggi di sistema (Segoe UI)
        let mut ncm = NONCLIENTMETRICSW { cbSize: std::mem::size_of::<NONCLIENTMETRICSW>() as u32, ..Default::default() };
        let _ = SystemParametersInfoW(SPI_GETNONCLIENTMETRICS, ncm.cbSize, Some(&mut ncm as *mut NONCLIENTMETRICSW as *mut _), SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0));
        let font: HFONT = CreateFontIndirectW(&ncm.lfMessageFont);

        let child = |cls: PCWSTR, text: &str, st: u32, exs: WINDOW_EX_STYLE, x: i32, y: i32, w: i32, h: i32, id: usize| -> HWND {
            let h = CreateWindowExW(
                exs,
                cls,
                &HSTRING::from(text),
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
        let m = px(14);
        let (bw, bh) = (px(88), px(28));
        child(w!("STATIC"), domanda, 0, WINDOW_EX_STYLE(0), m, px(12), cw - 2 * m, px(20), 0);
        let edit = child(w!("EDIT"), attuale, WS_TABSTOP.0 | ES_AUTOHSCROLL, WS_EX_CLIENTEDGE, m, px(38), cw - 2 * m, px(26), 100);
        child(w!("BUTTON"), "OK", WS_TABSTOP.0 | BS_DEFPUSHBUTTON, WINDOW_EX_STYLE(0), cw - m - 2 * bw - px(8), ch - m - bh, bw, bh, ID_OK);
        child(w!("BUTTON"), "Annulla", WS_TABSTOP.0, WINDOW_EX_STYLE(0), cw - m - bw, ch - m - bh, bw, bh, ID_ANNULLA);

        SendMessageW(edit, EM_LIMITTEXT, Some(WPARAM(max)), Some(LPARAM(0)));
        SendMessageW(edit, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
        EDIT.with(|e| e.set(edit));

        let _ = ShowWindow(dlg, SW_SHOW);
        let _ = SetForegroundWindow(dlg);
        let _ = SetFocus(Some(edit));

        // ciclo dei messaggi della finestrella; gli sfondi continuano ad animarsi
        let mut msg = MSG::default();
        while IsWindow(Some(dlg)).as_bool() {
            let got = GetMessageW(&mut msg, None, 0, 0).0;
            if got == 0 {
                // è arrivato WM_QUIT (Esci): lo rimetto in coda per il ciclo principale
                PostQuitMessage(msg.wParam.0 as i32);
                let _ = DestroyWindow(dlg);
                break;
            }
            if got == -1 {
                break;
            }
            if !IsDialogMessageW(dlg, &msg).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        let _ = DeleteObject(font.into());
        RISPOSTA.with(|r| r.borrow_mut().take())
    }
}
