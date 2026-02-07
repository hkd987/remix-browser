#!/usr/bin/env bash
set -euo pipefail

# ---------------------------------------------------------------------------
# Test suite for ensure_symlink_on_path() from scripts/ensure-built.sh
# ---------------------------------------------------------------------------

TESTS_RUN=0
TESTS_PASSED=0
TESTS_FAILED=0

pass() {
    TESTS_PASSED=$((TESTS_PASSED + 1))
    TESTS_RUN=$((TESTS_RUN + 1))
    echo "  PASS: $1" >&2
}

fail() {
    TESTS_FAILED=$((TESTS_FAILED + 1))
    TESTS_RUN=$((TESTS_RUN + 1))
    echo "  FAIL: $1" >&2
}

# ---------------------------------------------------------------------------
# Inline the log() and ensure_symlink_on_path() functions under test.
# We copy them verbatim so the tests exercise the real implementation.
# ---------------------------------------------------------------------------
log() { echo "$@" >&2; }

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

# ===========================================================================
# Test 1: Creates symlink when none exists
# ===========================================================================
test_creates_symlink() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local HOME="$tmpdir"
    local fake_binary="$tmpdir/fake-remix-browser"
    echo '#!/bin/sh' > "$fake_binary"
    chmod +x "$fake_binary"

    # Put link_dir on PATH so the PATH warning branch is not exercised
    local PATH="$tmpdir/.local/bin:$PATH"

    ensure_symlink_on_path "$fake_binary"

    local link="$tmpdir/.local/bin/remix-browser"
    if [ -L "$link" ] && [ "$(readlink "$link")" = "$fake_binary" ]; then
        pass "creates symlink when none exists"
    else
        fail "creates symlink when none exists"
    fi
}

# ===========================================================================
# Test 2: Idempotent — no-op when symlink already correct
# ===========================================================================
test_idempotent() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local HOME="$tmpdir"
    local fake_binary="$tmpdir/fake-remix-browser"
    echo '#!/bin/sh' > "$fake_binary"
    chmod +x "$fake_binary"

    mkdir -p "$tmpdir/.local/bin"
    ln -s "$fake_binary" "$tmpdir/.local/bin/remix-browser"

    local PATH="$tmpdir/.local/bin:$PATH"

    # Capture inode before
    local inode_before
    inode_before="$(stat -f '%i' "$tmpdir/.local/bin/remix-browser" 2>/dev/null \
                   || stat -c '%i' "$tmpdir/.local/bin/remix-browser" 2>/dev/null)"

    ensure_symlink_on_path "$fake_binary"

    # Verify symlink unchanged
    local inode_after
    inode_after="$(stat -f '%i' "$tmpdir/.local/bin/remix-browser" 2>/dev/null \
                  || stat -c '%i' "$tmpdir/.local/bin/remix-browser" 2>/dev/null)"

    if [ -L "$tmpdir/.local/bin/remix-browser" ] \
       && [ "$(readlink "$tmpdir/.local/bin/remix-browser")" = "$fake_binary" ] \
       && [ "$inode_before" = "$inode_after" ]; then
        pass "idempotent when symlink already correct"
    else
        fail "idempotent when symlink already correct"
    fi
}

# ===========================================================================
# Test 3: Updates stale symlink pointing to wrong target
# ===========================================================================
test_updates_stale_symlink() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local HOME="$tmpdir"
    local fake_binary="$tmpdir/fake-remix-browser"
    echo '#!/bin/sh' > "$fake_binary"
    chmod +x "$fake_binary"

    # Create a stale symlink pointing elsewhere
    mkdir -p "$tmpdir/.local/bin"
    ln -s "/nonexistent/old-binary" "$tmpdir/.local/bin/remix-browser"

    local PATH="$tmpdir/.local/bin:$PATH"

    ensure_symlink_on_path "$fake_binary"

    if [ -L "$tmpdir/.local/bin/remix-browser" ] \
       && [ "$(readlink "$tmpdir/.local/bin/remix-browser")" = "$fake_binary" ]; then
        pass "updates stale symlink pointing to wrong target"
    else
        fail "updates stale symlink pointing to wrong target"
    fi
}

# ===========================================================================
# Test 4: Does not overwrite a real file (standalone install)
# ===========================================================================
test_no_overwrite_real_file() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local HOME="$tmpdir"
    local fake_binary="$tmpdir/fake-remix-browser"
    echo '#!/bin/sh' > "$fake_binary"
    chmod +x "$fake_binary"

    # Place a real file (not a symlink) at the destination
    mkdir -p "$tmpdir/.local/bin"
    echo '#!/bin/sh' > "$tmpdir/.local/bin/remix-browser"
    chmod +x "$tmpdir/.local/bin/remix-browser"

    local PATH="$tmpdir/.local/bin:$PATH"

    ensure_symlink_on_path "$fake_binary"

    # Verify the real file is still there and is NOT a symlink
    if [ -f "$tmpdir/.local/bin/remix-browser" ] && [ ! -L "$tmpdir/.local/bin/remix-browser" ]; then
        pass "does not overwrite a real file (standalone install)"
    else
        fail "does not overwrite a real file (standalone install)"
    fi
}

# ===========================================================================
# Test 5: PATH warning emitted when link directory not on PATH
# ===========================================================================
test_path_warning() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'rm -rf "$tmpdir"' RETURN

    local HOME="$tmpdir"
    local fake_binary="$tmpdir/fake-remix-browser"
    echo '#!/bin/sh' > "$fake_binary"
    chmod +x "$fake_binary"

    # Strip any .local/bin from PATH so the warning fires
    local PATH="/usr/bin:/bin"

    local stderr_output
    stderr_output="$(ensure_symlink_on_path "$fake_binary" 2>&1 1>/dev/null)"

    if echo "$stderr_output" | grep -q "not in your PATH"; then
        pass "PATH warning emitted when link directory not on PATH"
    else
        fail "PATH warning emitted when link directory not on PATH (got: $stderr_output)"
    fi
}

# ===========================================================================
# Test 6: Handles gracefully when no writable directory available
# ===========================================================================
test_no_writable_dir() {
    local tmpdir
    tmpdir="$(mktemp -d)"
    trap 'chmod -R u+rwx "$tmpdir"; rm -rf "$tmpdir"' RETURN

    # Point HOME to an unwritable directory so mkdir -p fails
    local unwritable="$tmpdir/noaccess"
    mkdir -p "$unwritable"
    chmod 000 "$unwritable"

    local HOME="$unwritable"
    local fake_binary="$tmpdir/fake-remix-browser"
    echo '#!/bin/sh' > "$fake_binary"
    chmod +x "$fake_binary"

    # Also ensure /usr/local/bin is not writable (it typically isn't for normal users)
    local PATH="/usr/bin:/bin"

    # The function should return 0 without error
    if ensure_symlink_on_path "$fake_binary" 2>/dev/null; then
        pass "handles gracefully when no writable directory available"
    else
        fail "handles gracefully when no writable directory available"
    fi
}

# ===========================================================================
# Run all tests
# ===========================================================================
echo "Running ensure_symlink_on_path() tests..." >&2
echo "" >&2

test_creates_symlink
test_idempotent
test_updates_stale_symlink
test_no_overwrite_real_file
test_path_warning
test_no_writable_dir

echo "" >&2
echo "-------------------------------------" >&2
echo "Results: $TESTS_PASSED/$TESTS_RUN passed, $TESTS_FAILED failed" >&2
echo "-------------------------------------" >&2

if [ "$TESTS_FAILED" -gt 0 ]; then
    exit 1
fi

exit 0
