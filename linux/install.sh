#!/usr/bin/env bash
# ASCII Zoo per Linux: installa o aggiorna con un solo comando.
#
#   curl -fsSL https://raw.githubusercontent.com/Tressilloide/ascii-zoo/main/linux/install.sh | bash
#
# oppure, da un clone della repo:
#
#   bash linux/install.sh                installa (o aggiorna) e avvia
#   bash linux/install.sh --compila      come sopra, ma compila dal sorgente (questo clone, se
#                                        lo script viene da un clone) invece di scaricare
#   bash linux/install.sh --disinstalla  toglie programma, icona, voce del menu, avvio automatico,
#                                        l'estensione per il clic sul desktop e le versioni
#                                        vecchie fatte come estensione di GNOME
#
# (con curl: ... | bash -s -- --disinstalla)
#
# Cosa fa:
#   1. scarica il programma già compilato dall'ultima Release (lo compila GitHub Actions su
#      Ubuntu 22.04, per x86_64 e aarch64) e ne controlla lo SHA256; installa le librerie per
#      farlo partire, solo se mancano (apt, dnf o pacman: chiede la password di sudo)
#   2. se la Release non ha il programma per Linux (le versioni fino alla 0.4.0) o per questo
#      processore, o se non parte, lo compila dal sorgente: installa le librerie per
#      compilarlo, Rust con rustup se manca (in ~/.cargo, senza toccare il PATH), e compila
#      (qualche minuto la prima volta, poi molto meno)
#   3. lo copia in ~/.local/bin (al posto della versione di prima, che ferma), mette icona e
#      voce nel menu delle applicazioni, lo fa partire all'accesso e lo avvia: l'icona a forma
#      di koi compare nella barra in alto
#   4. su GNOME 45 o successivo mette la piccola estensione zoo-click@tressilloide.github.io
#      (linux/gnome), che passa ad ASCII Zoo i clic sul desktop: sotto Wayland un programma
#      non li vede. GNOME la carica dal prossimo accesso
#   5. se trova una versione vecchia fatta come estensione di GNOME Shell (uuid ascii-zoo…@…),
#      la spegne e la toglie, così non girano due sfondi insieme
#
# Tutto resta nella cartella dell'utente; con sudo si installano solo i pacchetti di sistema
# e si toglie l'eventuale copia di sistema di una versione vecchia (schermata di accesso).
# Variabili per le prove automatiche: ASCII_ZOO_NON_AVVIARE=1 installa senza avviarlo;
# ASCII_ZOO_RELEASE_URL cambia da dove si scarica il programma (es. file:///cartella).
#
# Tutto è dentro main(), chiamata all'ultima riga: con "curl | bash" bash legge lo script
# per intero prima di eseguire qualcosa, e un download interrotto non esegue mezzo script.

set -euo pipefail

main() {
  local REPO="https://github.com/Tressilloide/ascii-zoo.git"
  local RELEASE="${ASCII_ZOO_RELEASE_URL:-https://github.com/Tressilloide/ascii-zoo/releases/latest/download}"
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

  local COMPILA=0
  case "${1:-}" in
    --disinstalla)
      ferma
      rm -f "$BIN" "$MENU" "$ICONA" "$AUTOSTART"
      if command -v update-desktop-database >/dev/null; then update-desktop-database "$DATA/applications" 2>/dev/null || true; fi
      togli_estensione "$DATA"
      togli_estensione "$DATA" "$ESTENSIONE_VECCHIA"
      togli_vecchie "$DATA"
      say "ASCII Zoo disinstallato. Le impostazioni restano in $CONF/AsciiZoo: cancellala a mano se non servono più."
      say "Rust (in ~/.cargo), se c'è, non l'ho toccato: se l'aveva installato questo script e non ti serve, 'rustup self uninstall'."
      return 0
      ;;
    --compila) COMPILA=1 ;;
    "") ;;
    *) errore "opzione sconosciuta: $1 (ci sono solo --compila e --disinstalla)" ;;
  esac

  # 1-2. il programma, l'icona e l'estensione per GNOME: dalla Release o compilati qui
  local PROG SVG GNOME
  SCARICATO=$(mktemp -d)
  trap 'rm -rf "$SCARICATO"' EXIT
  if [ "$COMPILA" = 1 ] || ! scarica "$SCARICATO"; then
    compila
  fi

  # 3. installazione nella cartella dell'utente
  ferma
  install -Dm755 "$PROG" "$BIN"
  install -Dm644 "$SVG" "$ICONA"
  mkdir -p "$(dirname "$MENU")" "$(dirname "$AUTOSTART")"
  { voce "$BIN"; printf 'Categories=Utility;\nStartupNotify=false\n'; } > "$MENU"
  # lo stesso file che scrive il programma con "Apri al login" (si toglie dallo stesso menu)
  { voce "$BIN"; printf 'X-GNOME-Autostart-enabled=true\n'; } > "$AUTOSTART"
  if command -v update-desktop-database >/dev/null; then update-desktop-database "$DATA/applications" 2>/dev/null || true; fi
  if command -v gtk-update-icon-cache >/dev/null; then gtk-update-icon-cache -q -t "$DATA/icons/hicolor" 2>/dev/null || true; fi
  say "Installato in $BIN (parte da solo all'accesso; si toglie dal menu: 'Apri al login')"

  # 4. clic sul desktop su GNOME
  installa_estensione "$GNOME" "$DATA"

  # 5. solo ora che la versione nuova è al suo posto si toglie quella vecchia
  togli_vecchie "$DATA"

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

# Scarica nella cartella $1 il programma già compilato dall'ultima Release: un .tar.gz con
# il programma, l'icona e l'estensione per GNOME, più il suo SHA256. Lo compila GitHub
# Actions su Ubuntu 22.04, quindi va anche sulle distribuzioni più nuove. Imposta PROG, SVG
# e GNOME (variabili di main). Falso se non c'è (le Release fino alla 0.4.0 non l'hanno),
# se non c'è per questo processore o se non parte: allora si compila.
scarica() {
  local dir="$1" arch nome
  case "$(uname -m)" in
    x86_64 | amd64) arch=x86_64 ;;
    aarch64 | arm64) arch=aarch64 ;;
    *) say "Per questo processore ($(uname -m)) non c'è il programma già compilato: lo compilo"; return 1 ;;
  esac
  nome="ascii-zoo-linux-$arch"
  say "Scarico il programma già compilato ($nome.tar.gz)"
  if ! curl -fsSL --proto '=https,file' -o "$dir/$nome.tar.gz" "$RELEASE/$nome.tar.gz" </dev/null 2>/dev/null ||
    ! curl -fsSL --proto '=https,file' -o "$dir/$nome.tar.gz.sha256" "$RELEASE/$nome.tar.gz.sha256" </dev/null 2>/dev/null; then
    say "L'ultima versione pubblicata non ha ancora il programma per Linux: lo compilo dal sorgente"
    return 1
  fi
  if ! (cd "$dir" && sha256sum -c --quiet "$nome.tar.gz.sha256" >/dev/null 2>&1); then
    avviso "Lo SHA256 di $nome.tar.gz non corrisponde a quello pubblicato: non lo uso, lo compilo dal sorgente"
    return 1
  fi
  tar -xzf "$dir/$nome.tar.gz" -C "$dir"
  PROG="$dir/$nome/ascii-zoo" SVG="$dir/$nome/ascii-zoo.svg" GNOME="$dir/$nome/gnome"
  if [ ! -x "$PROG" ] || [ ! -f "$SVG" ]; then
    avviso "Il pacchetto scaricato è incompleto: lo compilo dal sorgente"
    return 1
  fi
  librerie "$PROG"
  if ! "$PROG" --version >/dev/null 2>&1; then
    avviso "Il programma scaricato non parte su questo sistema: lo compilo dal sorgente"
    return 1
  fi
  say "Scaricato: $("$PROG" --version), SHA256 verificato"
}

# Le librerie per far partire il programma (non quelle per compilarlo), solo se mancano:
# quelle collegate le dice ldd; AppIndicator il programma la carica da sé all'avvio; per le
# finestrelle dei nomi serve zenity (o kdialog).
librerie() {
  local prog="$1" PKG
  local ldc
  ldc=$(command -v ldconfig || echo /sbin/ldconfig)
  if ! ldd "$prog" 2>/dev/null | grep -q "not found" &&
    "$ldc" -p 2>/dev/null | grep -qE 'lib(ayatana-)?appindicator3\.so\.1' &&
    { command -v zenity >/dev/null || command -v kdialog >/dev/null; }; then
    return 0
  fi
  if command -v apt-get >/dev/null; then
    PKG="libwebkit2gtk-4.1-0 libgtk-3-0 libayatana-appindicator3-1 libxdo3 zenity"
    say "Installo le librerie che servono con apt (può chiedere la password)"
    sudo apt-get update -qq </dev/null
    # shellcheck disable=SC2086
    if ! sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $PKG </dev/null; then
      # shellcheck disable=SC2086
      spiega_apt $PKG
      errore "apt non è riuscito a installare le librerie (vedi sopra)"
    fi
  elif command -v dnf >/dev/null; then
    PKG="webkit2gtk4.1 gtk3 libayatana-appindicator-gtk3 libxdo zenity"
    say "Installo le librerie che servono con dnf (può chiedere la password)"
    # shellcheck disable=SC2086
    sudo dnf install -y -q $PKG </dev/null || errore "dnf non è riuscito a installare le librerie"
  elif command -v pacman >/dev/null; then
    PKG="webkit2gtk-4.1 gtk3 libayatana-appindicator xdotool zenity"
    say "Installo le librerie che servono con pacman (può chiedere la password)"
    # shellcheck disable=SC2086
    sudo pacman -S --needed --noconfirm $PKG </dev/null || errore "pacman non è riuscito a installare le librerie"
  else
    avviso "Non riconosco il gestore di pacchetti: servono WebKitGTK 4.1, GTK 3, libayatana-appindicator3, libxdo e zenity. Provo comunque."
  fi
}

# Compila dal sorgente: le librerie per compilarlo, il codice (questo clone o uno in ~/.cache),
# Rust e cargo. Imposta PROG, SVG e GNOME (variabili di main).
compila() {
  # librerie di sistema (stdin staccato: con "curl | bash" è lo script stesso)
  local PKG
  if command -v apt-get >/dev/null; then
    PKG="build-essential pkg-config curl git libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev libxdo-dev zenity"
    say "Installo le librerie per compilarlo con apt (può chiedere la password)"
    sudo apt-get update -qq </dev/null
    # shellcheck disable=SC2086
    if ! sudo env DEBIAN_FRONTEND=noninteractive apt-get install -y -qq $PKG </dev/null; then
      # shellcheck disable=SC2086
      spiega_apt $PKG
      errore "apt non è riuscito a installare le librerie (vedi sopra)"
    fi
  elif command -v dnf >/dev/null; then
    PKG="gcc gcc-c++ make pkgconf-pkg-config curl git webkit2gtk4.1-devel gtk3-devel libayatana-appindicator-gtk3-devel libxdo-devel zenity"
    say "Installo le librerie per compilarlo con dnf (può chiedere la password)"
    # shellcheck disable=SC2086
    sudo dnf install -y -q $PKG </dev/null
  elif command -v pacman >/dev/null; then
    PKG="base-devel curl git webkit2gtk-4.1 gtk3 libayatana-appindicator xdotool zenity"
    say "Installo le librerie per compilarlo con pacman (può chiedere la password)"
    # shellcheck disable=SC2086
    sudo pacman -S --needed --noconfirm $PKG </dev/null
  else
    avviso "Non riconosco il gestore di pacchetti: servono WebKitGTK 4.1, GTK 3, libayatana-appindicator3, libxdo, git, curl e un compilatore C. Provo comunque."
  fi

  # il codice: questa cartella, se lo script viene da un clone della repo, se no un clone in ~/.cache
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

  # Rust e compilazione
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
  PROG="$TARGET/release/ascii-zoo" SVG="$DIR/linux/ascii-zoo.svg" GNOME="$DIR/linux/gnome"
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

# Quando apt non riesce a installare le librerie, il caso più comune dopo un aggiornamento
# di versione di Ubuntu: un pacchetto è installato a una build poi ritirata dall'archivio
# (es. libbz2-1.0 1.0.8-6ubuntu0.1) e il suo -dev chiede la versione esatta che c'è
# nell'archivio (1.0.8-6build2). Qui lo si riconosce e si dice come sistemarlo, con una
# simulazione (senza sudo, non cambia niente). Il downgrade non lo fa lo script: se
# trascina via altri pacchetti può togliere il desktop.
spiega_apt() {
  local sim dip pkg ver stato inst src bin bver
  local -a vecchi=() famiglia=()
  sim=$(LC_ALL=C apt-get -s install "$@" 2>&1 </dev/null || true)
  # "Depends: libfoo (= 1.2-3)" (apt classico) o "Depends libfoo (= 1.2-3)" (solver3)
  while IFS= read -r dip; do
    pkg=${dip%% *}; pkg=${pkg%%:*}; ver=${dip#*"(= "}; ver=${ver%)}
    stato=; inst=
    IFS=' ' read -r stato inst < <(dpkg-query -W -f='${db:Status-Abbrev} ${Version}\n' "$pkg" 2>/dev/null | head -n1) || true
    if [ "${stato:-}" != ii ] || [ "${inst:-}" = "$ver" ]; then continue; fi
    case " ${vecchi[*]} " in *" $pkg "*) continue ;; esac
    vecchi+=("$pkg")
    avviso "$pkg è installato alla versione $inst, ma le librerie da installare chiedono $ver"
    # gli altri pacchetti dello stesso sorgente alla stessa versione vanno spostati insieme
    src=$(dpkg-query -W -f='${source:Package}' "$pkg" 2>/dev/null | head -n1 || true)
    while IFS=' ' read -r bin bver; do
      [ "$bver" = "$inst" ] || continue
      case " ${famiglia[*]} " in *" $bin=$ver "*) ;; *) famiglia+=("$bin=$ver") ;; esac
    done < <(dpkg-query -W -f='${db:Status-Abbrev}|${binary:Package}|${source:Package}|${Version}\n' 2>/dev/null \
               | awk -F'|' -v s="$src" '$1 ~ /^ii/ && $3 == s { print $2, $4 }')
  done < <(printf '%s\n' "$sim" | grep -oE 'Depends:? [A-Za-z0-9.+:-]+ \(= [^)]+\)' | sed -E 's/^Depends:? //' | sort -u)

  [ "${#famiglia[@]}" -gt 0 ] || return 0
  avviso "Succede dopo un aggiornamento di versione di Ubuntu, quando una build viene ritirata dall'archivio."
  avviso "Si sistema riportando questi pacchetti alla versione dell'archivio, tutti insieme: ${famiglia[*]}"
  sim=$(LC_ALL=C apt-get -s install --allow-downgrades "${famiglia[@]}" "$@" 2>&1 </dev/null) || {
    printf '%s\n' "$sim" | grep -E '^(E:| )' | head -n 15 >&2
    avviso "Anche così apt non trova una soluzione (simulazione qui sopra): serve sistemarlo a mano."
    return 0
  }
  local tolti
  tolti=$(printf '%s\n' "$sim" | grep -c '^Remv' || true)
  if [ "$tolti" -gt 0 ]; then
    printf '%s\n' "$sim" | grep '^Remv' | head -n 15 >&2
    avviso "La simulazione toglierebbe $tolti pacchetti (qui sopra): NON farlo, rischi di perdere il desktop. Serve sistemarlo a mano."
  else
    avviso "Ho simulato il downgrade: non toglie nessun pacchetto. Per farlo, e poi rilanciare questo script:"
    printf '\n  sudo apt-get install --allow-downgrades %s %s\n\n' "${famiglia[*]}" "$*" >&2
  fi
}

# Le versioni vecchie fatte come estensione di GNOME Shell (lo sfondo disegnato dentro
# gnome-shell, uuid "ascii-zoo…@…"): insieme al programma girerebbero due sfondi uno sopra
# l'altro. Si spengono nella sessione (il loro disable() ferma anche il renderer) e si
# tolgono: la copia dell'utente senza sudo, quella di sistema per la schermata di accesso
# (/usr/share/gnome-shell/extensions e il file che la attiva in /usr/share/gdm/dconf) con sudo.
togli_vecchie() {
  local data="$1" d uuid
  local -a utente=() sistema=() uuids=()
  for d in "$data"/gnome-shell/extensions/ascii-zoo*@*; do
    if [ -f "$d/metadata.json" ]; then utente+=("$d"); fi
  done
  for d in /usr/share/gnome-shell/extensions/ascii-zoo*@*; do
    if [ -f "$d/metadata.json" ]; then sistema+=("$d"); fi
  done
  for d in "${utente[@]}" "${sistema[@]}"; do
    uuid=${d##*/}
    case " ${uuids[*]} " in *" $uuid "*) ;; *) uuids+=("$uuid") ;; esac
  done
  for d in /usr/share/gdm/dconf/*ascii-zoo*; do
    if [ -f "$d" ]; then sistema+=("$d"); fi
  done
  [ "${#utente[@]}" -gt 0 ] || [ "${#sistema[@]}" -gt 0 ] || return 0

  say "Tolgo la versione vecchia fatta come estensione di GNOME: ${uuids[*]:-attivazione nella schermata di accesso}"
  if command -v gnome-extensions >/dev/null; then
    for uuid in "${uuids[@]}"; do timeout 15 gnome-extensions disable "$uuid" </dev/null 2>/dev/null || true; done
  fi
  if [ "${#utente[@]}" -gt 0 ]; then rm -rf -- "${utente[@]}"; fi
  if [ "${#sistema[@]}" -gt 0 ]; then
    say "La copia per la schermata di accesso è di sistema: la tolgo con sudo (può chiedere la password)"
    if ! sudo rm -rf -- "${sistema[@]}" </dev/null; then
      avviso "Non sono riuscito a toglierla: lo puoi fare a mano con  sudo rm -rf ${sistema[*]}"
      return 0
    fi
    if [ -x /usr/share/gdm/generate-config ]; then sudo /usr/share/gdm/generate-config </dev/null || true; fi
  fi
}

# Clic sul desktop su GNOME: l'estensione zoo-click@… (vedi linux/gnome) vede i clic sul
# desktop, anche sopra le icone di DING, e li passa ad ASCII Zoo. L'uuid non comincia per
# "ascii-zoo", se no togli_vecchie la prenderebbe per una versione vecchia dello sfondo.
# GNOME carica le estensioni nuove solo all'accesso: la si mette in enabled-extensions e
# funziona dal prossimo. Se l'utente l'ha spenta (disabled-extensions) resta spenta.
ESTENSIONE=zoo-click@tressilloide.github.io
# nome della stessa estensione fino alla 0.4 (messa da questo script): si toglie
ESTENSIONE_VECCHIA=zoo-clic@tressilloide.github.io
installa_estensione() {
  local src="$1/$ESTENSIONE" data="$2" ver dest lista prima=""
  togli_estensione "$data" "$ESTENSIONE_VECCHIA"
  command -v gnome-shell >/dev/null || return 0
  [ -f "$src/extension.js" ] || return 0
  ver=$(gnome-shell --version 2>/dev/null | grep -oE '[0-9]+' | head -n1 || true)
  if [ -z "$ver" ] || [ "$ver" -lt 45 ]; then
    avviso "Il clic sul desktop su GNOME richiede GNOME 45 o successivo (qui: ${ver:-?}): lo sfondo funziona lo stesso, senza clic."
    return 0
  fi
  dest="$data/gnome-shell/extensions/$ESTENSIONE"
  if [ -f "$dest/extension.js" ]; then prima=$(cat "$dest/extension.js"); fi
  mkdir -p "$dest"
  cp "$src/extension.js" "$dest/extension.js"
  # la versione di GNOME in uso va nell'elenco, se no GNOME non la carica
  if grep -q "\"$ver\"" "$src/metadata.json"; then
    cp "$src/metadata.json" "$dest/metadata.json"
  else
    sed -E "s/(\"shell-version\": *\[)/\1\"$ver\", /" "$src/metadata.json" > "$dest/metadata.json"
  fi
  if lista=$(gsettings get org.gnome.shell disabled-extensions 2>/dev/null) && [[ "$lista" == *"'$ESTENSIONE'"* ]]; then
    avviso "L'estensione per il clic sul desktop ($ESTENSIONE) è spenta in Estensioni: riaccendila lì se vuoi i clic."
    return 0
  fi
  if lista=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null) && [[ "$lista" != *"'$ESTENSIONE'"* ]]; then
    case "$lista" in
      "@as []" | "[]") lista="['$ESTENSIONE']" ;;
      *) lista="${lista%]}, '$ESTENSIONE']" ;;
    esac
    gsettings set org.gnome.shell enabled-extensions "$lista" 2>/dev/null || true
  fi
  if [ "$(gsettings get org.gnome.shell disable-user-extensions 2>/dev/null)" = true ]; then
    avviso "Le estensioni di GNOME sono spente (Estensioni → Estensioni dell'utente): senza, niente clic sul desktop."
  elif [ "$prima" != "$(cat "$dest/extension.js")" ]; then
    say "Clic sul desktop: estensione di GNOME $ESTENSIONE installata. Funziona dal prossimo accesso (esci e rientra)."
  fi
}

# Per spegnerla la si toglie da enabled-extensions (GNOME la spegne da sé). Non con
# "gnome-extensions disable", che la metterebbe anche in disabled-extensions: reinstallando
# sembrerebbe spenta dall'utente.
togli_estensione() {
  local uuid="${2:-$ESTENSIONE}" dest lista nuova
  dest="$1/gnome-shell/extensions/$uuid"
  [ -d "$dest" ] || return 0
  if lista=$(gsettings get org.gnome.shell enabled-extensions 2>/dev/null) && [[ "$lista" == *"'$uuid'"* ]]; then
    nuova=$(printf '%s' "$lista" | sed -E "s/'${uuid//./\\.}'(, )?//; s/, \]/]/")
    gsettings set org.gnome.shell enabled-extensions "$nuova" 2>/dev/null || true
  fi
  rm -rf -- "$dest"
}

# voce .desktop comune a menu e avvio automatico
voce() {
  printf '[Desktop Entry]\nType=Application\nName=ASCII Zoo\nComment=Sfondo animato con animali ASCII\nExec="%s"\nIcon=ascii-zoo\nTerminal=false\n' "$1"
}

main "$@"
