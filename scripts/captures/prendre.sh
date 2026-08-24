#!/bin/bash
# Refait les captures du logiciel qui illustrent le site.
#
#   scripts/captures/prendre.sh <chemin/vers/Cockpit.AppImage> [dossier de sortie] [fr|en]
#
# La LANGUE est le troisieme argument, parce que le site vitrine a besoin des memes ecrans dans
# les deux : un anglophone ne comprend rien a des captures en francais. Elle passe par
# `COCKPIT_LANGUE`, que le logiciel lit au demarrage — piloter les menus pour changer de langue
# aurait rendu les captures dependantes de la position d'une entree de menu.
#
# Les deux jeux, en une fois :
#   scripts/captures/prendre.sh Cockpit.AppImage captures/fr fr
#   scripts/captures/prendre.sh Cockpit.AppImage captures/en en
#
# Le logiciel est lance sous ecran virtuel avec une base de DEMONSTRATION : aucune donnee
# reelle n'apparait sur les images, qui finissent sur un site public.
#
# Prerequis, tous absents d'une machine nue :
#   - Xvfb, xwininfo, dbus-run-session      (paquets xvfb, x11-utils, dbus)
#   - python3-gi                            (la capture passe par Gdk, comme les bancs de rendu)
#   - python3-xlib                          (les clics passent par XTEST)
# Sans droits administrateur : `apt-get download <paquet>` puis `dpkg-deb -x` dans un prefixe a
# soi, et `PYTHONPATH` vers son `dist-packages`.
#
# Pourquoi un dossier de travail dans /tmp et pas ailleurs : le socket des terminaux est limite
# a ~108 octets sous Unix, et un chemin profond fait echouer l'ouverture d'un terminal sans que
# rien ne dise pourquoi.
set -euo pipefail

APPIMAGE="${1:?chemin vers le fichier AppImage attendu}"
SORTIE="${2:-captures}"
LANGUE="${3:-fr}"
case "$LANGUE" in
  fr|en) ;;
  *) echo "langue inconnue : $LANGUE (attendu fr ou en)" >&2; exit 1 ;;
esac
TRAVAIL=/tmp/cockpit-captures
ECRAN=:99
OUTILS="$(cd "$(dirname "$0")" && pwd)/outils.py"

mkdir -p "$SORTIE"
rm -rf "$TRAVAIL"
mkdir -p "$TRAVAIL/run" "$TRAVAIL/home/projets"
chmod 700 "$TRAVAIL/run"

# ── Des projets qui ont l'air vrais : de vrais depots git, du vrai code a colorer.
for p in boutique-vinyles api-facturation site-vitrine; do
  d="$TRAVAIL/home/projets/$p"
  mkdir -p "$d/src"
  printf '# %s\n\nProjet de demonstration.\n' "$p" > "$d/README.md"
  printf '{\n  "name": "%s",\n  "version": "1.0.0"\n}\n' "$p" > "$d/package.json"
  printf 'export function bonjour(nom) {\n  return `Bonjour ${nom}`;\n}\n' > "$d/src/index.js"
  git -C "$d" init -q -b main
  git -C "$d" config user.email demo@exemple.test
  git -C "$d" config user.name Demo
  git -C "$d" add -A && git -C "$d" commit -q -m "Poser les bases du projet"
  printf 'export const version = "1.1.0";\n' > "$d/src/version.js"
  git -C "$d" add -A && git -C "$d" commit -q -m "Ajouter le numero de version"
done

# Une modification non validee, pour que l'onglet Git ait un diff a montrer.
cat > "$TRAVAIL/home/projets/boutique-vinyles/src/panier.js" <<'JS'
export function totalDuPanier(articles) {
  return articles.reduce((somme, a) => somme + a.prix * a.quantite, 0);
}

export function fraisDePort(total) {
  if (total >= 5000) return 0;
  return total > 2000 ? 490 : 690;
}
JS
cat > "$TRAVAIL/home/projets/boutique-vinyles/src/index.js" <<'JS'
import { totalDuPanier, fraisDePort } from "./panier.js";

export function bonjour(nom) {
  return `Bonjour ${nom}`;
}

export function resume(articles) {
  const total = totalDuPanier(articles);
  return { total, port: fraisDePort(total) };
}
JS

# Une invite sobre : sans .zshrc, le shell affiche son assistant de premier lancement.
cat > "$TRAVAIL/home/.zshrc" <<'ZSH'
autoload -Uz colors && colors
PROMPT='%F{blue}%1~%f %F{green}❯%f '
export PAGER=cat
alias ls='ls --color=auto'
ZSH

export DISPLAY="$ECRAN"
export HOME="$TRAVAIL/home"
export XDG_RUNTIME_DIR="$TRAVAIL/run"
export XDG_DATA_HOME="$TRAVAIL/home/.local/share"
export XDG_CONFIG_HOME="$TRAVAIL/home/.config"
export XDG_CACHE_HOME="$TRAVAIL/home/.cache"
export COCKPIT_DB="$TRAVAIL/demo.db"
# Le logiciel la lit au demarrage et bascule avant le premier rendu utile.
export COCKPIT_LANGUE="$LANGUE"

nettoyer() {
  pkill -f "dbus-run-session -- $APPIMAGE" 2>/dev/null || true
  [ -n "${XVFB:-}" ] && kill "$XVFB" 2>/dev/null || true
}
trap nettoyer EXIT

Xvfb "$ECRAN" -screen 0 1680x1050x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
sleep 3

# Un bus DBus a nous : le verrou d'instance unique passe par la, et sans ca le lancement rend
# la main a l'instance deja ouverte en sortant avec un code 0.
lancer() { dbus-run-session -- "$APPIMAGE" >"$TRAVAIL/app.log" 2>&1 & sleep 22; }
clic()   { python3 "$OUTILS" cliquer "$1" "$2"; sleep "${3:-3}"; }
frappe() { python3 "$OUTILS" taper "$1"; sleep "${2:-2}"; }
prise()  { python3 "$OUTILS" capturer "$SORTIE/$1.png"; }

echo "── premier lancement : creation du schema par le logiciel"
unset DBUS_SESSION_BUS_ADDRESS
lancer
pkill -f "dbus-run-session -- $APPIMAGE" || true
sleep 3

echo "── remplissage de la base de demonstration"
python3 "$OUTILS" preparer "$COCKPIT_DB" "$TRAVAIL/home/projets"

echo "── relance et captures"
lancer
prise taches
clic 104 168          # le projet, dans la barre laterale
clic 728 98 4         # onglet Terminal
clic 790 512 10       # « Ouvrir un terminal »
clic 800 400 1        # le focus dans le terminal
frappe "git status --short --branch"
frappe "git log --oneline -3"
frappe "ls src" 3
prise terminal
clic 879 98 5         # onglet Git
clic 392 304 4        # le fichier modifie, pour afficher son diff
prise git
clic 813 98 5         # onglet Fichiers
clic 391 296 4        # un fichier, pour le montrer colore
prise fichiers

echo "── fait, dans $SORTIE (langue : $LANGUE)"
