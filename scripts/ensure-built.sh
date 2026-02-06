#!/usr/bin/env bash
set -euo pipefail

# All logging goes to stderr — stdout is reserved for MCP transport
log() { echo "$@" >&2; }

# Determine project directory
if [ -n "${CLAUDE_PLUGIN_ROOT:-}" ]; then
    PROJECT_DIR="$CLAUDE_PLUGIN_ROOT"
else
    SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
    PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
fi

BINARY="$PROJECT_DIR/bin/remix-browser"

# If binary already exists, run it
if [ -f "$BINARY" ]; then
    exec "$BINARY" "$@"
fi

REPO="hkd987/remix-browser"

# Detect OS
OS="$(uname -s)"
case "$OS" in
    Darwin)  OS_TAG="apple-darwin" ;;
    Linux)   OS_TAG="unknown-linux-gnu" ;;
    MINGW*|MSYS*|CYGWIN*) OS_TAG="pc-windows-msvc" ;;
    *)
        log "Unsupported OS: $OS"
        exit 1
        ;;
esac

# Detect architecture
ARCH="$(uname -m)"
case "$ARCH" in
    arm64|aarch64) ARCH_TAG="aarch64" ;;
    x86_64)        ARCH_TAG="x86_64" ;;
    *)
        log "Unsupported architecture: $ARCH"
        exit 1
        ;;
esac

TARGET="${ARCH_TAG}-${OS_TAG}"

# Set archive extension
if [ "$OS_TAG" = "pc-windows-msvc" ]; then
    ARCHIVE="remix-browser-${TARGET}.zip"
else
    ARCHIVE="remix-browser-${TARGET}.tar.gz"
fi

# Try to download from GitHub Releases
download_binary() {
    log "Fetching latest release version..."
    VERSION="$(curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
        | grep '"tag_name"' | head -1 | sed 's/.*"tag_name": *"//;s/".*//')" || return 1

    if [ -z "$VERSION" ]; then
        log "Could not determine latest version."
        return 1
    fi

    DOWNLOAD_URL="https://github.com/${REPO}/releases/download/${VERSION}/${ARCHIVE}"
    log "Downloading remix-browser ${VERSION} for ${TARGET}..."

    mkdir -p "$PROJECT_DIR/bin"
    TMPDIR_DL="$(mktemp -d)"
    trap 'rm -rf "$TMPDIR_DL"' EXIT

    if ! curl -fsSL -o "$TMPDIR_DL/$ARCHIVE" "$DOWNLOAD_URL"; then
        log "Download failed: $DOWNLOAD_URL"
        return 1
    fi

    # Extract
    if [ "$OS_TAG" = "pc-windows-msvc" ]; then
        unzip -o "$TMPDIR_DL/$ARCHIVE" -d "$PROJECT_DIR/bin" >&2
    else
        tar -xzf "$TMPDIR_DL/$ARCHIVE" -C "$PROJECT_DIR/bin" >&2
    fi

    chmod +x "$BINARY"
    log "remix-browser ${VERSION} installed to $BINARY"
    return 0
}

# Try to build from source
build_from_source() {
    if ! command -v cargo >/dev/null 2>&1; then
        return 1
    fi

    log "Building remix-browser from source (this may take a minute)..."
    cd "$PROJECT_DIR"
    cargo build --release >&2

    mkdir -p "$PROJECT_DIR/bin"
    cp "$PROJECT_DIR/target/release/remix-browser" "$BINARY"
    chmod +x "$BINARY"
    log "remix-browser built and installed to $BINARY"
    return 0
}

# Try download first, then build from source
if download_binary; then
    exec "$BINARY" "$@"
fi

log "Download failed, attempting to build from source..."

if build_from_source; then
    exec "$BINARY" "$@"
fi

log ""
log "ERROR: Could not install remix-browser."
log ""
log "To install manually, either:"
log "  1. Download a release from https://github.com/${REPO}/releases"
log "     and place the binary at: $BINARY"
log "  2. Install Rust (https://rustup.rs) and run:"
log "     cd $PROJECT_DIR && cargo build --release"
log "     cp target/release/remix-browser bin/"
exit 1
