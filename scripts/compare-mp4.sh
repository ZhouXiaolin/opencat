#!/usr/bin/env bash
# Sampled web-vs-engine frame diff via the inspect ChromeDriver harness.
#
# Methodology: hard pixel metrics (k3diff), NOT SSIM — SSIM is too coarse and
# hides localized misalignment. Each sample is judged by mae / maxd / p8.
#
# Design decision (why not whole web MP4?):
# - Inspect oracle / web ground truth is raw RGBA from web/test-oracle.html
#   (CanvasKit readPixels), not WebAV exportMp4.
# - Facade leaves @webav/av-cliper external; headless whole-video export is not
#   the inspect contract and re-encoding would muddy the metrics.
# - So this script samples frames every INTERVAL_SECS (default 0.5s) on both
#   engine and web through opencat-web-compare, which reuses
#   opencat_engine::inspect::browser::{BrowserHarness, WebAppServer}.
#
# For whole-video native-vs-reference comparison, use tools/k3diff.py.
#
# Usage (from branch worktree):
#   ./scripts/compare-mp4.sh
#   ./scripts/compare-mp4.sh examples/profile-showcase.jsonl
#   INTERVAL_SECS=0.5 MAX_SAMPLES=20 ./scripts/compare-mp4.sh examples/profile-showcase.jsonl
#   MAX_MAE=1.0 MAX_FRAC=0.02 ./scripts/compare-mp4.sh examples/xhs-neo-brutalism.xml
#
# Env:
#   INTERVAL_SECS       sample period in seconds (default 0.5)
#   MAX_SAMPLES         optional cap on number of samples
#   MAX_MAE             per-frame MAE ceiling (default 1.0)
#   MAX_MAXD            per-frame max-channel-delta ceiling (0 = off)
#   FRAC_THRESHOLD      k3diff ladder threshold for the fraction gate (default 8)
#   MAX_FRAC            allowed fraction above FRAC_THRESHOLD (default 0.02)
#   SAVE_ALL=1          keep engine/web/diff PNGs for every sample
#   SKIP_BUILD=1        reuse existing opencat-web-compare binary
#   CHROME_BIN / CHROMEDRIVER_BIN / CHROMEDRIVER_URL
#   SKIA_BINARIES_URL
#
# Prerequisites:
#   chromedriver + Chrome,
#   (cd crates/opencat-web/web && bun install && bun run build)
#   (cd web && bun install)
set -euo pipefail

EXAMPLE="${1:-examples/profile-showcase.jsonl}"
REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

STEM="$(basename "$EXAMPLE")"
STEM="${STEM%.*}"
OUT_DIR="${OUT_DIR:-$REPO/out/compare-mp4-${STEM}}"
INTERVAL_SECS="${INTERVAL_SECS:-0.5}"
MAX_MAE="${MAX_MAE:-1.0}"
MAX_MAXD="${MAX_MAXD:-0}"
FRAC_THRESHOLD="${FRAC_THRESHOLD:-8}"
MAX_FRAC="${MAX_FRAC:-0.02}"

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "Error: required command not found: $1" >&2
        exit 1
    }
}

need_cmd cargo

if [ ! -f "$EXAMPLE" ]; then
    echo "Error: example not found: $EXAMPLE" >&2
    exit 1
fi

branch=$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')
sha=$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo '?')

echo "========================================"
echo "  Web vs Engine sampled k3diff"
echo "  (inspect ChromeDriver / test-oracle.html)"
echo "  Example:   $EXAMPLE"
echo "  Repo:      $REPO ($branch @ $sha)"
echo "  Report:    $OUT_DIR"
echo "  Interval:  ${INTERVAL_SECS}s"
echo "  Gate:      mae<=${MAX_MAE} maxd<=${MAX_MAXD} p${FRAC_THRESHOLD}<=${MAX_FRAC}"
echo "========================================"
echo ""

if [ ! -f "$REPO/crates/opencat-web/web/dist/opencat.js" ]; then
    echo "Error: web facade missing at crates/opencat-web/web/dist/opencat.js" >&2
    echo "Build: (cd crates/opencat-web/web && bun install && bun run build)" >&2
    exit 1
fi
if [ ! -f "$REPO/web/node_modules/canvaskit-wasm/bin/full/canvaskit.js" ]; then
    echo "Error: CanvasKit missing under web/node_modules" >&2
    echo "Install: (cd web && bun install)" >&2
    exit 1
fi

if [ "${SKIP_BUILD:-0}" != "1" ]; then
    echo "--- Build opencat-web-compare ---"
    cargo build --bin opencat-web-compare --release
    echo ""
else
    echo "--- SKIP_BUILD=1 ---"
    echo ""
fi

BIN="$REPO/target/release/opencat-web-compare"
if [ ! -x "$BIN" ]; then
    echo "Error: binary missing: $BIN" >&2
    exit 1
fi

args=(
    "$BIN" "$EXAMPLE"
    --out-dir "$OUT_DIR"
    --interval-secs "$INTERVAL_SECS"
    --max-mae "$MAX_MAE"
    --max-maxd "$MAX_MAXD"
    --frac-threshold "$FRAC_THRESHOLD"
    --max-frac "$MAX_FRAC"
)
if [ -n "${MAX_SAMPLES:-}" ]; then
    args+=(--max-samples "$MAX_SAMPLES")
fi
if [ "${SAVE_ALL:-0}" = "1" ]; then
    args+=(--save-all)
fi

echo "--- Sample + k3diff ---"
set +e
"${args[@]}"
code=$?
set -e
echo ""

if [ -f "$OUT_DIR/summary.txt" ]; then
    echo "--- Summary ---"
    cat "$OUT_DIR/summary.txt"
fi

echo "CSV:     $OUT_DIR/pixel.csv"
echo "JSON:    $OUT_DIR/summary.json"
echo "Summary: $OUT_DIR/summary.txt"
echo "========================================"
exit "$code"
