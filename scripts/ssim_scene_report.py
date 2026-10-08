#!/usr/bin/env python3
"""Summarize an ffmpeg `ssim` stats_file into a per-scene + worst-frame report.

ffmpeg emits one line per compared frame in the form:
    n:1 mse_avg:... ssim_y:0.98 ssim_u:0.99 ssim_v:0.99 All:0.981234 (...)
and a final summary line `SSIM average: ... (....)`.

This script extracts the `All:` SSIM per frame, buckets frames into the
k3-promo scene windows, and prints min/mean/median per scene plus the worst N
frames. Output goes to stdout, a CSV, and a summary file.
"""
from __future__ import annotations

import argparse
import csv
import re
import statistics
import sys
from pathlib import Path

# k3-promo scene windows as [start_frame, end_frame) — frame index at 30 fps.
# Non-overlapping, ordered; each frame is assigned to the first matching scene.
# Derived from the time windows documented in examples/k3-promo.xml.
SCENES: list[tuple[str, int, int]] = [
    ("opencard", 0, 34),       # 0.000 – 1.12 s
    ("sceneA",   34, 137),     # 1.12  – 4.56 s (lattice field + pills)
    ("sceneB",   137, 163),    # 4.56  – 5.43 s (one model)
    ("sceneC",   163, 220),    # 5.43  – 7.33 s (decode + ghost cols)
    ("flash",    220, 222),    # 7.33  – 7.39 s (difference flash)
    ("sceneDE",  222, 337),    # 7.39  – 11.23 s (pin waves + boxes)
    ("sceneFG",  337, 411),    # 11.23 – 13.70 s (brace card)
    ("sceneHI",  411, 494),    # 13.70 – 16.47 s (dissolve → K3 wordmark)
]

_N_RE = re.compile(r"n:(\d+)")
_ALL_RE = re.compile(r"All:([0-9.]+)")


def scene_of(frame: int) -> str:
    for name, lo, hi in SCENES:
        if lo <= frame < hi:
            return name
    return "tail"


def parse(path: Path) -> list[tuple[int, float]]:
    rows: list[tuple[int, float]] = []
    for line in path.read_text(errors="replace").splitlines():
        nm = _N_RE.search(line)
        am = _ALL_RE.search(line)
        if not (nm and am):
            continue
        try:
            rows.append((int(nm.group(1)), float(am.group(1))))
        except ValueError:
            continue
    rows.sort(key=lambda r: r[0])
    return rows


def pct(values: list[float], q: float) -> float:
    if not values:
        return float("nan")
    s = sorted(values)
    k = int(round(q * (len(s) - 1)))
    return s[k]


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("stats", type=Path, help="ffmpeg ssim stats_file")
    ap.add_argument("--out-dir", type=Path, default=Path("out/k3-promo-ssim"))
    ap.add_argument("--fps", type=float, default=30.0)
    ap.add_argument("--csv", type=Path, default=None)
    ap.add_argument("--summary", type=Path, default=None)
    ap.add_argument("--worst", type=int, default=20)
    args = ap.parse_args()

    rows = parse(args.stats)
    if not rows:
        print(f"No SSIM rows parsed from {args.stats}", file=sys.stderr)
        return 2

    frames = [r[0] for r in rows]
    sims = [r[1] for r in rows]

    # Per-scene buckets.
    by_scene: dict[str, list[float]] = {}
    for f, s in rows:
        by_scene.setdefault(scene_of(f), []).append(s)

    lines: list[str] = []
    def out(text: str = "") -> None:
        print(text)
        lines.append(text)

    out("=" * 64)
    out(f"  SSIM: {len(rows)} frames  (frames {frames[0]}–{frames[-1]})")
    out("=" * 64)
    out("")
    out("  Global SSIM")
    out("  -----------")
    out(f"    mean   : {statistics.fmean(sims):.6f}")
    out(f"    median : {statistics.median(sims):.6f}")
    out(f"    min    : {min(sims):.6f}")
    out(f"    max    : {max(sims):.6f}")
    out(f"    p10    : {pct(sims, 0.10):.6f}")
    out(f"    p25    : {pct(sims, 0.25):.6f}")
    out(f"    <0.80  : {sum(1 for s in sims if s < 0.80)} frames")
    out(f"    <0.90  : {sum(1 for s in sims if s < 0.90)} frames")
    out(f"    <0.95  : {sum(1 for s in sims if s < 0.95)} frames")
    out("")

    out("  Per-scene SSIM (mean / min / median / n)")
    out("  ---------------------------------------")
    out(f"    {'scene':<10} {'mean':>9} {'min':>9} {'median':>9} {'p25':>9} {'n':>5}")
    for name, lo, hi in SCENES:
        bucket = by_scene.get(name, [])
        if not bucket:
            out(f"    {name:<10} {'—':>9} {'—':>9} {'—':>9} {'—':>9} {0:>5}  (f{lo}–{hi})")
            continue
        out(
            f"    {name:<10} {statistics.fmean(bucket):>9.6f} {min(bucket):>9.6f} "
            f"{statistics.median(bucket):>9.6f} {pct(bucket, 0.25):>9.6f} {len(bucket):>5}  (f{lo}–{hi})"
        )
    other = by_scene.get("tail", [])
    if other:
        out(f"    {'tail':<10} {statistics.fmean(other):>9.6f} {min(other):>9.6f} "
            f"{statistics.median(other):>9.6f} {pct(other, 0.25):>9.6f} {len(other):>5}")
    out("")

    worst = sorted(rows, key=lambda r: r[1])[: args.worst]
    out(f"  Worst {len(worst)} frames")
    out(f"  {'frame':>6} {'time':>8} {'scene':<10} {'ssim':>10}")
    for f, s in worst:
        out(f"  {f:>6} {f / args.fps:>7.3f}s {scene_of(f):<10} {s:>10.6f}")
    out("")
    out("=" * 64)

    args.out_dir.mkdir(parents=True, exist_ok=True)
    csv_path = args.csv or (args.out_dir / "ssim_per_frame.csv")
    with csv_path.open("w", newline="") as fh:
        w = csv.writer(fh)
        w.writerow(["frame", "time", "scene", "ssim"])
        for f, s in rows:
            w.writerow([f, f"{f / args.fps:.4f}", scene_of(f), f"{s:.6f}"])

    summary_path = args.summary or (args.out_dir / "summary.txt")
    summary_path.write_text("\n".join(lines) + "\n")

    print(f"\nCSV:     {csv_path}")
    print(f"Summary: {summary_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
