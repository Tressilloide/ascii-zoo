// ASCII Zoo: lo sfondo sotto Wayland, senza XWayland (GNOME Shell 45 e successivi).
//
// Sotto Wayland un programma normale non può mettersi al posto dello sfondo, e passando da
// XWayland ogni fotogramma fa un giro lunghissimo: GTK 3 su X11 non può prendere il
// fotogramma di WebKit dalla GPU, lo copia in memoria e poi XWayland lo ricarica sulla GPU
// (misurato con GNOME 51 e la scala al 125%: ~0,8 core in più, e non più di 20 fotogrammi
// al secondo). Allora ASCII Zoo, quando trova questa estensione, apre per ogni schermo una
// finestra Wayland normale con il titolo "@ascii-zoo-sfondo!x,y,larghezza,altezza" (lo
// schermo, in pixel logici), e qui:
//   - la finestra si minimizza e si tiene minimizzata, su tutti gli spazi di lavoro. Va
//     minimizzata appena compare (segnale "map", senza animazioni), non prima: minimizzata
//     prima di comparire non avrebbe niente da mostrare nel clone. Compare e sparisce nello
//     stesso giro, prima che GNOME ridisegni lo schermo: non si vede mai;
//   - il suo contenuto si mostra come Clutter.Clone dentro lo sfondo di GNOME di quello
//     schermo (sul desktop, nella panoramica e nelle miniature degli spazi di lavoro): sotto
//     tutte le finestre e le icone del desktop, e i clic non lo toccano;
//   - non compare in Alt+Tab, nella panoramica, nella dash o tra le app aperte;
//   - si dice ad ASCII Zoo (D-Bus, oggetto /io/github/tressilloide/AsciiZoo/Sfondo sul nome
//     di GNOME Shell) se uno schermo ha un'app a schermo intero o è coperto da finestre, e
//     dov'è l'area di lavoro (barra in alto, dock), così lo sfondo va in pausa quando non
//     si vede e i widget restano fuori dalle barre.
// Una finestra si prende solo se il titolo è quello e il programma è davvero ascii-zoo
// (/proc/PID/exe). Senza l'estensione, o con una versione vecchia, ASCII Zoo resta su
// XWayland come prima. Lo stesso schema delle estensioni di sfondi animati (Hanabi).

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import * as Background from 'resource:///org/gnome/shell/ui/background.js';
import * as Workspace from 'resource:///org/gnome/shell/ui/workspace.js';
import * as WorkspaceThumbnail from 'resource:///org/gnome/shell/ui/workspaceThumbnail.js';
import * as WindowManager from 'resource:///org/gnome/shell/ui/windowManager.js';
import {InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';

export const PREFISSO = '@ascii-zoo-sfondo!';
const PERCORSO = '/io/github/tressilloide/AsciiZoo/Sfondo';
const INTERFACCIA = 'io.github.tressilloide.AsciiZoo.Sfondo';
// per ogni schermo: x, y, larghezza, altezza (pixel logici di GNOME); area di lavoro (x, y,
// larghezza, altezza); un'app a schermo intero; coperto da finestre; la finestra di ASCII Zoo
// per quello schermo è al suo posto; il connettore ("eDP-1", "HDMI-1"…: ASCII Zoo lega a
// questo nome le impostazioni per schermo, come sotto X11)
const TIPO = 'a(iiiiiiiibbbs)';
const XML = `<node>
  <interface name="${INTERFACCIA}">
    <method name="Stato"><arg type="${TIPO}" direction="out" name="schermi"/></method>
    <signal name="Cambiato"><arg type="${TIPO}" name="schermi"/></signal>
  </interface>
</node>`;
// lo stato si ricontrolla a ogni cambio di finestre, e comunque ogni tanto (spostamenti,
// ridimensionamenti): ma solo mentre c'è uno sfondo di ASCII Zoo
const GIRO_MS = 1000;
// punti per lato con cui si controlla se le finestre coprono l'area di lavoro
const CAMPIONI = 8;

// il titolo dice che è uno sfondo di ASCII Zoo (controllo veloce, per gli override)
export function titoloSfondo(w) {
    return (w?.get_title?.() ?? '').startsWith(PREFISSO);
}

// il programma della finestra è proprio ascii-zoo
function programmaZoo(w) {
    const pid = w.get_pid();
    if (!(pid > 0))
        return false;
    try {
        const exe = GLib.file_read_link(`/proc/${pid}/exe`);
        return GLib.path_get_basename(exe).startsWith('ascii-zoo');
    } catch {
        return false;
    }
}

// "@ascii-zoo-sfondo!x,y,l,a" → {x, y, l, a}
function geometria(w) {
    const n = w.get_title().slice(PREFISSO.length).split(',').map(Number);
    if (n.length < 4 || n.some(v => !Number.isFinite(v)))
        return null;
    return {x: n[0], y: n[1], l: n[2], a: n[3]};
}

export class Sfondo {
    // finestraDelDesktop: dalla parte dei clic, per non contare le icone del desktop come
    // finestre che coprono lo sfondo
    constructor(finestraDelDesktop) {
        this._delDesktop = finestraDelDesktop;
    }

    enable() {
        this._finestre = new Map(); // MetaWindow → {geo, segnali}
        this._sfondi = new Set();   // {attore, clone, monitor}
        this._attese = new Map();   // MetaWindow → segnale notify::title (titolo non ancora pronto)
        this._ultimo = '';
        this._giro = 0;
        this._errori = 0;

        this._override();

        this._dbus = Gio.DBusExportedObject.wrapJSObject(XML, this);
        this._dbus.export(Gio.DBus.session, PERCORSO);

        this._segnali = [
            [global.display, global.display.connect('window-created', (_d, w) => this._protetto(() => this._prova(w)))],
            // dopo il "map" di GNOME Shell (collegato prima): la finestra è appena comparsa
            [global.window_manager, global.window_manager.connect('map', (_wm, a) => this._protetto(() => this._comparsa(a)))],
            [global.display, global.display.connect('in-fullscreen-changed', () => this._protetto(() => this._aggiorna()))],
            [global.display, global.display.connect('restacked', () => this._protetto(() => this._aggiorna()))],
            [global.workspace_manager, global.workspace_manager.connect('active-workspace-changed', () => this._protetto(() => this._aggiorna()))],
            [Main.layoutManager, Main.layoutManager.connect('monitors-changed', () => this._protetto(() => this._ricollega()))],
        ];
        for (const w of global.display.list_all_windows())
            this._protetto(() => this._prova(w));
        // gli sfondi di GNOME fatti prima dell'estensione (il desktop, e la panoramica, che
        // GNOME prepara all'avvio): si trovano e si prepara anche lì il posto per ASCII Zoo
        this._cercaSfondi(global.stage);
    }

    disable() {
        for (const [o, id] of this._segnali ?? [])
            o.disconnect(id);
        this._segnali = [];
        this._fermaGiro();
        for (const [w, id] of this._attese ?? [])
            w.disconnect(id);
        this._attese?.clear();
        for (const [w, f] of this._finestre ?? []) {
            if (f.attesa)
                GLib.source_remove(f.attesa);
            for (const id of f.segnali)
                w.disconnect(id);
        }
        this._finestre?.clear();
        for (const s of [...this._sfondi ?? []])
            s.clone.destroy();
        this._sfondi?.clear();
        this._dbus?.unexport();
        this._dbus = null;
        this._iniezioni?.clear();
        this._iniezioni = null;
    }

    _protetto(f) {
        try {
            return f();
        } catch (e) {
            if (this._errori++ < 5)
                console.error(`ASCII Zoo sfondo: ${e}\n${e.stack}`);
            return undefined;
        }
    }

    // Lo sfondo di ASCII Zoo non è una finestra per GNOME Shell: niente Alt+Tab, panoramica,
    // dash, elenco delle app aperte. E ogni sfondo di GNOME (desktop, panoramica, miniature)
    // si fa con il posto per il clone.
    _override() {
        const io = this;
        const inj = this._iniezioni = new InjectionManager();
        inj.overrideMethod(Background.BackgroundManager.prototype, '_createBackgroundActor', originale => function (...a) {
            const attore = originale.apply(this, a);
            io._protetto(() => io._adorna(attore, this._monitorIndex));
            return attore;
        });
        // niente animazioni quando compare o si minimizza
        inj.overrideMethod(WindowManager.WindowManager.prototype, '_shouldAnimateActor', originale => function (attore, ...a) {
            return titoloSfondo(attore?.meta_window) ? false : originale.call(this, attore, ...a);
        });
        inj.overrideMethod(Shell.Global.prototype, 'get_window_actors', originale => function (...a) {
            return originale.apply(this, a).filter(x => !titoloSfondo(x.meta_window));
        });
        inj.overrideMethod(Workspace.Workspace.prototype, '_isOverviewWindow', originale => function (w, ...a) {
            return titoloSfondo(w) ? false : originale.call(this, w, ...a);
        });
        inj.overrideMethod(WorkspaceThumbnail.WorkspaceThumbnail.prototype, '_isOverviewWindow', originale => function (w, ...a) {
            return titoloSfondo(w) ? false : originale.call(this, w, ...a);
        });
        inj.overrideMethod(Meta.Display.prototype, 'get_tab_list', originale => function (...a) {
            return originale.apply(this, a).filter(w => !titoloSfondo(w));
        });
        inj.overrideMethod(Shell.WindowTracker.prototype, 'get_window_app', originale => function (w, ...a) {
            return titoloSfondo(w) ? null : originale.call(this, w, ...a);
        });
        inj.overrideMethod(Shell.App.prototype, 'get_windows', originale => function (...a) {
            return originale.apply(this, a).filter(w => !titoloSfondo(w));
        });
        inj.overrideMethod(Shell.App.prototype, 'get_n_windows', () => function () {
            return this.get_windows().length;
        });
        inj.overrideMethod(Shell.AppSystem.prototype, 'get_running', originale => function (...a) {
            return originale.apply(this, a).filter(app => app.get_n_windows() > 0);
        });
    }

    // una finestra nuova (o già aperta all'accensione): è uno sfondo di ASCII Zoo?
    _prova(w) {
        if (!w || this._finestre.has(w))
            return;
        if (!titoloSfondo(w)) {
            // GTK può mettere il titolo dopo: si aspetta, per le finestre senza titolo
            if (!w.get_title() && !this._attese.has(w)) {
                const id = w.connect('notify::title', () => this._protetto(() => {
                    w.disconnect(id);
                    this._attese.delete(w);
                    this._prova(w);
                }));
                this._attese.set(w, id);
            }
            return;
        }
        if (w.get_client_type() !== Meta.WindowClientType.WAYLAND || !programmaZoo(w))
            return;
        const geo = geometria(w);
        if (!geo)
            return;
        const tieni = () => this._protetto(() => {
            if (!w.minimized)
                w.minimize();
        });
        const segnali = [
            w.connect('notify::minimized', tieni),
            w.connect('unmanaged', () => this._protetto(() => this._via(w))),
        ];
        this._finestre.set(w, {geo, segnali});
        w.stick();
        // già comparsa (l'estensione si è accesa dopo ASCII Zoo): si minimizza subito; se no
        // al "map" (_comparsa)
        if (w.get_compositor_private()?.visible)
            w.minimize();
        console.log(`ASCII Zoo sfondo: finestra per lo schermo ${geo.x},${geo.y} ${geo.l}×${geo.a}`);
        // l'attore della finestra arriva dopo la finestra (Wayland: al primo disegno): lo si
        // aspetta a intervalli, per al massimo 10 secondi (di solito lo porta già il "map")
        if (!w.get_compositor_private()) {
            let volte = 0;
            const f = this._finestre.get(w);
            f.attesa = GLib.timeout_add(GLib.PRIORITY_DEFAULT, 100, () => {
                if (w.get_compositor_private() || ++volte > 100 || !this._finestre.has(w)) {
                    f.attesa = 0;
                    this._protetto(() => this._ricollega());
                    return GLib.SOURCE_REMOVE;
                }
                return GLib.SOURCE_CONTINUE;
            });
        }
        this._ricollega();
        this._avviaGiro();
    }

    // una finestra è appena comparsa: se è uno sfondo di ASCII Zoo, si minimizza
    _comparsa(attore) {
        const w = attore?.meta_window;
        if (!w || !this._finestre.has(w))
            return;
        w.minimize();
        this._ricollega();
    }

    _via(w) {
        const f = this._finestre.get(w);
        if (!f)
            return;
        if (f.attesa)
            GLib.source_remove(f.attesa);
        for (const id of f.segnali)
            w.disconnect(id);
        this._finestre.delete(w);
        this._ricollega();
        if (this._finestre.size === 0)
            this._fermaGiro();
    }

    // la finestra di ASCII Zoo per lo schermo `i` di GNOME
    _finestraPer(i) {
        const m = Main.layoutManager.monitors[i];
        if (!m)
            return null;
        let vicina = null;
        for (const [w, f] of this._finestre) {
            const g = f.geo;
            if (g.x === m.x && g.y === m.y && g.l === m.width && g.a === m.height)
                return w;
            // stessa posizione ma misura diversa (scala appena cambiata): va bene finché
            // ASCII Zoo non rifà la finestra
            const cx = g.x + g.l / 2, cy = g.y + g.a / 2;
            if (cx >= m.x && cy >= m.y && cx < m.x + m.width && cy < m.y + m.height)
                vicina = w;
        }
        return vicina;
    }

    _cercaSfondi(attore) {
        for (const figlio of attore.get_children()) {
            if (figlio instanceof Meta.BackgroundActor)
                this._adorna(figlio, figlio.monitor);
            else
                this._cercaSfondi(figlio);
        }
    }

    // dentro lo sfondo di GNOME di uno schermo, un clone della finestra di ASCII Zoo
    _adorna(attore, monitor) {
        if (!attore || monitor === null || monitor === undefined || monitor < 0 || attore._asciiZoo)
            return;
        attore._asciiZoo = true;
        attore.layout_manager = new Clutter.BinLayout();
        const clone = new Clutter.Clone({
            x_expand: true, y_expand: true,
            x_align: Clutter.ActorAlign.FILL, y_align: Clutter.ActorAlign.FILL,
            reactive: false,
        });
        attore.add_child(clone);
        const s = {attore, clone, monitor};
        this._sfondi.add(s);
        attore.connect('destroy', () => this._sfondi?.delete(s));
        clone.connect('destroy', () => {
            this._sfondi?.delete(s);
            attore._asciiZoo = false;
        });
        this._collega(s);
    }

    _collega(s) {
        const w = this._finestraPer(s.monitor);
        const sorgente = w?.get_compositor_private() ?? null;
        if (s.clone.source !== sorgente)
            s.clone.source = sorgente;
        s.clone.visible = sorgente !== null;
    }

    _ricollega() {
        for (const s of this._sfondi)
            this._collega(s);
        this._aggiorna();
    }

    _avviaGiro() {
        if (this._giro)
            return;
        this._giro = GLib.timeout_add(GLib.PRIORITY_DEFAULT_IDLE, GIRO_MS, () => {
            this._protetto(() => this._aggiorna());
            return GLib.SOURCE_CONTINUE;
        });
    }

    _fermaGiro() {
        if (this._giro)
            GLib.source_remove(this._giro);
        this._giro = 0;
    }

    // le finestre visibili su uno schermo coprono tutta la sua area di lavoro?
    _coperto(area, finestre) {
        if (!finestre.length)
            return false;
        for (let i = 0; i < CAMPIONI; i++) {
            for (let j = 0; j < CAMPIONI; j++) {
                const px = area.x + (i + 0.5) * area.width / CAMPIONI;
                const py = area.y + (j + 0.5) * area.height / CAMPIONI;
                if (!finestre.some(r => px >= r.x && py >= r.y && px < r.x + r.width && py < r.y + r.height))
                    return false;
            }
        }
        return true;
    }

    // connettore di ogni schermo, per numero (quello di Main.layoutManager.monitors); dove
    // mutter non lo dice: ""
    _connettori() {
        const c = new Map();
        try {
            for (const lm of global.backend.get_monitor_manager().get_logical_monitors?.() ?? [])
                c.set(lm.get_number(), lm.get_monitors()[0]?.get_connector() ?? '');
        } catch {
            // senza connettori ASCII Zoo usa il nome che gli dà GTK
        }
        return c;
    }

    _schermi() {
        const connettori = this._connettori();
        const lavoro = global.workspace_manager.get_active_workspace();
        const visibili = lavoro.list_windows()
            .filter(w => !w.minimized && w.showing_on_its_workspace?.() !== false &&
                !titoloSfondo(w) && !this._delDesktop(w) &&
                w.get_window_type() !== Meta.WindowType.DESKTOP)
            .map(w => w.get_frame_rect());
        return Main.layoutManager.monitors.map((m, i) => {
            const a = Main.layoutManager.getWorkAreaForMonitor(i);
            const intero = global.display.get_monitor_in_fullscreen(i);
            return [m.x, m.y, m.width, m.height, a.x, a.y, a.width, a.height,
                intero, intero || this._coperto(a, visibili), this._finestraPer(i) !== null,
                connettori.get(i) ?? ''];
        });
    }

    _aggiorna() {
        if (!this._dbus || this._finestre.size === 0)
            return;
        const schermi = this._schermi();
        const chiave = JSON.stringify(schermi);
        if (chiave === this._ultimo)
            return;
        this._ultimo = chiave;
        this._dbus.emit_signal('Cambiato', new GLib.Variant(`(${TIPO})`, [schermi]));
    }

    // D-Bus: lo stato adesso (ASCII Zoo lo chiede all'avvio, e così sa che l'estensione c'è)
    Stato() {
        return this._protetto(() => this._schermi()) ?? [];
    }
}
