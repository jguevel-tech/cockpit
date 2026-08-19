#!/bin/bash
# Scenario minimal : afficher beaucoup d'emoji (la documentation integree en est pleine) et
# regarder si le moteur de rendu tient. But : isoler l'assertion Skia sur les polices emoji
# en couleur, sans dependre de la creation d'un projet.
set -u
export HOME=/home/recette DISPLAY=:98
Xvfb :98 -screen 0 1400x900x24 > /sortie/xvfb.log 2>&1 &
sleep 2
/app/cockpit/AppRun > /sortie/app.log 2>&1 &
sleep 14
import -window root /sortie/A-demarrage.png
etat() { ps -eo comm | grep -c -i webkitweb || true; }
echo "processus de rendu avant : $(etat)" >> /sortie/verdict.txt
# Bouton « i » du bandeau : la documentation, pleine d'emoji
python3 /recette/pilote.py "clic:1171,24" "attendre:6"
import -window root /sortie/B-doc.png
echo "processus de rendu apres doc : $(etat)" >> /sortie/verdict.txt
# Parcourir toutes les sections, donc tous les emoji
for y in 126 162 198 234 270 306 342 378 414 450 486; do
  python3 /recette/pilote.py "clic:120,$y" "attendre:1.5" > /dev/null
done
import -window root /sortie/C-sections.png
echo "processus de rendu apres parcours : $(etat)" >> /sortie/verdict.txt
grep -iE "assertion|colrv1|skia|Segmentation" /sortie/app.log | head -5 >> /sortie/verdict.txt || true
