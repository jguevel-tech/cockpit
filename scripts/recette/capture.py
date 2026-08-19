"""Capture l'ecran du serveur X courant dans un PNG."""
import sys, gi
gi.require_version("Gdk", "3.0")
from gi.repository import Gdk
w = Gdk.get_default_root_window()
pb = Gdk.pixbuf_get_from_window(w, 0, 0, w.get_width(), w.get_height())
pb.savev(sys.argv[1], "png", [], [])
print("capture:", sys.argv[1])
