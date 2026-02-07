#!/bin/sh
# Standalone installer for remix-browser
# Usage: curl -fsSL https://raw.githubusercontent.com/hkd987/remix-browser/main/scripts/install.sh | sh
set -eu

REPO="hkd987/remix-browser"

log() { echo "$@" >&2; }

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Darwin)  OS_TAG="apple-darwin" ;;
    Linux)   OS_TAG="unknown-linux-gnu" ;;
    MINGW*|MSYS*|CYGWIN*) OS_TAG="pc-windows-msvc" ;;
    *)
        log "Error: Unsupported OS: $OS"
        exit 1
        ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
    arm64|aarch64) ARCH_TAG="aarch64" ;;
    x86_64)        ARCH_TAG="x86_64" ;;
    *)
        log "Error: Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

# macOS: always use aarch64 binary (runs on Intel via Rosetta 2)
if [ "$OS_TAG" = "apple-darwin" ]; then
    ARCH_TAG="aarch64"
fi

TARGET="${ARCH_TAG}-${OS_TAG}"

# Set archive extension
if [ "$OS_TAG" = "pc-windows-msvc" ]; then
    ARCHIVE="remix-browser-${TARGET}.zip"
else
    ARCHIVE="remix-browser-${TARGET}.tar.gz"
fi

# Determine install directory
INSTALL_DIR="/usr/local/bin"
if [ ! -w "$INSTALL_DIR" ] 2>/dev/null; then
    INSTALL_DIR="$HOME/.local/bin"
    mkdir -p "$INSTALL_DIR"
fi

log "Installing remix-browser for ${TARGET}..."

# Fetch latest version
log "Fetching latest release..."
VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
    | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')" || {
    log "Error: Could not fetch latest release from GitHub."
    exit 1
}

if [ -z "$VERSION" ]; then
    log "Error: Could not determine latest version."
    exit 1
fi

DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
log "Downloading remix-browser ${VERSION}..."

# Download to temp directory
TMPDIR_DL="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_DL"' EXIT

if ! curl -fsSL -o "$TMPDIR_DL/$ARCHIVE" "$DOWNLOAD_URL"; then
    log "Error: Download failed: $DOWNLOAD_URL"
    log ""
    log "Please check https://github.com/${REPO}/releases for available downloads."
    exit 1
fi

# Extract to temp directory first
if [ "$OS_TAG" = "pc-windows-msvc" ]; then
    unzip -o "$TMPDIR_DL/$ARCHIVE" -d "$TMPDIR_DL/extracted" >&2
else
    mkdir -p "$TMPDIR_DL/extracted"
    tar -xzf "$TMPDIR_DL/$ARCHIVE" -C "$TMPDIR_DL/extracted" >&2
fi

# Install binary
cp "$TMPDIR_DL/extracted/remix-browser" "$INSTALL_DIR/remix-browser"
chmod +x "$INSTALL_DIR/remix-browser"

INSTALL_PATH="$INSTALL_DIR/remix-browser"

log ""
log "remix-browser ${VERSION} installed successfully!"
log ""
log "Binary location: ${INSTALL_PATH}"
log ""
log "Add to your Claude Code MCP config (~/.claude/mcp.json):"
log ""
log "{"
log "  \"mcpServers\": {"
log "    \"remix-browser\": {"
log "      \"command\": \"${INSTALL_PATH}\""
log "    }"
log "  }"
log "}"

# Check if install dir is in PATH
case ":${PATH}:" in
    *":${INSTALL_DIR}:"*) ;;
    *)
        log ""
        log "NOTE: ${INSTALL_DIR} is not in your PATH."
        log "Add it with: export PATH=\"${INSTALL_DIR}:\$PATH\""
        ;;
esac
