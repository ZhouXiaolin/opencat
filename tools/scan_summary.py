#!/usr/bin/env python3
"""汇总 e5-scan 各工程的 k3diff summary.json（独立于扫描脚本，可反复跑）。"""
import json, os, glob

OUT = r"F:\OpenCat\out\sweep\e5-scan"
rows = []
for d in sorted(glob.glob(os.path.join(OUT, "*-diff"))):
    name = os.path.basename(d)[:-5]
    sj = os.path.join(d, "summary.json")
    if not os.path.exists(sj):
        continue
    agg = json.load(open(sj)).get("agg") or {}
    rows.append((name, agg.get("avg_p8"), agg.get("mae_mean"), agg.get("frames")))
rows.sort(key=lambda r: (r[1] is None, r[1] or 0))
for name, p8, mae, frames in rows:
    flag = "  <-- >2%" if (p8 or 0) > 0.02 else ""
    print(f"{name:42s} p8={p8:.4f} mae={mae:7.2f} {frames}f{flag}")
