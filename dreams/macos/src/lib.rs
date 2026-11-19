//! Lo screensaver di ASCII Zoo come modulo di macOS, `ASCII Zoo.saver`: quello che si
//! sceglie in Impostazioni di Sistema > Salvaschermo.
//!
//! macOS carica questa libreria (l'eseguibile del bundle) nel suo processo dei
//! salvaschermo, `legacyScreenSaver`, e crea una vista `AsciiZooDreamsView` per ogni
//! schermo, più le anteprime piccole delle impostazioni: tutte nello stesso processo.
//! Ogni vista ha il suo `Motore` di ascii-dreams (le stesse scene di Windows), che
//! disegna in un buffer di pixel; `drawRect:` lo copia sullo schermo.
//!
//! - la classe si registra quando il modulo viene caricato (`REGISTRA`): macOS la cerca
//!   per nome (NSPrincipalClass nell'Info.plist) subito dopo
//! - si disegna ai pixel veri dello schermo (2 per punto sugli schermi Retina), a 30
//!   fotogrammi al secondo (15 a batteria)
//! - nessun panic arriva a Objective-C: ogni metodo passa da `protetto`
//! - le impostazioni: il processo è in una sandbox e la cartella dei dati è quella del
//!   suo container, dove screensaver.json non c'è; quindi valgono quelle di serie
//!   (tutte le scene a rotazione, orologio, caratteri medi)
//! - da macOS 14 legacyScreenSaver non ferma più i moduli e resta acceso: alla fine del
//!   salvaschermo il processo esce da solo (vedi `prepara_uscita`)

#![cfg(target_os = "macos")]

use std::cell::{Cell, RefCell};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::sync::Once;
use std::time::{Duration, Instant};

use ascii_dreams::{Config, Motore};
use objc2::rc::{Allocated, Retained};
use objc2::runtime::{AnyClass, AnyObject, NSObject};
use objc2::{define_class, msg_send, sel, ClassType, DefinedClass, MainThreadOnly};
use objc2_app_kit::{NSGraphicsContext, NSResponder, NSView};
use objc2_core_foundation::{CFData, CFRetained};
use objc2_core_graphics::{
    kCGColorSpaceSRGB, CGBitmapInfo, CGColorRenderingIntent, CGColorSpace, CGContext, CGDataProvider, CGImage,
    CGImageAlphaInfo, CGImageByteOrderInfo, CGInterpolationQuality,
};
use objc2_foundation::{
    ns_string, NSDistributedNotificationCenter, NSNotification, NSNotificationSuspensionBehavior, NSProcessInfo, NSRect,
};
use objc2_screen_saver::ScreenSaverView;

/// secondi fra due fotogrammi: 30 al secondo, 15 a batteria (come su Windows)
const QUADRO: f64 = 1.0 / 30.0;
const QUADRO_BATTERIA: f64 = 1.0 / 15.0;
/// sotto questa altezza in punti la vista è un'anteprima, qualunque cosa dica isPreview
/// (su macOS 26 a volte è sbagliato)
const ALTEZZA_ANTEPRIMA: f64 = 400.0;
/// righe di testo nelle anteprime: caratteri più grandi, che si leggono ancora (come in win.rs)
const RIGHE_ANTEPRIMA: u32 = 18;
/// lato massimo del buffer in pixel, per non chiedere memoria assurda
const LATO_MAX: usize = 16384;
/// il colore di fondo delle scene (SFONDO in painter.rs), prima del primo fotogramma
const FONDO: [f64; 3] = [6.0 / 255.0, 8.0 / 255.0, 12.0 / 255.0];

/// lo stato di una vista
struct Stato {
    /// creato al primo fotogramma: prima non si sa quanti pixel ha la vista
    motore: RefCell<Option<Motore>>,
    seme: u64,
    /// quando è stato fatto l'ultimo fotogramma
    ultimo: Cell<Option<Instant>>,
    /// secondi fra due fotogrammi (QUADRO o QUADRO_BATTERIA)
    quadro: Cell<f64>,
    /// quando ricontrollare la batteria
    controllo: Cell<Option<Instant>>,
    /// c'è stato un panic: da lì in poi solo il fondo
    rotto: Cell<bool>,
}

define_class!(
    // SAFETY: ScreenSaverView è fatta per essere estesa; la vista non ha Drop e i metodi
    // ridefiniti hanno le stesse firme di ScreenSaverView e NSView
    #[unsafe(super(ScreenSaverView, NSView, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AsciiZooDreamsView"]
    #[ivars = Stato]
    struct AsciiZooDreamsView;

    impl AsciiZooDreamsView {
        /// solo lo stato vuoto e il metodo di ScreenSaverView: niente che possa andare in panic
        #[unsafe(method_id(initWithFrame:isPreview:))]
        fn init_with_frame_is_preview(this: Allocated<Self>, frame: NSRect, anteprima: bool) -> Option<Retained<Self>> {
            let this = this.set_ivars(Stato {
                motore: RefCell::new(None),
                seme: ascii_dreams::seme(),
                ultimo: Cell::new(None),
                quadro: Cell::new(QUADRO),
                controllo: Cell::new(None),
                rotto: Cell::new(false),
            });
            let vista: Option<Retained<Self>> = unsafe { msg_send![super(this), initWithFrame: frame, isPreview: anteprima] };
            if let Some(v) = &vista {
                unsafe { v.setAnimationTimeInterval(QUADRO) };
            }
            vista
        }

        #[unsafe(method(startAnimation))]
        fn start_animation(&self) {
            let _: () = unsafe { msg_send![super(self), startAnimation] };
            self.ivars().ultimo.set(None);
        }

        #[unsafe(method(stopAnimation))]
        fn stop_animation(&self) {
            let _: () = unsafe { msg_send![super(self), stopAnimation] };
            // il buffer di uno schermo 5K pesa 60 MB: si libera subito, al prossimo
            // startAnimation il motore si rifà
            protetto(self, || drop(self.ivars().motore.take()));
            self.ivars().ultimo.set(None);
        }

        #[unsafe(method(animateOneFrame))]
        fn animate_one_frame(&self) {
            protetto(self, || self.fotogramma());
        }

        #[unsafe(method(drawRect:))]
        fn draw_rect(&self, _rect: NSRect) {
            protetto(self, || self.disegna());
        }

        /// la vista copre sempre tutto il suo rettangolo: macOS non deve disegnare cosa c'è sotto
        #[unsafe(method(isOpaque))]
        fn is_opaque(&self) -> bool {
            true
        }

        /// niente finestra delle opzioni (vedi in cima: valgono quelle di serie)
        #[unsafe(method(hasConfigureSheet))]
        fn has_configure_sheet(&self) -> bool {
            false
        }
    }
);

impl AsciiZooDreamsView {
    /// un'anteprima delle impostazioni: lo dice isPreview, o la vista è piccola
    fn piccola(&self) -> bool {
        let anteprima = unsafe { self.isPreview() };
        anteprima || self.bounds().size.height < ALTEZZA_ANTEPRIMA
    }

    /// i pixel veri della vista: i punti per il backingScaleFactor dello schermo
    fn pixel(&self) -> (usize, usize) {
        let r = self.convertRectToBacking(self.bounds());
        let lato = |v: f64| (v.round().max(0.0) as usize).min(LATO_MAX);
        (lato(r.size.width), lato(r.size.height))
    }

    /// avanza la scena e chiede di ridisegnare la vista
    fn fotogramma(&self) {
        let s = self.ivars();
        if s.rotto.get() {
            return;
        }
        let adesso = Instant::now();
        // macOS può chiamare più spesso di animationTimeInterval: si salta quello che avanza
        if let Some(u) = s.ultimo.get() {
            if adesso.duration_since(u).as_secs_f64() < s.quadro.get() * 0.8 {
                return;
            }
        }
        let dt = s.ultimo.replace(Some(adesso)).map(|u| adesso.duration_since(u).as_secs_f32()).unwrap_or(0.0);
        let (w, h) = self.pixel();
        if w == 0 || h == 0 {
            return;
        }
        let piccola = self.piccola();
        if !piccola {
            prepara_uscita();
        }
        {
            let mut m = s.motore.borrow_mut();
            // primo fotogramma, oppure la vista ha cambiato misura o schermo (Retina o no)
            if m.as_ref().map(|m| (m.w, m.h)) != Some((w, h)) {
                *m = Some(nuovo_motore(w, h, piccola, s.seme));
            }
            if let Some(m) = m.as_mut() {
                m.passo(dt, ascii_dreams::ora_adesso());
            }
        }
        // a batteria la metà dei fotogrammi; si ricontrolla ogni 2 secondi
        if s.controllo.get().is_none_or(|c| adesso >= c) {
            s.controllo.set(Some(adesso + Duration::from_secs(2)));
            let q = if a_batteria() { QUADRO_BATTERIA } else { QUADRO };
            if q != s.quadro.get() {
                s.quadro.set(q);
                unsafe { self.setAnimationTimeInterval(q) };
            }
        }
        self.setNeedsDisplay(true);
    }

    /// copia l'ultimo fotogramma nella vista (o il fondo, se non c'è ancora)
    fn disegna(&self) {
        let Some(gc) = NSGraphicsContext::currentContext() else { return };
        let cg = gc.CGContext();
        let b = self.bounds();
        let Ok(m) = self.ivars().motore.try_borrow() else { return };
        match m.as_ref().and_then(immagine) {
            Some(img) => {
                // un pixel del buffer per un pixel dello schermo; se la misura non torna
                // (un attimo, durante un cambio) meglio squadrato che sfocato
                CGContext::set_interpolation_quality(Some(&cg), CGInterpolationQuality::None);
                CGContext::draw_image(Some(&cg), b, Some(&img));
            }
            None => {
                let [r, g, bl] = FONDO;
                CGContext::set_rgb_fill_color(Some(&cg), r, g, bl, 1.0);
                CGContext::fill_rect(Some(&cg), b);
            }
        }
    }
}

fn nuovo_motore(w: usize, h: usize, piccola: bool, seme: u64) -> Motore {
    // nella sandbox screensaver.json non c'è: di solito sono le impostazioni di serie
    let mut cfg = Config::carica();
    let mut righe = cfg.righe;
    if piccola {
        // come l'anteprima di Windows: caratteri grandi e niente orologio
        cfg.orologio = false;
        righe = RIGHE_ANTEPRIMA;
    }
    Motore::new(w, h, &cfg, righe, seme)
}

/// Il buffer del motore come CGImage. I pixel sono u32 0x00RRGGBB, cioè in memoria
/// B, G, R, 0 (little endian, sia su Apple Silicon sia su Intel): "32 bit little endian,
/// primo byte da saltare" per CoreGraphics. I byte si copiano: così l'immagine non
/// dipende dal buffer, che al prossimo fotogramma cambia.
fn immagine(m: &Motore) -> Option<CFRetained<CGImage>> {
    // SAFETY: un Vec<u32> visto come byte, stessa memoria e lunghezza per 4
    let byte = unsafe { std::slice::from_raw_parts(m.buf.as_ptr().cast::<u8>(), m.buf.len() * 4) };
    let dati = CFData::from_bytes(byte);
    let fornitore = CGDataProvider::with_cf_data(Some(&dati))?;
    // sRGB: gli stessi colori dello sfondo nel browser, anche sugli schermi P3
    let spazio = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB }))?;
    let info = CGBitmapInfo(CGImageByteOrderInfo::Order32Little.0 | CGImageAlphaInfo::NoneSkipFirst.0);
    // SAFETY: misure, righe (w * 4 byte) e dati coerenti; decode nullo vuol dire nessuno
    unsafe {
        CGImage::new(
            m.w,
            m.h,
            8,
            32,
            m.w * 4,
            Some(&spazio),
            info,
            Some(&fornitore),
            std::ptr::null(),
            false,
            CGColorRenderingIntent::RenderingIntentDefault,
        )
    }
}

/// Esegue `f` senza lasciar passare un panic verso Objective-C (che non saprebbe cosa
/// farne): dopo un panic la vista mostra solo il fondo.
fn protetto(vista: &AsciiZooDreamsView, f: impl FnOnce()) {
    if catch_unwind(AssertUnwindSafe(f)).is_err() {
        let s = vista.ivars();
        s.rotto.set(true);
        if let Ok(mut m) = s.motore.try_borrow_mut() {
            *m = None;
        }
    }
}

#[link(name = "IOKit", kind = "framework")]
extern "C" {
    /// secondi di batteria rimasti; -2 = illimitato (alimentatore collegato o Mac senza batteria)
    fn IOPSGetTimeRemainingEstimate() -> f64;
}

fn a_batteria() -> bool {
    unsafe { IOPSGetTimeRemainingEstimate() != -2.0 }
}

// ------------------------------------------------------------ fine del salvaschermo

define_class!(
    // SAFETY: NSObject non chiede niente alle sottoclassi; niente Drop
    #[unsafe(super(NSObject))]
    #[name = "AsciiZooDreamsUscita"]
    struct Uscita;

    impl Uscita {
        /// "com.apple.screensaver.willstop": fra 2 secondi il processo esce
        #[unsafe(method(salvaschermoFinito:))]
        fn salvaschermo_finito(&self, _notifica: &NSNotification) {
            let _: () = unsafe {
                msg_send![self, performSelector: sel!(esci:), withObject: None::<&AnyObject>, afterDelay: 2.0f64]
            };
        }

        #[unsafe(method(esci:))]
        fn esci(&self, _niente: Option<&AnyObject>) {
            std::process::exit(0);
        }
    }
);

/// Da macOS 14 legacyScreenSaver non ferma i moduli quando il salvaschermo finisce: le
/// viste restano vive e continuano ad animare di nascosto, e i processi si accumulano.
/// I moduli che lo sanno (Aerial, per esempio) ascoltano la notifica distribuita
/// "com.apple.screensaver.willstop" e chiudono il processo con exit(0): qui lo stesso,
/// con 2 secondi di margine come Aerial, mentre macOS toglie il salvaschermo.
/// Solo nei processi che mostrano il salvaschermo vero (si chiama dai fotogrammi delle
/// viste grandi): quelli con le sole anteprime delle impostazioni restano.
fn prepara_uscita() {
    static UNA_VOLTA: Once = Once::new();
    UNA_VOLTA.call_once(|| {
        // macOS 26 dice "16" ai programmi compilati con un SDK più vecchio: va bene lo stesso
        if NSProcessInfo::processInfo().operatingSystemVersion().majorVersion < 14 {
            return;
        }
        let Some(classe) = classe_uscita() else { return };
        let osservatore: Retained<AnyObject> = unsafe { msg_send![classe, new] };
        // DeliverImmediately: arriva anche se l'app che ci ospita non è in primo piano
        unsafe {
            NSDistributedNotificationCenter::defaultCenter().addObserver_selector_name_object_suspensionBehavior(
                &osservatore,
                sel!(salvaschermoFinito:),
                Some(ns_string!("com.apple.screensaver.willstop")),
                None,
                NSNotificationSuspensionBehavior::DeliverImmediately,
            );
        }
        // il centro notifiche non lo tiene in vita: resta per tutta la vita del processo
        std::mem::forget(osservatore);
    });
}

/// la classe dell'osservatore; se un'altra copia del modulo (un'altra versione) l'ha già
/// registrata nello stesso processo va bene la sua, ricrearla andrebbe in panic
fn classe_uscita() -> Option<&'static AnyClass> {
    AnyClass::get(c"AsciiZooDreamsUscita").or_else(|| catch_unwind(Uscita::class).ok())
}

// ------------------------------------------------------------ caricamento

/// macOS cerca la classe per nome (NSPrincipalClass) appena caricato il modulo, ma
/// define_class! la crea solo al primo uso: la si crea al caricamento, come il
/// costruttore di una libreria C (sezione __mod_init_func, la chiama dyld)
#[used]
#[link_section = "__DATA,__mod_init_func"]
static REGISTRA: extern "C" fn() = registra;

extern "C" fn registra() {
    // se un'altra copia del modulo l'ha già registrata (una versione vecchia rimasta nel
    // processo) si usa quella: ricrearla andrebbe in panic
    let _ = catch_unwind(|| {
        if AnyClass::get(c"AsciiZooDreamsView").is_none() {
            let _ = AsciiZooDreamsView::class();
        }
    });
}
