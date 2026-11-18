//! Windows (linker MSVC):
//! - incorpora app.manifest nell'eseguibile (controlli di Windows moderni);
//! - aggiunge le informazioni sul file (nome del prodotto, versione, autori) che si
//!   vedono in Proprietà → Dettagli. La firma del codice (SignPath) le controlla:
//!   il nome del prodotto deve essere quello del progetto e la versione quella della build.

fn main() {
    println!("cargo:rerun-if-changed=app.manifest");
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc") {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap()).join("app.manifest");
        println!("cargo:rustc-link-arg-bins=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg-bins=/MANIFESTINPUT:{}", manifest.display());

        info_file();
    }
}

/// winresource c'è solo quando si compila su Windows (vedi Cargo.toml)
#[cfg(windows)]
fn info_file() {
    // FileVersion e ProductVersion arrivano da Cargo.toml
    let mut res = winresource::WindowsResource::new();
    res.set("ProductName", "ASCII Zoo")
        .set("FileDescription", "ASCII Zoo")
        .set("CompanyName", "ASCII Things")
        .set("LegalCopyright", "Copyright (c) 2026 Eric Cristelli and the ASCII Zoo contributors. MIT License.")
        .set("OriginalFilename", "ascii-zoo.exe")
        .set("InternalName", "ascii-zoo");
    res.compile().expect("informazioni sul file (winresource)");
}

#[cfg(not(windows))]
fn info_file() {}
