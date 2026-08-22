"""Outils du harnais de captures : preparer une base de demonstration, cliquer, taper, capturer.

Lance par `prendre.sh`, pas directement. Chaque sous-commande fait UNE chose, pour qu'on puisse
reprendre une capture rate sans tout refaire.

Trois pieges sont payes ici, et chacun a coute du temps :

1. **La capture doit venir de LA FENETRE, pas de la racine.** Sous ecran virtuel, une capture de
   la racine rend une image noire.
2. **`Cockpit` refuse une seconde instance, et le verrou passe par DBus** — partage quel que soit
   le `HOME`. Sans bus prive, le lancement se contente de rendre la main a l'instance ouverte et
   sort avec un code 0, ce qui ressemble a un succes.
3. **Le chemin du socket des terminaux est limite a ~108 octets.** Un dossier de travail profond
   fait echouer l'ouverture d'un terminal sans que rien ne dise pourquoi : d'ou un chemin court.
"""
from __future__ import annotations

import os
import re
import sqlite3
import subprocess
import sys
import time


# ── Capture ───────────────────────────────────────────────────────────────────────────────────

def capturer(sortie: str, largeur: int = 0) -> None:
    import gi

    gi.require_version("Gdk", "3.0")
    gi.require_version("GdkX11", "3.0")
    from gi.repository import Gdk, GdkPixbuf, GdkX11

    arbre = subprocess.run(["xwininfo", "-root", "-children"], capture_output=True, text=True).stdout
    candidats = [
        (int(m.group(1), 16), int(m.group(2)), int(m.group(3)))
        for m in re.finditer(r'(0x[0-9a-f]+) "Cockpit":.*?(\d+)x(\d+)\+', arbre)
    ]
    # Les autres fenetres du meme nom sont des 10x10 techniques.
    candidats = [c for c in candidats if c[1] > 200 and c[2] > 200]
    if not candidats:
        sys.exit("aucune fenetre Cockpit de taille utile : l'application n'est pas affichee")
    xid, largeur_reelle, hauteur = max(candidats, key=lambda c: c[1] * c[2])

    fenetre = GdkX11.X11Window.foreign_new_for_display(GdkX11.X11Display.get_default(), xid)
    if fenetre is None:
        sys.exit(f"fenetre {xid:#x} introuvable cote Gdk")

    image = Gdk.pixbuf_get_from_window(fenetre, 0, 0, largeur_reelle, hauteur)
    if image is None:
        sys.exit("la fenetre n'a rendu aucun pixel")

    if largeur and largeur != largeur_reelle:
        image = image.scale_simple(
            largeur, round(hauteur * largeur / largeur_reelle), GdkPixbuf.InterpType.BILINEAR
        )

    image.savev(sortie, "png", [], [])
    print(f"  {sortie}  {image.get_width()}x{image.get_height()}")


# ── Pilotage ──────────────────────────────────────────────────────────────────────────────────

def _ecran():
    from Xlib import display

    return display.Display(os.environ.get("DISPLAY", ":99"))


def cliquer(x: int, y: int) -> None:
    from Xlib import X
    from Xlib.ext import xtest

    d = _ecran()
    d.screen().root.warp_pointer(x, y)
    d.sync()
    time.sleep(0.2)
    xtest.fake_input(d, X.ButtonPress, 1)
    d.sync()
    time.sleep(0.08)
    xtest.fake_input(d, X.ButtonRelease, 1)
    d.sync()


def taper(texte: str, entree: bool = True) -> None:
    """Frappe un texte, majuscules et symboles compris.

    Le piege : un caractere ne suffit pas a designer une touche. Sur un clavier francais, « @ »
    partage sa touche avec « 2 », et une frappe sans modificateur produit « 2 ». On demande donc
    au serveur X ou vit chaque symbole, et on presse Maj — ou AltGr — quand il faut.
    """
    from Xlib import X, XK
    from Xlib.ext import xtest

    d = _ecran()
    noms = {" ": "space", "-": "minus", ".": "period", "/": "slash", "@": "at",
            ":": "colon", "_": "underscore", "\n": "Return"}

    def touche(car: str):
        sym = XK.string_to_keysym(noms.get(car, car))
        if not sym:
            return None
        for code, rang in d.keysym_to_keycodes(sym):
            # rang 0 = touche nue, 1 = avec Maj, 2/3 = avec AltGr (troisieme niveau).
            if rang in (0, 1, 2, 3):
                return code, rang
        return None

    maj = d.keysym_to_keycode(XK.string_to_keysym("Shift_L"))
    altgr = d.keysym_to_keycode(XK.string_to_keysym("ISO_Level3_Shift"))

    for car in texte + ("\n" if entree else ""):
        trouve = touche(car)
        if trouve is None:
            continue
        code, rang = trouve
        modificateur = maj if rang == 1 else (altgr if rang in (2, 3) else None)

        if modificateur:
            xtest.fake_input(d, X.KeyPress, modificateur); d.sync(); time.sleep(0.02)
        xtest.fake_input(d, X.KeyPress, code); d.sync(); time.sleep(0.03)
        xtest.fake_input(d, X.KeyRelease, code); d.sync(); time.sleep(0.02)
        if modificateur:
            xtest.fake_input(d, X.KeyRelease, modificateur); d.sync()
        time.sleep(0.03)


# ── Base de demonstration ─────────────────────────────────────────────────────────────────────

def preparer(base: str, projets: str) -> None:
    """Remplit une base DEJA creee par l'application avec des donnees credibles.

    Aucune donnee reelle : ces captures finissent sur un site public.
    """
    c = sqlite3.connect(base)
    x = c.cursor()

    # L'ecran d'accueil masquerait la capture, et la remontee d'erreurs n'a rien a faire ici.
    x.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('compte_accueil_vu','1')")
    x.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('error_reporting','off')")

    x.execute("INSERT INTO project_folders (name, position) VALUES ('Clients', 0)")
    clients = x.lastrowid
    x.execute("INSERT INTO project_folders (name, position) VALUES ('Interne', 1)")
    interne = x.lastrowid

    for nom, dossier, pos, desc in (
        ("boutique-vinyles", clients, 0, "Boutique en ligne, refonte du panier"),
        ("api-facturation", clients, 1, "API de facturation et relances"),
        ("site-vitrine", interne, 2, "Site public et blog"),
    ):
        x.execute(
            "INSERT INTO projects (name, path, folder_id, position, description) VALUES (?,?,?,?,?)",
            (nom, f"{projets}/{nom}", dossier, pos, desc),
        )

    for p, t, done, pos, prog, due in (
        ("boutique-vinyles", "Refaire le tunnel de paiement", 0, 0, 60, None),
        ("boutique-vinyles", "Corriger le calcul des frais de port", 0, 1, 30, "2026-08-25"),
        ("boutique-vinyles", "Passer les images en webp", 1, 2, 100, None),
        ("boutique-vinyles", "Ecrire les tests du panier", 0, 3, 10, "2026-08-29"),
        ("api-facturation", "Relances automatiques a J+7", 0, 0, 80, "2026-08-23"),
        ("api-facturation", "Export comptable en CSV", 0, 1, 0, None),
    ):
        x.execute(
            "INSERT INTO todos (project, text, done, position, progress, due_date)"
            " VALUES (?,?,?,?,?,?)",
            (p, t, done, pos, prog, due),
        )

    for p, lab, url, pos in (
        ("boutique-vinyles", "Prod", "https://exemple.test", 0),
        ("boutique-vinyles", "Recette", "https://recette.exemple.test", 1),
        ("api-facturation", "Documentation", "https://docs.exemple.test", 0),
    ):
        x.execute("INSERT INTO urls (project, label, url, position) VALUES (?,?,?,?)", (p, lab, url, pos))

    for p, lab, cmd, pos in (
        ("boutique-vinyles", "Developpement", "npm run dev", 0),
        ("boutique-vinyles", "Tests", "npm test", 1),
        ("api-facturation", "Serveur", "php -S localhost:8000", 0),
    ):
        x.execute(
            "INSERT INTO project_commands (project, label, command, position) VALUES (?,?,?,?)",
            (p, lab, cmd, pos),
        )

    c.commit()
    print(f"  base remplie : {x.execute('select count(*) from projects').fetchone()[0]} projets")


# ── Entree ────────────────────────────────────────────────────────────────────────────────────

if __name__ == "__main__":
    quoi = sys.argv[1]
    if quoi == "capturer":
        capturer(sys.argv[2], int(sys.argv[3]) if len(sys.argv) > 3 else 0)
    elif quoi == "cliquer":
        cliquer(int(sys.argv[2]), int(sys.argv[3]))
    elif quoi == "taper":
        taper(sys.argv[2])
    elif quoi == "preparer":
        preparer(sys.argv[2], sys.argv[3])
    else:
        sys.exit(f"sous-commande inconnue : {quoi}")
