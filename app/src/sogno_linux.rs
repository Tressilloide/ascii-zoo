//! Linux: lo screensaver vero e proprio, un processo a parte (`ascii-zoo --salvaschermo`)
//! che l'app lancia quando non si usa il PC (vedi salvaschermo_linux.rs), come su Windows
//! il file .scr. Le scene sono quelle di dreams/ (il nome viene da lì): `ascii_dreams::Motore`
//! disegna ogni fotogramma in un buffer di pixel, che qui va in una superficie cairo e da
//! lì nella finestra GTK.
//!
//! - una finestra senza bordi a schermo intero per ogni monitor, "sempre in primo piano":
//!   GNOME può non darle il focus (prevenzione del furto del focus) e allora, senza, la
//!   metterebbe sotto la finestra attiva. Provato in GNOME Shell 46: così resta sopra e
//!   la barra in alto sparisce anche senza focus. In più la si attiva con l'ora del server X
//!   di adesso (vedi `porta_davanti`), e si riprova nel primo secondo e mezzo: in CI a volte
//!   GNOME la lasciava sotto, senza focus
//! - sotto Wayland passa da XWayland, come lo sfondo: Wayland non permette a un programma
//!   di stare "sempre in primo piano"
//! - nasconde il cursore ed esce al primo movimento del mouse (più di 12 pixel), tasto o
//!   clic, ma non nei primi 0,6 secondi (il movimento che ha appena chiuso un menu), come
//!   su Windows. I tasti arrivano solo se la finestra ha il focus: per questo l'app lo
//!   chiude anche da sé, quando GNOME le dice che qualcuno ha usato mouse o tastiera
//! - 30 fotogrammi al secondo, 15 a batteria
//!
//! Per provarlo: `ascii-zoo --salvaschermo` (schermo intero) oppure
//! `ascii-zoo --salvaschermo --finestra [--scena aurora]` (in una finestra, Esc chiude).

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::time::{Duration, Instant};

use ascii_dreams::{Config, Motore};
use gtk::prelude::*;
use gtk::{cairo, gdk, glib};

use crate::power;
use crate::shared::zoo_log;

/// Attiva la finestra e la mette in cima. GNOME attiva una finestra nuova solo con un'ora
/// più recente dell'ultimo input dell'utente (prevenzione del furto del focus); `present()`
/// di GTK non ne passa nessuna. L'ora del server X di adesso lo è sempre: lo screensaver
/// parte proprio perché da minuti non c'è input.
fn porta_davanti(w: &gtk::Window) {
    if let Some(x) = w.window().and_then(|gw| gw.downcast::<gdkx11::X11Window>().ok()) {
        w.present_with_time(gdkx11::functions::x11_get_server_time(&x));
    } else {
        w.present();
    }
}

/// all'inizio non si esce: il mouse che si muove ancora dopo aver chiuso un menu
const GRAZIA: Duration = Duration::from_millis(600);
/// pixel di movimento del mouse che fanno uscire (sotto, un tavolo che vibra)
const SOGLIA: f64 = 12.0;
/// ogni quanto si ricontrolla la batteria
const CONTROLLO: Duration = Duration::from_secs(2);

/// uno schermo: la sua area di disegno, il motore e la superficie dove si copia il fotogramma
struct Schermo {
    area: gtk::DrawingArea,
    /// si crea quando si conosce la misura vera dell'area (e si rifà se cambia)
    motore: Option<Motore>,
    superficie: Option<cairo::ImageSurface>,
    seme: u64,
}

struct Stato {
    schermi: RefCell<Vec<Schermo>>,
    cfg: Config,
    /// true a schermo intero, false in una finestra normale (per provarlo)
    intero: bool,
    inizio: Instant,
    /// dove era il mouse la prima volta che si è mosso
    mouse: Cell<Option<(f64, f64)>>,
    prima: Cell<Instant>,
    /// durata di un fotogramma: 1/30 di secondo, 1/15 a batteria
    quadro: Cell<Duration>,
    prossimo_controllo: Cell<Instant>,
    finito: Cell<bool>,
}

impl Stato {
    fn esci(&self, motivo: &str) {
        if !self.finito.replace(true) {
            zoo_log!("screensaver: esco ({motivo})");
            gtk::main_quit();
        }
    }

    fn pronto(&self) -> bool {
        self.inizio.elapsed() > GRAZIA
    }
}

fn valore(args: &[String], nome: &str) -> Option<String> {
    args.iter().position(|a| a == nome).and_then(|i| args.get(i + 1)).cloned()
}

/// `args`: quello che c'è dopo `--salvaschermo`
pub fn avvia(args: &[String]) {
    let intero = !args.iter().any(|a| a == "--finestra");
    // l'app, sotto Wayland, ha già messo GDK_BACKEND=x11 (lo si eredita); lanciato a mano no
    if std::env::var_os("WAYLAND_DISPLAY").is_some() && std::env::var_os("GDK_BACKEND").is_none() {
        std::env::set_var("GDK_BACKEND", "x11");
    }
    if gtk::init().is_err() {
        eprintln!("ascii-zoo: screensaver: nessuno schermo a cui collegarsi");
        std::process::exit(1);
    }
    let Some(display) = gdk::Display::default() else { return };

    // orologio e grandezza dei caratteri come su Windows (screensaver.json, se c'è); la
    // scena la sceglie il menu dell'app e arriva come argomento
    let mut cfg = Config::carica();
    if let Some(s) = valore(args, "--scena") {
        cfg.scena = s;
    }
    let st = Rc::new(Stato {
        schermi: RefCell::new(Vec::new()),
        cfg,
        intero,
        inizio: Instant::now(),
        mouse: Cell::new(None),
        prima: Cell::new(Instant::now()),
        quadro: Cell::new(Duration::from_secs_f64(1.0 / 30.0)),
        prossimo_controllo: Cell::new(Instant::now()),
        finito: Cell::new(false),
    });

    let seme = ascii_dreams::seme();
    if intero {
        let vuoto = gdk::Cursor::for_display(&display, gdk::CursorType::BlankCursor);
        for i in 0..display.n_monitors() {
            let Some(mon) = display.monitor(i) else { continue };
            let g = mon.geometry();
            let w = gtk::Window::new(gtk::WindowType::Toplevel);
            w.set_title("ASCII Zoo screensaver");
            w.set_decorated(false);
            w.set_skip_taskbar_hint(true);
            w.set_skip_pager_hint(true);
            w.set_keep_above(true);
            // posizione e misura del monitor: senza un gestore delle finestre (es. Xvfb)
            // la richiesta di schermo intero non la esegue nessuno
            w.move_(g.x(), g.y());
            w.set_default_size(g.width(), g.height());
            aggiungi_schermo(&st, &w, seme.wrapping_add(i as u64 * 7919));
            // a schermo intero già quando compare, non un attimo dopo
            w.fullscreen_on_monitor(&WidgetExt::screen(&w).unwrap_or_else(|| display.default_screen()), i);
            w.show_all();
            if let Some(gw) = w.window() {
                gw.set_cursor(vuoto.as_ref());
            }
            porta_davanti(&w);
            for ms in [300, 800, 1500] {
                let w = w.clone();
                glib::timeout_add_local_once(Duration::from_millis(ms), move || {
                    if !w.is_active() {
                        porta_davanti(&w);
                    }
                });
            }
            zoo_log!("screensaver: schermo {i}: {}×{} a {},{}", g.width(), g.height(), g.x(), g.y());
        }
    } else {
        let w = gtk::Window::new(gtk::WindowType::Toplevel);
        w.set_title("ASCII Zoo screensaver (Esc per chiudere)");
        w.set_default_size(1280, 720);
        aggiungi_schermo(&st, &w, seme);
        w.show_all();
    }
    if st.schermi.borrow().is_empty() {
        return;
    }
    let s = st.clone();
    glib::idle_add_local_once(move || fotogramma(s));
    gtk::main();
}

/// mette nella finestra l'area di disegno di uno schermo e collega mouse e tastiera
fn aggiungi_schermo(st: &Rc<Stato>, w: &gtk::Window, seme: u64) {
    let i = st.schermi.borrow().len();
    let area = gtk::DrawingArea::new();
    let s = st.clone();
    area.connect_draw(move |_, cr| {
        let schermi = s.schermi.try_borrow();
        match schermi.as_ref().ok().and_then(|v| v.get(i)).and_then(|x| x.superficie.as_ref()) {
            Some(sup) if cr.set_source_surface(sup, 0.0, 0.0).is_ok() => {
                let _ = cr.paint();
            }
            _ => {
                cr.set_source_rgb(0.0, 0.0, 0.0);
                let _ = cr.paint();
            }
        }
        // il contesto non tiene più la superficie: al prossimo fotogramma la si riscrive
        cr.set_source_rgb(0.0, 0.0, 0.0);
        glib::Propagation::Stop
    });
    w.add(&area);
    st.schermi.borrow_mut().push(Schermo { area, motore: None, superficie: None, seme });

    let s = st.clone();
    w.connect_delete_event(move |_, _| {
        s.esci("finestra chiusa");
        glib::Propagation::Stop
    });
    w.add_events(
        gdk::EventMask::POINTER_MOTION_MASK
            | gdk::EventMask::BUTTON_PRESS_MASK
            | gdk::EventMask::KEY_PRESS_MASK
            | gdk::EventMask::SCROLL_MASK,
    );
    let s = st.clone();
    w.connect_key_press_event(move |_, e| {
        if !s.intero {
            if e.keyval() == gdk::keys::constants::Escape {
                s.esci("Esc");
            }
        } else if s.pronto() {
            s.esci("tasto");
        }
        glib::Propagation::Stop
    });
    if !st.intero {
        return;
    }
    let s = st.clone();
    w.connect_motion_notify_event(move |_, e| {
        // il punto di partenza è dove si ferma il mouse alla fine della grazia
        let (x, y) = e.root();
        match s.mouse.get() {
            Some((x0, y0)) if s.pronto() => {
                if (x - x0).abs() + (y - y0).abs() > SOGLIA {
                    s.esci("mouse");
                }
            }
            _ => s.mouse.set(Some((x, y))),
        }
        glib::Propagation::Stop
    });
    let s = st.clone();
    w.connect_button_press_event(move |_, _| {
        if s.pronto() {
            s.esci("clic");
        }
        glib::Propagation::Stop
    });
    let s = st.clone();
    w.connect_scroll_event(move |_, _| {
        if s.pronto() {
            s.esci("rotellina");
        }
        glib::Propagation::Stop
    });
}

/// copia il fotogramma (0x00RRGGBB, come il formato RGB24 di cairo) nella superficie;
/// false se la superficie è ancora in uso e non la si può scrivere
fn copia(m: &Motore, sup: &mut cairo::ImageSurface) -> bool {
    let passo = sup.stride() as usize;
    let Ok(mut dati) = sup.data() else { return false };
    for (y, riga) in m.buf.chunks_exact(m.w).enumerate() {
        // SAFETY: si leggono gli stessi byte della riga di u32, senza cambiarli
        let byte = unsafe { std::slice::from_raw_parts(riga.as_ptr() as *const u8, riga.len() * 4) };
        if let Some(dst) = dati.get_mut(y * passo..y * passo + byte.len()) {
            dst.copy_from_slice(byte);
        }
    }
    true
}

/// un fotogramma su tutti gli schermi, poi si aspetta il prossimo
fn fotogramma(st: Rc<Stato>) {
    if st.finito.get() {
        return;
    }
    let adesso = Instant::now();
    if adesso >= st.prossimo_controllo.get() {
        st.prossimo_controllo.set(adesso + CONTROLLO);
        st.quadro.set(Duration::from_secs_f64(if power::on_ac() { 1.0 / 30.0 } else { 1.0 / 15.0 }));
    }
    let dt = (adesso - st.prima.replace(adesso)).as_secs_f32().min(0.25);
    let ora = ascii_dreams::ora_adesso();
    for s in st.schermi.borrow_mut().iter_mut() {
        // pixel veri: con la scala di GTK (schermi HiDPI) l'area ne ha di più
        let scala = s.area.scale_factor().max(1);
        let (w, h) = ((s.area.allocated_width() * scala) as usize, (s.area.allocated_height() * scala) as usize);
        if w < 2 || h < 2 {
            continue; // finestra non ancora mostrata
        }
        if s.motore.as_ref().map(|m| (m.w, m.h)) != Some((w, h)) {
            s.motore = Some(Motore::new(w, h, &st.cfg, st.cfg.righe, s.seme));
            s.superficie = None;
        }
        let Some(m) = s.motore.as_mut() else { continue };
        m.passo(dt, ora);
        let scritta = s.superficie.as_mut().is_some_and(|sup| copia(m, sup));
        if !scritta {
            // la prima volta, o se la superficie di prima era ancora in uso: una nuova
            s.superficie = cairo::ImageSurface::create(cairo::Format::Rgb24, w as i32, h as i32).ok();
            if let Some(sup) = s.superficie.as_mut() {
                sup.set_device_scale(scala as f64, scala as f64);
                copia(m, sup);
            }
        }
        s.area.queue_draw();
    }
    let attesa = st.quadro.get().saturating_sub(adesso.elapsed());
    glib::timeout_add_local_once(attesa, move || fotogramma(st));
}
