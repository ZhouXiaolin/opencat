#!/usr/bin/env python3
"""Compare a rendered PNG frame segment against a reference video.

Pairs `frame_NNNN.png` files from `--frames` renders with the same frame index
of the reference video, prints per-frame mae / p8 (same definitions as
k3diff.py: p8 = fraction of pixels whose max-channel delta exceeds 8) and
writes side-by-side `pair-NNNN.png` (ref | test | heat) when `--out` is given.

Usage:
  frame_pair.py REF_VIDEO PNG_DIR [--out DIR] [--shift N]
"""
import argparse
import glob
import os
import re
import subprocess

import numpy as np
from PIL import Image


def ref_frames(path, indices, w, h):
    expr = "+".join(f"eq(n\\,{i})" for i in indices)
    p = subprocess.run(
        ["ffmpeg", "-v", "error", "-i", path, "-vf", f"select='{expr}'",
         "-fps_mode", "passthrough", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        stdout=subprocess.PIPE, check=True,
    )
    n = w * h * 3
    buf = p.stdout
    return [np.frombuffer(buf[k * n:(k + 1) * n], np.uint8).reshape(h, w, 3)
            for k in range(len(buf) // n)]


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref")
    ap.add_argument("pngdir")
    ap.add_argument("--out", default="")
    ap.add_argument("--shift", type=int, default=0, help="ref index = test index + shift")
    args = ap.parse_args()

    files = sorted(glob.glob(os.path.join(args.pngdir, "frame_*.png")))
    idx = [int(re.search(r"frame_(\d+)", f).group(1)) for f in files]
    tests = [np.asarray(Image.open(f).convert("RGB")) for f in files]
    h, w = tests[0].shape[:2]
    refs = ref_frames(args.ref, [i + args.shift for i in idx], w, h)
    if args.out:
        os.makedirs(args.out, exist_ok=True)
    p8s = []
    for i, t, r in zip(idx, tests, refs):
        d = np.abs(t.astype(np.int16) - r.astype(np.int16))
        dm = d.max(axis=2)
        mae = d.mean()
        p8 = (dm > 8).mean()
        p8s.append(p8)
        print(f"f{i:05d} mae={mae:7.3f} p8={p8 * 100:6.2f}%")
        if args.out:
            heat = np.zeros_like(t)
            heat[..., 0] = np.clip(dm.astype(np.float32) * 8, 0, 255).astype(np.uint8)
            Image.fromarray(np.concatenate([r, t, heat], axis=1)).save(
                os.path.join(args.out, f"pair-{i:04d}.png"))
    if p8s:
        print(f"avg p8={np.mean(p8s) * 100:.2f}% over {len(p8s)} frames")


if __name__ == "__main__":
    main()
