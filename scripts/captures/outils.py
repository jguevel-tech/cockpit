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

def _lecture(image):
    """Rend une fonction (x, y) -> (r, v, b) sur un pixbuf. Lire les octets une fois : un appel
    par pixel a travers gi coute plus que tout le reste du harnais."""
    octets = image.get_pixels()
    pas = image.get_rowstride()
    canaux = image.get_n_channels()

    def pixel(x: int, y: int):
        i = y * pas + x * canaux
        return octets[i], octets[i + 1], octets[i + 2]

    return pixel


def _pixbuf_fenetre():
    """La fenetre de l'application, rendue en image.

    Trois pieges paye ici : la capture doit venir de LA FENETRE (la racine rend du noir sous
    ecran virtuel), les fenetres 10x10 du meme nom sont techniques, et DEUX fenetres utiles
    veulent dire une instance de trop — il ne faut alors pas en choisir une, mais refuser.
    """
    import gi

    gi.require_version("Gdk", "3.0")
    gi.require_version("GdkX11", "3.0")
    from gi.repository import Gdk, GdkX11

    arbre = subprocess.run(["xwininfo", "-root", "-children"], capture_output=True, text=True).stdout
    candidats = [
        (int(m.group(1), 16), int(m.group(2)), int(m.group(3)))
        for m in re.finditer(r'(0x[0-9a-f]+) "Cockpit":.*?(\d+)x(\d+)\+', arbre)
    ]
    # Les autres fenetres du meme nom sont des 10x10 techniques.
    candidats = [c for c in candidats if c[1] > 200 and c[2] > 200]
    if not candidats:
        sys.exit("aucune fenetre Cockpit de taille utile : l'application n'est pas affichee")
    # DEUX fenetres utiles, c'est une instance de trop, et il ne faut pas EN CHOISIR une : les
    # clics vont a celle du dessus et la capture venait de l'autre — quatre fois la meme image,
    # sans une ligne d'erreur. On refuse, bruyamment.
    if len(candidats) > 1:
        tailles = ", ".join(f"{x:#x} {w}x{h}" for x, w, h in candidats)
        sys.exit(f"{len(candidats)} fenetres Cockpit affichees ({tailles}) : une instance de trop")
    xid, largeur_reelle, hauteur = candidats[0]

    fenetre = GdkX11.X11Window.foreign_new_for_display(GdkX11.X11Display.get_default(), xid)
    if fenetre is None:
        sys.exit(f"fenetre {xid:#x} introuvable cote Gdk")

    image = Gdk.pixbuf_get_from_window(fenetre, 0, 0, largeur_reelle, hauteur)
    if image is None:
        sys.exit("la fenetre n'a rendu aucun pixel")

    return image


def capturer(sortie: str, largeur: int = 0) -> None:
    # La fenetre d'abord : c'est elle qui declare la version de Gdk avant tout import.
    image = _pixbuf_fenetre()
    from gi.repository import GdkPixbuf

    largeur_reelle = image.get_width()
    hauteur = image.get_height()

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


# Les sept onglets d'un projet, dans l'ordre ou la barre les affiche.
ONGLETS = ("workspace", "docker", "terminal", "fichiers", "git", "plugins", "reglages")

# La bande horizontale ou vit la barre d'onglets, et la colonne ou commence le panneau de
# droite : a gauche, c'est la barre laterale, qui porte du texte a la meme hauteur.
BANDE_ONGLETS = (86, 112)
DEBUT_PANNEAU = 310


def onglet(nom: str) -> None:
    """Clique un onglet de projet en LISANT la barre a l'ecran, sans coordonnee ecrite en dur.

    **Un onglet ne se clique pas a une abscisse fixe : son libelle est traduit.** « Fichiers »
    et « Files » n'ont pas la meme largeur, donc en anglais un clic cale sur le francais tombait
    sur l'onglet VOISIN — et la capture montrait Git en croyant montrer les fichiers, sans une
    ligne d'erreur. Le controle sur les captures identiques ne voit rien : deux mauvais onglets
    donnent bien deux images differentes.

    On decoupe donc la bande de la barre en groupes de pixels encres separes par du vide : le
    premier groupe du panneau est le NOM du projet, les sept suivants sont les onglets. C'est la
    fenetre qui dit ou ils sont, comme l'outil dit toujours son propre etat.
    """
    import collections

    image = _pixbuf_fenetre()
    haut, bas = BANDE_ONGLETS
    pixels = _lecture(image)

    compte = collections.Counter(
        pixels(x, y) for x in range(image.get_width()) for y in range(haut, bas)
    )
    fond = compte.most_common(1)[0][0]

    def encree(x: int) -> bool:
        return any(
            sum(abs(a - b) for a, b in zip(pixels(x, y), fond)) > 18 for y in range(haut, bas)
        )

    groupes, debut, vide = [], None, 0
    for x in range(DEBUT_PANNEAU, image.get_width()):
        if encree(x):
            if debut is None:
                debut = x
            vide = 0
        elif debut is not None:
            vide += 1
            if vide >= 14:
                groupes.append((debut, x - vide))
                debut, vide = None, 0
    if debut is not None:
        groupes.append((debut, image.get_width() - 1))

    # Le premier groupe est le nom du projet ; restent les sept onglets. Un autre compte veut
    # dire que la barre a change de forme : on s'arrete au lieu de cliquer a cote.
    onglets = groupes[1:]
    if len(onglets) != len(ONGLETS):
        sys.exit(
            f"{len(onglets)} onglets lus dans la barre au lieu de {len(ONGLETS)} "
            f"(groupes : {groupes}) : la barre a change, les captures seraient fausses"
        )

    if nom not in ONGLETS:
        sys.exit(f"onglet inconnu : {nom} (attendus : {', '.join(ONGLETS)})")
    gauche, droite = onglets[ONGLETS.index(nom)]
    cliquer((gauche + droite) // 2, (haut + bas) // 2)


def arreter(marqueur: str, patience: float = 15.0) -> None:
    """Arrete l'application lancee par le harnais, et RIEN d'autre.

    Le `pkill -f "dbus-run-session -- <AppImage>"` d'avant ne visait que l'enveloppe. Le binaire
    monte depuis l'AppImage s'appelle `cockpit` et SURVIVAIT : la relance ouvrait donc une
    seconde fenetre, les clics allaient a l'une et la capture venait de l'autre.

    On ne peut pas viser le nom du programme : la machine de developpement fait tourner sa
    PROPRE installation de Cockpit, qui porte exactement le meme nom, avec les terminaux de
    quelqu'un dedans. Le critere est donc une variable d'environnement posee sur ce
    lancement-ci — le shell du harnais ne l'a pas, il ne peut pas se tuer lui-meme.
    """
    import signal

    def vises() -> list[int]:
        trouves = []
        for entree in os.listdir("/proc"):
            if not entree.isdigit():
                continue
            try:
                with open(f"/proc/{entree}/environ", "rb") as f:
                    if marqueur.encode() in f.read():
                        trouves.append(int(entree))
            except OSError:
                continue  # process disparu entre-temps, ou pas a nous : rien a arreter.
        return trouves

    premiers = vises()
    for pid in premiers:
        try:
            os.kill(pid, signal.SIGTERM)
        except OSError:
            pass

    fin = time.time() + patience
    while time.time() < fin:
        restants = vises()
        if not restants:
            print(f"  arrete : {len(premiers)} process")
            return
        time.sleep(0.5)

    for pid in vises():
        try:
            os.kill(pid, signal.SIGKILL)
        except OSError:
            pass
    print(f"  arrete de force : {len(premiers)} process")


# ── Base de demonstration ─────────────────────────────────────────────────────────────────────

# Le jeu de demonstration, dans les deux langues. Le site a besoin des memes ecrans en
# francais et en anglais : une capture ou l'interface est traduite mais dont les projets et les
# taches restent en francais ne montre pas le logiciel a un anglophone, elle lui montre le
# logiciel de quelqu'un d'autre.
#
# Les echeances sont RELATIVES au jour de la prise, pas des dates ecrites en dur : sinon
# « aujourd'hui » devient « en retard de 40 j » a la release suivante, sans que rien ne le dise.
DEMO = {
    "fr": {
        "dossiers": ("Clients", "Interne"),
        "projets": (
            ("boutique-vinyles", "Boutique en ligne, refonte du panier"),
            ("api-facturation", "API de facturation et relances"),
            ("site-vitrine", "Site public et blog"),
        ),
        "taches": (
            (0, "Refaire le tunnel de paiement", 0, 60, None),
            (0, "Corriger le calcul des frais de port", 0, 30, 0),
            (0, "Passer les images en webp", 1, 100, None),
            (0, "Ecrire les tests du panier", 0, 10, 4),
            (1, "Relances automatiques a J+7", 0, 80, -2),
            (1, "Export comptable en CSV", 0, 0, None),
        ),
        "adresses": (
            (0, "Prod", "https://exemple.test"),
            (0, "Recette", "https://recette.exemple.test"),
            (1, "Documentation", "https://docs.exemple.test"),
        ),
        "commandes": (
            (0, "Developpement", "npm run dev"),
            (0, "Tests", "npm test"),
            (1, "Serveur", "php -S localhost:8000"),
        ),
    },
    "en": {
        "dossiers": ("Clients", "Internal"),
        "projets": (
            ("vinyl-store", "Online store, cart rewrite"),
            ("billing-api", "Billing and dunning API"),
            ("landing-site", "Public site and blog"),
        ),
        "taches": (
            (0, "Rebuild the checkout flow", 0, 60, None),
            (0, "Fix the shipping fee calculation", 0, 30, 0),
            (0, "Move images to webp", 1, 100, None),
            (0, "Write the cart tests", 0, 10, 4),
            (1, "Automatic dunning on day 7", 0, 80, -2),
            (1, "Accounting export as CSV", 0, 0, None),
        ),
        "adresses": (
            (0, "Prod", "https://example.test"),
            (0, "Staging", "https://staging.example.test"),
            (1, "Documentation", "https://docs.example.test"),
        ),
        "commandes": (
            (0, "Development", "npm run dev"),
            (0, "Tests", "npm test"),
            (1, "Server", "php -S localhost:8000"),
        ),
    },
}


def preparer(base: str, projets: str, langue: str = "fr") -> None:
    """Remplit une base DEJA creee par l'application avec des donnees credibles.

    Aucune donnee reelle : ces captures finissent sur un site public.
    """
    import datetime

    jeu = DEMO[langue]
    aujourdhui = datetime.date.today()

    c = sqlite3.connect(base)
    x = c.cursor()

    # L'ecran d'accueil masquerait la capture, et la remontee d'erreurs n'a rien a faire ici.
    x.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('compte_accueil_vu','1')")
    x.execute("INSERT OR REPLACE INTO settings (key, value) VALUES ('error_reporting','off')")

    dossiers = []
    for position, nom in enumerate(jeu["dossiers"]):
        x.execute("INSERT INTO project_folders (name, position) VALUES (?, ?)", (nom, position))
        dossiers.append(x.lastrowid)

    noms = []
    for position, (nom, desc) in enumerate(jeu["projets"]):
        # Les deux premiers projets chez le client, le troisieme en interne.
        dossier = dossiers[0] if position < 2 else dossiers[1]
        x.execute(
            "INSERT INTO projects (name, path, folder_id, position, description) VALUES (?,?,?,?,?)",
            (nom, f"{projets}/{nom}", dossier, position, desc),
        )
        noms.append(nom)

    for position, (projet, texte, fait, avancement, jours) in enumerate(jeu["taches"]):
        echeance = None if jours is None else (aujourdhui + datetime.timedelta(days=jours)).isoformat()
        x.execute(
            "INSERT INTO todos (project, text, done, position, progress, due_date)"
            " VALUES (?,?,?,?,?,?)",
            (noms[projet], texte, fait, position, avancement, echeance),
        )

    for position, (projet, libelle, adresse) in enumerate(jeu["adresses"]):
        x.execute(
            "INSERT INTO urls (project, label, url, position) VALUES (?,?,?,?)",
            (noms[projet], libelle, adresse, position),
        )

    for position, (projet, libelle, commande) in enumerate(jeu["commandes"]):
        x.execute(
            "INSERT INTO project_commands (project, label, command, position) VALUES (?,?,?,?)",
            (noms[projet], libelle, commande, position),
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
    elif quoi == "arreter":
        arreter(sys.argv[2])
    elif quoi == "onglet":
        onglet(sys.argv[2])
    elif quoi == "preparer":
        preparer(sys.argv[2], sys.argv[3], sys.argv[4] if len(sys.argv) > 4 else "fr")
    else:
        sys.exit(f"sous-commande inconnue : {quoi}")
