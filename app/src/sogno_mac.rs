//! macOS: "Prova adesso" del sottomenu Screensaver. L'app lancia sé stessa come
//! `ascii-zoo --salvaschermo [--scena id]`, un processo a parte come `/s` su Windows e
//! sogno_linux.rs su Linux. Qui si carica il modulo `ASCII Zoo.saver` che sta dentro l'app
//! (lo stesso che macOS fa girare come salvaschermo, con la stessa scena: la legge da
//! screensaver.json), si mette una sua vista a tutto schermo su ogni schermo, sopra la barra
//! dei menu e il Dock, e ci si chiude al primo tasto, clic o movimento del mouse.
//!
//! La vista è una ScreenSaverView: `startAnimation` accende il suo timer, che chiama
//! `animateOneFrame` ogni `animationTimeInterval` (1/30 s, 1/15 a batteria), come fa macOS.
//! `--scena id` sceglie la scena senza toccare le impostazioni: il modulo la legge dalla
//! variabile d'ambiente ASCII_ZOO_SCENA (solo qui: nel salvaschermo vero non c'è).

use std::cell::Cell;
use std::path::Path;
use std::ptr::NonNull;
use std::time::{Duration, Instant};

use block2::RcBlock;
use objc2::rc::{Allocated, Retained};
use objc2::runtime::AnyClass;
use objc2::{define_class, msg_send, MainThreadOnly};
use objc2_app_kit::{
    NSApplication, NSApplicationActivationPolicy, NSApplicationDidResignActiveNotification, NSBackingStoreType, NSColor,
    NSCursor, NSEvent, NSEventMask, NSEventType, NSResponder, NSScreen, NSView, NSWindow, NSWindowCollectionBehavior,
    NSWindowStyleMask,
};
use objc2_foundation::{MainThreadMarker, NSBundle, NSNotification, NSNotificationCenter, NSObject, NSPoint, NSRect, NSString};

use crate::shared::zoo_log;

/// kCGScreenSaverWindowLevel: sopra la barra dei menu, il Dock e le altre finestre
const LIVELLO: isize = 1000;
/// i primi movimenti del mouse non contano (arrivano anche all'apertura delle finestre)
const CALMA: Duration = Duration::from_millis(1200);
/// punti di movimento del mouse che chiudono lo screensaver
const MOVIMENTO: f64 = 12.0;

define_class!(
    // SAFETY: una NSWindow con un solo metodo ridefinito, con la stessa firma
    #[unsafe(super(NSWindow, NSResponder, NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "AsciiZooProvaSalvaschermo"]
    struct Finestra;

    impl Finestra {
        /// una finestra senza bordi di solito non prende la tastiera (né i movimenti del mouse)
        #[unsafe(method(canBecomeKeyWindow))]
        fn can_become_key_window(&self) -> bool {
            true
        }
    }
);

/// argomento `--salvaschermo`: non torna
pub fn esegui(args: &[String], modulo: Option<&Path>) -> ! {
    let Some(mtm) = MainThreadMarker::new() else {
        eprintln!("ascii-zoo --salvaschermo: non sul thread principale");
        std::process::exit(2);
    };
    if let Some(i) = args.iter().position(|a| a == "--scena") {
        if let Some(s) = args.get(i + 1) {
            std::env::set_var("ASCII_ZOO_SCENA", s);
        }
    }
    let Some(classe) = modulo.and_then(carica) else {
        zoo_log!("screensaver: il modulo ASCII Zoo.saver non c'è o non si carica");
        std::process::exit(1);
    };

    let app = NSApplication::sharedApplication(mtm);
    // niente icona nel Dock, ma può prendere tastiera e mouse
    app.setActivationPolicy(NSApplicationActivationPolicy::Accessory);

    let mut finestre: Vec<Retained<NSWindow>> = Vec::new();
    for schermo in NSScreen::screens(mtm).iter() {
        if let Some(f) = finestra(mtm, classe, schermo.frame()) {
            finestre.push(f);
        }
    }
    if finestre.is_empty() {
        zoo_log!("screensaver: nessuna vista");
        std::process::exit(1);
    }
    zoo_log!("screensaver: prova su {} schermi", finestre.len());

    // la prima finestra prende la tastiera; il cursore sparisce finché non si muove
    finestre[0].makeKeyWindow();
    #[allow(deprecated)]
    app.activateIgnoringOtherApps(true);
    NSCursor::setHiddenUntilMouseMoves(true);

    let inizio = Instant::now();
    let mosso = Cell::new(0.0f64);
    let maschera = NSEventMask::KeyDown
        | NSEventMask::LeftMouseDown
        | NSEventMask::RightMouseDown
        | NSEventMask::OtherMouseDown
        | NSEventMask::MouseMoved
        | NSEventMask::ScrollWheel;
    let blocco = RcBlock::new(move |e: NonNull<NSEvent>| -> *mut NSEvent {
        let e = unsafe { e.as_ref() };
        let esci = match e.r#type() {
            NSEventType::MouseMoved => {
                if inizio.elapsed() >= CALMA {
                    mosso.set(mosso.get() + e.deltaX().abs() + e.deltaY().abs());
                }
                mosso.get() > MOVIMENTO
            }
            NSEventType::ScrollWheel => inizio.elapsed() >= CALMA,
            _ => true,
        };
        if esci {
            std::process::exit(0);
        }
        e as *const NSEvent as *mut NSEvent
    });
    let _monitor = unsafe { NSEvent::addLocalMonitorForEventsMatchingMask_handler(maschera, &blocco) };
    // se un'altra app passa davanti (Cmd-Tab, una notifica cliccata) la prova finisce
    let davanti = RcBlock::new(|_: NonNull<NSNotification>| std::process::exit(0));
    let _osservatore = unsafe {
        NSNotificationCenter::defaultCenter().addObserverForName_object_queue_usingBlock(
            Some(NSApplicationDidResignActiveNotification),
            None,
            None,
            &davanti,
        )
    };
    app.run();
    std::process::exit(0);
}

/// carica il bundle del modulo e dà la sua classe (NSPrincipalClass, una ScreenSaverView)
fn carica(modulo: &Path) -> Option<&'static AnyClass> {
    let percorso = NSString::from_str(modulo.to_str()?);
    let bundle = NSBundle::bundleWithPath(&percorso)?;
    // SAFETY: il modulo è il nostro (dentro l'app), lo stesso che carica macOS
    if !unsafe { bundle.load() } {
        return None;
    }
    bundle.principalClass()
}

/// una finestra senza bordi grande quanto lo schermo, con la vista del modulo dentro
fn finestra(mtm: MainThreadMarker, classe: &AnyClass, cornice: NSRect) -> Option<Retained<NSWindow>> {
    let w: Retained<Finestra> = unsafe {
        msg_send![
            Finestra::alloc(mtm),
            initWithContentRect: cornice,
            styleMask: NSWindowStyleMask::Borderless,
            backing: NSBackingStoreType::Buffered,
            defer: false
        ]
    };
    let w: Retained<NSWindow> = Retained::into_super(w);
    unsafe { w.setReleasedWhenClosed(false) };
    w.setLevel(LIVELLO);
    w.setOpaque(true);
    w.setBackgroundColor(Some(&NSColor::blackColor()));
    w.setAcceptsMouseMovedEvents(true);
    w.setCollectionBehavior(
        NSWindowCollectionBehavior::CanJoinAllSpaces
            | NSWindowCollectionBehavior::FullScreenAuxiliary
            | NSWindowCollectionBehavior::Stationary,
    );
    let r = NSRect::new(NSPoint::new(0.0, 0.0), cornice.size);
    let vista: Option<Retained<NSView>> = unsafe {
        let a: Allocated<NSView> = msg_send![classe, alloc];
        msg_send![a, initWithFrame: r, isPreview: false]
    };
    let vista = vista?;
    w.setContentView(Some(&vista));
    w.setFrame_display(cornice, true);
    w.orderFrontRegardless();
    let _: () = unsafe { msg_send![&*vista, startAnimation] };
    Some(w)
}
