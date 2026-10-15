#!/usr/bin/env bash
# 缓存消融扫描：每轮只开一个 OC_ABLATE 变体，**串行**跑全片（独占机器，各轮
# 之间不争抢 CPU/内存），记录：
#   elapsed_s  该轮墙钟耗时（含编码）
#   mae/p8     该轮 mp4 与基线 mp4 的逐像素差（k3diff）
#
# 两个结论一起出：
#   - elapsed 变大 = 该缓存有正向收益；几乎不变 = 该缓存是死重量。
#   - mae/p8 非零 = 该缓存不是透明缓存（关掉会改渲染结果），需要单独查。
#
# 用法: tools/ablate-sweep.sh [input] [out-subdir]
set -uo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

INPUT="${1:-examples/k3-promo/k3-promo.jsx}"
NAME="${2:-$(basename "${INPUT%.*}")}"
BIN="$REPO/target/release/opencat"
OUT="$REPO/out/ablate/$NAME"
mkdir -p "$OUT"

if [ ! -x "$BIN" ]; then
    echo "missing $BIN — run: cargo build --release --features profile --bin opencat" >&2
    exit 1
fi

VARIANTS=(
    "base:"
    "layout_merkle:layout_merkle"
    "display_merkle:display_merkle"
    "font_system:font_system"
    "swash:swash"
    "video_frames:video_frames"
    "runtime_effects:runtime_effects"
    "glyph_rasters:glyph_rasters"
    "generated_images:generated_images"
    "liangyi_compile:liangyi_compile"
)

render_one() {
    local name="$1" tokens="$2"
    local mp4="$OUT/$name.mp4" log="$OUT/$name.log"
    local start end
    start=$(date +%s)
    if [ -n "$tokens" ]; then
        OC_ABLATE="$tokens" "$BIN" render "$INPUT" "$mp4" >"$log" 2>&1
    else
        env -u OC_ABLATE "$BIN" render "$INPUT" "$mp4" >"$log" 2>&1
    fi
    local code=$?
    end=$(date +%s)
    echo $(( end - start ))
    return $code
}

printf 'variant\telapsed_s\tmae\tavg_p8\tmaxd\tframes\n' > "$OUT/summary.tsv"

BASE_MP4=""
for entry in "${VARIANTS[@]}"; do
    name="${entry%%:*}"
    tokens="${entry#*:}"

    echo "=== [$name] OC_ABLATE='${tokens}'"
    elapsed=$(render_one "$name" "$tokens") || {
        echo "  FAILED (see $OUT/$name.log)" >&2
        printf '%s\tFAIL\t-\t-\t-\t-\n' "$name" >> "$OUT/summary.tsv"
        continue
    }
    echo "  elapsed=${elapsed}s"

    if [ -z "$BASE_MP4" ]; then
        BASE_MP4="$OUT/$name.mp4"
        printf '%s\t%s\t0\t0\t0\t-\n' "$name" "$elapsed" >> "$OUT/summary.tsv"
        continue
    fi

    python3 "$REPO/tools/k3diff.py" "$BASE_MP4" "$OUT/$name.mp4" \
        --out "$OUT/diff-$name" >"$OUT/diff-$name.log" 2>&1
    read -r mae p8 maxd frames < <(python3 - "$OUT/diff-$name/summary.json" <<'PY'
import json, sys
d = json.load(open(sys.argv[1]))
a = d["agg"]
print(a["mae_mean"], a["avg_p8"], a["maxd_max"], a["frames"])
PY
)
    echo "  vs base: mae=$mae p8=$p8 maxd=$maxd"
    printf '%s\t%s\t%s\t%s\t%s\t%s\n' "$name" "$elapsed" "$mae" "$p8" "$maxd" "$frames" \
        >> "$OUT/summary.tsv"
done

echo
echo "=== $NAME ablation summary ==="
column -t -s$'\t' "$OUT/summary.tsv"
