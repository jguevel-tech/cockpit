#!/bin/bash
# Fabrique la demonstration animee de la promesse du logiciel : on ferme la fenetre, les
# terminaux continuent de tourner.
#
#   scripts/captures/demo-terminaux.sh <chemin/vers/le/binaire> <dossier de sortie> [fr|en]
#
# Pourquoi un GIF et pas une capture : la promesse porte sur ce qui se passe QUAND LA FENETRE
# N'EST PLUS LA. Une image fixe ne peut pas la montrer, et une phrase ne la prouve pas.
#
# Ce que la sequence fait, et pourquoi elle est convaincante : un compteur qui affiche l'HEURE
# tourne dans un terminal. On tue l'application — pas le service —, on attend, on relance. Le
# compteur a AVANCE pendant l'absence, et les horodatages le prouvent ligne par ligne. C'est la
# seule facon de montrer que rien n'a ete rejoue.
#
# Meme socle que `prendre.sh` : ecran virtuel, base de demonstration, arret cible par une
# variable d'environnement propre a ce lancement (voir l'en-tete de `outils.py`).
set -euo pipefail

# PAS D'APOSTROPHE dans un message de `${1:?...}` : bash y parse les quotes, meme a l'interieur
# de guillemets doubles, et l'apostrophe ouvre une chaine qui ne se ferme jamais. Le script
# entier devient alors une erreur de syntaxe a la derniere ligne, qui n'a rien a voir.
BINAIRE="${1:?chemin du binaire attendu}"
SORTIE="${2:-demo}"
LANGUE="${3:-en}"
case "$LANGUE" in fr|en) ;; *) echo "langue inconnue : $LANGUE" >&2; exit 1 ;; esac

TRAVAIL=/tmp/cockpit-demo
ECRAN=:96
ICI="$(cd "$(dirname "$0")" && pwd)"
OUTILS="$ICI/outils.py"
IMAGES="$TRAVAIL/images"

mkdir -p "$SORTIE"
# Le lancement precedent laisse des montages FUSE : sans les demonter, le nettoyage echoue.
for montage in "$TRAVAIL"/run/gvfs "$TRAVAIL"/run/doc; do
  [ -d "$montage" ] && { fusermount -u "$montage" 2>/dev/null || fusermount -uz "$montage" 2>/dev/null; } || true
done
rm -rf "$TRAVAIL"
mkdir -p "$TRAVAIL/run" "$TRAVAIL/home/projets/boutique-vinyles/src" "$IMAGES"
chmod 700 "$TRAVAIL/run"

PROJET="$TRAVAIL/home/projets/boutique-vinyles"
if [ "$LANGUE" = en ]; then PROJET="$TRAVAIL/home/projets/vinyl-store"; fi
mkdir -p "$PROJET/src"

# Le compteur est un SCRIPT et non une commande tapee : la frappe passe par XTEST, et les
# symboles d'un `while` en une ligne (`$`, `(`, `;`) dependent de la disposition du clavier.
# Un `./compteur.sh` ne contient que des lettres, un point et une barre — sans surprise.
cat > "$PROJET/compteur.sh" <<'SH'
#!/bin/sh
# Ce qui compte pour la demonstration : chaque ligne porte l'HEURE. Le trou dans les
# horodatages, pendant que l'application etait fermee, est la preuve que rien n'a ete rejoue.
i=0
while : ; do
  i=$((i + 1))
  printf 'build step %3d   %s\n' "$i" "$(date +%H:%M:%S)"
  sleep 1
done
SH
chmod +x "$PROJET/compteur.sh"
printf 'export const version = "1.1.0";\n' > "$PROJET/src/index.js"
git -C "$PROJET" init -q -b main
git -C "$PROJET" config user.email demo@exemple.test
git -C "$PROJET" config user.name Demo
git -C "$PROJET" add -A
git -C "$PROJET" commit -q -m "Set up the project"

export DISPLAY="$ECRAN"
unset WAYLAND_DISPLAY
export GDK_BACKEND=x11 GIO_USE_VFS=local
export HOME="$TRAVAIL/home"
export XDG_RUNTIME_DIR="$TRAVAIL/run"
export XDG_DATA_HOME="$TRAVAIL/home/.local/share"
export XDG_CONFIG_HOME="$TRAVAIL/home/.config"
export XDG_CACHE_HOME="$TRAVAIL/home/.cache"
export COCKPIT_DB="$TRAVAIL/demo.db"
export COCKPIT_LANGUE="$LANGUE"

cat > "$TRAVAIL/home/.zshrc" <<'ZSH'
autoload -Uz colors && colors
PROMPT='%F{blue}%1~%f %F{green}❯%f '
export PAGER=cat
ZSH

JETON="demo-terminaux-$$"
arreter_tout() { python3 "$OUTILS" arreter "COCKPIT_HARNAIS=$JETON"; }
nettoyer() { arreter_tout; [ -n "${XVFB:-}" ] && kill "$XVFB" 2>/dev/null || true; }
trap nettoyer EXIT

Xvfb "$ECRAN" -screen 0 1680x1050x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
sleep 3

lancer() { COCKPIT_HARNAIS="$JETON" dbus-run-session -- "$BINAIRE" >>"$TRAVAIL/app.log" 2>&1 & sleep 22; }
clic() { python3 "$OUTILS" cliquer "$1" "$2"; sleep "${3:-2}"; }
frappe() { python3 "$OUTILS" taper "$1"; sleep "${2:-2}"; }

NUMERO=0
image() { NUMERO=$((NUMERO + 1)); python3 "$OUTILS" capturer "$(printf '%s/%03d-%s.png' "$IMAGES" "$NUMERO" "$1")" >/dev/null; }
# Sans fenetre, il reste l'ECRAN : c'est exactement l'image qu'on veut a cet instant.
image_ecran() { NUMERO=$((NUMERO + 1)); python3 "$OUTILS" capturer "$(printf '%s/%03d-%s.png' "$IMAGES" "$NUMERO" "$1")" --racine >/dev/null; }

echo "── premier lancement : creation du schema"
unset DBUS_SESSION_BUS_ADDRESS
lancer
arreter_tout
sleep 2
python3 "$OUTILS" preparer "$COCKPIT_DB" "$TRAVAIL/home/projets" "$LANGUE"

echo "── mise en place : un terminal, et un compteur qui tourne"
lancer
clic 104 168 3        # le projet, dans la barre laterale
python3 "$OUTILS" onglet terminal
sleep 4
clic 790 512 10       # « ouvrir un terminal »
clic 800 400 1        # le focus dedans
frappe "./compteur.sh" 6

echo "── phase 1 : ca tourne, fenetre ouverte"
for _ in 1 2 3 4 5; do image ouverte; sleep 1; done

echo "── on ferme l'APPLICATION, pas le service"
# `arreter_tout` tuerait aussi le service (il herite du meme environnement) : on ne vise donc
# QUE le processus de l'application, celui qui n'a pas le drapeau de service.
python3 "$OUTILS" arreter "COCKPIT_HARNAIS=$JETON" --sauf-service
sleep 2
image_ecran fermee
echo "── on attend : le compteur, lui, continue"
sleep 8
image_ecran attente

echo "── on relance"
lancer
# Au retour, l'application ouvre sur son tableau de bord : c'est ce qu'on voit vraiment, donc
# c'est ce qu'on montre. Puis on retourne au terminal — sans ces deux clics, la legende
# annoncerait « meme terminal » au-dessus d'un ecran qui n'en montre aucun.
image relancee
# UN SEUL CLIC, et c'est le geste naturel : la barre laterale porte une section TERMINAUX des
# qu'un terminal tourne — donc, ici, apres la fermeture. Cliquer son entree y ramene
# directement. Au passage, cette section est la PREUVE que le service a survecu.
#
# Ne PAS viser le projet a la place : la section TERMINAUX pousse la liste des projets vers le
# bas, et les coordonnees du debut de la sequence ne designent plus la meme ligne.
clic 104 106 4        # le terminal, dans la barre laterale
sleep 2
echo "── ce que le service a garde pendant l'absence :"
python3 "$OUTILS" arreter "COCKPIT_HARNAIS=$JETON" --lister-service || true
for _ in 1 2 3 4; do image revenue; sleep 1; done

echo "── assemblage du GIF"
python3 "$ICI/animer.py" "$IMAGES" "$SORTIE/terminaux-persistants.gif" "$LANGUE"
echo "── fait : $SORTIE/terminaux-persistants.gif"
