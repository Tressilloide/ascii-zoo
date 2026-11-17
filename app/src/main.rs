//! ASCII Zoo: sfondo animato per Windows, macOS e Linux.
//!
//! - lo sfondo è l'HTML in assets/, incorporato nell'eseguibile
//! - su ogni monitor una finestra con dentro una WebView viene messa dietro le icone del desktop
//! - un'icona (area di notifica su Windows, barra dei menu su macOS, barra in alto su
//!   Linux) ha tutte le opzioni
//!
//! La parte comune (specie, menu, impostazioni) è in `shared`; quella che mette le
//! finestre sul desktop è in `win` (Windows), `mac` (macOS) e `linux` (Linux).

#![cfg_attr(windows, windows_subsystem = "windows")]

mod config;
mod shared;

#[cfg_attr(target_os = "macos", path = "dialog_mac.rs")]
#[cfg_attr(target_os = "linux", path = "dialog_linux.rs")]
mod dialog;
#[cfg_attr(target_os = "macos", path = "power_mac.rs")]
#[cfg_attr(target_os = "linux", path = "power_linux.rs")]
mod power;
#[cfg_attr(target_os = "macos", path = "stats_mac.rs")]
#[cfg_attr(target_os = "linux", path = "stats_linux.rs")]
mod stats;

#[cfg(windows)]
mod desktop;
#[cfg(windows)]
mod win;

#[cfg(target_os = "macos")]
mod mac;

#[cfg(target_os = "linux")]
mod linux;

#[cfg(not(any(windows, target_os = "macos", target_os = "linux")))]
compile_error!("ASCII Zoo per ora funziona su Windows, macOS e Linux");

fn main() {
    #[cfg(windows)]
    win::run();
    #[cfg(target_os = "macos")]
    mac::run();
    #[cfg(target_os = "linux")]
    linux::run();
}
