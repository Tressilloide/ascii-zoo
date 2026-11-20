// Non fa niente: è accesa prima di zoo-click, e quando la prova la spegne e la riaccende GNOME
// spegne e riaccende in cascata anche zoo-click (come quando l'utente tocca un'altra estensione)
import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class Prima extends Extension {
    enable() {}
    disable() {}
}
