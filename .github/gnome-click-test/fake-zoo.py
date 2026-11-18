# Finto ASCII Zoo: prende il nome io.github.tressilloide.AsciiZoo come il programma vero
# (con "Clic sul desktop agli animali" acceso) e scrive le chiamate Clic che riceve.
import time
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

XML = '''<node><interface name="io.github.tressilloide.AsciiZoo">
<method name="Clic"><arg type="d" direction="in"/><arg type="d" direction="in"/>
<arg type="d" direction="in"/><arg type="d" direction="in"/></method>
</interface></node>'''


def chiamata(conn, sender, path, iface, metodo, params, inv):
    print(time.strftime('%H:%M:%S'), metodo, params.unpack(), flush=True)
    inv.return_value(None)


bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
bus.register_object('/io/github/tressilloide/AsciiZoo', Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0],
                    chiamata, None, None)
Gio.bus_own_name_on_connection(bus, 'io.github.tressilloide.AsciiZoo', Gio.BusNameOwnerFlags.NONE, None, None)
GLib.MainLoop().run()
