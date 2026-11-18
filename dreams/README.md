# dreams: the ASCII Zoo screensaver

The Windows screensaver of ASCII Zoo, as a small Rust library (`lib.rs`) that the app calls when Windows starts it with `/s`, `/p` or `/c`, plus a tiny program (`main.rs`) to run it on its own while working on it.

- `pittore.rs` rasterizes JetBrains Mono once per character size with fontdue, precomputes a soft glow per glyph and draws the character grid straight into a pixel buffer, split across CPU cores.
- `scene/` holds the scenes (`stelle`, `pioggia`, `fuoco`, `aurora`, `vita`, `mare`): each one only fills a grid of characters, colours and brightness. `regia.rs` rotates them with a fade and `orologio.rs` adds the drifting clock.
- `win.rs` puts the buffer on every monitor with GDI and speaks the screensaver protocol; settings are in `%APPDATA%\AsciiZoo\screensaver.json`.

```
cargo run --release -- --finestra --scena aurora          # in a window (Esc closes)
cargo run --release -- --png aurora.png --scena aurora    # one frame to a PNG, no window, any OS
cargo run --release -- --fotogrammi frames --secondi 10   # every frame, to make a video
cargo run --release -- /s                                 # full screen, like Windows does
```
