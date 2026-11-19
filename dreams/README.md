# dreams: the ASCII Zoo screensaver

The Windows screensaver of ASCII Zoo, as a small Rust library (`lib.rs`) that the app calls when Windows starts it with `/s`, `/p` or `/c`, plus a tiny program (`main.rs`) to run it on its own while working on it.

- `painter.rs` rasterizes JetBrains Mono once per character size with fontdue, precomputes a soft glow per glyph and draws the character grid straight into a pixel buffer, split across CPU cores.
- `scenes/` holds the scenes (`stelle`, `pioggia`, `fuoco`, `aurora`, `vita`, `mare`): each one only fills a grid of characters, colours and brightness. `director.rs` rotates them with a fade and `clock.rs` adds the drifting clock.
- `win.rs` puts the buffer on every monitor with GDI and speaks the screensaver protocol; settings are in `%APPDATA%\AsciiZoo\screensaver.json`.
- `macos/` is the macOS module, `ASCII Zoo.saver`: a small `cdylib` with a `ScreenSaverView` subclass (objc2) that gives every screen and every preview its own `Motore` and draws its buffer in `drawRect:`. The app carries it inside `ASCII Zoo.app` and installs it into `~/Library/Screen Savers`; the `macos` job of the workflow builds it and checks in CI that it loads and draws.
- On Linux there is no system that starts screensavers any more (GNOME dropped it), so the app does: after the idle minutes chosen in its menu it runs itself as `ascii-zoo --salvaschermo`, which puts `Motore` in a full-screen GTK window per monitor ([`app/src/sogno_linux.rs`](../app/src/sogno_linux.rs)). `ascii-zoo --salvaschermo --finestra --scena aurora` shows it in a normal window.

```
cargo run --release -- --finestra --scena aurora          # in a window (Esc closes)
cargo run --release -- --png aurora.png --scena aurora    # one frame to a PNG, no window, any OS
cargo run --release -- --fotogrammi frames --secondi 10   # every frame, to make a video
cargo run --release -- /s                                 # full screen, like Windows does
```
