# ASCII Zoo — programma per Windows (Rust)

Mette lo sfondo animato (`sfondo/index.html`) dietro le icone del desktop, su tutti i monitor. Non serve Lively.

## Cosa fa
- Sfondo HTML incorporato nell'eseguibile, mostrato con WebView2 dietro le icone
  (funziona anche con il desktop "raised" di Windows 11 24H2+).
- Una finestra per ogni monitor collegato. Se un monitor viene collegato o scollegato,
  cambia risoluzione o Explorer riparte, gli sfondi vengono rifatti da soli.
- Icona nell'area di notifica (una koi) con il menu:
  - **Animale**: a rotazione oppure uno fisso
  - **Rotazione**: ogni quanto cambia animale (da 30 secondi a un'ora) e quali animali far girare
    (almeno uno resta sempre spuntato)
  - **Nomi degli animali**: il nome dell'animale più grande di ogni specie, scritto in una
    finestrella (Invio = OK, Esc = annulla, vuoto = nessun nome, al massimo 16 lettere);
    di serie le koi hanno KARPISA
  - **Koi nello stagno in carica** (vale con "A rotazione")
  - **Pausa a batteria**, **Pausa con app a schermo intero** (solo sullo schermo dell'app)
  - **Avvia con Windows**, **Ricarica sfondo**, **Esci**
- Impostazioni salvate in `%APPDATA%\AsciiZoo\config.json`.

> Su un PC fisso (senza batteria) Windows risulta sempre "in carica": con
> **Koi nello stagno in carica** attivo lo sfondo mostra sempre le koi.
> Per avere la rotazione degli animali togli la spunta a quella voce.

## Compilare
Servono Rust (toolchain `x86_64-pc-windows-msvc`) e i Build Tools di Visual Studio
con il Windows SDK. Da PowerShell, nella cartella `app`:

```
cargo build --release --locked
```

L'eseguibile è in `target\release\ascii-zoo.exe`. Conviene copiarlo in una cartella
fissa (es. `%LOCALAPPDATA%\AsciiZoo`) prima di attivare **Avvia con Windows**,
perché l'avvio automatico ricorda il percorso da cui è stato lanciato.

> Lo sfondo è una copia in `assets/`: dopo aver modificato `sfondo/index.html`
> (nella radice della repo) copialo in `assets/ascii-zoo-sfondo.html` e ricompila.
> La build su GitHub fallisce se le due copie sono diverse.

## Pubblicare una versione
L'exe delle Releases lo compila GitHub Actions ([`../.github/workflows/windows.yml`](../.github/workflows/windows.yml)),
mai il PC di qualcuno. Il workflow compila anche a ogni push e pull request (l'exe si
trova tra gli artifact della run), ma pubblica solo con un tag:

1. aggiorna `version` in `Cargo.toml` (es. `0.2.0`) e fai `cargo build` per aggiornare `Cargo.lock`;
2. fai il commit su `main`;
3. crea e pubblica il tag con la stessa versione:
   ```
   git tag v0.2.0
   git push origin v0.2.0
   ```

Il workflow controlla che tag e versione coincidano, compila, calcola l'SHA256, crea
l'attestazione di provenienza e pubblica la Release con `ascii-zoo.exe` e `ascii-zoo.exe.sha256`.

## File
| File | Cosa contiene |
|---|---|
| `src/main.rs` | finestre e WebView per monitor, menu, timer, cambi di schermo |
| `src/dialog.rs` | la finestrella per scrivere il nome di un animale |
| `src/desktop.rs` | aggancio dietro le icone (WorkerW / Progman) |
| `src/power.rs` | batteria e app a schermo intero |
| `src/config.rs` | impostazioni (nomi e rotazione compresi) e avvio con Windows |
| `build.rs`, `app.manifest` | incorporano il manifest dei controlli moderni di Windows (solo linker MSVC) |
