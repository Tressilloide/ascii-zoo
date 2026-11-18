//! Lo screensaver di ASCII Zoo: scene fatte di caratteri (stelle, pioggia, falò,
//! aurora, gioco della vita, mare di notte) con un motore nativo, senza WebView,
//! perché Windows lo rilancia da zero ogni volta e deve partire all'istante.
//!
//! L'app (`app/`) chiama `avvia` quando Windows la lancia come salvaschermo; il
//! programma `ascii-dreams` di questa cartella fa lo stesso, per provarlo da solo.
//!
//! Windows avvia i salvaschermo (.scr) con questi argomenti:
//!   /s           a schermo intero
//!   /p <hwnd>    anteprima nella finestrella delle impostazioni
//!   /c[:hwnd]    finestra delle impostazioni (anche senza argomenti)
//! In più:
//!   --finestra [--scena id]          in una finestra normale, per provarlo
//!   --installa                       lo copia nella cartella utente e apre le impostazioni di Windows
//!   --png file.png [opzioni]         disegna un fotogramma senza finestra
//!   --fotogrammi cartella [opzioni]  salva tutti i fotogrammi (per farne un video)
//!     opzioni: --scena id  --secondi n  --dim 1920x1080  --senza-orologio

mod config;
mod grid;
mod clock;
mod painter;
mod director;
mod scenes;
#[cfg(windows)]
mod win;

use config::Config;
use clock::Ora;

/// true se Windows ha lanciato il programma come salvaschermo: `/s`, `/p`, `/c`
/// (anche `-s` o `/s:...`), oppure senza argomenti ma da un file `.scr`
/// (Windows apre così le impostazioni)
pub fn chiamata_da_salvaschermo(args: &[String]) -> bool {
    if let Some(a) = args.first() {
        let a = a.to_ascii_lowercase();
        let b = a.as_bytes();
        return b.len() >= 2 && (b[0] == b'/' || b[0] == b'-') && matches!(b[1], b's' | b'p' | b'c') && (b.len() == 2 || b[2] == b':');
    }
    std::env::current_exe()
        .ok()
        .and_then(|p| p.extension().map(|e| e.eq_ignore_ascii_case("scr")))
        .unwrap_or(false)
}

/// Copia il programma come `ascii-zoo.scr` nella cartella utente e apre le
/// impostazioni del salvaschermo di Windows con lui già scelto.
#[cfg(windows)]
pub fn installa() {
    win::installa();
}

/// Se lo screensaver è già installato ma è di una versione diversa da questo
/// programma, lo aggiorna (così dopo un aggiornamento dell'app non resta indietro).
#[cfg(windows)]
pub fn aggiorna_se_installato() {
    win::aggiorna_se_installato();
}

/// Fa quello che dicono gli argomenti (senza il nome del programma).
pub fn avvia(args: &[String]) {
    let args: Vec<String> = args.to_vec();
    let primo = args.first().map(|s| s.to_ascii_lowercase()).unwrap_or_default();

    if primo == "--png" || primo == "--fotogrammi" {
        if let Err(e) = senza_finestra(&args) {
            eprintln!("errore: {e}");
            std::process::exit(1);
        }
        return;
    }

    #[cfg(windows)]
    {
        let valore = |nome: &str| args.iter().position(|a| a == nome).and_then(|i| args.get(i + 1)).cloned();
        // le scelte passate da riga di comando (pulsante "Preview") vincono su quelle salvate
        let mut cfg = Config::carica();
        if let Some(n) = valore("--minuti").and_then(|s| s.parse().ok()) {
            cfg.minuti = n;
        }
        if let Some(n) = valore("--righe").and_then(|s| s.parse().ok()) {
            cfg.righe = n;
        }
        if args.iter().any(|a| a == "--senza-orologio") {
            cfg.orologio = false;
        }
        if primo.starts_with("/s") || primo.starts_with("-s") {
            if let Some(s) = valore("--scena") {
                cfg.scena = s;
            }
        }
        if primo == "--finestra" {
            let mut cfg = cfg;
            if let Some(s) = valore("--scena") {
                cfg.scena = s;
            }
            win::avvia(win::Modo::Finestra, &cfg);
        } else if primo == "--installa" {
            win::installa();
        } else if primo.starts_with("/s") || primo.starts_with("-s") {
            win::avvia(win::Modo::SchermoIntero, &cfg);
        } else if primo.starts_with("/p") || primo.starts_with("-p") {
            // "/p 1234" oppure "/p:1234"
            let n = primo.split(':').nth(1).map(str::to_string).or_else(|| args.get(1).cloned());
            if let Some(h) = n.and_then(|s| s.trim().parse::<isize>().ok()) {
                win::avvia(win::Modo::Anteprima(h), &cfg);
            }
        } else {
            win::impostazioni(cfg);
        }
    }
    #[cfg(not(windows))]
    {
        eprintln!("Per ora il salvaschermo funziona su Windows. Qui si può usare: --png file.png [--scena id]");
    }
}

/// ora del sistema (locale su Windows, UTC altrove)
pub fn ora_adesso() -> Ora {
    #[cfg(windows)]
    {
        let s = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
        Ora {
            ore: s.wHour as u32,
            minuti: s.wMinute as u32,
            secondi: s.wSecond as u32,
            giorno_settimana: s.wDayOfWeek as u32,
            giorno: s.wDay as u32,
            mese: s.wMonth as u32,
        }
    }
    #[cfg(not(windows))]
    {
        let secs = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
        let giorni = secs / 86400;
        let (anno_g, mese, giorno) = civile(giorni as i64);
        let _ = anno_g;
        Ora {
            ore: (secs / 3600 % 24) as u32,
            minuti: (secs / 60 % 60) as u32,
            secondi: (secs % 60) as u32,
            giorno_settimana: ((giorni + 4) % 7) as u32,
            giorno,
            mese,
        }
    }
}

#[cfg(not(windows))]
fn civile(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + if m <= 2 { 1 } else { 0 }, m, d)
}

/// seme casuale dall'orologio
pub fn seme() -> u64 {
    std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos() as u64).unwrap_or(42)
}

fn senza_finestra(args: &[String]) -> Result<(), String> {
    let valore = |nome: &str| args.iter().position(|a| a == nome).and_then(|i| args.get(i + 1)).cloned();
    let uscita = args.get(1).cloned().ok_or("manca il nome del file o della cartella")?;
    let tutti = args[0] == "--fotogrammi";
    let mut cfg = Config::default();
    cfg.scena = valore("--scena").unwrap_or_else(|| "stelle".into());
    cfg.orologio = !args.iter().any(|a| a == "--senza-orologio");
    if let Some(r) = valore("--righe").and_then(|s| s.parse().ok()) {
        cfg.righe = r;
    }
    let secondi: f32 = valore("--secondi").and_then(|s| s.parse().ok()).unwrap_or(4.0);
    let (w, h) = valore("--dim")
        .and_then(|d| {
            let (a, b) = d.split_once('x')?;
            Some((a.parse().ok()?, b.parse().ok()?))
        })
        .unwrap_or((1920usize, 1080usize));

    let pittore = painter::Pittore::new(h / cfg.righe as usize);
    let (gw, gh) = pittore.misura(w, h);
    let mut regia = director::Regia::new(gw, gh, &cfg, 7);
    let mut buf = vec![0u32; w * h];
    let fps = 30.0;
    let n = (secondi * fps) as usize;
    let ora = Ora { ore: 23, minuti: 41, secondi: 0, giorno_settimana: 2, giorno: 18, mese: 11 };
    if tutti {
        std::fs::create_dir_all(&uscita).map_err(|e| e.to_string())?;
    }
    let inizio = std::time::Instant::now();
    for i in 0..n.max(1) {
        let mut o = ora;
        o.secondi = (i as f32 / fps) as u32 % 60;
        let luce = regia.passo(1.0 / fps, o);
        if tutti || i + 1 == n.max(1) {
            pittore.dipingi(&regia.griglia, &mut buf, w, h, luce);
            let nome = if tutti { format!("{uscita}/{i:05}.png") } else { uscita.clone() };
            salva_png(&nome, &buf, w, h)?;
        }
    }
    // tempo medio di un fotogramma completo (simulazione + disegno)
    let t0 = std::time::Instant::now();
    for _ in 0..30 {
        let luce = regia.passo(1.0 / fps, ora);
        pittore.dipingi(&regia.griglia, &mut buf, w, h, luce);
    }
    eprintln!(
        "{} fotogrammi in {:.1}s; un fotogramma: {:.1} ms; griglia {}x{}",
        n,
        inizio.elapsed().as_secs_f32(),
        t0.elapsed().as_secs_f32() * 1000.0 / 30.0,
        gw,
        gh
    );
    Ok(())
}

fn salva_png(nome: &str, buf: &[u32], w: usize, h: usize) -> Result<(), String> {
    let f = std::fs::File::create(nome).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(f), w as u32, h as u32);
    enc.set_color(png::ColorType::Rgb);
    enc.set_depth(png::BitDepth::Eight);
    let mut wr = enc.write_header().map_err(|e| e.to_string())?;
    let mut dati = Vec::with_capacity(w * h * 3);
    for &p in buf {
        dati.extend_from_slice(&[(p >> 16) as u8, (p >> 8) as u8, p as u8]);
    }
    wr.write_image_data(&dati).map_err(|e| e.to_string())
}
