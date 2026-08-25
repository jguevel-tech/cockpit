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
# Le lancement precedent a laisse des montages FUSE dans son dossier de travail (gvfs, et le
# portail de documents) : sans les demonter, le `rm -rf` echoue sur « peripherique occupe » et le
# harnais s'arrete avant d'avoir rien fait. `GIO_USE_VFS` plus bas evite le premier des deux.
for montage in "$TRAVAIL"/run/gvfs "$TRAVAIL"/run/doc; do
  [ -d "$montage" ] && { fusermount -u "$montage" 2>/dev/null || fusermount -uz "$montage" 2>/dev/null; } || true
done
rm -rf "$TRAVAIL"
mkdir -p "$TRAVAIL/run" "$TRAVAIL/home/projets"
chmod 700 "$TRAVAIL/run"

# ── Des projets qui ont l'air vrais : de vrais depots git, du vrai code a colorer.
#
# TOUT ce qui se lit sur l'image suit la langue : noms de projets, messages de commit, noms de
# fonctions, noms de fichiers. Une capture ou l'interface est en anglais et le contenu en
# francais est a moitie faite, et c'est le premier plan de la page d'accueil qui le montre.
if [ "$LANGUE" = fr ]; then
  PROJETS="boutique-vinyles api-facturation site-vitrine"
  VITRINE=boutique-vinyles
  MODULE=panier
  COMMIT_1="Poser les bases du projet"
  COMMIT_2="Ajouter le numero de version"
  RESUME_README="Projet de demonstration."
else
  PROJETS="vinyl-store billing-api landing-site"
  VITRINE=vinyl-store
  MODULE=cart
  COMMIT_1="Set up the project"
  COMMIT_2="Add the version number"
  RESUME_README="Demonstration project."
fi

for p in $PROJETS; do
  d="$TRAVAIL/home/projets/$p"
  mkdir -p "$d/src"
  printf '# %s\n\n%s\n' "$p" "$RESUME_README" > "$d/README.md"
  printf '{\n  "name": "%s",\n  "version": "1.0.0"\n}\n' "$p" > "$d/package.json"
  if [ "$LANGUE" = fr ]; then
    printf 'export function bonjour(nom) {\n  return `Bonjour ${nom}`;\n}\n' > "$d/src/index.js"
  else
    printf 'export function hello(name) {\n  return `Hello ${name}`;\n}\n' > "$d/src/index.js"
  fi
  git -C "$d" init -q -b main
  git -C "$d" config user.email demo@exemple.test
  git -C "$d" config user.name Demo
  git -C "$d" add -A && git -C "$d" commit -q -m "$COMMIT_1"
  printf 'export const version = "1.1.0";\n' > "$d/src/version.js"
  git -C "$d" add -A && git -C "$d" commit -q -m "$COMMIT_2"
done

# Une modification non validee, pour que l'onglet Git ait un diff a montrer.
if [ "$LANGUE" = fr ]; then
cat > "$TRAVAIL/home/projets/$VITRINE/src/$MODULE.js" <<'JS'
export function totalDuPanier(articles) {
  return articles.reduce((somme, a) => somme + a.prix * a.quantite, 0);
}

export function fraisDePort(total) {
  if (total >= 5000) return 0;
  return total > 2000 ? 490 : 690;
}
JS
cat > "$TRAVAIL/home/projets/$VITRINE/src/index.js" <<'JS'
import { totalDuPanier, fraisDePort } from "./panier.js";

export function bonjour(nom) {
  return `Bonjour ${nom}`;
}

export function resume(articles) {
  const total = totalDuPanier(articles);
  return { total, port: fraisDePort(total) };
}
JS
else
cat > "$TRAVAIL/home/projets/$VITRINE/src/$MODULE.js" <<'JS'
export function cartTotal(items) {
  return items.reduce((sum, i) => sum + i.price * i.quantity, 0);
}

export function shippingFee(total) {
  if (total >= 5000) return 0;
  return total > 2000 ? 490 : 690;
}
JS
cat > "$TRAVAIL/home/projets/$VITRINE/src/index.js" <<'JS'
import { cartTotal, shippingFee } from "./cart.js";

export function hello(name) {
  return `Hello ${name}`;
}

export function summary(items) {
  const total = cartTotal(items);
  return { total, shipping: shippingFee(total) };
}
JS
fi

# Une invite sobre : sans .zshrc, le shell affiche son assistant de premier lancement.
cat > "$TRAVAIL/home/.zshrc" <<'ZSH'
autoload -Uz colors && colors
PROMPT='%F{blue}%1~%f %F{green}❯%f '
export PAGER=cat
alias ls='ls --color=auto'
ZSH

export DISPLAY="$ECRAN"
# La capture passe par Gdk, qui prefere Wayland des qu'une session en propose un : sur une
# machine de bureau elle mourait alors sur « Expected GdkX11.X11Display », alors que l'ecran
# virtuel etait bien la. Le harnais parle X11, et lui seul.
unset WAYLAND_DISPLAY
export GDK_BACKEND=x11
# Sans ca, gvfsd monte son systeme de fichiers dans le dossier de travail et le lancement
# SUIVANT ne peut plus le nettoyer. Rien ici n'a besoin de lire un dossier distant.
export GIO_USE_VFS=local
export HOME="$TRAVAIL/home"
export XDG_RUNTIME_DIR="$TRAVAIL/run"
export XDG_DATA_HOME="$TRAVAIL/home/.local/share"
export XDG_CONFIG_HOME="$TRAVAIL/home/.config"
export XDG_CACHE_HOME="$TRAVAIL/home/.cache"
export COCKPIT_DB="$TRAVAIL/demo.db"
# Le logiciel la lit au demarrage et bascule avant le premier rendu utile.
export COCKPIT_LANGUE="$LANGUE"

# Le jeton qui designe NOTRE lancement, et lui seul. La machine de developpement fait tourner
# sa propre installation de Cockpit : viser le nom du programme la tuerait aussi.
JETON="harnais-captures-$$"

arreter() { python3 "$OUTILS" arreter "COCKPIT_HARNAIS=$JETON"; }

nettoyer() {
  arreter
  [ -n "${XVFB:-}" ] && kill "$XVFB" 2>/dev/null || true
}
trap nettoyer EXIT

Xvfb "$ECRAN" -screen 0 1680x1050x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
sleep 3

# Un bus DBus a nous : le verrou d'instance unique passe par la, et sans ca le lancement rend
# la main a l'instance deja ouverte en sortant avec un code 0.
lancer() {
  COCKPIT_HARNAIS="$JETON" dbus-run-session -- "$APPIMAGE" >"$TRAVAIL/app.log" 2>&1 &
  sleep 22
}
clic()   { python3 "$OUTILS" cliquer "$1" "$2"; sleep "${3:-3}"; }
# Un onglet se designe par son NOM et non par une abscisse : son libelle est traduit, et un clic
# cale sur le francais tombait sur l'onglet voisin en anglais. Le harnais LIT la barre.
onglet() { python3 "$OUTILS" onglet "$1"; sleep "${2:-4}"; }
frappe() { python3 "$OUTILS" taper "$1"; sleep "${2:-2}"; }

# Deux captures identiques veulent dire qu'un clic n'a pas porte, ou que l'image ne vient pas de
# la fenetre pilotee. Le harnais a rendu QUATRE fois la meme image sans une ligne d'erreur : il
# refuse maintenant de finir sur un jeu de captures qui se repetent.
EMPREINTES=""
prise() {
  python3 "$OUTILS" capturer "$SORTIE/$1.png"
  empreinte=$(md5sum "$SORTIE/$1.png" | cut -c1-32)
  case "$EMPREINTES" in
    *"$empreinte"*)
      echo "── ECHEC : $1.png est identique a une capture precedente." >&2
      echo "   Un clic n'a pas porte, ou les coordonnees ne correspondent plus a l'interface." >&2
      exit 1
      ;;
  esac
  EMPREINTES="$EMPREINTES $empreinte"
}

echo "── premier lancement : creation du schema par le logiciel"
unset DBUS_SESSION_BUS_ADDRESS
lancer
arreter

echo "── remplissage de la base de demonstration"
python3 "$OUTILS" preparer "$COCKPIT_DB" "$TRAVAIL/home/projets" "$LANGUE"

echo "── relance et captures"
lancer
prise taches
clic 104 168          # le projet, dans la barre laterale
onglet terminal
clic 790 512 10       # « Ouvrir un terminal »
clic 800 400 1        # le focus dans le terminal
frappe "git status --short --branch"
frappe "git log --oneline -3"
frappe "ls src" 3
prise terminal
onglet git 5
clic 392 304 4        # le fichier modifie, pour afficher son diff
prise git
onglet fichiers 5
clic 360 273 2        # deplier le dossier `src`
clic 391 296 4        # le premier fichier dedans, pour le montrer colore
prise fichiers

echo "── fait, dans $SORTIE (langue : $LANGUE)"
