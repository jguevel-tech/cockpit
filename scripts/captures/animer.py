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
PHASES = {
    "fr": {
        "ouverte": ("Un build tourne dans un terminal de Cockpit", 900),
        "fermee": ("On ferme Cockpit — la fenetre disparait", 1800),
        "attente": ("Huit secondes plus tard. Le shell, lui, n'a jamais cesse.", 2200),
        "relancee": ("On rouvre Cockpit…", 1200),
        "revenue": ("Meme terminal — et l'horloge n'a jamais cesse", 1100),
    },
    "en": {
        "ouverte": ("A build is running in a Cockpit terminal", 900),
        "fermee": ("You close Cockpit — the window is gone", 1800),
        "attente": ("Eight seconds later. The shell never stopped.", 2200),
        "relancee": ("Reopen Cockpit…", 1200),
        "revenue": ("Same terminal — and the clock never stopped", 1100),
    },
}

BANDE = 52
FOND = (18, 16, 14)
TEXTE = (245, 241, 232)
ACCENT = (244, 83, 31)


def phase_de(chemin: str) -> str:
    """`012-revenue.png` -> `revenue`."""
    return os.path.basename(chemin).rsplit("-", 1)[-1].rsplit(".", 1)[0]


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
        accent = phase in ("fermee", "attente")
        with Image.open(chemin) as brute:
            images.append(legender(brute.convert("RGB"), texte, accent))
        durees.append(duree)

    # La derniere image reste plus longtemps : c'est celle qu'on regarde en se demandant si
    # c'est vrai, et un GIF qui reboucle trop vite ne laisse pas le temps de lire les heures.
    durees[-1] = 3000

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
