# ASCII Zoo — programma per Windows (Rust)

Mette `ascii-zoo-sfondo.html` come sfondo animato dietro le icone del desktop.

## Cosa fa
- Sfondo HTML incorporato nell'eseguibile, mostrato con WebView2 dietro le icone
  (funziona anche con il desktop "raised" di Windows 11 24H2+).
- Icona nell'area di notifica (una koi) con il menu:
  - **Animale**: a rotazione oppure uno fisso
  - **Koi nello stagno in carica** (vale con "A rotazione")
  - **Pausa a batteria**, **Pausa con app a schermo intero**
  - **Avvia con Windows**, **Ricarica sfondo**, **Esci**
- Impostazioni salvate in `%APPDATA%\AsciiZoo\config.json`.

## Compilare
Servono Rust (toolchain `x86_64-pc-windows-msvc`) e i Build Tools di Visual Studio
con il Windows SDK. Da PowerShell:

```
cargo build --release
```

L'eseguibile è in `target\release\ascii-zoo.exe`.

> Lo sfondo è una copia in `assets/`: dopo aver modificato
> `ascii-zoo-sfondo.html` copialo lì e ricompila.

## File
| File | Cosa contiene |
|---|---|
| `src/main.rs` | finestra, WebView, menu, timer |
| `src/desktop.rs` | aggancio dietro le icone (WorkerW / Progman) |
| `src/power.rs` | batteria e app a schermo intero |
| `src/config.rs` | impostazioni e avvio con Windows |
