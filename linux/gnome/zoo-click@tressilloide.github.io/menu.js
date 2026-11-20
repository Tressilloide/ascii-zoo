// ASCII Zoo: il menu dell'icona nella barra in alto (GNOME Shell 45 e successivi).
//
// Su GNOME il menu dell'icona lo disegna GNOME Shell (estensione AppIndicator), e i sottomenu
// si aprono dentro il menu stesso, sotto la loro voce. Il menu di ASCII Zoo ha una ventina di
// voci: su uno schermo 1920×1080 al 125% restavano ~60 pixel per il sottomenu aperto, e si
// scorreva un animale alla volta. Qui, solo per il menu di ASCII Zoo, quando si apre un
// sottomenu le altre voci si nascondono e il sottomenu ha tutta l'altezza; richiudendolo (o
// chiudendo il menu) tornano. Vale anche per i sottomenu dentro un sottomenu.
// Ci si aggancia ai sottomenu standard di GNOME Shell (PopupSubMenu), non all'estensione
// AppIndicator: gli altri menu non cambiano.

import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import {InjectionManager} from 'resource:///org/gnome/shell/extensions/extension.js';

// il menu di ASCII Zoo: ha queste voci in alto
const VOCI_ZOO = ['Animale', 'Esci'];

function etichetta(voce) {
    return voce?.label?.text ?? '';
}

function menuZoo(menu) {
    const voci = menu?._getMenuItems?.() ?? [];
    return VOCI_ZOO.every(n => voci.some(v => etichetta(v) === n));
}

export class MenuZoo {
    enable() {
        this._nascoste = new Map(); // sottomenu aperto → voci nascoste per lui
        this._errori = 0;
        const io = this;
        this._iniezioni = new InjectionManager();
        this._iniezioni.overrideMethod(PopupMenu.PopupSubMenu.prototype, 'open', originale => function (...a) {
            io._protetto(() => io._apri(this));
            return originale.apply(this, a);
        });
        this._iniezioni.overrideMethod(PopupMenu.PopupSubMenu.prototype, 'close', originale => function (...a) {
            const r = originale.apply(this, a);
            io._protetto(() => io._chiudi(this));
            return r;
        });
    }

    disable() {
        this._iniezioni?.clear();
        this._iniezioni = null;
        for (const sub of [...this._nascoste?.keys() ?? []])
            this._protetto(() => this._chiudi(sub));
        this._nascoste = null;
    }

    _protetto(f) {
        try {
            f();
        } catch (e) {
            if (this._errori++ < 5)
                console.error(`ASCII Zoo menu: ${e}`);
        }
    }

    // prima che il sottomenu si apra (GNOME decide allora se gli serve la barra di
    // scorrimento): si nascondono le voci intorno a lui e intorno ai sottomenu che lo contengono
    _apri(sub) {
        if (sub.isOpen || sub.isEmpty() || this._nascoste.has(sub) || !menuZoo(sub._getTopMenu()))
            return;
        const nascoste = [];
        for (let voce = sub.sourceActor; voce?._parent; voce = voce._parent.sourceActor) {
            // solo le voci: un altro sottomenu aperto lo chiude GNOME stesso, e non va rimostrato
            for (const altra of voce._parent._getMenuItems()) {
                if (altra !== voce && altra.visible) {
                    altra.hide();
                    nascoste.push(altra);
                }
            }
        }
        this._nascoste.set(sub, nascoste);
    }

    _chiudi(sub) {
        const nascoste = this._nascoste?.get(sub);
        if (!nascoste)
            return;
        this._nascoste.delete(sub);
        for (const a of nascoste)
            a.show();
    }
}
