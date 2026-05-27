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

echo "=== E2E first-run test ==="
echo "Temp HOME: $TMP_HOME"

# Run gh --version (allowed by default, should trigger first download)
echo ""
echo "Running: $GH_BIN --version"
"$GH_BIN" --version || true

echo ""
echo "=== Checking artifacts ==="

SYMLINK="$XDG_DATA_HOME/gh-guard/bin/gh-real"
if [[ -L "$SYMLINK" ]]; then
    TARGET=$(readlink "$SYMLINK")
    echo "✅ symlink exists: $SYMLINK -> $TARGET"
    if [[ -f "$TARGET" ]]; then
        echo "✅ symlink target exists and is a file"
    else
        echo "❌ symlink target missing: $TARGET"
        exit 1
    fi
else
    echo "❌ symlink missing: $SYMLINK"
    exit 1
fi

METADATA=$(dirname "$TARGET")/metadata.json
if [[ -f "$METADATA" ]]; then
    echo "✅ metadata exists: $METADATA"
    cat "$METADATA"
else
    echo "❌ metadata missing"
    exit 1
fi

STATE="$XDG_DATA_HOME/gh-guard/state.json"
if [[ -f "$STATE" ]]; then
    echo ""
    echo "✅ state exists: $STATE"
    cat "$STATE"
else
    echo "❌ state missing"
    exit 1
fi

LOG_DIR="$XDG_DATA_HOME/gh-guard/logs"
if [[ -d "$LOG_DIR" ]]; then
    LOG_COUNT=$(find "$LOG_DIR" -name '*.jsonl' | wc -l)
    echo ""
    echo "✅ log directory exists with $LOG_COUNT jsonl file(s)"
    for f in "$LOG_DIR"/*.jsonl; do
        echo "--- $f ---"
        cat "$f"
    done
else
    echo "❌ log directory missing"
    exit 1
fi

echo ""
echo "=== All checks passed ==="
