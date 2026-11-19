#!/usr/bin/env bash
# 语料线 walk_sheet（宪法 §11 线二）：hyperframes-launches 全语料逐例
# engine VK 全片渲染 + k3diff 对参考 swgl，覆盖渲染门三例之外的广度。
# 用法：bash scripts/walksheet.sh（env 可覆盖 OPENCAT_ROOT/REF_ROOT/OUT_ROOT，
# 惯例同 scripts/regression-gate.sh 的 REF_ROOT）。
# 判读口径（2026-11-18 首轮，20/20 渲染成功零缺陷）：
#   - ≤0.02 过门；insp/vfx 的 0.15 级=sin-hash 噪声场地板（§11 地板注）；
#   - liquid 0.337=已知 3D 未真重建（opencat-three 待做）；
#   - 其余 0.023-0.126 区间 worst 帧目视全部=动画时序相位窗口差
#     （标题位移/滚动进度），渲染要素一致——判读规则同 §11 地板注：
#     对基线同段判，不设硬门。
# 结果增量追加 $OUT/summary.tsv；异常例抽帧目视（ffmpeg select worst 帧）。
set -u
ROOT="${OPENCAT_ROOT:-/home/solaren/Projects/opencat}"
REF="${REF_ROOT:-/home/solaren/Projects/hyperframes-launches}"
OUT="${OUT_ROOT:-$ROOT/out/walksheet}"
mkdir -p "$OUT"
ENV=(env LD_PRELOAD=$ROOT/tools/angle-linux/libvulkan.so.1
  VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.json
  OC_ANGLE_DIR=$ROOT/tools/angle-linux OC_GPU_PIPELINE=1)
TSV=$OUT/summary.tsv
echo -e "name\trender-exit\tavg_p8\tnote" > "$TSV"

for n in claude-design-send-hyperframes-launch claude-paper-launch cloud-render-launch \
  codex-five-hour-limit-replica figma-launch frame-md-launch-storyboard \
  HF-heygen-stripe hyperframes-launch inspector-launch k3-promo liquid-brand-refraction \
  pr-to-video-launch sfx-music-launch spacex-launch texture-launch-video timeline-launch \
  variables-launch vfx-heygen-combined website-to-hyperframes; do
  ref=$(ls "$REF/$n/renders/"*_swgl.mp4 2>/dev/null | head -1)
  echo "=== walk $n ==="
  rm -f "$OUT/$n.mp4"
  "${ENV[@]}" "$ROOT/target/release/opencat" render "$REF/$n/index.html" "$OUT/$n.mp4" \
    --base-dir "$REF/$n" > "$OUT/$n.render.log" 2>&1
  rc=$?
  if [ $rc -ne 0 ]; then
    echo -e "$n\t$rc\t\tRENDER-FAIL" >> "$TSV"
    continue
  fi
  if [ -z "$ref" ]; then
    echo -e "$n\t$rc\t\tNO-REF(目视)" >> "$TSV"
    continue
  fi
  python3 "$ROOT/tools/k3diff.py" "$ref" "$OUT/$n.mp4" --out "$OUT/k3diff-$n" > /dev/null 2>&1
  p8=$(python3 -I -c "
import json
try:
    d=json.load(open('$OUT/k3diff-$n/summary.json'))
    print(round(d['agg']['avg_p8'],5))
except Exception as e:
    print('DIFF-ERR')")
  echo -e "$n\t$rc\t$p8\t" >> "$TSV"
done
echo "WALK-DONE" >> "$TSV"
cat "$TSV"
