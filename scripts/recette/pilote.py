"""Pilote souris/clavier d'un serveur X, via XTEST. Sert aux scenarios de recette."""
import ctypes, sys, time

xlib = ctypes.CDLL("libX11.so.6")
xtst = ctypes.CDLL("libXtst.so.6")

# Les prototypes doivent etre declares : sans cela ctypes traite le Display* rendu par
# XOpenDisplay comme un entier 32 bits, tronque l'adresse, et le premier appel segfaulte.
# Le defaut ne se voit que si l'adresse depasse 32 bits — donc au hasard de la machine.
xlib.XOpenDisplay.restype = ctypes.c_void_p
xlib.XOpenDisplay.argtypes = [ctypes.c_char_p]
xlib.XStringToKeysym.restype = ctypes.c_ulong
xlib.XStringToKeysym.argtypes = [ctypes.c_char_p]
xlib.XKeysymToKeycode.restype = ctypes.c_ubyte
xlib.XKeysymToKeycode.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
xlib.XFlush.argtypes = [ctypes.c_void_p]
xlib.XCloseDisplay.argtypes = [ctypes.c_void_p]
xlib.XDefaultRootWindow.restype = ctypes.c_ulong
xlib.XDefaultRootWindow.argtypes = [ctypes.c_void_p]
xlib.XQueryTree.argtypes = [
    ctypes.c_void_p, ctypes.c_ulong,
    ctypes.POINTER(ctypes.c_ulong), ctypes.POINTER(ctypes.c_ulong),
    ctypes.POINTER(ctypes.POINTER(ctypes.c_ulong)), ctypes.POINTER(ctypes.c_uint),
]
xlib.XFetchName.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.POINTER(ctypes.c_char_p)]
xlib.XSetInputFocus.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_int, ctypes.c_ulong]
xlib.XRaiseWindow.argtypes = [ctypes.c_void_p, ctypes.c_ulong]
xlib.XGetWindowAttributes.argtypes = [ctypes.c_void_p, ctypes.c_ulong, ctypes.c_void_p]

# Une fenetre pas encore affichee refuse le focus (BadMatch), et l'erreur X par defaut
# TUE le programme. On l'ignore : le focus sera retente.
GESTIONNAIRE = ctypes.CFUNCTYPE(ctypes.c_int, ctypes.c_void_p, ctypes.c_void_p)
_silence = GESTIONNAIRE(lambda d, e: 0)
xlib.XSetErrorHandler(_silence)


class Attributs(ctypes.Structure):
    """Debut de XWindowAttributes : seul `map_state` nous interesse."""
    _fields_ = [
        ("x", ctypes.c_int), ("y", ctypes.c_int),
        ("width", ctypes.c_int), ("height", ctypes.c_int),
        ("border_width", ctypes.c_int), ("depth", ctypes.c_int),
        ("visual", ctypes.c_void_p), ("root", ctypes.c_ulong),
        ("class_", ctypes.c_int), ("bit_gravity", ctypes.c_int),
        ("win_gravity", ctypes.c_int), ("backing_store", ctypes.c_int),
        ("backing_planes", ctypes.c_ulong), ("backing_pixel", ctypes.c_ulong),
        ("save_under", ctypes.c_int), ("colormap", ctypes.c_ulong),
        ("map_installed", ctypes.c_int), ("map_state", ctypes.c_int),
    ]
xtst.XTestFakeKeyEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeButtonEvent.argtypes = [ctypes.c_void_p, ctypes.c_uint, ctypes.c_int, ctypes.c_ulong]
xtst.XTestFakeMotionEvent.argtypes = [
    ctypes.c_void_p, ctypes.c_int, ctypes.c_int, ctypes.c_int, ctypes.c_ulong,
]

dpy = xlib.XOpenDisplay(None)
if not dpy:
    raise SystemExit("impossible d'ouvrir le serveur X (DISPLAY absent ?)")

TOUCHES = {
    " ": "space", "-": "minus", "/": "slash", ".": "period", "_": "underscore",
    "\\": "backslash", "'": "apostrophe", "(": "parenleft", ")": "parenright",
    ":": "colon", ",": "comma", "=": "equal", "*": "asterisk", "$": "dollar",
    ">": "greater", "<": "less", "|": "bar", "&": "ampersand", ";": "semicolon",
}

def touche(nom, maj=False):
    kc = xlib.XKeysymToKeycode(dpy, xlib.XStringToKeysym(nom.encode()))
    shift = xlib.XKeysymToKeycode(dpy, xlib.XStringToKeysym(b"Shift_L"))
    if maj:
        xtst.XTestFakeKeyEvent(dpy, shift, True, 0)
    xtst.XTestFakeKeyEvent(dpy, kc, True, 0)
    xtst.XTestFakeKeyEvent(dpy, kc, False, 8)
    if maj:
        xtst.XTestFakeKeyEvent(dpy, shift, False, 0)
    xlib.XFlush(dpy)

def taper(texte):
    for ch in texte:
        if ch in TOUCHES:
            touche(TOUCHES[ch])
        elif ch.isupper():
            touche(ch.lower(), maj=True)
        else:
            touche(ch)
        time.sleep(0.03)

def focus(titre="Cockpit", essais=20, pause=0.5):
    """Donne le focus clavier a la fenetre de l'application.

    Sans gestionnaire de fenetres — le cas d'un Xvfb nu — personne n'attribue le focus :
    les clics passent, mais les frappes sont perdues. C'est ce qui faisait echouer un
    scenario sur deux, de facon apparemment aleatoire.

    On attend que la fenetre soit AFFICHEE : la demander trop tot rend BadMatch, et le
    focus n'est pas pose.
    """
    for _ in range(essais):
        if _focus_une_fois(titre):
            return True
        time.sleep(pause)
    return False


def _focus_une_fois(titre):
    racine = xlib.XDefaultRootWindow(dpy)
    r, p = ctypes.c_ulong(), ctypes.c_ulong()
    enfants = ctypes.POINTER(ctypes.c_ulong)()
    n = ctypes.c_uint()
    if not xlib.XQueryTree(dpy, racine, ctypes.byref(r), ctypes.byref(p),
                           ctypes.byref(enfants), ctypes.byref(n)):
        return False
    for i in range(n.value):
        fen = enfants[i]
        nom = ctypes.c_char_p()
        if xlib.XFetchName(dpy, fen, ctypes.byref(nom)) and nom.value:
            if titre.encode() in nom.value and _affichee(fen):
                xlib.XRaiseWindow(dpy, fen)
                xlib.XSetInputFocus(dpy, fen, 2, 0)  # RevertToParent, CurrentTime
                xlib.XFlush(dpy)
                return True
    # Faute de titre reconnu, la derniere fenetre AFFICHEE fait l'affaire.
    for i in range(n.value - 1, -1, -1):
        if _affichee(enfants[i]):
            xlib.XRaiseWindow(dpy, enfants[i])
            xlib.XSetInputFocus(dpy, enfants[i], 2, 0)
            xlib.XFlush(dpy)
            return True
    return False


def _affichee(fen):
    attrs = Attributs()
    if not xlib.XGetWindowAttributes(dpy, fen, ctypes.byref(attrs)):
        return False
    return attrs.map_state == 2  # IsViewable


def clic(x, y, bouton=1):
    xtst.XTestFakeMotionEvent(dpy, -1, int(x), int(y), 0)
    xlib.XFlush(dpy); time.sleep(0.15)
    xtst.XTestFakeButtonEvent(dpy, bouton, True, 0)
    xtst.XTestFakeButtonEvent(dpy, bouton, False, 20)
    xlib.XFlush(dpy)

for arg in sys.argv[1:]:
    genre, _, reste = arg.partition(":")
    if genre == "clic":
        x, y, *b = reste.split(",")
        clic(int(x), int(y), int(b[0]) if b else 1)
    elif genre == "molette":
        # Bouton du milieu. X numerote 1=gauche 2=milieu 3=droit ; le DOM 0=gauche 1=milieu.
        x, y = reste.split(",")
        clic(int(x), int(y), 2)
    elif genre == "taper":
        taper(reste)
    elif genre == "touche":
        touche(reste)
    elif genre == "focus":
        print("focus obtenu:", focus(reste or "Cockpit"))
    elif genre == "attendre":
        time.sleep(float(reste))
    print("fait:", arg, flush=True)
xlib.XCloseDisplay(dpy)
