#!/usr/bin/env bash
# Frame-by-frame pixel diff (k3diff) regression: render an example in this
# worktree and compare it byte-for-byte against the same example rendered on a
# reference checkout — the same hard-pixel methodology as tools/k3diff.py
# (mae / maxd / changed-pixel fractions), NOT SSIM.
#
# Usage:
#   ./scripts/compare-k3diff.sh                              # default example
#   ./scripts/compare-k3diff.sh examples/profile-showcase.jsonl
#
# Env:
#   REF_DIR   directory holding the reference render (default: this repo)
#   SKIA_BINARIES_URL  forwarded to cargo (set to file:///tmp/skia-binaries.tar.gz
#                      in sandboxes that can't source-build skia-bindings).
#
# The reference video must already exist at $REF_DIR/$OUT_REL — render it in the
# reference checkout first:
#   cargo run --release --features profile -- examples/<stem>.<ext>
set -euo pipefail

EXAMPLE="${1:-examples/xhs-neo-brutalism.xml}"

MAIN_DIR="$(cd "$(dirname "$0")/.." && pwd)"
REF_DIR="${REF_DIR:-$MAIN_DIR}"

# Derive the output stem from the example path: examples/foo.bar -> out/foo.mp4
STEM="$(basename "$EXAMPLE")"
STEM="${STEM%.*}"
OUT_REL="out/${STEM}.mp4"

REF_VIDEO="$REF_DIR/$OUT_REL"
TEST_VIDEO="$MAIN_DIR/$OUT_REL"
OUT_DIR="$MAIN_DIR/out/compare-k3diff-${STEM}"

if [ ! -f "$REF_VIDEO" ]; then
    echo "Error: reference video not found at $REF_VIDEO" >&2
    echo "Render it in the reference checkout (REF_DIR=$REF_DIR) first:" >&2
    echo "  cargo run --release --features profile -- $EXAMPLE" >&2
    exit 1
fi

echo "--- Step 1: Building opencat in this checkout ---"
cargo build --bin opencat --release --features profile
echo ""

echo "--- Step 2: Rendering $EXAMPLE ---"
./target/release/opencat "$EXAMPLE"
echo ""

echo "--- Step 3: k3diff (hard pixel metrics, no SSIM) ---"
python3 "$MAIN_DIR/tools/k3diff.py" "$REF_VIDEO" "$TEST_VIDEO" --out "$OUT_DIR"
echo ""

echo "Report:  $OUT_DIR/summary.json"
echo "Per-frame CSV: $OUT_DIR/pixel.csv"
echo "========================================"
