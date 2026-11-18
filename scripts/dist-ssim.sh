#!/usr/bin/env bash
# 标准 web dist 工程对参考 swgl 的帧对齐检查。
# 用法: scripts/dist-ssim.sh <工程目录> <参考mp4> <帧列表>
#   scripts/dist-ssim.sh examples/k3-promo \
#     ~/Projects/hyperframes-launches/k3-promo/renders/k3-promo_2026-10-06_swgl.mp4 \
#     45,70,141,160,180,211,282,350,430,493
set -euo pipefail

REPO="$(cd "$(dirname "$0")/.." && pwd)"
cd "$REPO"

PROJ="${1:?用法: dist-ssim.sh <工程目录> <参考mp4> <帧列表>}"
REF="${2:?参考 swgl mp4}"
FRAMES="${3:-70}"

BIN="${BIN:-$REPO/target/debug/opencat}"
DIST="$REPO/$PROJ/dist/index.html"
OUT="$REPO/out/dist-ssim/$(basename "$PROJ")"
mkdir -p "$OUT"

cargo build -q -p opencat --bin opencat

"$BIN" render "$DIST" --frames "$FRAMES" "$OUT"

for f in "$OUT"/frame_*.png; do
  n="$(basename "$f" .png)"; n="${n#frame_}"
  ffmpeg -v error -i "$REF" -vf "select=eq(n\,$((10#$n)))" -frames:v 1 -y "$OUT/ref_$n.png"
  printf '%s  ' "$n"
  ffmpeg -v info -i "$OUT/ref_$n.png" -i "$f" -lavfi ssim -f null - 2>&1 \
    | rg -o 'All:[0-9.]+' | tail -1
done
