#!/usr/bin/env bash
# Sampled web-vs-engine SSIM via the inspect ChromeDriver harness.
#
# Design decision (why not whole web MP4?):
# - Inspect oracle / web ground truth is raw RGBA from web/test-oracle.html
#   (CanvasKit readPixels), not WebAV exportMp4.
# - Facade leaves @webav/av-cliper external; headless whole-video export is not
#   the inspect contract and re-encoding would muddy SSIM.
# - So this script samples frames every INTERVAL_SECS (default 0.5s) on both
#   engine and web through opencat-web-compare, which reuses
#   opencat_engine::inspect::browser::{BrowserHarness, WebAppServer}.
#
# For native-vs-native whole-video SSIM (main vs branch), use compare-ssim.sh.
#
# Usage (from branch worktree):
#   ./scripts/compare-mp4.sh
#   ./scripts/compare-mp4.sh examples/profile-showcase.jsonl
#   INTERVAL_SECS=0.5 MAX_SAMPLES=20 ./scripts/compare-mp4.sh examples/profile-showcase.jsonl
#   MIN_SSIM=0.99 VIDEO_MIN_SSIM=0.97 ./scripts/compare-mp4.sh examples/xhs-neo-brutalism.xml
#
# Env:
#   INTERVAL_SECS       sample period in seconds (default 0.5)
#   MAX_SAMPLES         optional cap on number of samples
#   MIN_SSIM            strict threshold (default 0.99)
#   VIDEO_MIN_SSIM      soft threshold for video-decoder tolerance (default 0.97)
#   SAVE_ALL=1          keep engine/web/diff PNGs for every sample
#   SKIP_BUILD=1        reuse existing opencat-web-compare binary
#   CHROME_BIN / CHROMEDRIVER_BIN / CHROMEDRIVER_URL
#   SKIA_BINARIES_URL
#
# Prerequisites:
#   chromedriver + Chrome, ffmpeg (for per-frame SSIM),
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
MIN_SSIM="${MIN_SSIM:-0.99}"
VIDEO_MIN_SSIM="${VIDEO_MIN_SSIM:-0.97}"

need_cmd() {
    command -v "$1" >/dev/null 2>&1 || {
        echo "Error: required command not found: $1" >&2
        exit 1
    }
}

need_cmd cargo
need_cmd ffmpeg
need_cmd python3

if [ ! -f "$EXAMPLE" ]; then
    echo "Error: example not found: $EXAMPLE" >&2
    exit 1
fi

branch=$(git -C "$REPO" rev-parse --abbrev-ref HEAD 2>/dev/null || echo '?')
sha=$(git -C "$REPO" rev-parse --short HEAD 2>/dev/null || echo '?')

echo "========================================"
echo "  Web vs Engine sampled SSIM"
echo "  (inspect ChromeDriver / test-oracle.html)"
echo "  Example:   $EXAMPLE"
echo "  Repo:      $REPO ($branch @ $sha)"
echo "  Report:    $OUT_DIR"
echo "  Interval:  ${INTERVAL_SECS}s"
echo "  Threshold: min=${MIN_SSIM} video=${VIDEO_MIN_SSIM}"
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
    --min-ssim "$MIN_SSIM"
    --video-min-ssim "$VIDEO_MIN_SSIM"
)
if [ -n "${MAX_SAMPLES:-}" ]; then
    args+=(--max-samples "$MAX_SAMPLES")
fi
if [ "${SAVE_ALL:-0}" = "1" ]; then
    args+=(--save-all)
fi

echo "--- Sample + SSIM ---"
set +e
"${args[@]}"
code=$?
set -e
echo ""

if [ -f "$OUT_DIR/summary.txt" ]; then
    echo "--- Summary ---"
    cat "$OUT_DIR/summary.txt"
fi

echo "CSV:     $OUT_DIR/ssim_samples.csv"
echo "Summary: $OUT_DIR/summary.txt"
echo "========================================"
exit "$code"
