#!/usr/bin/env bash
# Prova dell'estensione per il clic sul desktop (linux/gnome) in un GNOME Shell senza
# schermo (--headless) con le icone di DING, come su Ubuntu. Al posto di ASCII Zoo c'è
# finto-zoo.py, che prende lo stesso nome su D-Bus e scrive le chiamate Clic che riceve;
# zoo-prova-clic@test simula i clic con un mouse virtuale: due sul desktop (sopra DING),
# uno su una finestra normale, uno sulla barra in alto e uno per chiudere il menu che
# quel clic ha aperto. Devono arrivare solo i primi due.
#
#   bash .github/prova-clic-gnome/prova.sh
#
# Serve gnome-shell 45 o successivo con DING (Ubuntu: gnome-shell-extension-desktop-icons-ng),
# zenity, python3-gi. Tutto (impostazioni, bus di sessione) sta in una cartella temporanea.
set -euo pipefail
QUI="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO="$(cd "$QUI/../.." && pwd)"
T="$(mktemp -d)"
trap 'rm -rf "$T"' EXIT
mkdir -p "$T"/{home/Desktop,config,data/gnome-shell/extensions,cache,run}
chmod 700 "$T/run"
echo ciao > "$T/home/Desktop/prova.txt"
cp -r "$REPO/linux/gnome/zoo-clic@tressilloide.github.io" "$QUI/zoo-prova-clic@test" "$T/data/gnome-shell/extensions/"
ver=$(gnome-shell --version | grep -oE '[0-9]+' | head -n1)
for d in "$T"/data/gnome-shell/extensions/*/; do
  # la versione di GNOME in uso, come fa install.sh
  grep -q "\"$ver\"" "$d/metadata.json" || sed -i -E "s/(\"shell-version\": *\[)/\1\"$ver\", /" "$d/metadata.json"
done

cat > "$T/sessione.sh" <<FINE
python3 "$QUI/finto-systemd.py" &
python3 "$QUI/finto-zoo.py" > "$T/clic.log" 2>&1 &
gsettings set org.gnome.shell enabled-extensions "['ding@rastersoft.com', 'zoo-clic@tressilloide.github.io', 'zoo-prova-clic@test']"
gsettings set org.gnome.desktop.interface enable-animations false
timeout 45 gnome-shell --headless --wayland --virtual-monitor 1920x1080 > "$T/shell.log" 2>&1 || true
FINE
env -i HOME="$T/home" XDG_CONFIG_HOME="$T/config" XDG_DATA_HOME="$T/data" XDG_CACHE_HOME="$T/cache" \
  XDG_RUNTIME_DIR="$T/run" PATH=/usr/bin:/bin XDG_CURRENT_DESKTOP=GNOME XDG_SESSION_TYPE=wayland LANG=C.UTF-8 \
  timeout 90 dbus-run-session -- bash "$T/sessione.sh" > "$T/sessione.log" 2>&1 || true

echo "--- GNOME Shell"
grep -E "PROVA|ASCII Zoo clic|JS ERROR|zoo-clic" "$T/shell.log" || true
echo "--- clic arrivati ad ASCII Zoo"
cat "$T/clic.log"
ok=1
for atteso in "(700.0, 500.0, 1920.0, 1080.0)" "(1500.0, 900.0, 1920.0, 1080.0)"; do
  grep -qF "$atteso" "$T/clic.log" || { echo "MANCA il clic sul desktop $atteso"; ok=0; }
done
n=$(grep -c "Clic" "$T/clic.log" || true)
[ "$n" = 2 ] || { echo "ATTESI 2 clic, arrivati $n"; ok=0; }
grep -q "PROVA finestra normale aperta" "$T/shell.log" || { echo "la finestra normale non si è aperta: prova incompleta"; ok=0; }
if grep -q "JS ERROR.*zoo-clic" "$T/shell.log"; then echo "errore JavaScript nell'estensione"; ok=0; fi
if [ "$ok" != 1 ]; then
  echo "--- fine del log della sessione"; tail -n 60 "$T/sessione.log"
  echo "--- fine del log di GNOME Shell"; tail -n 60 "$T/shell.log"
  echo "PROVA FALLITA"
  exit 1
fi
echo "PROVA RIUSCITA"
