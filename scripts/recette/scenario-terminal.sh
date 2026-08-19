#!/bin/bash
# Reproduire l'assertion Skia (polices emoji COLRv1) dans le VRAI chemin : un terminal
# xterm.js rendu par WebGL. En deux phases, parce que le schema de la base est cree par
# l'application elle-meme : on la lance une premiere fois, on inscrit un projet, on relance.
set -u
export HOME=/home/recette DISPLAY=:98
export COCKPIT_DB=/sortie/data.db
mkdir -p "$HOME/projet"
Xvfb :98 -screen 0 1400x900x24 > /sortie/xvfb.log 2>&1 &
sleep 2

echo "phase 1 : creation du schema" > /sortie/verdict.txt
/app/cockpit/AppRun > /sortie/app1.log 2>&1 &
sleep 12
pkill -f "AppRun|cockpit" 2>/dev/null; sleep 3
sqlite3 /sortie/data.db "insert into projects(name,path,compose_file,description,depends_on,position) values('banc','/home/recette/projet','','',' []',0);" 2>>/sortie/verdict.txt
sqlite3 /sortie/data.db "select name,path from projects;" >> /sortie/verdict.txt 2>&1

echo "phase 2 : ouverture d'un terminal" >> /sortie/verdict.txt
/app/cockpit/AppRun > /sortie/app2.log 2>&1 &
sleep 14
import -window root /sortie/A-demarrage.png
rendu() { ps -eo comm | grep -c -i webkitweb || true; }
echo "rendu au demarrage : $(rendu)" >> /sortie/verdict.txt
python3 /recette/pilote.py "clic:130,120" "attendre:3" "clic:527,85" "attendre:12" > /dev/null
import -window root /sortie/B-terminal.png
echo "rendu apres ouverture du terminal : $(rendu)" >> /sortie/verdict.txt
tmux -L cockpit list-sessions >> /sortie/verdict.txt 2>&1 || echo "(aucune session)" >> /sortie/verdict.txt
# Emoji produit par le shell
python3 /recette/pilote.py "clic:700,300" "attendre:1" "taper:printf '\\xF0\\x9F\\x90\\xB3\\n'" "touche:Return" "attendre:8" > /dev/null
import -window root /sortie/C-emoji.png
echo "rendu apres emoji : $(rendu)" >> /sortie/verdict.txt
grep -iE "assertion|colrv1|skia|__n <|Segmentation" /sortie/app2.log | head -4 >> /sortie/verdict.txt || true
