# Finto systemd sul bus di sessione della prova: GNOME Shell, prima di avviare XWayland,
# chiede a systemd di far partire un target; qui StartUnit/StopUnit riescono subito.
import gi
gi.require_version('Gio', '2.0')
from gi.repository import Gio, GLib

XML = '''<node><interface name="org.freedesktop.systemd1.Manager">
<method name="StartUnit"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="o" direction="out"/></method>
<method name="StopUnit"><arg type="s" direction="in"/><arg type="s" direction="in"/><arg type="o" direction="out"/></method>
<method name="Subscribe"/>
<signal name="JobRemoved"><arg type="u"/><arg type="o"/><arg type="s"/><arg type="s"/></signal>
</interface></node>'''
n = [0]


def chiamata(conn, sender, path, iface, metodo, params, inv):
    if metodo not in ('StartUnit', 'StopUnit'):
        inv.return_value(None)
        return
    n[0] += 1
    job = '/org/freedesktop/systemd1/job/%d' % n[0]
    inv.return_value(GLib.Variant('(o)', (job,)))
    unita = params.unpack()[0]

    def finito():
        conn.emit_signal(None, '/org/freedesktop/systemd1', 'org.freedesktop.systemd1.Manager',
                         'JobRemoved', GLib.Variant('(uoss)', (n[0], job, unita, 'done')))
        return False
    GLib.timeout_add(50, finito)


bus = Gio.bus_get_sync(Gio.BusType.SESSION, None)
bus.register_object('/org/freedesktop/systemd1', Gio.DBusNodeInfo.new_for_xml(XML).interfaces[0], chiamata, None, None)
Gio.bus_own_name_on_connection(bus, 'org.freedesktop.systemd1', Gio.BusNameOwnerFlags.NONE, None, None)
GLib.MainLoop().run()
