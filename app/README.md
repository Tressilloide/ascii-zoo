# ASCII Zoo — programma per Windows, macOS e Linux (Rust)

Mette lo sfondo animato (`sfondo/index.html`) dietro le icone del desktop, su tutti gli schermi.

## Cosa fa
- Sfondo HTML incorporato nell'eseguibile, mostrato con una WebView (WebView2 su Windows,
  WKWebView su macOS, WebKitGTK su Linux).
- **Windows**: una finestra dietro le icone per ogni monitor (funziona anche con il desktop
  "raised" di Windows 11 24H2+). Se un monitor viene collegato o scollegato, cambia
  risoluzione o Explorer riparte, gli sfondi vengono rifatti da soli.
- **macOS**: una finestra per ogni schermo al livello del desktop: sopra l'immagine di sfondo,
  sotto le icone del Finder e tutte le finestre, in tutti gli Spazi, trasparente ai clic e
  assente da Mission Control. Gli schermi vengono ricontrollati ogni 2 secondi.
- **Linux**: una finestra X11 di tipo "desktop" per ogni schermo: sotto tutte le finestre,
  su tutti gli spazi di lavoro, fuori dalla barra delle applicazioni e trasparente ai clic.
  Sotto Wayland passa da XWayland (`GDK_BACKEND=x11`), perché Wayland non permette a un
  programma normale di mettersi al posto dello sfondo. Una sola copia per utente.
- Icona a forma di koi (area di notifica su Windows, barra dei menu su macOS, barra in alto
  su Linux con AppIndicator; niente icona nel Dock) con il menu:
  - **Animale**: a rotazione oppure uno fisso
  - **Rotazione**: ogni quanto cambia animale (da 30 secondi a un'ora) e quali animali far girare
    (almeno uno resta sempre spuntato)
  - **Nomi degli animali**: il nome dell'animale più grande di ogni specie, scritto in una
    finestrella (Invio = OK, Esc = annulla, vuoto = nessun nome, al massimo 16 lettere);
    di serie le koi hanno KARPISA
  - **Koi nello stagno in carica** (vale con "A rotazione")
  - **Pausa a batteria**; solo su Windows anche **Pausa con app a schermo intero** (solo
    sullo schermo dell'app; su macOS le app a schermo intero stanno in uno Spazio a parte) e
    **Pausa quando le finestre coprono il desktop** (schermo per schermo: ogni mezzo secondo
    si prova una griglia di punti, uno ogni 160 pixel circa; se nessuno cade sul desktop,
    quello schermo si ferma e riparte appena il desktop torna a vedersi)
  - **Avvia con Windows** / **Apri al login**, **Ricarica sfondo**, **Esci**
- Impostazioni salvate in `%APPDATA%\AsciiZoo\config.json` (Windows),
  `~/Library/Application Support/AsciiZoo/config.json` (macOS) o
  `~/.config/AsciiZoo/config.json` (Linux, `$XDG_CONFIG_HOME` se impostata).
- Avvio automatico: valore `AsciiZoo` nella chiave `HKCU\…\Run` (Windows),
  `~/Library/LaunchAgents/io.github.tressilloide.asciizoo.plist` (macOS) o
  `~/.config/autostart/ascii-zoo.desktop` (Linux), solo se attivato.

> Su un computer fisso (senza batteria) il sistema risulta sempre "in carica": con
> **Koi nello stagno in carica** attivo lo sfondo mostra sempre le koi.
> Per avere la rotazione degli animali togli la spunta a quella voce.

## Compilare

### Windows
Servono Rust (toolchain `x86_64-pc-windows-msvc`) e i Build Tools di Visual Studio
con il Windows SDK. Da PowerShell, nella cartella `app`:

```
cargo build --release --locked
```

L'eseguibile è in `target\release\ascii-zoo.exe`. Conviene copiarlo in una cartella
fissa (es. `%LOCALAPPDATA%\AsciiZoo`) prima di attivare **Avvia con Windows**,
perché l'avvio automatico ricorda il percorso da cui è stato lanciato.

### macOS
Servono Rust e gli strumenti da riga di comando di Xcode (`xcode-select --install`).
Nella cartella `app`:

```
cargo build --release --locked
```

L'eseguibile è `target/release/ascii-zoo` e si può avviare così. Per avere l'app vera
(`ASCII Zoo.app`, senza icona nel Dock) e il DMG, guarda il job `macos` di
[`../.github/workflows/build.yml`](../.github/workflows/build.yml): mette insieme l'app
con `app/macos/Info.plist` e crea il DMG con `hdiutil`.

### Linux
Il modo semplice è [`../linux/install.sh`](../linux/install.sh), che installa anche le
librerie. A mano servono Rust e i pacchetti di sviluppo di WebKitGTK 4.1, GTK 3,
libayatana-appindicator3 e libxdo (su Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev
libayatana-appindicator3-dev libxdo-dev build-essential pkg-config`). Nella cartella `app`:

```
cargo build --release --locked
```

L'eseguibile è `target/release/ascii-zoo`. Il job `linux` del workflow esegue
`install.sh` per intero su Ubuntu 24.04 e avvia il programma sotto Xvfb come prova.

> Lo sfondo è una copia in `assets/`: dopo aver modificato `sfondo/index.html`
> (nella radice della repo) copialo in `assets/ascii-zoo-sfondo.html` e ricompila.
> La build su GitHub fallisce se le due copie sono diverse.

## Pubblicare una versione
Exe e DMG delle Releases li compila GitHub Actions ([`../.github/workflows/build.yml`](../.github/workflows/build.yml)),
mai il PC di qualcuno. Il workflow compila anche a ogni push e pull request (exe e DMG si
trovano tra gli artifact della run, e su macOS l'app viene avviata e fotografata come prova),
ma pubblica solo con un tag:

1. aggiorna `version` in `Cargo.toml` (es. `0.2.0`) e fai `cargo build` per aggiornare `Cargo.lock`;
2. fai il commit su `main`;
3. crea e pubblica il tag con la stessa versione:
   ```
   git tag v0.2.0
   git push origin v0.2.0
   ```

Il workflow controlla che tag e versione coincidano, compila, calcola gli SHA256, crea
l'attestazione di provenienza e pubblica la Release con `ascii-zoo.exe`,
`ascii-zoo-macos.dmg` e i loro `.sha256`.

## File
| File | Cosa contiene |
|---|---|
| `src/main.rs` | sceglie la parte per il sistema in uso |
| `src/shared.rs` | parte comune: specie, menu dell'icona, impostazioni da mandare allo sfondo |
| `src/win.rs` | Windows: finestre e WebView per monitor, timer, cambi di schermo, clic sul desktop, margini della barra delle applicazioni |
| `src/desktop.rs` | Windows: aggancio dietro le icone (WorkerW / Progman) |
| `src/power.rs`, `src/dialog.rs` | Windows: batteria e schermo intero, finestrella dei nomi |
| `src/stats.rs`, `src/stats_mac.rs` | CPU, RAM (e batteria su Windows) per il widget "pc" |
| `src/mac.rs` | macOS: finestre al livello del desktop, barra dei menu, controllo degli schermi |
| `src/power_mac.rs`, `src/dialog_mac.rs` | macOS: batteria, finestrella dei nomi |
| `src/linux.rs` | Linux: finestre di tipo "desktop", icona AppIndicator, controllo degli schermi, margini della barra in alto |
| `src/power_linux.rs`, `src/stats_linux.rs`, `src/dialog_linux.rs` | Linux: batteria e CPU/RAM da `/sys` e `/proc`, finestrella dei nomi (zenity o kdialog) |
| `src/config.rs` | impostazioni (nomi, rotazione, widget, animale per schermo) e avvio automatico |
| `macos/Info.plist` | descrizione dell'app per macOS |
| `build.rs`, `app.manifest` | incorporano il manifest dei controlli moderni di Windows (solo linker MSVC) |
