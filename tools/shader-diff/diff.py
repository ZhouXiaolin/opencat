#!/usr/bin/env python3
"""shader-diff — 两棵渲染结果树的逐组件像素对拍。

对拍契约（与 k3diff.py 同款指标，对象从视频帧换成组件帧）：

  REF  通常是 oracle（shaders 原库，headless Chrome WebGPU）
  TEST 通常是 ours（opencat-shaders wgpu）

目录契约：REF/TEST 下 <Component>/default/f<frame>.png。
帧号与 props 见各自 manifest.json；成对条件 = 相对路径一致。

指标（0..255 RGB）：
  mae   均值绝对误差          maxd  单像素最大通道差
  p<T>  最大通道差 > T 的像素占比（T ∈ 0,2,4,8,16,32,64,128）
  bbox  超阈像素包围盒（定位差异区域）

用法：
  python3 diff.py out-oracle out-ours [--out diff-report] \
      [--gate-mae 0.5] [--gate-p8 0.001]

产物：diff-report/rows.json、diff-report/heatmaps/<...>.png、
     diff-report/report.md；退出码 0=全过 / 1=有超门 / 2=有缺失。
"""
import argparse
import json
import sys
from pathlib import Path

import numpy as np
from PIL import Image

THRESHOLDS = [2, 4, 8, 16, 32, 64, 128]


def heat(dm: np.ndarray) -> np.ndarray:
    """k3diff 同款热图：0 → 黑，超阈 → 红，量级 → 亮度。"""
    hi = np.clip(dm.astype(np.float32) * 8.0, 0, 255).astype(np.uint8)
    rgb = np.zeros(dm.shape + (3,), np.uint8)
    rgb[..., 0] = hi
    rgb[..., 1] = (hi // 3).astype(np.uint8)
    return rgb


def metrics(a: np.ndarray, b: np.ndarray) -> dict:
    d = np.abs(a.astype(np.int16) - b.astype(np.int16))
    dm = d.max(axis=2)
    row = {
        "mae": round(float(d.mean()), 4),
        "maxd": int(dm.max()),
    }
    for t in THRESHOLDS:
        mask = dm > t
        row[f"p{t}"] = float(mask.mean())
        if mask.any():
            ys, xs = np.nonzero(mask)
            row["bbox"] = [int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())]
    return row


def pair_rows(ref: Path, test: Path) -> list[dict]:
    rows = []
    for rp in sorted(ref.rglob("f*.png")):
        rel = rp.relative_to(ref)
        tp = test / rel
        entry = {"pair": str(rel)}
        if not tp.exists():
            entry["status"] = "missing"
            rows.append(entry)
            continue
        a = np.asarray(Image.open(rp).convert("RGB"))
        b = np.asarray(Image.open(tp).convert("RGB"))
        if a.shape != b.shape:
            entry["status"] = "size-mismatch"
            entry["ref"] = list(a.shape)
            entry["test"] = list(b.shape)
            rows.append(entry)
            continue
        entry["status"] = "ok"
        entry.update(metrics(a, b))
        rows.append(entry)
    # TEST 独有文件（REF 没有的）也报出来
    for tp in sorted(test.rglob("f*.png")):
        rel = tp.relative_to(test)
        if not (ref / rel).exists():
            rows.append({"pair": str(rel), "status": "test-only"})
    return rows


def gate(row: dict, gate_mae: float, gate_p8: float) -> str:
    if row["status"] != "ok":
        return row["status"]
    if row["mae"] > gate_mae or row["p8"] > gate_p8:
        return "FAIL"
    return "pass"


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("ref")
    ap.add_argument("test")
    ap.add_argument("--out", default="diff-report")
    ap.add_argument("--gate-mae", type=float, default=0.5)
    ap.add_argument("--gate-p8", type=float, default=0.001)
    args = ap.parse_args()

    ref, test = Path(args.ref), Path(args.test)
    out = Path(args.out)
    (out / "heatmaps").mkdir(parents=True, exist_ok=True)

    rows = pair_rows(ref, test)
    for r in rows:
        r["gate"] = gate(r, args.gate_mae, args.gate_p8)
        if r["status"] == "ok":
            hp = out / "heatmaps" / Path(r["pair"]).with_suffix(".heat.png")
            hp.parent.mkdir(parents=True, exist_ok=True)
            a = np.asarray(Image.open(ref / r["pair"]).convert("RGB"))
            b = np.asarray(Image.open(test / r["pair"]).convert("RGB"))
            dm = np.abs(a.astype(np.int16) - b.astype(np.int16)).max(axis=2)
            Image.fromarray(heat(dm)).save(hp)

    # 按组件聚合：取最差一对（mae 最高）
    by_comp: dict[str, list[dict]] = {}
    for r in rows:
        comp = Path(r["pair"]).parts[0]
        by_comp.setdefault(comp, []).append(r)
    summary = []
    for comp, rs in by_comp.items():
        oks = [r for r in rs if r["status"] == "ok"]
        worst = max(oks, key=lambda r: r["mae"]) if oks else None
        summary.append({
            "component": comp,
            "pairs": len(rs),
            "non-ok": len(rs) - len(oks),
            "worst": worst,
            "gate": "FAIL" if any(r["gate"] == "FAIL" for r in rs)
            else ("missing" if any(r["status"] != "ok" for r in rs) else "pass"),
        })
    summary.sort(key=lambda s: (-(s["worst"] or {}).get("mae", 0) if s["worst"] else 0, s["component"]))

    (out / "rows.json").write_text(json.dumps(rows, indent=2))

    lines = [
        "# shader-diff report",
        "",
        f"REF `{ref}` vs TEST `{test}` · gate: mae≤{args.gate_mae}, p8≤{args.gate_p8} (provisional)",
        "",
        "| component | worst mae | worst maxd | worst p8 | pairs | gate |",
        "|---|---|---|---|---|---|",
    ]
    for s in summary:
        w = s["worst"]
        p8 = f"{w['p8']:.2e}" if w else "—"
        mae = w["mae"] if w else "—"
        maxd = w["maxd"] if w else "—"
        lines.append(
            f"| {s['component']} | {mae} | {maxd} | {p8} | {s['pairs']} (+{s['non-ok']}) | {s['gate']} |"
        )
    (out / "report.md").write_text("\n".join(lines) + "\n")

    n_fail = sum(1 for r in rows if r["gate"] == "FAIL")
    n_missing = sum(1 for r in rows if r["status"] in ("missing", "size-mismatch", "test-only"))
    print(f"pairs={len(rows)} fail={n_fail} missing/other={n_missing}")
    for s in summary[:15]:
        w = s["worst"]
        if w:
            print(f"  {s['component']:<28} mae={w['mae']:<8} maxd={w['maxd']:<5} p8={w['p8']:.2e} gate={s['gate']}")
    if n_missing:
        return 2
    return 1 if n_fail else 0


if __name__ == "__main__":
    sys.exit(main())
