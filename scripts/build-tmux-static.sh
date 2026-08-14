#!/bin/sh
# Construit un tmux STATIQUE (musl) destine a etre embarque dans l'AppImage.
#
# Pourquoi : tmux est le socle des terminaux persistants de Cockpit, et demander a
# l'utilisateur de l'installer lui-meme est inacceptable (premier retour utilisateur,
# 2026-08-14). Le binaire est lie statiquement contre musl : aucune dependance, il
# tourne sur n'importe quelle distro.
#
# Pourquoi Alpine : musl y est natif et libevent/ncurses existent en version statique
# dans apk — rien a compiler d'autre que tmux lui-meme, contrairement a une chaine
# musl-cross sur Ubuntu ou il faudrait builder les trois.
#
# Usage : scripts/build-tmux-static.sh <chemin/de/sortie>
# Necessite Docker. Utilise par la CI (release.yml) et testable en local a l'identique.

set -eu

OUT="${1:?usage: build-tmux-static.sh <fichier de sortie>}"
TMUX_VERSION="3.5a"
TMUX_SHA256="16216bd0877170dfcc64157085ba9013610b12b082548c7c9542cc0103198951"
ALPINE="alpine:3.20"

OUT_DIR=$(mkdir -p "$(dirname "$OUT")" && cd "$(dirname "$OUT")" && pwd)
OUT_NAME=$(basename "$OUT")

docker run --rm -v "$OUT_DIR:/out" "$ALPINE" sh -eu -c "
  apk add --no-cache build-base bison libevent-dev libevent-static ncurses-dev ncurses-static file > /dev/null
  wget -q https://github.com/tmux/tmux/releases/download/$TMUX_VERSION/tmux-$TMUX_VERSION.tar.gz
  echo '$TMUX_SHA256  tmux-$TMUX_VERSION.tar.gz' | sha256sum -c -
  tar xzf tmux-$TMUX_VERSION.tar.gz
  cd tmux-$TMUX_VERSION
  ./configure --enable-static > /dev/null
  make -j\$(nproc) > /dev/null
  strip tmux
  # Preuve que le binaire est bien autonome (le message de ldd varie selon la libc,
  # celui de file est stable)
  file ./tmux | grep -q 'statically linked' || { echo 'BINAIRE NON STATIQUE'; file ./tmux; exit 1; }
  ./tmux -V
  cp tmux /out/$OUT_NAME
"

echo "OK: $OUT ($(du -h "$OUT" | cut -f1))"
