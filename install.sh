#!/bin/bash
set -euo pipefail

REPO="ris-xeryx/NEPIX_TUI"
VERSION="${NEPIX_VERSION:-latest}"
BIN="nepix"

case "$(uname -s)" in
    Linux)  OS="linux"   ;;
    Darwin) OS="macos"   ;;
    *)      echo "OS no soportado: $(uname -s)" >&2; exit 1 ;;
esac

case "$(uname -m)" in
    x86_64|amd64) ARCH="x86_64" ;;
    aarch64|arm64) ARCH="aarch64" ;;
    *) echo "Arquitectura no soportada: $(uname -m)" >&2; exit 1 ;;
esac

if [ "$VERSION" = "latest" ]; then
    URL="https://github.com/${REPO}/releases/latest/download/${BIN}-${OS}-${ARCH}"
else
    URL="https://github.com/${REPO}/releases/download/${VERSION}/${BIN}-${OS}-${ARCH}"
fi

INSTALL_DIR="${HOME}/.local/bin"
mkdir -p "$INSTALL_DIR"

echo "Descargando nepix ${VERSION}... (${OS}-${ARCH})"
curl -fsSL "$URL" -o "${INSTALL_DIR}/${BIN}"
chmod +x "${INSTALL_DIR}/${BIN}"

echo "Instalado en ${INSTALL_DIR}/${BIN}"

if ! echo "$PATH" | grep -q "$INSTALL_DIR"; then
    echo "Agrega a tu PATH: export PATH=\"\$HOME/.local/bin:\$PATH\""
fi
