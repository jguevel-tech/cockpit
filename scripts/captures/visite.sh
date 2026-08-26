#!/bin/bash
# La visite guidee du logiciel, en vingt secondes : un ecran par fonctionnalite.
#
#   scripts/captures/visite.sh <chemin/vers/le/binaire> <dossier de sortie> [fr|en]
#
# C'est l'image qui ouvre le README et le site. Ce qu'elle doit faire tenir en vingt secondes :
# ce que le logiciel SAIT FAIRE, pas comment il le fait. Un ecran, une phrase, on avance.
#
# Meme socle que `prendre.sh` : ecran virtuel, base de demonstration, arret cible par une
# variable d'environnement propre a ce lancement.
set -euo pipefail

# Pas d'apostrophe dans un message de `${1:?...}` : bash y parse les quotes.
BINAIRE="${1:?chemin du binaire attendu}"
SORTIE="${2:-visite}"
LANGUE="${3:-en}"
case "$LANGUE" in fr|en) ;; *) echo "langue inconnue : $LANGUE" >&2; exit 1 ;; esac

TRAVAIL=/tmp/cockpit-visite
ECRAN=:94
ICI="$(cd "$(dirname "$0")" && pwd)"
OUTILS="$ICI/outils.py"
IMAGES="$TRAVAIL/images"

mkdir -p "$SORTIE"
for montage in "$TRAVAIL"/run/gvfs "$TRAVAIL"/run/doc; do
  [ -d "$montage" ] && { fusermount -u "$montage" 2>/dev/null || fusermount -uz "$montage" 2>/dev/null; } || true
done
rm -rf "$TRAVAIL"
mkdir -p "$TRAVAIL/run" "$TRAVAIL/home/projets" "$IMAGES"
chmod 700 "$TRAVAIL/run"

# ── Les projets de demonstration : les memes que les captures fixes, pour que l'ensemble
#    raconte la meme histoire. Tout suit la langue.
if [ "$LANGUE" = fr ]; then
  PROJETS="boutique-vinyles api-facturation site-vitrine"
  VITRINE=boutique-vinyles
  COMMIT_1="Poser les bases du projet"
  COMMIT_2="Ajouter le numero de version"
else
  PROJETS="vinyl-store billing-api landing-site"
  VITRINE=vinyl-store
  COMMIT_1="Set up the project"
  COMMIT_2="Add the version number"
fi

for p in $PROJETS; do
  d="$TRAVAIL/home/projets/$p"
  mkdir -p "$d/src"
  printf '# %s\n' "$p" > "$d/README.md"
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

# Une modification non validee : l'onglet Git a besoin d'un diff a montrer.
if [ "$LANGUE" = fr ]; then
cat > "$TRAVAIL/home/projets/$VITRINE/src/panier.js" <<'JS'
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

export function resume(articles) {
  const total = totalDuPanier(articles);
  return { total, port: fraisDePort(total) };
}
JS
else
cat > "$TRAVAIL/home/projets/$VITRINE/src/cart.js" <<'JS'
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

export function summary(items) {
  const total = cartTotal(items);
  return { total, shipping: shippingFee(total) };
}
JS
fi

cat > "$TRAVAIL/home/.zshrc" <<'ZSH'
autoload -Uz colors && colors
PROMPT='%F{blue}%1~%f %F{green}❯%f '
export PAGER=cat
alias ls='ls --color=auto'
ZSH

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

JETON="visite-$$"
arreter() { python3 "$OUTILS" arreter "COCKPIT_HARNAIS=$JETON"; }
nettoyer() { arreter; [ -n "${XVFB:-}" ] && kill "$XVFB" 2>/dev/null || true; }
trap nettoyer EXIT

Xvfb "$ECRAN" -screen 0 1680x1050x24 -nolisten tcp >/dev/null 2>&1 &
XVFB=$!
sleep 3

lancer() { COCKPIT_HARNAIS="$JETON" dbus-run-session -- "$BINAIRE" >"$TRAVAIL/app.log" 2>&1 & sleep 22; }
clic()   { python3 "$OUTILS" cliquer "$1" "$2"; sleep "${3:-2}"; }
frappe() { python3 "$OUTILS" taper "$1"; sleep "${2:-2}"; }
onglet() { python3 "$OUTILS" onglet "$1"; sleep "${2:-3}"; }

NUMERO=0
# Deux images identiques dans une visite veulent dire qu'un clic n'a pas porte : le harnais des
# captures fixes refuse de finir dessus, et il n'y a pas de raison d'etre plus tolerant ici.
EMPREINTES=""
image() {
  NUMERO=$((NUMERO + 1))
  cible="$(printf '%s/%02d-%s.png' "$IMAGES" "$NUMERO" "$1")"
  python3 "$OUTILS" capturer "$cible" >/dev/null
  empreinte=$(md5sum "$cible" | cut -c1-32)
  case "$EMPREINTES" in
    *"$empreinte"*) echo "── ECHEC : $1 est identique a un ecran precedent (un clic n'a pas porte)" >&2; exit 1 ;;
  esac
  EMPREINTES="$EMPREINTES $empreinte"
}

echo "── premier lancement : creation du schema"
unset DBUS_SESSION_BUS_ADDRESS
lancer
arreter
sleep 2
python3 "$OUTILS" preparer "$COCKPIT_DB" "$TRAVAIL/home/projets" "$LANGUE"

echo "── la visite"
lancer
image taches            # le tableau de bord : les taches de tous les projets

clic 398 187 3          # « Monitoring », dans la colonne du tableau de bord
# Sans ce clic, l'ecran affiche « cliquez sur Instantane ou Direct » : la legende promettrait
# des chiffres au-dessus d'une carte vide.
clic 1140 111 5         # « Instantane » : les mesures apparaissent
image monitoring

clic 104 168 3          # le projet, dans la barre laterale
# La colonne Notes est un arbre : sans clic sur une note, elle affiche « selectionnez un
# fichier », et la legende parlerait de notes qu'on ne voit pas.
clic 360 232 3          # la premiere note
image workspace         # notes et taches du projet, cote a cote

onglet terminal
clic 790 512 10         # « ouvrir un terminal »
clic 800 400 1          # le focus dedans
frappe "git status --short --branch"
frappe "git log --oneline -3" 3
image terminal

onglet git 4
clic 392 304 4          # le fichier modifie, pour afficher son diff
image git

onglet fichiers 4
clic 360 273 2          # deplier `src`
clic 391 296 4          # le premier fichier dedans
image fichiers

clic 1155 28 3          # la roue crantee : les parametres
clic 372 269 4          # l'entree « IA »
image ia                # n'importe quel agent, au choix

echo "── assemblage"
python3 "$ICI/animer.py" "$IMAGES" "$SORTIE/visite.gif" "$LANGUE"
