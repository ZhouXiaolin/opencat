#!/usr/bin/env bash
# @omp 可重复测量回路（单帧 script 段）。两腿同语料交替跑，输出 min-of-N。
#
#   腿 JS      = OC_OMP_DISABLE=1，原循环在 V8 主线程跑（整帧墙钟）
#   腿 iso-Tn  = isolate 团队 n worker（[omp] take wall）
#
# 用法：OC_BIN=target/release/opencat.exe scripts/omp-bench.sh <html> [N]
# 前提：设 OC_ANGLE_DIR 指向 swangle 目录，否则 GPU 段报错（script 段不受影响）。
set -u
IN=${1:?usage: OMP-bench <input.html> [N]}
N=${2:-3}
BIN=${OC_BIN:-target/release/opencat.exe}
OUT=$(mktemp -d)
t0() { date +%s%N; }
wall_ms() { echo $(( ($2 - $1) / 1000000 )); }

echo "in=$IN bin=$BIN n=$N"
best=999999
for _ in $(seq "$N"); do
  a=$(t0); OC_OMP_DISABLE=1 "$BIN" render "$IN" "$OUT/js" --frames 120:121 >/dev/null 2>&1; b=$(t0)
  ms=$(wall_ms "$a" "$b"); [ "$ms" -lt "$best" ] && best=$ms
done
echo "leg JS       whole-frame ${best}ms"

for t in 1 2 4 6 8; do
  best=999999
  for _ in $(seq "$N"); do
    line=$(OC_OMP_NUM_THREADS=$t OC_OMP_TRACE=1 "$BIN" render "$IN" "$OUT/t$t" --frames 120:121 2>&1 \
      | grep -oE 'take wall: [0-9.]+(s|ms)' | head -1)
    [ -z "$line" ] && { echo "leg T$t      (no omp take — 未接管，见 --trace)"; break; }
    ms=$(echo "$line" | awk '{v=$3; print (v ~ /ms$/) ? v+0 : (v*1000)}' | cut -d. -f1)
    [ "$ms" -lt "$best" ] && best=$ms
  done
  [ -n "${line:-}" ] && echo "leg T$t      take ${best}ms"
done
rm -rf "$OUT"
