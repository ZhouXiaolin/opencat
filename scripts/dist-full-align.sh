#!/usr/bin/env bash
# 标准 web dist 工程全片对齐：渲染完整 MP4，再对参考 swgl 跑 k3diff + SSIM。
# 用法: scripts/dist-full-align.sh <工程目录> <参考mp4> [输出名]
#   scripts/dist-full-align.sh examples/k3-promo \
#     ~/Projects/hyperframes-launches/k3-promo/renders/k3-promo_2026-10-06_swgl.mp4
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

PROJ="${1:?用法: dist-full-align.sh <工程目录> <参考mp4> [输出名]}"
REF="${2:?参考 swgl mp4}"
STEM="${3:-$(basename "$PROJ")}"
BIN="${BIN:-$REPO/target/debug/opencat}"
DIST="$REPO/$PROJ/dist/index.html"
OUT="$REPO/out/full-align/$STEM"
mkdir -p "$OUT"

# GPU 全片渲染需要 ANGLE/Vulkan 环境（同 scripts/walksheet.sh）。
ENV=(env LD_PRELOAD="$REPO/tools/angle-linux/libvulkan.so.1"
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.json
  OC_SKIA_BACKEND=vulkan OC_ANGLE_DIR="$REPO/tools/angle-linux" OC_GPU_PIPELINE=1)

echo "=== [$STEM] 1/3: render dist -> mp4 ==="
cargo build -q -p opencat --bin opencat
"${ENV[@]}" "$BIN" render "$DIST" --base-dir "$REPO/$PROJ/dist" "$OUT/$STEM.mp4"

echo "=== [$STEM] 2/3: k3diff (hard pixel metrics) ==="
python3 "$REPO/tools/k3diff.py" "$REF" "$OUT/$STEM.mp4" --out "$OUT/k3diff"

echo "=== [$STEM] 3/3: ffmpeg SSIM (all frames) ==="
ffmpeg -v info -i "$REF" -i "$OUT/$STEM.mp4" -lavfi ssim -f null - 2>&1 \
  | rg -o 'All:[0-9.]+' | tail -1 | tee "$OUT/ssim.txt"

echo "=== [$STEM] DONE ==="
