//! Lo screensaver da solo, per provarlo senza l'app: `cargo run --release -- --finestra`
//! (gli argomenti sono quelli descritti in lib.rs).

#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    ascii_dreams::avvia(&args);
}
