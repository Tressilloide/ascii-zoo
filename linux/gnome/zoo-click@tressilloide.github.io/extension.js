// ASCII Zoo: sfondo sotto Wayland e clic sul desktop (GNOME Shell 45 e successivi).
//
// Due cose:
//   1. lo sfondo vero e proprio senza XWayland: vedi sfondo.js;
//   2. i clic sul desktop, qui sotto.
// Resta accesa anche con lo schermo bloccato ("session-modes" in metadata.json): GNOME
// altrimenti la spegnerebbe a ogni blocco e lo sfondo sparirebbe e tornerebbe a ogni sblocco.
// Con lo schermo bloccato i clic non si guardano.
//
// Clic. Lo sfondo di ASCII Zoo è sotto tutte le finestre, trasparente ai clic.
// Sotto Wayland un programma non vede i clic fatti sulle altre finestre, e su Ubuntu le
// icone del desktop (DING) stanno in una finestra a tutto schermo sopra lo sfondo: i clic
// li vede solo GNOME Shell. Questa estensione, quando si preme il tasto sinistro su una
// zona del desktop (lo sfondo o la finestra delle icone; non le finestre normali, la barra
// in alto, la panoramica o un menu aperto), manda il punto ad ASCII Zoo con D-Bus. Non
// blocca né cambia il clic, e non registra niente.
//
// Come: mutter consegna i clic sulle finestre Wayland direttamente al programma, senza
// passare dagli attori di Clutter che un'estensione può ascoltare. Si legge allora lo stato
// del tasto (global.get_pointer) ogni 40 ms, e al momento in cui viene premuto si chiede a
// Clutter cosa c'è sotto. Ma solo mentre:
//   1. ASCII Zoo è acceso e vuole i clic: ha il suo nome sul bus di sessione (lo tiene solo
//      con "Clic sul desktop agli animali" spuntato);
//   2. si sta usando il computer: dopo 2 secondi senza mouse né tastiera il controllo si
//      ferma e riparte al primo movimento (monitor di inattività di mutter).
// Quando nessuno tocca niente non c'è nessun risveglio.

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import {Sfondo} from './sfondo.js';

const NOME = 'io.github.tressilloide.AsciiZoo';
const PERCORSO = '/io/github/tressilloide/AsciiZoo';
const GIRO_MS = 40;
const FERMO_MS = 2000;
// un clic che chiude un menu o la panoramica non è un clic sul desktop: il menu si chiude
// appena si preme, prima che il controllo lo veda, quindi si ricorda per un po' che c'era
const DOPO_MENU_US = 300 * 1000;

// la finestra fa parte del desktop?
function finestraDelDesktop(w) {
    if (!w)
        return false;
    // ASCII Zoo stesso, le icone del desktop sotto X11, file manager che fanno da desktop
    if (w.get_window_type() === Meta.WindowType.DESKTOP)
        return true;
    // DING (Ubuntu) e derivati: sotto Wayland è una finestra normale tenuta in fondo,
    // segnata da DING con customJS_ding; il titolo è "Desktop Icons 1", "Desktop Icons 2"…
    const ding = w.customJS_ding;
    if (ding && (ding._keepAtBottom || ding.keepAtBottom))
        return true;
    return (w.get_title() ?? '').startsWith('Desktop Icons ');
}

export default class ZooClic extends Extension {
    enable() {
        this._sfondo = new Sfondo(finestraDelDesktop);
        this._protetto(() => this._sfondo.enable());
        this._giro = 0;
        this._fermo = 0;
        this._attivo = 0;
        this._giu = false;
        this._menuFino = 0;
        this._errori = 0;
        this._inattivita = global.backend.get_core_idle_monitor();
        this._osserva = Gio.bus_watch_name(Gio.BusType.SESSION, NOME,
            Gio.BusNameWatcherFlags.NONE,
            () => this._protetto(() => this._accendi()),
            () => this._protetto(() => this._spegni()));
    }

    disable() {
        this._protetto(() => this._sfondo?.disable());
        this._sfondo = null;
        if (this._osserva)
            Gio.bus_unwatch_name(this._osserva);
        this._osserva = 0;
        this._spegni();
        this._inattivita = null;
    }

    // ogni callback: un errore qui non deve mai disturbare GNOME Shell
    _protetto(f) {
        try {
            f();
        } catch (e) {
            if (this._errori++ < 5)
                console.error(`ASCII Zoo clic: ${e}`);
        }
    }

    // ASCII Zoo vuole i clic
    _accendi() {
        if (this._fermo)
            return;
        this._fermo = this._inattivita.add_idle_watch(FERMO_MS,
            () => this._protetto(() => this._dormi()));
        this._sveglia();
    }

    // ASCII Zoo chiuso, o non vuole più i clic
    _spegni() {
        this._fermaGiro();
        if (this._fermo)
            this._inattivita?.remove_watch(this._fermo);
        if (this._attivo)
            this._inattivita?.remove_watch(this._attivo);
        this._fermo = this._attivo = 0;
    }

    // qualcuno usa il computer: si guarda il tasto
    _sveglia() {
        this._attivo = 0;
        if (!this._giro) {
            this._giro = GLib.timeout_add(GLib.PRIORITY_DEFAULT, GIRO_MS, () => {
                this._protetto(() => this._guarda());
                return GLib.SOURCE_CONTINUE;
            });
        }
    }

    // 2 secondi senza mouse né tastiera: ci si ferma fino al prossimo movimento
    _dormi() {
        this._fermaGiro();
        if (!this._attivo) {
            this._attivo = this._inattivita.add_user_active_watch(
                () => this._protetto(() => this._sveglia()));
        }
    }

    _fermaGiro() {
        if (this._giro)
            GLib.source_remove(this._giro);
        this._giro = 0;
    }

    _guarda() {
        const [x, y, mods] = global.get_pointer();
        const giu = (mods & Clutter.ModifierType.BUTTON1_MASK) !== 0;
        const ora = GLib.get_monotonic_time();
        if (Main.overview.visible || Main.modalCount > 0 || Main.sessionMode.isLocked)
            this._menuFino = ora + DOPO_MENU_US;
        else if (giu && !this._giu && ora > this._menuFino && this._sulDesktop(x, y))
            this._manda(x, y);
        this._giu = giu;
    }

    _sulDesktop(x, y) {
        const sotto = global.stage.get_actor_at_pos(Clutter.PickMode.REACTIVE, x, y);
        if (!sotto || sotto === global.stage)
            return true;
        for (let a = sotto; a; a = a.get_parent()) {
            if (a instanceof Meta.WindowActor)
                return finestraDelDesktop(a.get_meta_window());
            // lo sfondo di GNOME, senza finestre sopra
            if (a === global.window_group)
                return true;
        }
        // barra in alto, dock, notifiche…
        return false;
    }

    _manda(x, y) {
        const parametri = new GLib.Variant('(dddd)', [x, y, global.stage.width, global.stage.height]);
        Gio.DBus.session.call(NOME, PERCORSO, NOME, 'Clic', parametri, null,
            Gio.DBusCallFlags.NO_AUTO_START, 1000, null, (bus, esito) => {
                try {
                    bus.call_finish(esito);
                } catch {
                    // ASCII Zoo chiuso nel frattempo: niente da fare
                }
            });
    }
}
