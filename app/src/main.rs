//! ASCII Zoo: sfondo animato per Windows e macOS.
//!
//! - lo sfondo è l'HTML in assets/, incorporato nell'eseguibile
//! - su ogni monitor una finestra con dentro una WebView viene messa dietro le icone del desktop
//! - un'icona (area di notifica su Windows, barra dei menu su macOS) ha tutte le opzioni
//!
//! La parte comune (specie, menu, impostazioni) è in `shared`; quella che mette le
//! finestre sul desktop è in `win` (Windows) e `mac` (macOS).

#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod shared;

#[cfg_attr(target_os = "macos", path = "dialog_mac.rs")]
mod dialog;
#[cfg_attr(target_os = "macos", path = "power_mac.rs")]
mod power;
#[cfg_attr(target_os = "macos", path = "stats_mac.rs")]
mod stats;

#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod win;

#[cfg(target_os = "macos")]
mod mac;

#[cfg(not(any(windows, target_os = "macos")))]
compile_error!("ASCII Zoo per ora funziona su Windows e macOS");

fn main() {
    #[cfg(windows)]
    win::run();
    #[cfg(target_os = "macos")]
    mac::run();
}
