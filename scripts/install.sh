#!/usr/bin/env sh
#
# Installeur de Cockpit.
#
#   curl -fsSL https://raw.githubusercontent.com/jguevel-tech/cockpit/main/scripts/install.sh | sh
#
# Telecharge la derniere AppImage publiee, l'installe dans ~/.local/bin et cree une entree
# de menu. Aucun privilege root requis. Une fois installe, Cockpit se met a jour tout seul :
# ce script ne sert qu'a la premiere installation.
#
# POSIX sh volontairement (pas de bashisme) : le script doit tourner sous dash, le /bin/sh
# par defaut sur Debian et Ubuntu.

set -eu

REPO="jguevel-tech/cockpit"
BIN_DIR="${HOME}/.local/bin"
APP_DIR="${HOME}/.local/share/cockpit"
DESKTOP_DIR="${HOME}/.local/share/applications"
ICON_DIR="${HOME}/.local/share/icons/hicolor/128x128/apps"

# Couleurs seulement si la sortie est un terminal (sinon on pollue les logs/pipes).
if [ -t 1 ]; then
  BOLD=$(printf '\033[1m'); DIM=$(printf '\033[2m')
  RED=$(printf '\033[31m'); GREEN=$(printf '\033[32m'); RESET=$(printf '\033[0m')
else
  BOLD=''; DIM=''; RED=''; GREEN=''; RESET=''
fi

info() { printf '%s\n' "$1"; }
step() { printf '%s==>%s %s\n' "$BOLD" "$RESET" "$1"; }
die()  { printf '%serreur :%s %s\n' "$RED" "$RESET" "$1" >&2; exit 1; }

# --- Verifications prealables ---

[ "$(uname -s)" = "Linux" ] || die "Cockpit ne cible que Linux pour l'instant (detecte : $(uname -s))."

case "$(uname -m)" in
  x86_64|amd64) ;;
  *) die "Architecture non supportee : $(uname -m). Seul x86_64 est publie." ;;
esac

if command -v curl >/dev/null 2>&1; then
  DL="curl -fsSL"
  DL_OUT="curl -fsSL -o"
elif command -v wget >/dev/null 2>&1; then
  DL="wget -qO-"
  DL_OUT="wget -qO"
else
  die "curl ou wget est requis."
fi

# tmux et git ne sont pas necessaires a l'installation, mais Cockpit s'en sert au quotidien.
MISSING=""
for cmd in tmux git; do
  command -v "$cmd" >/dev/null 2>&1 || MISSING="$MISSING $cmd"
done

# --- Derniere version publiee ---

step "Recherche de la derniere version"
TAG=$($DL "https://api.github.com/repos/${REPO}/releases/latest" \
  | sed -n 's/.*"tag_name" *: *"\([^"]*\)".*/\1/p' | head -n 1)
[ -n "$TAG" ] || die "Impossible de determiner la derniere version (GitHub injoignable ou aucune release publiee)."

VERSION="${TAG#v}"
ASSET="Cockpit_${VERSION}_amd64.AppImage"
URL="https://github.com/${REPO}/releases/download/${TAG}/${ASSET}"
info "  version ${GREEN}${VERSION}${RESET}"

# --- Telechargement ---

step "Telechargement de l'AppImage"
mkdir -p "$APP_DIR" "$BIN_DIR" "$DESKTOP_DIR" "$ICON_DIR"
TMP="${APP_DIR}/.${ASSET}.part"
$DL_OUT "$TMP" "$URL" || die "Telechargement echoue : $URL"
[ -s "$TMP" ] || die "Fichier telecharge vide."

chmod +x "$TMP"
mv -f "$TMP" "${APP_DIR}/Cockpit.AppImage"

# Lien stable : l'AppImage se remplace elle-meme lors des mises a jour, le lien ne bouge pas.
ln -sf "${APP_DIR}/Cockpit.AppImage" "${BIN_DIR}/cockpit"

# --- Integration au bureau ---

step "Integration au menu des applications"
# L'icone est extraite de l'AppImage elle-meme : pas de fichier a heberger a cote.
( cd "$APP_DIR" && "${APP_DIR}/Cockpit.AppImage" --appimage-extract 'usr/share/icons/hicolor/128x128/apps/*.png' >/dev/null 2>&1 ) || true
if [ -d "${APP_DIR}/squashfs-root" ]; then
  find "${APP_DIR}/squashfs-root" -name '*.png' -exec cp -f {} "${ICON_DIR}/cockpit.png" \; 2>/dev/null || true
  rm -rf "${APP_DIR}/squashfs-root"
fi

cat > "${DESKTOP_DIR}/cockpit.desktop" <<DESKTOP
[Desktop Entry]
Type=Application
Name=Cockpit
Comment=One place to run all your projects
Exec=${BIN_DIR}/cockpit
Icon=cockpit
Terminal=false
Categories=Development;
DESKTOP

command -v update-desktop-database >/dev/null 2>&1 \
  && update-desktop-database "$DESKTOP_DIR" >/dev/null 2>&1 || true

# --- Bilan ---

printf '\n%sCockpit %s installe.%s\n\n' "$GREEN" "$VERSION" "$RESET"
info "  binaire   ${BIN_DIR}/cockpit"
info "  AppImage  ${APP_DIR}/Cockpit.AppImage"
printf '\n'

case ":${PATH}:" in
  *":${BIN_DIR}:"*) info "Lance ${BOLD}cockpit${RESET} depuis un terminal, ou via le menu des applications." ;;
  *)
    printf '%s%s n%sest pas dans ton PATH. Ajoute cette ligne a ton ~/.profile ou ~/.zshrc :%s\n' \
      "$BOLD" "$BIN_DIR" "'" "$RESET"
    printf '\n    export PATH="$HOME/.local/bin:$PATH"\n\n'
    ;;
esac

if [ -n "$MISSING" ]; then
  printf '%sDependances manquantes pour certaines fonctions :%s%s\n' "$BOLD" "$MISSING" "$RESET"
  printf '%s  tmux -> terminaux persistants   git -> onglet Git%s\n' "$DIM" "$RESET"
  printf '%s  sudo apt install%s%s\n\n' "$DIM" "$MISSING" "$RESET"
fi

info "${DIM}Les mises a jour se font ensuite depuis l'application (cloche dans l'en-tete).${RESET}"
