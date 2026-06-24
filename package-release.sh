#!/bin/bash
# Script para empaquetar binarios para GitHub Releases.
# Ejecutar en cada plataforma o usar GitHub Actions.
set -euo pipefail

BIN="nepix"
TARGET_DIR="target/release"
RELEASE_DIR="release"

mkdir -p "$RELEASE_DIR"

case "$(uname -s)" in
    Linux)  OS="linux"   ; EXT=""   ;;
    Darwin) OS="macos"   ; EXT=""   ;;
    MINGW*|MSYS*) OS="windows" ; EXT=".exe" ;;
    *) echo "Plataforma no soportada: $(uname -s)" >&2; exit 1 ;;
esac

ARCH=$(uname -m)
case "$ARCH" in
    x86_64) ARCH="x86_64" ;;
    aarch64) ARCH="aarch64" ;;
    *) ARCH="$ARCH" ;;
esac

SRC="${TARGET_DIR}/${BIN}${EXT}"
DST="${RELEASE_DIR}/${BIN}-${OS}-${ARCH}${EXT}"

if [ ! -f "$SRC" ]; then
    echo "No encontrado: $SRC. Compila primero con: cargo build --release" >&2
    exit 1
fi

cp "$SRC" "$DST"
echo "Creado: $DST"

# También copia install.sh para subirlo al release
if [ -f install.sh ]; then
    cp install.sh "$RELEASE_DIR/install.sh"
    echo "Creado: $RELEASE_DIR/install.sh"
fi
