#!/usr/bin/env bash
#
# Render all .seqdiag files in this directory to SVG (and optionally PNG).
#
# Usage:
#   ./render_all.sh          # SVG only
#   ./render_all.sh --png    # SVG + PNG
#

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"
OUTPUT_DIR="$SCRIPT_DIR/output"
BINARY="$REPO_ROOT/target/release/seqdiagsvg"

ALSO_PNG=false
if [[ "${1:-}" == "--png" ]]; then
    ALSO_PNG=true
fi

# Build release binary
echo "Building seqdiagsvg (release)..."
cargo build --release --manifest-path "$REPO_ROOT/Cargo.toml" 2>&1 | tail -1

mkdir -p "$OUTPUT_DIR"

count=0
errors=0

for src in "$SCRIPT_DIR"/*.seqdiag; do
    [ -f "$src" ] || continue
    name="$(basename "$src" .seqdiag)"

    # SVG
    if "$BINARY" "$src" "$OUTPUT_DIR/$name.svg" 2>&1; then
        count=$((count + 1))
    else
        echo "  FAILED: $name.svg"
        errors=$((errors + 1))
    fi

    # PNG (optional)
    if $ALSO_PNG; then
        if "$BINARY" "$src" "$OUTPUT_DIR/$name.png" 2>&1; then
            : # already counted above
        else
            echo "  FAILED: $name.png"
            errors=$((errors + 1))
        fi
    fi
done

echo ""
echo "Done. $count diagrams rendered to $OUTPUT_DIR"
if [ $errors -gt 0 ]; then
    echo "  ($errors failures)"
    exit 1
fi
