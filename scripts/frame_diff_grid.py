#!/usr/bin/env python3
"""Locate WHERE two videos differ on a given frame.

Reads no images from disk — it pipes raw RGB out of ffmpeg for both the
reference and the port at the same frame index, then computes a per-cell MSE
grid. Use this when SSIM says a frame is bad but you need to know whether the
delta lives in the bars region, the text region, or is a global tint shift.

Frame index is 1-based to match the ffmpeg `ssim` stats_file (`n:` field).
"""
from __future__ import annotations

import argparse
import subprocess
import sys
from pathlib import Path

import numpy as np


def frame_raw(path: str, n0: int, w: int, h: int) -> np.ndarray:
    cmd = [
        "ffmpeg", "-hide_banner", "-loglevel", "error",
        "-i", path,
        "-vf", f"select=eq(n\\,{n0}),scale={w}:{h}:flags=area",
        "-frames:v", "1",
        "-f", "rawvideo", "-pix_fmt", "rgb24", "-",
    ]
    raw = subprocess.run(cmd, capture_output=True, check=True).stdout
    arr = np.frombuffer(raw, dtype=np.uint8)
    if arr.size != w * h * 3:
        raise SystemExit(f"expected {w*h*3} bytes, got {arr.size} for {path} n={n0}")
    return arr.reshape(h, w, 3).astype(np.float32)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("ref")
    ap.add_argument("port")
    ap.add_argument("--frame", type=int, required=True, help="1-based frame index")
    ap.add_argument("--fps", type=float, default=30.0)
    ap.add_argument("-W", "--width", type=int, default=320)
    ap.add_argument("-H", "--height", type=int, default=180)
    ap.add_argument("--cols", type=int, default=8)
    ap.add_argument("--rows", type=int, default=6)
    args = ap.parse_args()

    n0 = args.frame - 1
    a = frame_raw(args.ref, n0, args.width, args.height)
    b = frame_raw(args.port, n0, args.width, args.height)
    # Per-pixel squared error, averaged over channels → (H, W)
    se = ((a - b) ** 2).mean(axis=2)

    cw = args.width // args.cols
    ch = args.height // args.rows
    grid = np.zeros((args.rows, args.cols), dtype=np.float32)
    for r in range(args.rows):
        for c in range(args.cols):
            block = se[r * ch:(r + 1) * ch, c * cw:(c + 1) * cw]
            grid[r, c] = block.mean()

    mse = (se.mean()) / (255.0 ** 2)  # normalized MSE in [0,1]
    psnr = float("inf") if mse == 0 else -10 * np.log10(mse)

    print(f"frame {args.frame} (t={n0/args.fps:.3f}s)  "
          f"global nMSE={mse:.5f}  PSNR={psnr:.1f}dB  "
          f"(grid {args.cols}x{args.rows}, each {cw}x{ch}px @ {args.width}x{args.height})")
    print()
    gmax = grid.max()
    # Each cell: raw MSE (0-65025) and a 0-9 intensity bucket.
    print("    " + " ".join(f"c{c:<2}" for c in range(args.cols)))
    for r in range(args.rows):
        cells = []
        for c in range(args.cols):
            v = grid[r, c]
            bucket = " " if gmax == 0 else int(round(v / gmax * 9))
            cells.append(f"{bucket}  ")
        y0 = r * ch / args.height
        print(f"r{r} " + " ".join(cells) + f"  y0={y0:.2f}")

    print()
    print("intensity 0 (identical) → 9 (worst cell on this frame). buckets relative to this frame's max.")
    # Top differing cells with normalized screen coords.
    flat = []
    for r in range(args.rows):
        for c in range(args.cols):
            flat.append((grid[r, c], r, c))
    flat.sort(reverse=True)
    print("\nworst cells (mse, screen-region):")
    for v, r, c in flat[:5]:
        x = (c + 0.5) / args.cols
        y = (r + 0.5) / args.rows
        region = ("left " if x < 0.4 else "right" if x > 0.6 else "ctr  ") + " " + \
                 ("top" if y < 0.4 else "bot" if y > 0.6 else "mid")
        print(f"  r{r} c{c}  mse={v:9.1f}  x={x:.2f} y={y:.2f}  {region}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
