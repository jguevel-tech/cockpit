#!/usr/bin/env bash
# Build l'interface et le backend puis lance la coquille Electron en dev.
set -e
exec >/tmp/cockpit-dev.log 2>&1
cd "$(dirname "$0")/.."
npm run build
cargo +1.90 build --manifest-path backend/Cargo.toml
cd coquille && exec npm run dev
