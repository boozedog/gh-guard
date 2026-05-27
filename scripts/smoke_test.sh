#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
GH_BIN="$PROJECT_DIR/target/debug/gh"

TMP_HOME=$(mktemp -d)
trap 'rm -rf "$TMP_HOME"' EXIT

export HOME="$TMP_HOME"
export XDG_CONFIG_HOME="$TMP_HOME/.config"
export XDG_DATA_HOME="$TMP_HOME/.local/share"
export XDG_CACHE_HOME="$TMP_HOME/.cache"

echo "=== Smoke test ==="
echo ""

echo "1. gh --version (allowed by default)"
"$GH_BIN" --version
echo ""

echo "2. gh auth status (gated — should prompt for sudo, will fail in non-interactive test)"
"$GH_BIN" auth status || echo "(expected failure: sudo not available / non-interactive)"
echo ""

echo "3. gh api user (gated — same expected failure)"
"$GH_BIN" api user || echo "(expected failure: sudo not available / non-interactive)"
echo ""

echo "4. gh guard status"
"$GH_BIN" guard status
echo ""

echo "=== Smoke test complete ==="
