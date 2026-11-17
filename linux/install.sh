#!/usr/bin/env bash
# ASCII Zoo per Linux: installa o aggiorna con un solo comando.
#
#   curl -fsSL https://raw.githubusercontent.com/Tressilloide/ascii-zoo/main/linux/install.sh | bash
#
# oppure, da un clone della repo:
#
#   bash linux/install.sh                installa (o aggiorna) e avvia
#   bash linux/install.sh --disinstalla  toglie programma, icona, voce del menu e avvio automatico
#
# (con curl: ... | bash -s -- --disinstalla)
#
# Cosa fa:
#   1. installa le librerie per compilarlo (apt, dnf o pacman: chiede la password di sudo)
#   2. installa Rust con rustup se manca (in ~/.cargo, senza toccare il PATH)
#   3. compila il programma (qualche minuto la prima volta, poi molto meno)
#   4. lo copia in ~/.local/bin, mette icona e voce nel menu delle applicazioni,
#      lo fa partire all'accesso e lo avvia: l'icona a forma di koi compare nella barra in alto
#
# Tutto resta nella cartella dell'utente; con sudo si installano solo i pacchetti di sistema.
# Variabile ASCII_ZOO_NON_AVVIARE=1: installa senza avviarlo (es. per le prove automatiche).
#
# Tutto è dentro main(), chiamata all'ultima riga: con "curl | bash" bash legge lo script
# per intero prima di eseguire qualcosa, e un download interrotto non esegue mezzo script.

set -euo pipefail

main() {
  local REPO="https://github.com/Tressilloide/ascii-zoo.git"
  local DATA="${XDG_DATA_HOME:-$HOME/.local/share}"
  local CONF="${XDG_CONFIG_HOME:-$HOME/.config}"
  local BIN="$HOME/.local/bin/ascii-zoo"
  local MENU="$DATA/applications/ascii-zoo.desktop"
  local ICONA="$DATA/icons/hicolor/scalable/apps/ascii-zoo.svg"
  local AUTOSTART="$CONF/autostart/ascii-zoo.desktop"
  local CACHE="${XDG_CACHE_HOME:-$HOME/.cache}/ascii-zoo/sorgente"

  [ "$(uname -s)" = Linux ] || errore "questo script è per Linux"
  [ "$(id -u)" -ne 0 ] || errore "non eseguirlo come root: si installa nella tua cartella utente (sudo lo chiede da solo per i pacchetti)"
  # il percorso finisce nei file .desktop: niente caratteri che lì hanno un significato speciale
  case "$BIN" in *[\"\\\`\$%]*) errore "il percorso $BIN contiene caratteri speciali (\" \\ \` \$ %)" ;; esac

  if [ "${1:-}" = "--disinstalla" ]; then
    ferma
    rm -f "$BIN" "$MENU" "$ICONA" "$AUTOSTART"
    if command -v update-desktop-database >/dev/null; then update-desktop-database "$DATA/applications" 2>/dev/null || true; fi
    say "ASCII Zoo disinstallato. Le impostazioni restano in $CONF/AsciiZoo: cancellala a mano se non servono più."
    say "Rust (in ~/.cargo) non l'ho toccato: se l'aveva installato questo script e non ti serve, 'rustup self uninstall'."
    return 0
  fi
  [ -z "${1:-}" ] || errore "opzione sconosciuta: $1 (l'unica è --disinstalla)"

  # 1. librerie di sistema (stdin staccato: con "curl | bash" è lo script stesso)
  local PKG
  if command -v apt-get >/dev/null; then
    PKG="build-essential pkg-config curl git libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev zenity"
    say "Installo le librerie necessarie con apt (può chiedere la password)"
    sudo apt-get update -qq </dev/null
    # shellcheck disable=SC2086
    sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $PKG </dev/null
  elif command -v dnf >/dev/null; then
    PKG="gcc gcc-c++ make pkgconf-pkg-config curl git webkit2gtk4.1-devel gtk3-devel libayatana-appindicator-gtk3-devel libxdo-devel zenity"
    say "Installo le librerie necessarie con dnf (può chiedere la password)"
    # shellcheck disable=SC2086
    sudo dnf install -y -q $PKG </dev/null
  elif command -v pacman >/dev/null; then
    PKG="base-devel curl git webkit2gtk-4.1 gtk3 libayatana-appindicator xdotool zenity"
    say "Installo le librerie necessarie con pacman (può chiedere la password)"
    # shellcheck disable=SC2086
    sudo pacman -S --needed --noconfirm $PKG </dev/null
  else
    avviso "Non riconosco il gestore di pacchetti: servono WebKitGTK 4.1, GTK 3, libayatana-appindicator3, libxdo, git, curl e un compilatore C. Provo comunque."
  fi

  # 2. il codice: questa cartella, se lo script viene da un clone della repo, se no un clone in ~/.cache
  local DIR SCRIPT="${BASH_SOURCE[0]:-}"
  if [ -n "$SCRIPT" ] && [ -f "$(dirname "$SCRIPT")/../app/Cargo.toml" ]; then
    DIR="$(cd "$(dirname "$SCRIPT")/.." && pwd)"
  elif [ -d "$CACHE/.git" ]; then
    say "Aggiorno il codice in $CACHE"
    git -C "$CACHE" pull --ff-only -q </dev/null
    DIR="$CACHE"
  else
    say "Scarico il codice da $REPO"
    mkdir -p "$(dirname "$CACHE")"
    git clone --depth 1 -q "$REPO" "$CACHE" </dev/null
    DIR="$CACHE"
  fi

  # 3. Rust e compilazione
  if [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    say "Installo Rust con rustup (in ~/.cargo, serve solo per compilare)"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y -q --profile minimal --default-toolchain stable --no-modify-path
  fi
  "$HOME/.cargo/bin/rustup" toolchain install stable --profile minimal </dev/null >/dev/null 2>&1 || true
  say "Compilo ASCII Zoo (la prima volta ci vuole qualche minuto)"
  (cd "$DIR/app" && "$HOME/.cargo/bin/cargo" +stable build --release --locked </dev/null)
  # se CARGO_TARGET_DIR è impostata, cargo mette lì il programma compilato
  local TARGET="${CARGO_TARGET_DIR:-$DIR/app/target}"
  case "$TARGET" in /*) ;; *) TARGET="$DIR/app/$TARGET" ;; esac

  # 4. installazione nella cartella dell'utente
  ferma
  install -Dm755 "$TARGET/release/ascii-zoo" "$BIN"
  install -Dm644 "$DIR/linux/ascii-zoo.svg" "$ICONA"
  mkdir -p "$(dirname "$MENU")" "$(dirname "$AUTOSTART")"
  { voce "$BIN"; printf 'Categories=Utility;\nStartupNotify=false\n'; } > "$MENU"
  # lo stesso file che scrive il programma con "Apri al login" (si toglie dallo stesso menu)
  { voce "$BIN"; printf 'X-GNOME-Autostart-enabled=true\n'; } > "$AUTOSTART"
  if command -v update-desktop-database >/dev/null; then update-desktop-database "$DATA/applications" 2>/dev/null || true; fi
  if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true; fi
  say "Installato in $BIN (parte da solo all'accesso; si toglie dal menu: 'Apri al login')"

  # GNOME mostra le icone nella barra solo con l'estensione AppIndicator (su Ubuntu c'è di serie)
  if [[ "${XDG_CURRENT_DESKTOP:-}" == *GNOME* ]] && command -v gnome-extensions >/dev/null; then
    if ! gnome-extensions list --enabled 2>/dev/null | grep -qi appindicator; then
      avviso "L'icona nella barra in alto richiede l'estensione AppIndicator di GNOME: su Ubuntu è 'ubuntu-appindicators' (di serie), altrove 'gnome-shell-extension-appindicator'. Installala e attivala, poi esci e rientra."
    fi
  fi

  if [ "${ASCII_ZOO_NON_AVVIARE:-}" = 1 ]; then
    return 0
  fi
  if [ -n "${DISPLAY:-}${WAYLAND_DISPLAY:-}" ]; then
    nohup "$BIN" </dev/null >/dev/null 2>&1 &
    disown || true
    say "ASCII Zoo è partito: clic sulla koi nella barra in alto per le impostazioni."
  else
    say "Non vedo una sessione grafica: partirà al prossimo accesso, o dal menu delle applicazioni ('ASCII Zoo')."
  fi
}

say() { printf '\033[1;36m==>\033[0m %s\n' "$*"; }
avviso() { printf '\033[1;33m!!\033[0m %s\n' "$*" >&2; }
errore() { printf '\033[1;31mErrore:\033[0m %s\n' "$*" >&2; exit 1; }

# ferma la copia già in esecuzione (solo quella dell'utente)
ferma() {
  pkill -x -u "$(id -u)" ascii-zoo 2>/dev/null || true
  local _
  for _ in $(seq 20); do pgrep -x -u "$(id -u)" ascii-zoo >/dev/null || return 0; sleep 0.25; done
}

# voce .desktop comune a menu e avvio automatico
voce() {
  printf '[Desktop Entry]\nType=Application\nName=ASCII Zoo\nComment=Sfondo animato con animali ASCII\nExec="%s"\nIcon=ascii-zoo\nTerminal=false\n' "$1"
}

main "$@"
