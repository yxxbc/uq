#!/usr/bin/env bash
set -e

# ==============================================================================
# uq (Unquarantine) One-line Installer
# GitHub: https://github.com/yxxbc/uq
# ==============================================================================

VERSION="v0.1.0"
REPO="yxxbc/uq"

# Check OS
OS="$(uname -s)"
if [ "$OS" != "Darwin" ]; then
    echo "[-] Error: uq only supports macOS (Darwin). Current OS: $OS"
    exit 1
fi

# Detect Architecture
ARCH="$(uname -m)"
case "$ARCH" in
    arm64|aarch64)
        TARGET_ARCH="arm64"
        ;;
    x86_64)
        TARGET_ARCH="x86_64"
        ;;
    *)
        echo "[-] Error: Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

echo "[*] Installing uq ($VERSION) for macOS ($TARGET_ARCH)..."

ARCHIVE_NAME="uq-${VERSION}-macos-${TARGET_ARCH}.tar.gz"
DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE_NAME}"

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

echo "[*] Downloading $DOWNLOAD_URL ..."
curl -fL --progress-bar "$DOWNLOAD_URL" -o "$TMP_DIR/$ARCHIVE_NAME"

tar -xzf "$TMP_DIR/$ARCHIVE_NAME" -C "$TMP_DIR"

# Determine install location without requiring interactive sudo if possible
INSTALL_DIR=""
if [ -w "/usr/local/bin" ]; then
    INSTALL_DIR="/usr/local/bin"
elif [ -d "/opt/homebrew/bin" ] && [ -w "/opt/homebrew/bin" ]; then
    INSTALL_DIR="/opt/homebrew/bin"
else
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

cp "$TMP_DIR/uq" "$INSTALL_DIR/uq"
chmod +x "$INSTALL_DIR/uq"
echo "[+] Binary installed to $INSTALL_DIR/uq"

# Ensure INSTALL_DIR is in PATH for this session
export PATH="$INSTALL_DIR:$PATH"

# Auto install background service
"$INSTALL_DIR/uq" service install

echo ""
echo "[+] uq installed and service activated successfully!"
