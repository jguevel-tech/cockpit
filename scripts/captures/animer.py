"""Assemble les images d'une demonstration en un GIF, legendes comprises.

Lance par `demo-terminaux.sh`, pas directement.

**Pourquoi des legendes DANS l'image** : ce GIF part sur des sites ou personne ne lit le texte
autour, et la phrase qu'il doit prouver — « les terminaux survivent a la fermeture » — n'a aucun
sens sans dire a quel moment la fenetre a ete fermee. Trois lignes suffisent.

**Pourquoi un GIF et pas une video** : il s'affiche tout seul dans un README GitHub, dans un
message Reddit et sur une page web, sans lecteur, sans clic et sans son. Le prix est le poids,
qu'on tient par la largeur et par le nombre de couleurs.
"""
from __future__ import annotations

import glob
import os
import sys

from PIL import Image, ImageDraw, ImageFont

# Largeur du GIF. 900 px : lisible dans un README, et le poids reste tenable.
LARGEUR = 900
# Le nombre de couleurs decide du poids. 128 suffit pour une interface sombre.
COULEURS = 128
POLICE = "/usr/share/fonts/truetype/dejavu/DejaVuSans-Bold.ttf"

# Une legende et une duree par phase. La phase est le suffixe du nom de fichier.
#
# Les durees font la longueur du film. **Vingt secondes, pas plus** : c'est ce qu'on regarde
# avant de decider si on telecharge. Un ecran a lire tient en 2,5 s ; le dernier reste plus
# longtemps, parce qu'un GIF qui reboucle sans respirer donne l'impression d'un bug.
PHASES = {
    "fr": {
        "taches": ("Les taches de TOUS vos projets, au meme endroit", 2600),
        "monitoring": ("Processeur, memoire, disques — et des alertes quand ca serre", 2400),
        "workspace": ("Notes et taches du projet, cote a cote", 2400),
        "terminal": ("Des terminaux qui survivent a la fermeture de la fenetre", 2800),
        "git": ("Git sans quitter : etat, diff colore, commit, branches", 2800),
        "fichiers": ("Vos fichiers, colores, avec saut a la definition", 2600),
        "ia": ("N'importe quel agent IA — c'est vous qui choisissez", 3000),
        "ouverte": ("Un build tourne dans un terminal de Cockpit", 900),
        "fermee": ("On ferme Cockpit — la fenetre disparait", 1800),
        "attente": ("Huit secondes plus tard. Le shell, lui, n'a jamais cesse.", 2200),
        "relancee": ("On rouvre Cockpit…", 1200),
        "revenue": ("Meme terminal — et l'horloge n'a jamais cesse", 1100),
    },
    "en": {
        "taches": ("Every project's open tasks, in one place", 2600),
        "monitoring": ("CPU, memory, disks — with alerts when it gets tight", 2400),
        "workspace": ("The project's notes and tasks, side by side", 2400),
        "terminal": ("Terminals that outlive the window you closed", 2800),
        "git": ("Git without leaving: status, coloured diff, commit, branches", 2800),
        "fichiers": ("Your files, highlighted, with go-to-definition", 2600),
        "ia": ("Any AI agent — you pick, not us", 3000),
        "ouverte": ("A build is running in a Cockpit terminal", 900),
        "fermee": ("You close Cockpit — the window is gone", 1800),
        "attente": ("Eight seconds later. The shell never stopped.", 2200),
        "relancee": ("Reopen Cockpit…", 1200),
        "revenue": ("Same terminal — and the clock never stopped", 1100),
    },
}

# ── Ce qu'on masque avant de publier.
#
# **UN IDENTIFIANT REEL DE MACHINE N'A RIEN A FAIRE SUR UNE PAGE PUBLIQUE.** Le nom de l'hote
# vient du systeme (`System::host_name`) et ne se regle pas : on ne peut donc pas demander au
# logiciel d'en afficher un autre pour la demonstration. On le recouvre ici, a l'endroit ou la
# pastille se trouve, par un nom neutre — et si la couleur attendue n'est pas la, on ARRETE au
# lieu de peindre au hasard sur une interface qui a bouge.
MASQUES = {
    "monitoring": {
        "boite": (562, 139, 648, 163),
        "couleur": (109, 141, 255),
        "texte": {"fr": "MA-MACHINE", "en": "MY-LAPTOP"},
    },
}

BANDE = 52
FOND = (18, 16, 14)
TEXTE = (245, 241, 232)
ACCENT = (244, 83, 31)


def phase_de(chemin: str) -> str:
    """`012-revenue.png` -> `revenue`."""
    return os.path.basename(chemin).rsplit("-", 1)[-1].rsplit(".", 1)[0]


def masquer(image: Image.Image, phase: str, langue: str) -> None:
    """Recouvre un identifiant reel par un nom neutre. Sur place, avant toute mise a l'echelle."""
    masque = MASQUES.get(phase)
    if masque is None:
        return
    gauche, haut, droite, bas = masque["boite"]
    attendue = masque["couleur"]
    # Le coin de la pastille doit porter sa couleur : sinon l'interface a change de forme et on
    # s'apprete a peindre sur autre chose. Mieux vaut un echec bruyant qu'un masque de travers.
    lu = image.getpixel((gauche + 3, (haut + bas) // 2))
    if max(abs(a - b) for a, b in zip(lu, attendue)) > 40:
        sys.exit(
            f"masque {phase} : couleur {lu} au lieu de {attendue} — l'interface a bouge, "
            "les coordonnees du masque sont a revoir"
        )

    dessin = ImageDraw.Draw(image)
    dessin.rectangle([(gauche, haut), (droite, bas)], fill=attendue)
    try:
        police = ImageFont.truetype(POLICE, 12)
    except OSError:
        police = ImageFont.load_default()
    mot = masque["texte"][langue]
    largeur = dessin.textlength(mot, font=police)
    dessin.text(
        (gauche + (droite - gauche - largeur) / 2, haut + 4),
        mot,
        font=police,
        fill=(255, 255, 255),
    )


def legender(image: Image.Image, texte: str, accent: bool) -> Image.Image:
    """Ajoute la bande de legende sous l'image, a la largeur voulue."""
    hauteur = round(image.height * LARGEUR / image.width)
    reduite = image.resize((LARGEUR, hauteur), Image.LANCZOS)

    plan = Image.new("RGB", (LARGEUR, hauteur + BANDE), FOND)
    plan.paste(reduite, (0, 0))

    dessin = ImageDraw.Draw(plan)
    try:
        police = ImageFont.truetype(POLICE, 19)
    except OSError:
        # Sans la police du systeme, la legende reste lisible : mieux qu'un GIF sans legende.
        police = ImageFont.load_default()

    # Un filet de la couleur d'accent separe l'image de sa legende : sans lui, la bande se
    # confond avec le fond sombre de l'interface et le texte semble flotter dedans.
    dessin.rectangle([(0, hauteur), (LARGEUR, hauteur + 3)], fill=ACCENT if accent else (60, 56, 50))
    largeur_texte = dessin.textlength(texte, font=police)
    dessin.text(
        ((LARGEUR - largeur_texte) / 2, hauteur + 16),
        texte,
        font=police,
        fill=ACCENT if accent else TEXTE,
    )
    return plan


def main() -> None:
    dossier, sortie, langue = sys.argv[1], sys.argv[2], sys.argv[3]
    legendes = PHASES[langue]

    fichiers = sorted(glob.glob(os.path.join(dossier, "*.png")))
    if not fichiers:
        sys.exit(f"aucune image dans {dossier}")

    images: list[Image.Image] = []
    durees: list[int] = []
    for chemin in fichiers:
        phase = phase_de(chemin)
        if phase not in legendes:
            sys.exit(f"phase inconnue : {phase} ({chemin})")
        texte, duree = legendes[phase]
        # Les deux moments qui portent la demonstration sont en couleur d'accent : la fermeture
        # et l'attente. Le reste est du contexte.
        accent = phase in ("fermee", "attente", "ia")
        with Image.open(chemin) as brute:
            pleine = brute.convert("RGB")
            masquer(pleine, phase, langue)
            images.append(legender(pleine, texte, accent))
        durees.append(duree)

    # La derniere image reste un peu plus longtemps : un GIF qui reboucle sans respirer donne
    # l'impression d'un bug. La duree vient de la phase, pas d'une valeur ecrasee ici.
    durees[-1] = max(durees[-1], 3000)
    print(f"  duree totale : {sum(durees) / 1000:.1f} s")

    palette = [image.quantize(colors=COULEURS, method=Image.MEDIANCUT) for image in images]
    palette[0].save(
        sortie,
        save_all=True,
        append_images=palette[1:],
        duration=durees,
        loop=0,
        optimize=True,
        disposal=2,
    )
    poids = os.path.getsize(sortie) / 1_000_000
    print(f"  {sortie}  {len(palette)} images  {palette[0].width}x{palette[0].height}  {poids:.1f} Mo")


if __name__ == "__main__":
    main()
