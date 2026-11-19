# ASCII Zoo — programma per Windows, macOS e Linux (Rust)

Mette lo sfondo animato (`wallpaper/index.html`) dietro le icone del desktop, su tutti gli schermi.

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
  programma normale di mettersi al posto dello sfondo.
- **Una sola copia**, e la nuova prende il posto della vecchia: se all'avvio ne trova già
  una (magari una versione vecchia, anche da un'altra cartella) le chiede di uscire e, se
  dopo qualche secondo c'è ancora, la chiude per forza. Windows: WM_CLOSE alla finestra
  `AsciiZooControl` e attesa del mutex `AsciiZoo.SingleInstance`; macOS: le app con lo
  stesso bundle id (`terminate`, poi `forceTerminate`); Linux: il processo che tiene il
  socket astratto `ascii-zoo-$USER` (SIGTERM, poi SIGKILL). Se l'avvio automatico è attivo
  e lancia un altro eseguibile, lo fa puntare a sé (non da `target/`, dalla cartella
  temporanea, dal DMG o da una .app traslocata da Gatekeeper). Non cancella mai file.
- Icona a forma di koi (area di notifica su Windows, barra dei menu su macOS, barra in alto
  su Linux con AppIndicator; niente icona nel Dock) con il menu:
  - **Animale**: a rotazione oppure uno fisso
  - **Rotazione**: ogni quanto cambia animale (da 30 secondi a un'ora) e quali animali far girare
    (almeno uno resta sempre spuntato)
  - **Nomi degli animali**: il nome dell'animale più grande di ogni specie, scritto in una
    finestrella (Invio = OK, Esc = annulla, vuoto = nessun nome, al massimo 16 lettere);
    di serie le koi hanno KARPISA
  - **Tema**: **Scuro** (di serie), **Chiaro** o **Come il sistema**; la pagina riceve
    `zooSetting('tema', 'scuro' | 'chiaro' | 'auto')`. Con "Come il sistema" il programma
    legge il tema da sé, all'avvio e poi ogni 2 secondi, e lo manda solo quando cambia:
    Windows `AppsUseLightTheme` in `HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`;
    macOS l'aspetto effettivo dell'app (`NSApp.effectiveAppearance`, Aqua o DarkAqua); Linux
    il portale (`org.freedesktop.portal.Settings`, `org.freedesktop.appearance` `color-scheme`),
    poi `color-scheme` e `gtk-theme` di `org.gnome.desktop.interface`. Se non lo capisce
    manda `auto` e decide la pagina con `prefers-color-scheme`
  - **Orologio al centro** (di serie acceso): senza, la pagina riceve
    `zooSetting('orologio', false)` e gli animali e la scenografia usano anche il centro
  - **Koi nello stagno in carica** (vale con "A rotazione")
  - **Clic sul desktop agli animali**: un clic sinistro sul desktop arriva alla pagina con
    `zooPoke(x, y)`. Windows: un hook del mouse (`WH_MOUSE_LL`) che guarda la classe della
    finestra sotto il punto. macOS: un monitor globale di `NSEvent`; la finestra che riceve
    il clic (quella dell'evento, o `windowNumberAtPoint`) deve essere al livello delle icone
    del Finder o più giù. Non basta la prima finestra sotto il punto nella lista di
    `CGWindowListCopyWindowInfo`: il Dock ne ha una grande quanto lo schermo, trasparente ai
    clic fuori dalla sua striscia (lo ha mostrato la prova in CI). Linux: l'estensione di GNOME Shell in
    [`../linux/gnome`](../linux/gnome) chiama il metodo D-Bus `Clic(x, y, larghezza,
    altezza)` sul nome `io.github.tressilloide.AsciiZoo`, che il programma tiene solo con
    l'opzione accesa
  - **Pausa a batteria**, **Pausa con app a schermo intero** (solo sullo schermo dell'app) e
    **Pausa quando le finestre coprono il desktop**, uguali sui tre sistemi. Windows: ogni
    mezzo secondo si prova una griglia di punti, uno ogni 160 pixel circa; se nessuno cade
    sul desktop, quello schermo si ferma e riparte appena il desktop torna a vedersi. macOS:
    lo stato di occlusione della finestra dice se lo sfondo si vede; se sopra c'è una
    finestra di un'altra app grande quanto tutto lo schermo (`CGWindowListCopyWindowInfo`)
    vale la voce "schermo intero", se no quella delle finestre. Linux: con lo schermo
    bloccato o spento sempre; su GNOME, quando i fotogrammi della finestra rallentano perché
    non si vede (la `Sonda` in `linux.rs`): da lì app a schermo intero e finestre non si
    distinguono, basta una delle due voci
  - **Salva una foto dello sfondo**: a ogni pagina si chiede `zooFoto()` con
    `evaluate_script_with_callback` (un data URL PNG alla risoluzione dello schermo: fondo
    del tema, i canvas con il loro stile e i widget riscritti come testo). In un thread a
    parte il PNG si decodifica e si salva nella cartella Immagini, sottocartella `ASCII Zoo`
    (Windows `SHGetKnownFolderPath(FOLDERID_Pictures)`; macOS `~/Pictures`; Linux
    `XDG_PICTURES_DIR` da `user-dirs.dirs`, se no `~/Pictures`), con data e ora nel nome,
    `-schermo-N` con più schermi, senza mai sovrascrivere (`create_new`, poi ` (2)`, ` (3)`…).
    Poi la si mostra: Esplora risorse `/select,`, Finder `open -R`, su Linux
    `org.freedesktop.FileManager1.ShowItems` oppure `xdg-open` sulla cartella
  - **Screensaver** (solo Linux; su Windows c'è **Usa come screensaver…**, che apre le
    impostazioni di Windows): spento o dopo 1–30 minuti senza input, la scena, **Prova
    adesso**. GNOME non accetta più screensaver di altri: l'app lancia sé stessa come
    `ascii-zoo --salvaschermo` (un processo a parte, come il .scr su Windows, con
    `PR_SET_PDEATHSIG` perché non resti lì se l'app si chiude). Quando partire lo dice il
    monitor di inattività di mutter (`org.gnome.Mutter.IdleMonitor`: `AddIdleWatch` per i
    minuti, `AddUserActiveWatch` per chiuderlo al primo input, anche se la sua finestra non ha
    il focus e i tasti vanno altrove); senza mutter `org.freedesktop.ScreenSaver.GetSessionIdleTime`
    ogni 2 secondi. Non parte con `org.gnome.SessionManager.IsInhibited(8)` (o
    `org.freedesktop.PowerManagement.Inhibit.HasInhibit`), con lo schermo bloccato o con
    un'app a schermo intero (la `Sonda`), e si chiude con `ActiveChanged(true)` di
    `org.gnome.ScreenSaver` / `org.freedesktop.ScreenSaver`. Le finestre (una per monitor,
    senza bordi, a schermo intero e "sempre in primo piano", sotto XWayland) disegnano con
    cairo il buffer di `ascii_dreams::Motore`. Provato in GNOME Shell 46 senza schermo:
    senza "sempre in primo piano" una finestra a schermo intero senza focus finisce sotto
    quella attiva; una finestra override-redirect sta sopra ma lascia la barra in alto.
    `ascii-zoo --salvaschermo --finestra [--scena id]` lo apre in una finestra normale
  - **Avvia con Windows** / **Apri al login**, **Ricarica sfondo**, **Esci**
- Impostazioni salvate in `%APPDATA%\AsciiZoo\config.json` (Windows),
  `~/Library/Application Support/AsciiZoo/config.json` (macOS) o
  `~/.config/AsciiZoo/config.json` (Linux, `$XDG_CONFIG_HOME` se impostata).
- Avvio automatico: valore `AsciiZoo` nella chiave `HKCU\…\Run` (Windows),
  `~/Library/LaunchAgents/io.github.tressilloide.asciizoo.plist` (macOS) o
  `~/.config/autostart/ascii-zoo.desktop` (Linux), solo se attivato.

Con la variabile d'ambiente `ASCII_ZOO_LOG=1` il programma scrive su stderr cosa succede
(schermi trovati, pause e ripartenze con il motivo): la CI la usa nelle prove su macOS e Linux.

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
con `app/macos/Info.plist`, ci mette dentro lo screensaver (il modulo `ASCII Zoo.saver`,
compilato da [`../dreams/macos`](../dreams/macos)) e crea il DMG con `hdiutil`.

### Linux
Il modo semplice è [`../linux/install.sh`](../linux/install.sh): scarica il programma già
compilato dalla Release e, come riserva, lo compila (`--compila` per compilare sempre),
installando anche le librerie. A mano servono Rust e i pacchetti di sviluppo di WebKitGTK 4.1, GTK 3,
libayatana-appindicator3 e libxdo (su Ubuntu: `libwebkit2gtk-4.1-dev libgtk-3-dev
libayatana-appindicator3-dev libxdo-dev build-essential pkg-config`). Nella cartella `app`:

```
cargo build --release --locked
```

L'eseguibile è `target/release/ascii-zoo` (`ascii-zoo --version` scrive la versione ed esce).
Nel workflow:
- `linux-binario` compila il programma per x86_64 e aarch64 in un container Ubuntu 22.04
  (glibc 2.35: va anche sulle distribuzioni più nuove) e fa il pacchetto
  `ascii-zoo-linux-<arch>.tar.gz` (programma, icona, estensione di GNOME) con il suo `.sha256`;
- `linux-installa-binario` esegue `install.sh` su un Ubuntu 24.04 senza librerie di sviluppo,
  facendogli scaricare quel pacchetto come se fosse la Release (`ASCII_ZOO_RELEASE_URL`), e
  avvia il programma sotto Xvfb;
- `linux` prova la riserva: con una Release senza programma per Linux, `install.sh` compila
  questo commit; poi avvia il programma sotto Xvfb, prova il comando da una riga e la
  disinstallazione.
- `linux-gnome-clic` prova in un GNOME Shell senza schermo l'estensione per il clic sul
  desktop ([`../.github/gnome-click-test`](../.github/gnome-click-test)) e, con il programma
  di `linux-binario`, lo screensaver ([`../.github/gnome-screensaver-test`](../.github/gnome-screensaver-test)):
  deve partire dopo un minuto senza input (non sopra un'app a schermo intero, ma appena si
  chiude), stare sopra a tutto (anche sopra una finestra con il focus) con la barra in alto
  nascosta, chiudersi a un tasto, ripartire dopo un altro minuto e chiudersi quando mutter
  dice che qualcuno ha usato il PC (`ResetIdletime`). Niente mouse virtuale: in GNOME Shell
  senza schermo un movimento sopra le finestre X11 del programma vero blocca la shell.

> Lo sfondo è una copia in `assets/`: dopo aver modificato `wallpaper/index.html`
> (nella radice della repo) copialo in `assets/ascii-zoo-wallpaper.html` e ricompila.
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
`ascii-zoo-macos.dmg`, `ascii-zoo-linux-x86_64.tar.gz`, `ascii-zoo-linux-aarch64.tar.gz` e i
loro `.sha256`. Da quella Release in poi `install.sh` scarica il programma invece di compilarlo.

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
| `src/salvaschermo_mac.rs` | macOS: installa e aggiorna lo screensaver (`ASCII Zoo.saver`, da `../dreams/macos`) |
| `src/linux.rs` | Linux: finestre di tipo "desktop", icona AppIndicator, controllo degli schermi, margini della barra in alto |
| `src/salvaschermo_linux.rs` | Linux: quando far partire lo screensaver (inattività da mutter o `GetSessionIdleTime`, video, schermo bloccato), il suo sottomenu e il processo `ascii-zoo --salvaschermo` |
| `src/sogno_linux.rs` | Linux: lo screensaver vero e proprio, le scene di `../dreams` in una finestra GTK a schermo intero per monitor |
| `src/power_linux.rs`, `src/stats_linux.rs`, `src/dialog_linux.rs` | Linux: batteria e CPU/RAM da `/sys` e `/proc`, finestrella dei nomi (zenity o kdialog) |
| `src/config.rs` | impostazioni (nomi, rotazione, widget, animale per schermo) e avvio automatico |
| `macos/Info.plist` | descrizione dell'app per macOS |
| `build.rs`, `app.manifest` | incorporano il manifest dei controlli moderni di Windows (solo linker MSVC) |
