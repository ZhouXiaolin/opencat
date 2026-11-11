#!/usr/bin/env python3
"""Shifted video comparison: test frame (n + shift) vs ref frame n, sampled.

For references cut from a different edit (claude-design-send's swgl render
drops the first ~2.1s), same-index k3diff is meaningless; this reports the
p8/mae under a fixed frame offset.

Usage:
  shift_diff.py REF TEST --shift N [--step 10]
"""
import argparse
import subprocess

import numpy as np


def frames(path, w, h, step):
    p = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-i", path, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        stdout=subprocess.PIPE,
    )
    n = w * h * 3
    i = 0
    try:
        while True:
            buf = p.stdout.read(n)
            if len(buf) < n:
                break
            if i % step == 0:
                yield i, np.frombuffer(buf, np.uint8).reshape(h, w, 3)
            i += 1
    finally:
        p.stdout.close()
        p.wait()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref")
    ap.add_argument("test")
    ap.add_argument("--shift", type=int, required=True)
    ap.add_argument("--step", type=int, default=10)
    ap.add_argument("--size", default="1920x1080")
    args = ap.parse_args()
    w, h = map(int, args.size.split("x"))
    # Sample on the ref grid; test frames are pulled at ref index + shift.
    want = {}
    for i, f in frames(args.ref, w, h, args.step):
        want[i + args.shift] = f
    p8s, maes = [], []
    for j, t in frames(args.test, w, h, 1):
        r = want.get(j)
        if r is None:
            continue
        d = np.abs(t.astype(np.int16) - r.astype(np.int16))
        p8s.append((d.max(axis=2) > 8).mean())
        maes.append(d.mean())
    print(f"shift={args.shift} frames={len(p8s)} avg_p8={np.mean(p8s) * 100:.3f}% mae={np.mean(maes):.3f}")


if __name__ == "__main__":
    main()
