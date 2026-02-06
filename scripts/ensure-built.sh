#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BINARY="$PROJECT_DIR/target/release/remix-browser"

if [ ! -f "$BINARY" ]; then
    echo "Building remix-browser..." >&2
    cd "$PROJECT_DIR"
    cargo build --release >&2
fi

exec "$BINARY" "$@"
