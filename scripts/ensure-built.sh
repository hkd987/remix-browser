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

# Create a symlink on PATH so "remix-browser" is available system-wide.
# This function never fails the script — all error paths return 0.
ensure_symlink_on_path() {
    local real_binary="$1"

    # Skip on Windows
    case "$(uname -s)" in
        MINGW*|MSYS*|CYGWIN*) return 0 ;;
    esac

    # Guard against empty HOME
    if [ -z "${HOME:-}" ]; then
        return 0
    fi

    # Pick symlink directory
    local link_dir=""
    if [ -d "$HOME/.local/bin" ] || mkdir -p "$HOME/.local/bin" 2>/dev/null; then
        link_dir="$HOME/.local/bin"
    elif [ -w "/usr/local/bin" ]; then
        link_dir="/usr/local/bin"
    else
        return 0
    fi

    local link_path="$link_dir/remix-browser"

    # If a real file (not a symlink) exists at the destination, don't overwrite
    if [ -f "$link_path" ] && [ ! -L "$link_path" ]; then
        log "Standalone install found at $link_path; skipping symlink."
        return 0
    fi

    # If symlink already exists and points to the correct target, no-op
    if [ -L "$link_path" ]; then
        local current_target
        current_target="$(readlink "$link_path")" || true
        if [ "$current_target" = "$real_binary" ]; then
            return 0
        fi
        # Stale symlink — update it
        log "Updating symlink $link_path -> $real_binary"
        ln -sf "$real_binary" "$link_path" 2>/dev/null || return 0
    else
        # Nothing exists — create symlink
        log "Creating symlink $link_path -> $real_binary"
        ln -s "$real_binary" "$link_path" 2>/dev/null || return 0
    fi

    # Warn if the chosen directory is not on PATH
    case ":${PATH}:" in
        *":${link_dir}:"*) ;;
        *)
            log ""
            log "NOTE: ${link_dir} is not in your PATH."
            log "Add it with: export PATH=\"${link_dir}:\$PATH\""
            ;;
    esac

    return 0
}

# Run the binary, ensuring a symlink is on PATH first.
run_binary() {
    ensure_symlink_on_path "$BINARY"
    exec "$BINARY" "$@"
}

# If binary already exists, run it
if [ -f "$BINARY" ]; then
    run_binary "$@"
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
    run_binary "$@"
fi

log "Download failed, attempting to build from source..."

if build_from_source; then
    run_binary "$@"
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
