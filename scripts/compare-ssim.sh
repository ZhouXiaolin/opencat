#!/usr/bin/env bash
# Frame-by-frame SSIM regression: render an example in this worktree and compare
# its output against the same example rendered on the main checkout.
#
# Usage:
#   ./scripts/compare-ssim.sh                              # default example
#   ./scripts/compare-ssim.sh examples/profile-showcase.jsonl
#
# Env:
#   SKIA_BINARIES_URL  forwarded to cargo (set to file:///tmp/skia-binaries.tar.gz
#                      in sandboxes that can't source-build skia-bindings).
#
# The reference video must already exist at $MAIN_DIR/$OUT_REL — render it on
# main first:
#   cargo run --release --features profile -- examples/<stem>.<ext>
set -euo pipefail

EXAMPLE="${1:-examples/xhs-neo-brutalism.xml}"

MAIN_DIR="$(cd "$(dirname "$0")/.." && pwd)"
WORKTREE_DIR="/home/solaren/Projects/opencat-issue-2"

# Derive the output stem from the example path: examples/foo.bar -> out/foo.mp4
STEM="$(basename "$EXAMPLE")"
STEM="${STEM%.*}"
OUT_REL="out/${STEM}.mp4"

REF_VIDEO="$MAIN_DIR/$OUT_REL"
TEST_VIDEO="$WORKTREE_DIR/$OUT_REL"
OUT_DIR="$MAIN_DIR/out/compare-${STEM}"
STATS_FILE="$OUT_DIR/ssim_stats.txt"
REPORT_FILE="$OUT_DIR/ssim_report.txt"

main_branch=$(git -C "$MAIN_DIR" rev-parse --abbrev-ref HEAD)
worktree_branch=$(git -C "$WORKTREE_DIR" rev-parse --abbrev-ref HEAD)

mkdir -p "$OUT_DIR"

echo "========================================"
echo "  SSIM Comparison"
echo "  Example:  $EXAMPLE"
echo "  Reference: $MAIN_DIR ($main_branch)"
echo "  Test:      $WORKTREE_DIR ($worktree_branch)"
echo "========================================"
echo ""

if [ ! -f "$REF_VIDEO" ]; then
    echo "Error: reference video not found at $REF_VIDEO"
    echo "Run on $main_branch first:"
    echo "  cargo run --release --features profile -- $EXAMPLE"
    exit 1
fi

echo "--- Reference video: $(ffprobe -v error -count_frames -select_streams v:0 -show_entries stream=nb_read_frames -of csv=p=0 "$REF_VIDEO" 2>/dev/null || echo '?') frames ---"
echo "--- Reference streams: $(ffprobe -v error -show_entries stream=codec_type -of csv=p=0 "$REF_VIDEO" 2>/dev/null | tr '\n' '+' | sed 's/+$//') ---"
echo ""

echo "--- Step 1: Building opencat in worktree ($worktree_branch) ---"
cargo build --bin opencat --release --features profile
echo ""

echo "--- Step 2: Rendering $EXAMPLE in worktree ---"
./target/release/opencat "$EXAMPLE"
echo ""

echo "--- Step 3: Frame-by-frame SSIM ---"
ffmpeg -i "$REF_VIDEO" -i "$TEST_VIDEO" \
    -filter_complex "ssim=stats_file=$STATS_FILE" \
    -f null - 2>&1 | tee "$REPORT_FILE"
echo ""

echo "--- Step 4: Results ---"
awk '
{
    for(i=1;i<=NF;i++) {
        if($i ~ /^All:/) {
            split($i, a, ":")
            val = a[2] + 0
            n += 1
            sum += val
            if(n == 1) { min = max = val }
            else {
                if(val < min) min = val
                if(val > max) max = val
            }
        }
    }
}
END {
    printf "Frames compared: %d\n", n
    printf "SSIM min:  %.6f\n", min
    printf "SSIM max:  %.6f\n", max
    printf "SSIM avg:  %.6f\n", sum/n
}' "$STATS_FILE"
echo ""

last_line=$(tail -1 "$STATS_FILE" 2>/dev/null || true)
if [ -n "$last_line" ]; then
    echo "Global SSIM (last frame): $last_line"
fi

echo ""
echo "Full per-frame stats:    $STATS_FILE"
echo "FFmpeg log:              $REPORT_FILE"
echo "========================================"
