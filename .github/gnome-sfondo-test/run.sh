#!/usr/bin/env bash
# Prova dello sfondo di Linux sotto Wayland, senza XWayland: il programma vero e l'estensione
# zoo-click@tressilloide.github.io (linux/gnome, sfondo.js) in un GNOME Shell senza schermo
# (--headless) con le icone di DING, come su Ubuntu. zoo-sfondo-test@test:
#   1. avvia ASCII Zoo, che deve trovare l'estensione e mettersi in modo Wayland;
#   2. controlla che la finestra sia minimizzata, fuori da Alt+Tab, da get_window_actors e
#      dalle app, e mostrata da un clone dentro lo sfondo di GNOME; fotografa il desktop;
#   3. apre un'app a schermo intero: lo sfondo deve andare in pausa, e ripartire quando si chiude;
#   4. spegne e riaccende un'altra estensione accesa prima (zoo-prima@test): GNOME spegne e
#      riaccende in cascata anche zoo-click, che deve riprendere la finestra (di nuovo il
#      controllo del punto 2 e la foto), e ASCII Zoo deve restare nel modo Wayland;
#   5. spegne l'estensione: ASCII Zoo deve ripartire da solo su X11 (XWayland).
#
#   bash .github/gnome-sfondo-test/run.sh percorso/di/ascii-zoo [cartella per i risultati]
#
# Serve gnome-shell 45 o successivo con DING (Ubuntu: gnome-shell-extension-desktop-icons-ng),
# xwayland, python3-gi con GTK 3, imagemagick e le librerie del programma.
set -euo pipefail
QUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$QUI/../.." && pwd)"
BIN="$(realpath "${1:?manca il percorso di ascii-zoo}")"
USCITA="$(realpath "${2:-.}")"
T="$(mktemp -d)"
fine() {
  fusermount3 -u "$T/run/doc" 2>/dev/null || fusermount -u "$T/run/doc" 2>/dev/null || true
  rm -rf "$T"
}
trap fine EXIT
trap 'exit 143' TERM INT
mkdir -p "$T"/{home/Desktop,config/AsciiZoo,data/gnome-shell/extensions,cache,run,out}
chmod 700 "$T/run"
echo ciao > "$T/home/Desktop/prova.txt"
cp -r "$REPO/linux/gnome/zoo-click@tressilloide.github.io" "$QUI/zoo-sfondo-test@test" "$QUI/zoo-prima@test" \
  "$T/data/gnome-shell/extensions/"
ver=$(gnome-shell --version | grep -oE '[0-9]+' | head -n1)
for d in "$T"/data/gnome-shell/extensions/*/; do
  grep -q "\"$ver\"" "$d/metadata.json" || sed -i -E "s/(\"shell-version\": *\[)/\1\"$ver\", /" "$d/metadata.json"
done
echo '{"clic_desktop": true, "pausa_coperto": true, "pausa_schermo_intero": true, "salvaschermo_minuti": 0, "animale": 4}' \
  > "$T/config/AsciiZoo/config.json"
ding=""
[ -d /usr/share/gnome-shell/extensions/ding@rastersoft.com ] && ding="'ding@rastersoft.com', "

cat > "$T/sessione.sh" <<FINE
python3 "$REPO/.github/gnome-click-test/fake-systemd.py" &
gsettings set org.gnome.shell enabled-extensions "[$ding'zoo-prima@test', 'zoo-click@tressilloide.github.io', 'zoo-sfondo-test@test']"
gsettings set org.gnome.desktop.interface enable-animations false
ZOO_BIN="$BIN" ZOO_PROVA="$T/out" timeout 110 gnome-shell --headless --wayland --virtual-monitor 1920x1080 > "$T/shell.log" 2>&1 || true
FINE
env -i HOME="$T/home" XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" XDG_CACHE_HOME="$T/cache" \
  XDG_RUNTIME_DIR="$T/run" PATH=/usr/bin:/bin XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_TYPE=wayland LANG=C.UTF-8 \
  timeout 160 dbus-run-session -- bash "$T/sessione.sh" > "$T/sessione.log" 2>&1 || true
pkill -x ascii-zoo 2>/dev/null || true  # non -f: il percorso è anche tra gli argomenti di questo script

cp "$T/shell.log" "$USCITA/prova-sfondo-shell.log"
cp "$T/out/app.log" "$USCITA/prova-sfondo-app.log" 2>/dev/null || true
cp "$T"/out/desktop-*.png "$USCITA/" 2>/dev/null || true
echo "--- GNOME Shell"
grep -E "PROVA|ASCII Zoo|JS ERROR" "$T/shell.log" || true
echo "--- ASCII Zoo"
grep -E "sfondo|pausa|riparte|fotogrammi al|PASSO" "$T/out/app.log" 2>/dev/null | cut -c1-200 || true
ok=1
manca() { echo "MANCA: $1"; ok=0; }
c() { grep -qE "$2" "$1" || manca "$3"; }
# all'inizio e dopo la cascata di estensioni
due() { [ "$(grep -cE "$2" "$1")" -ge 2 ] || manca "$3 (all'inizio o dopo la cascata)"; }
S="$T/shell.log"; A="$T/out/app.log"
touch "$A"
c "$A" "sfondo: Wayland" "ASCII Zoo non ha trovato l'estensione (modo Wayland)"
c "$A" "l'estensione ha messo le finestre al posto dello sfondo" "l'estensione non ha preso la finestra"
due "$S" "PROVA finestre 1 minimizzate true" "la finestra non è minimizzata"
due "$S" "PROVA in alt-tab 0" "la finestra è in Alt+Tab"
due "$S" "PROVA in get_window_actors 0" "la finestra è in get_window_actors"
due "$S" "PROVA con app 0" "la finestra ha un'app (dash)"
due "$S" "PROVA cloni nello sfondo [1-9]" "nessun clone nello sfondo"
c "$A" "in pausa .*schermo intero true" "nessuna pausa con l'app a schermo intero"
c "$A" "riparte .*schermo intero false" "lo sfondo non è ripartito dopo l'app a schermo intero"
c "$A" "riparto su X11" "con l'estensione spenta ASCII Zoo non è ripartito su X11"
[ "$(grep -c "riparto su X11" "$A")" = 1 ] || manca "ASCII Zoo è ripartito su X11 più di una volta (anche con la cascata?)"
grep -A100 "riparto su X11" "$A" | grep -q "sfondo: X11" || manca "dopo la ripartenza non è su X11"
# fotogrammi della pagina in modo Wayland (senza GPU sono pochi, ma ci devono essere)
f=$(grep -m1 "fotogrammi al secondo" "$A" | grep -oE '[0-9]+\.[0-9]+ fotogrammi' | cut -d' ' -f1 || true)
awk -v f="${f:-0}" 'BEGIN { exit !(f > 2) }' || manca "la pagina non fa fotogrammi (${f:-nessuna misura})"
# le foto del desktop: lo sfondo di ASCII Zoo ha tanti colori, quello di GNOME uno solo
for n in 1 2; do
  if [ -f "$T/out/desktop-$n.png" ]; then
    colori=$(identify -format '%k' "$T/out/desktop-$n.png" 2>/dev/null || echo 0)
    echo "colori nella foto $n del desktop: $colori"
    [ "${colori:-0}" -gt 200 ] || manca "nella foto $n del desktop non si vede lo sfondo di ASCII Zoo"
  else
    manca "foto $n del desktop"
  fi
done
if grep -q "JS ERROR.*zoo-click" "$S"; then echo "errore JavaScript nell'estensione"; ok=0; fi
if [ "$ok" != 1 ]; then
  echo "--- sessione"; tail -n 20 "$T/sessione.log"
  echo "--- fine del log di GNOME Shell"; tail -n 60 "$S"
  echo "--- fine del log di ASCII Zoo"; tail -n 40 "$A"
  echo "PROVA FALLITA"
  exit 1
fi
echo "PROVA RIUSCITA"
