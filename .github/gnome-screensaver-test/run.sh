#!/usr/bin/env bash
# Prova dello screensaver su Linux, con il programma vero in un GNOME Shell senza schermo
# (--headless), come su Ubuntu ma senza le icone del desktop:
#   1. config con lo screensaver dopo 1 minuto;
#   2. l'estensione zoo-screensaver-test@test avvia ASCII Zoo e apre una finestra normale
#      (zenity), che ha il focus, e dal secondo 15 al 75 un'app a schermo intero;
#   3. senza mouse né tastiera, dopo un minuto deve partire `ascii-zoo --salvaschermo`, e
#      non prima; non sopra l'app a schermo intero ma appena si chiude (questo dipende dalla
#      `Sonda` di linux.rs, che guarda il ritmo dei fotogrammi: se in questa macchina
#      virtuale non se ne accorge è solo un avviso);
#   4. la sua finestra deve stare sopra a tutto (anche sopra zenity) e la barra in alto
#      deve sparire; l'estensione la fotografa;
#   5. un tasto da una tastiera virtuale lo deve chiudere;
#   6. dopo un altro minuto senza input deve ripartire; questa volta l'estensione chiede a
#      mutter ResetIdletime (quello che mutter fa a ogni input, ma che lo screensaver non
#      vede): lo deve chiudere ASCII Zoo, avvisato da mutter, come quando GNOME non dà il
#      focus allo screensaver e i tasti vanno a un'altra finestra.
#
# Niente mouse virtuale: in questo GNOME senza schermo e senza GPU un movimento del mouse
# sopra le finestre X11 di ASCII Zoo (anche solo lo sfondo, senza screensaver) blocca la
# shell per decine di secondi, mentre mutter aspetta XWayland.
#
#   bash .github/gnome-screensaver-test/run.sh percorso/di/ascii-zoo [cartella per i risultati]
#
# Serve gnome-shell 45 o successivo, xwayland, zenity, python3-gi con GTK 3
# (gir1.2-gtk-3.0) e le librerie del programma. Tutto (impostazioni, bus di sessione) sta in una cartella temporanea; il
# percorso del socket di Wayland deve stare in 108 byte, quindi TMPDIR deve essere corta.
set -euo pipefail
QUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$QUI/../.." && pwd)"
BIN="$(realpath "${1:?manca il percorso di ascii-zoo}")"
USCITA="$(realpath "${2:-.}")"
T="$(mktemp -d)"
fine() {
  # il portale dei documenti, se è partito, monta una cartella in XDG_RUNTIME_DIR
  fusermount3 -u "$T/run/doc" 2>/dev/null || fusermount -u "$T/run/doc" 2>/dev/null || true
  rm -rf "$T"
}
trap fine EXIT
trap 'exit 143' TERM INT
mkdir -p "$T"/{home,config/AsciiZoo,data/gnome-shell/extensions,cache,run}
chmod 700 "$T/run"
cp -r "$QUI/zoo-screensaver-test@test" "$T/data/gnome-shell/extensions/"
ver=$(gnome-shell --version | grep -oE '[0-9]+' | head -n1)
m="$T/data/gnome-shell/extensions/zoo-screensaver-test@test/metadata.json"
grep -q "\"$ver\"" "$m" || sed -i -E "s/(\"shell-version\": *\[)/\1\"$ver\", /" "$m"
# lo screensaver dopo 1 minuto, con l'aurora; niente clic sul desktop (non serve qui)
echo '{"salvaschermo_minuti": 1, "salvaschermo_scena": "aurora", "clic_desktop": false}' > "$T/config/AsciiZoo/config.json"

cat > "$T/sessione.sh" <<FINE
python3 "$REPO/.github/gnome-click-test/fake-systemd.py" &
gsettings set org.gnome.shell enabled-extensions "['zoo-screensaver-test@test']"
gsettings set org.gnome.desktop.interface enable-animations false
gnome-shell --headless --wayland --virtual-monitor 1920x1080 > "$T/shell.log" 2>&1 &
gs=\$!
inizio=\$(date +%s)
prima=no
# ogni secondo: c'è un processo "ascii-zoo --salvaschermo"?
for i in \$(seq 200); do
  sleep 1
  if pgrep -f -- "^$BIN --salvaschermo" > /dev/null; then ora=si; else ora=no; fi
  if [ "\$ora" != "\$prima" ]; then echo "\$(( \$(date +%s) - inizio )) s: screensaver \$ora" >> "$T/processi.log"; prima=\$ora; fi
  grep -qE "PROVA fine$" "$T/shell.log" && break
  kill -0 \$gs 2>/dev/null || { echo "GNOME Shell si è chiuso" >> "$T/processi.log"; break; }
done
pkill -f -- "^$BIN" || true
kill \$gs 2>/dev/null || true
sleep 1
FINE
env -i HOME="$T/home" XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" XDG_CACHE_HOME="$T/cache" \
  XDG_RUNTIME_DIR="$T/run" PATH=/usr/bin:/bin XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_TYPE=wayland LANG=C.UTF-8 \
  MUTTER_DEBUG_RESET_IDLETIME=1 ZOO_SCHERMO_INTERO="$QUI/schermo-intero.py" ZOO_BIN="$BIN" ZOO_LOG="$T/app.log" ZOO_FOTO="$USCITA/prova-screensaver-gnome.png" \
  timeout 260 dbus-run-session -- bash "$T/sessione.sh" > "$T/sessione.log" 2>&1 || true

echo "--- GNOME Shell"
grep -E "PROVA|JS ERROR" "$T/shell.log" || true
echo "--- ASCII Zoo (righe dello screensaver)"
grep -iE "screensaver|panic|error" "$T/app.log" || true
echo "--- processi"
cat "$T/processi.log" 2>/dev/null || true
cp "$T/app.log" "$USCITA/prova-screensaver-app.log" 2>/dev/null || true
cp "$T/shell.log" "$USCITA/prova-screensaver-shell.log" 2>/dev/null || true

ok=1
# righe di processi.log: "61 s: screensaver si", "67 s: screensaver no", ...
mapfile -t cambi < <(grep -oE '^[0-9]+ s: screensaver (si|no)' "$T/processi.log" || true)
partito=$(printf '%s\n' "${cambi[@]}" | grep -m1 ' si$' | cut -d' ' -f1 || true)
if [ -z "$partito" ]; then
  echo "LO SCREENSAVER NON È PARTITO"; ok=0
elif [ "$partito" -lt 55 ]; then
  echo "LO SCREENSAVER È PARTITO TROPPO PRESTO ($partito s, l'attesa è 1 minuto)"; ok=0
else
  echo "screensaver partito dopo $partito s"
  if grep -q "non parte: un'app è a schermo intero" "$T/app.log" && [ "$partito" -ge 74 ]; then
    echo "non è partito sopra l'app a schermo intero, ma appena si è chiusa"
  else
    echo "::warning::lo screensaver non ha visto l'app a schermo intero (la Sonda dipende dal ritmo dei fotogrammi di GNOME, qui senza GPU)"
  fi
fi
grep -q "PROVA screensaver VISIBILE" "$T/shell.log" || { echo "LA FINESTRA DELLO SCREENSAVER NON È SOPRA A TUTTO"; ok=0; }
if grep -q "PROVA giro 1: screensaver chiuso [0-9]* ms dopo l'input" "$T/shell.log"; then
  echo "giro 1: chiuso dopo il tasto"
else
  echo "GIRO 1: LO SCREENSAVER NON SI È CHIUSO DOPO IL TASTO"; ok=0
fi
if grep -q "PROVA giro 2: screensaver chiuso [0-9]* ms dopo l'input" "$T/shell.log" \
   && [ "$(grep -c "screensaver: lo chiudo (mouse o tastiera)" "$T/app.log")" -ge 1 ]; then
  echo "giro 2: ripartito dopo un altro minuto e chiuso da ASCII Zoo, avvisato da mutter"
else
  echo "GIRO 2: LO SCREENSAVER NON È RIPARTITO O NON L'HA CHIUSO ASCII ZOO"; ok=0
fi
[ "${#cambi[@]}" = 4 ] || { echo "ATTESI 4 CAMBI (parte, si chiude, riparte, si chiude), visti ${#cambi[@]}"; ok=0; }
if [ "$ok" != 1 ]; then
  echo "--- log di ASCII Zoo"; tail -n 60 "$T/app.log" 2>/dev/null || true
  echo "--- fine del log della sessione"; tail -n 40 "$T/sessione.log"
  echo "--- fine del log di GNOME Shell"; tail -n 60 "$T/shell.log"
  echo "PROVA FALLITA"
  exit 1
fi
echo "PROVA RIUSCITA"
