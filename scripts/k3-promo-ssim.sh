#!/usr/bin/env bash
# Per-frame + per-scene SSIM of the opencat k3-promo port against the
# reference mp4 rendered by the upstream hyperframes project.
#
# Reuses the ffmpeg `ssim` filter idea from compare-ssim.sh but targets a
# fixed external reference and adds a per-scene breakdown so we can see WHERE
# the port diverges, not just the global mean.
#
# Usage:
#   ./scripts/k3-promo-ssim.sh                 # render + compare
#   SKIP_RENDER=1 ./scripts/k3-promo-ssim.sh   # reuse out/k3-promo-current.mp4
#
# Env:
#   REF      reference mp4 (default: hyperframes-launches/k3-promo.mp4)
#   XML      markup to render  (default: examples/k3-promo/k3-promo.xml)
#   MP4      rendered output   (default: out/k3-promo-current.mp4)
#   BIN      opencat binary    (default: target/release/opencat)
#   OUT_DIR  report directory  (default: out/k3-promo-ssim)
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

REF="${REF:-/home/solaren/Projects/hyperframes-launches/k3-promo/k3-promo.mp4}"
XML="${XML:-examples/k3-promo/k3-promo.xml}"
MP4="${MP4:-$REPO/out/k3-promo-current.mp4}"
BIN="${BIN:-$REPO/target/release/opencat}"
OUT_DIR="${OUT_DIR:-$REPO/out/k3-promo-ssim}"
STATS="$OUT_DIR/ssim_stats.txt"
LOG="$OUT_DIR/ffmpeg.log"

mkdir -p "$OUT_DIR"

if [ ! -f "$REF" ]; then
    echo "Error: reference mp4 not found: $REF" >&2
    exit 1
fi

if [ "${SKIP_RENDER:-0}" != "1" ]; then
    echo "--- Render $XML → $MP4 ---"
    "$BIN" render "$XML" "$MP4"
else
    echo "--- SKIP_RENDER=1: reusing $MP4 ---"
fi

frames_of() {
    ffprobe -v error -count_frames -select_streams v:0 \
        -show_entries stream=nb_read_frames -of csv=p=0 "$1" 2>/dev/null \
        | tail -1
}
ref_frames=$(frames_of "$REF")
test_frames=$(frames_of "$MP4")
echo "--- Reference: $ref_frames frames | Port: $test_frames frames ---"
if [ "$ref_frames" != "$test_frames" ]; then
    echo "WARNING: frame counts differ; ffmpeg pairs by PTS up to the shorter stream." >&2
fi
echo ""

echo "--- Per-frame SSIM (ffmpeg) ---"
# Both streams are 30 fps CFR; reset PTS so frame indices line up 1:1. The
# shorter stream (reference) terminates the comparison, so any extra trailing
# frame on the port side is ignored.
ffmpeg -hide_banner -nostats \
    -i "$REF" -i "$MP4" \
    -lavfi "[0:v]setpts=PTS-STARTPTS[ref];[1:v]setpts=PTS-STARTPTS[test];[ref][test]ssim=stats_file=$STATS" \
    -f null - 2> "$LOG" || true

echo "--- Per-scene report ---"
python3 "$REPO/scripts/ssim_scene_report.py" "$STATS" \
    --out-dir "$OUT_DIR" --fps 30 \
    --csv "$OUT_DIR/ssim_per_frame.csv" \
    --summary "$OUT_DIR/summary.txt"

echo ""
echo "CSV:     $OUT_DIR/ssim_per_frame.csv"
echo "Stats:   $STATS"
echo "Summary: $OUT_DIR/summary.txt"
