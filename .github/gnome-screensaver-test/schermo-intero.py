# Solo per la prova automatica: un'app a schermo intero (finestra Wayland nativa) che resta
# aperta N secondi, come un video o un gioco che non chiede di bloccare l'inattività.
import os
import sys

os.environ['GDK_BACKEND'] = 'wayland'
import gi  # noqa: E402

gi.require_version('Gtk', '3.0')
from gi.repository import GLib, Gtk  # noqa: E402

w = Gtk.Window(title='App a schermo intero')
w.show_all()
w.fullscreen()
GLib.timeout_add_seconds(int(sys.argv[1]), Gtk.main_quit)
Gtk.main()
