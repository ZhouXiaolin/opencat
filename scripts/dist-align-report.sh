#!/usr/bin/env bash
# 汇总 out/full-align/* 的全片对齐结果（k3diff hard-pixel + ffmpeg SSIM）。
set -euo pipefail
cd "$(dirname "$0")/.."
python3 - <<'PY'
import json,glob,os
print(f"{'project':28} {'frames':>6} {'mae':>8} {'avg_p8':>9} {'ssim':>9}  worst-frame(p8)")
for d in sorted(glob.glob('out/full-align/*')):
    s=os.path.join(d,'k3diff','summary.json')
    t=os.path.join(d,'ssim.txt')
    if not (os.path.exists(s) and os.path.exists(t)): continue
    j=json.load(open(s)); a=j['agg']
    worst=max(j['rows'],key=lambda r:r['p8'])
    ssim=open(t).read().strip().replace('All:','')
    print(f"{os.path.basename(d):28} {a['frames']:6d} {a['mae_mean']:8.3f} {a['avg_p8']:9.5f} {ssim:>9}  f{worst['frame']}({worst['p8']:.4f})")
PY
