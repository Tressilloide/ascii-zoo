// Solo per la prova automatica (.github/gnome-screensaver-test/run.sh): avvia ASCII Zoo
// dentro GNOME Shell (così ha DISPLAY e WAYLAND_DISPLAY giusti), apre una finestra normale
// che ha il focus e, per un minuto, un'app a schermo intero. Quando compare la finestra
// dello screensaver guarda se sta davvero sopra a tutto, la fotografa e simula un input:
// lo screensaver si deve chiudere.
import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import Clutter from 'gi://Clutter';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const log = (...a) => console.log('PROVA', ...a);
const TITOLO = 'ASCII Zoo screensaver';

export default class ProvaScreensaver extends Extension {
    enable() {
        const seat = Clutter.get_default_backend().get_default_seat();
        this._tastiera = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
        this._inizio = GLib.get_monotonic_time();
        const secondi = () => ((GLib.get_monotonic_time() - this._inizio) / 1e6).toFixed(1);
        const dopo = (s, f) => GLib.timeout_add(GLib.PRIORITY_DEFAULT, s * 1000, () => {
            try {
                f();
            } catch (e) {
                log('errore', e);
            }
            return GLib.SOURCE_REMOVE;
        });

        dopo(2, () => Main.overview.hide());
        dopo(3, () => {
            log('avvio ASCII Zoo');
            GLib.spawn_async(null, ['bash', '-c', 'ASCII_ZOO_LOG=1 exec "$ZOO_BIN" 2> "$ZOO_LOG"'],
                null, GLib.SpawnFlags.SEARCH_PATH, null);
        });
        // una finestra normale con il focus, come quando ci si allontana dal PC
        dopo(8, () => GLib.spawn_command_line_async('zenity --info --text=finestra --width=500 --height=300'));
        // un'app a schermo intero dal secondo 15 al 75: lo screensaver non deve partirle sopra
        // (il minuto senza input finisce al secondo 60) ma subito dopo, se il PC è ancora inattivo
        dopo(15, () => {
            log('apro un\'app a schermo intero per 60 s');
            GLib.spawn_async(null, ['python3', GLib.getenv('ZOO_SCHERMO_INTERO'), '60'], null,
                GLib.SpawnFlags.SEARCH_PATH, null);
        });
        dopo(76, () => log('app a schermo intero chiusa'));

        // primo giro: tasto da una tastiera virtuale (lo vede lo screensaver, se ha il focus,
        // e mutter). Secondo giro, dopo un altro minuto senza input: ResetIdletime, quello che
        // mutter fa a ogni input, ma che lo screensaver non vede: lo deve chiudere ASCII Zoo.
        this._giro = 0;
        const guarda = (w) => {
            if (w._zooVisto || w.get_title() !== TITOLO)
                return;
            w._zooVisto = true;
            const giro = ++this._giro;
            log(`giro ${giro}: screensaver comparso dopo ${secondi()} s`);
            let input = null;
            w.connect('unmanaged', () => {
                const ms = input ? Math.round((GLib.get_monotonic_time() - input) / 1000) : null;
                log(ms === null ? `giro ${giro}: screensaver chiuso PRIMA dell'input`
                    : `giro ${giro}: screensaver chiuso ${ms} ms dopo l'input`);
                if (giro === 2)
                    dopo(2, () => log('fine'));
            });
            if (giro === 1) {
                dopo(4, () => {
                    this._stato(w);
                    this._foto();
                });
                dopo(6, () => {
                    log('giro 1: premo Maiusc');
                    input = GLib.get_monotonic_time();
                    this._tastiera.notify_keyval(input, Clutter.KEY_Shift_L, Clutter.KeyState.PRESSED);
                    this._tastiera.notify_keyval(GLib.get_monotonic_time(), Clutter.KEY_Shift_L, Clutter.KeyState.RELEASED);
                });
            } else {
                dopo(3, () => {
                    log('giro 2: ResetIdletime');
                    input = GLib.get_monotonic_time();
                    Gio.DBus.session.call('org.gnome.Mutter.IdleMonitor', '/org/gnome/Mutter/IdleMonitor/Core',
                        'org.gnome.Mutter.IdleMonitor', 'ResetIdletime', null, null, Gio.DBusCallFlags.NONE, -1, null,
                        (c, res) => {
                            try {
                                c.call_finish(res);
                            } catch (e) {
                                log('giro 2: ResetIdletime non riuscito', e);
                            }
                        });
                });
            }
        };
        this._id = global.display.connect('window-created', (d, w) => {
            guarda(w);
            // le finestre X11 a volte hanno il titolo solo dopo
            w.connect('notify::title', () => guarda(w));
        });
    }

    _stato(w) {
        const tutte = global.display.sort_windows_by_stacking(global.display.list_all_windows());
        const zenity = tutte.find(f => (f.get_wm_class() ?? '').toLowerCase().includes('zenity'));
        // cosa c'è davvero al centro dello schermo: la finestra dello screensaver?
        const scelto = global.stage.get_actor_at_pos(Clutter.PickMode.REACTIVE, 960, 540);
        let a = scelto;
        while (a && !(a instanceof Meta.WindowActor))
            a = a.get_parent();
        const centro = a ? a.get_meta_window().get_title() : `${scelto}`;
        log(`stato: focus=${w.has_focus()} fullscreen=${w.is_fullscreen()} sopra=${w.is_above()}`,
            `ultima-della-pila=${tutte[tutte.length - 1] === w} al-centro="${centro}"`,
            `monitor-in-fullscreen=${global.display.get_monitor_in_fullscreen(0)}`,
            `barra-in-alto=${Main.layoutManager.panelBox.visible}`,
            `zenity=${zenity ? 'aperta' : 'assente'}`);
        if (centro === TITOLO && !Main.layoutManager.panelBox.visible)
            log('screensaver VISIBILE sopra a tutto');
        else
            log('screensaver NON visibile sopra a tutto');
    }

    _foto() {
        const percorso = GLib.getenv('ZOO_FOTO');
        if (!percorso)
            return;
        try {
            const stream = Gio.File.new_for_path(percorso).replace(null, false, Gio.FileCreateFlags.NONE, null);
            new Shell.Screenshot().screenshot(false, stream, (o, res) => {
                try {
                    o.screenshot_finish(res);
                    log('foto salvata');
                } catch (e) {
                    log('foto non riuscita', e);
                }
                stream.close(null);
            });
        } catch (e) {
            log('foto non riuscita', e);
        }
    }

    disable() {
        if (this._id)
            global.display.disconnect(this._id);
        this._tastiera = null;
    }
}
