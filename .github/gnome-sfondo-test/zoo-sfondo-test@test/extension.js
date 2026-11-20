// Prova dello sfondo sotto Wayland (linux/gnome/zoo-click@…/sfondo.js) con ASCII Zoo vero:
// lo avvia, controlla che la sua finestra sia al posto dello sfondo e nascosta da Alt+Tab e
// dalle app, fotografa il desktop, apre un'app a schermo intero, poi spegne l'estensione.
// Scrive righe "PROVA …" nel log di GNOME Shell; run.sh le controlla.
import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const PREFISSO = '@ascii-zoo-sfondo!';
const PASSI = [[3, '_avvia'], [35, '_controlla'], [37, '_foto'], [40, '_schermoIntero'], [55, '_chiudi'],
    [62, '_cascata'], [64, '_riaccendi'], [72, '_controlla'], [73, '_foto'], [78, '_spegni'], [96, '_fine']];

function log(m) {
    console.log(`PROVA ${m}`);
}

export default class Prova extends Extension {
    enable() {
        // GNOME la spegne e riaccende quando si spegne zoo-click (accesa prima): una volta sola
        if (globalThis.zooProvaPartita)
            return;
        globalThis.zooProvaPartita = true;
        this._dir = GLib.getenv('ZOO_PROVA');
        this._bin = GLib.getenv('ZOO_BIN');
        for (const [s, passo] of PASSI) {
            GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, s, () => {
                try {
                    this[passo]();
                } catch (e) {
                    log(`errore in ${passo}: ${e}`);
                }
                return GLib.SOURCE_REMOVE;
            });
        }
    }

    disable() {}

    _lancia(argv, env = {}) {
        const l = new Gio.SubprocessLauncher({flags: Gio.SubprocessFlags.NONE});
        for (const [k, v] of Object.entries(env))
            l.setenv(k, v, true);
        return l.spawnv(argv);
    }

    _avvia() {
        Main.overview.hide();
        log('avvio ASCII Zoo');
        this._lancia(['bash', '-c', 'exec "$ZOO_BIN" >> "$ZOO_PROVA/app.log" 2>&1'], {ASCII_ZOO_LOG: '1'});
    }

    _controlla() {
        Main.overview.hide();
        const tutte = global.display.list_all_windows();
        const nostre = tutte.filter(w => (w.get_title() ?? '').startsWith(PREFISSO));
        log(`finestre ${nostre.length} minimizzate ${nostre.length > 0 && nostre.every(w => w.minimized)}`);
        const tab = global.display.get_tab_list(Meta.TabList.NORMAL_ALL, null).filter(w => nostre.includes(w));
        log(`in alt-tab ${tab.length}`);
        const attori = global.get_window_actors().filter(a => nostre.includes(a.meta_window));
        log(`in get_window_actors ${attori.length}`);
        const tracker = Shell.WindowTracker.get_default();
        log(`con app ${nostre.filter(w => tracker.get_window_app(w)).length}`);
        const sorgenti = new Set(nostre.map(w => w.get_compositor_private()));
        let cloni = 0;
        const cerca = a => {
            for (const f of a.get_children()) {
                if (f instanceof Clutter.Clone && sorgenti.has(f.source) && f.mapped)
                    cloni++;
                cerca(f);
            }
        };
        cerca(Main.layoutManager._backgroundGroup);
        log(`cloni nello sfondo ${cloni}`);
    }

    _foto() {
        this._foto_n = (this._foto_n ?? 0) + 1;
        const percorso = `${this._dir}/desktop-${this._foto_n}.png`;
        const stream = Gio.File.new_for_path(percorso).replace(null, false, Gio.FileCreateFlags.NONE, null);
        new Shell.Screenshot().screenshot(false, stream, (o, res) => {
            try {
                o.screenshot_finish(res);
                stream.close(null);
                log(`foto ${percorso}`);
            } catch (e) {
                log(`foto non riuscita: ${e}`);
            }
        });
    }

    _schermoIntero() {
        log('app a schermo intero');
        this._app = this._lancia(['python3', '-c',
            'import gi; gi.require_version("Gtk", "3.0"); from gi.repository import Gtk\n' +
            'w = Gtk.Window(title="schermo intero"); w.add(Gtk.Label(label="prova")); w.fullscreen(); w.show_all(); Gtk.main()'],
        {GDK_BACKEND: 'wayland'});
    }

    _chiudi() {
        log('chiudo l\'app a schermo intero');
        this._app?.force_exit();
    }

    // un'altra estensione, accesa prima, si spegne e si riaccende: GNOME spegne e riaccende
    // anche zoo-click, ma ASCII Zoo deve restare dov'è
    _cascata() {
        log('spengo zoo-prima');
        Main.extensionManager.disableExtension('zoo-prima@test');
    }

    _riaccendi() {
        log('riaccendo zoo-prima');
        Main.extensionManager.enableExtension('zoo-prima@test');
    }

    _spegni() {
        log('spengo l\'estensione');
        Main.extensionManager.disableExtension('zoo-click@tressilloide.github.io');
    }

    _fine() {
        log('fine');
    }
}
