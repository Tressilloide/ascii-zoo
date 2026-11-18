// Solo per la prova automatica (.github/prova-clic-gnome/prova.sh): simula clic con un
// mouse virtuale di Clutter, come li farebbe una persona.
import GLib from 'gi://GLib';
import Clutter from 'gi://Clutter';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

const log = (...a) => console.log('PROVA', ...a);

export default class ProvaClic extends Extension {
    enable() {
        const seat = Clutter.get_default_backend().get_default_seat();
        this._mouse = seat.create_virtual_device(Clutter.InputDeviceType.POINTER_DEVICE);
        const t = () => GLib.get_monotonic_time();
        const clic = (x, y, cosa) => {
            this._mouse.notify_absolute_motion(t(), x, y);
            GLib.timeout_add(GLib.PRIORITY_DEFAULT, 150, () => {
                log('clic', cosa, x, y);
                this._mouse.notify_button(t(), Clutter.BUTTON_PRIMARY, Clutter.ButtonState.PRESSED);
                GLib.timeout_add(GLib.PRIORITY_DEFAULT, 90, () => {
                    this._mouse.notify_button(t(), Clutter.BUTTON_PRIMARY, Clutter.ButtonState.RELEASED);
                    return GLib.SOURCE_REMOVE;
                });
                return GLib.SOURCE_REMOVE;
            });
        };
        const dopo = (s, f) => GLib.timeout_add(GLib.PRIORITY_DEFAULT, s * 1000, () => {
            try {
                f();
            } catch (e) {
                log('errore', e);
            }
            return GLib.SOURCE_REMOVE;
        });
        dopo(12, () => Main.overview.hide());
        dopo(15, () => clic(700, 500, 'desktop (DING)'));
        dopo(17, () => clic(1500, 900, 'desktop (DING) 2'));
        dopo(18, () => GLib.spawn_command_line_async('zenity --info --text=finestra --width=500 --height=300'));
        dopo(23, () => {
            const w = global.display.list_all_windows().find(f => (f.get_wm_class() ?? '').toLowerCase().includes('zenity'));
            if (!w) {
                log('finestra normale NON trovata');
                return;
            }
            log('finestra normale aperta');
            const r = w.get_frame_rect();
            clic(r.x + r.width / 2, r.y + 30, 'finestra normale');
        });
        dopo(25, () => clic(900, 8, 'barra in alto'));
        dopo(27, () => clic(300, 700, 'desktop per chiudere il menu'));
    }

    disable() {
        this._mouse = null;
    }
}
