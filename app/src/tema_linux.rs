//! Tema del sistema su Linux, per "Tema > Come il sistema". Non ci si fida di
//! prefers-color-scheme dentro WebKitGTK: si chiede al sistema.
//!
//! 1. il portale (org.freedesktop.portal.Settings, "org.freedesktop.appearance"
//!    "color-scheme": 1 scuro, 2 chiaro, 0 nessuna preferenza): GNOME, KDE e altri;
//! 2. le impostazioni di GNOME (org.gnome.desktop.interface): "color-scheme", poi il nome
//!    del tema GTK ("Yaru-dark", "Adwaita-dark"…).

use gtk::gio;
use gtk::glib;
use gtk::prelude::*;

/// Some(true) se il sistema è scuro, Some(false) se è chiaro, None se non si capisce
pub fn scuro() -> Option<bool> {
    match portale() {
        Some(1) => Some(true),
        Some(2) => Some(false),
        _ => gnome(),
    }
}

fn portale() -> Option<u32> {
    let bus = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE).ok()?;
    // ReadOne c'è dalla versione 2 del portale; Read nelle vecchie (una scatola in più)
    for metodo in ["ReadOne", "Read"] {
        let risposta = bus.call_sync(
            Some("org.freedesktop.portal.Desktop"),
            "/org/freedesktop/portal/desktop",
            "org.freedesktop.portal.Settings",
            metodo,
            Some(&("org.freedesktop.appearance", "color-scheme").to_variant()),
            None,
            gio::DBusCallFlags::NO_AUTO_START,
            300,
            gio::Cancellable::NONE,
        );
        if let Some(n) = risposta.ok().as_ref().and_then(numero) {
            return Some(n);
        }
    }
    None
}

/// il numero dentro (v), <v>, <<v>>…
fn numero(v: &glib::Variant) -> Option<u32> {
    if let Some(n) = v.get::<u32>() {
        return Some(n);
    }
    (v.is_container() && v.n_children() == 1).then(|| numero(&v.child_value(0))).flatten()
}

/// Impostazioni di GNOME. Uno schema o una chiave che non ci sono farebbero chiudere il
/// programma (le vecchie versioni non hanno "color-scheme"): prima si controlla che ci siano.
fn gnome() -> Option<bool> {
    const SCHEMA: &str = "org.gnome.desktop.interface";
    let schema = gio::SettingsSchemaSource::default()?.lookup(SCHEMA, true)?;
    let s = gio::Settings::new(SCHEMA);
    if schema.has_key("color-scheme") {
        match s.string("color-scheme").as_str() {
            "prefer-dark" => return Some(true),
            "prefer-light" => return Some(false),
            _ => {}
        }
    }
    if schema.has_key("gtk-theme") {
        let tema = s.string("gtk-theme").to_lowercase();
        if !tema.is_empty() {
            return Some(tema.contains("-dark"));
        }
    }
    None
}
