# ASCII Zoo — programma per Windows (Rust)

Mette lo sfondo animato (`sfondo/index.html`) dietro le icone del desktop, su tutti i monitor. Non serve Lively.

## Cosa fa
- Sfondo HTML incorporato nell'eseguibile, mostrato con WebView2 dietro le icone
  (funziona anche con il desktop "raised" di Windows 11 24H2+).
- Una finestra per ogni monitor collegato. Se un monitor viene collegato o scollegato,
  cambia risoluzione o Explorer riparte, gli sfondi vengono rifatti da soli.
- Icona nell'area di notifica (una koi) con il menu:
  - **Animale**: a rotazione oppure uno fisso
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

## File
| File | Cosa contiene |
|---|---|
| `src/main.rs` | finestre e WebView per monitor, menu, timer, cambi di schermo |
| `src/desktop.rs` | aggancio dietro le icone (WorkerW / Progman) |
| `src/power.rs` | batteria e app a schermo intero |
| `src/config.rs` | impostazioni e avvio con Windows |
