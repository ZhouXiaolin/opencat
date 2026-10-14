#!/usr/bin/env bash
# 对每个消融变体，量它相对**参考视频**的对齐度（tools/k3diff.py 硬像素）。
# 串行执行，不并行（避免 CPU 争抢污染耗时）。
#
# 用法: tools/ablate-align.sh <variant-dir> <reference.mp4>
set -uo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
DIR="${1:?variant dir}"
REF="${2:?reference mp4}"

printf 'variant\tavg_p8\tavg_p2\tmae\tmaxd\n' > "$DIR/ref-align.tsv"
for mp4 in "$DIR"/base.mp4 "$DIR"/layout_merkle.mp4 "$DIR"/display_merkle.mp4 \
           "$DIR"/font_system.mp4 "$DIR"/swash.mp4 "$DIR"/video_frames.mp4 \
           "$DIR"/runtime_effects.mp4 "$DIR"/glyph_rasters.mp4 "$DIR"/generated_images.mp4 \
           "$DIR"/liangyi_compile.mp4; do
    [ -f "$mp4" ] || continue
    name="$(basename "$mp4" .mp4)"
    python3 "$REPO/tools/k3diff.py" "$REF" "$mp4" --out "$DIR/ref-$name" >"$DIR/ref-$name.log" 2>&1
    read -r p8 p2 mae maxd < <(python3 - "$DIR/ref-$name/summary.json" <<'PY'
import json, sys
a = json.load(open(sys.argv[1]))["agg"]
print(a["avg_p8"], a["avg_p2"], a["mae_mean"], a["maxd_max"])
PY
)
    printf '%s\t%s\t%s\t%s\t%s\n' "$name" "$p8" "$p2" "$mae" "$maxd" >> "$DIR/ref-align.tsv"
    echo "[$name] p8=$p8 p2=$p2 mae=$mae"
done

echo
column -t -s$'\t' "$DIR/ref-align.tsv"
