#!/usr/bin/env python3
"""Coarse time-alignment scan between a reference video and our render.

For sampled reference frames, find the best-matching test frame in a +/-window
and report the offset. A near-constant offset means the reference is a different
cut (frame-index diff is meaningless); offsets ~0 mean the diff is real.

Usage: align_scan.py REF TEST [--step 60] [--window 90]
"""
import argparse
import subprocess
import sys

import numpy as np

W = H = 96


def frames(path, limit):
    p = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-i", path, "-vf", f"scale={W}:{H}",
         "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        stdout=subprocess.PIPE,
    )
    nb = W * H * 3
    out = []
    try:
        while len(out) < limit:
            buf = p.stdout.read(nb)
            if len(buf) < nb:
                break
            out.append(np.frombuffer(buf, np.uint8).reshape(H, W, 3).astype(np.int16))
    finally:
        p.stdout.close()
        p.wait()
    return np.stack(out)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref")
    ap.add_argument("test")
    ap.add_argument("--step", type=int, default=60)
    ap.add_argument("--window", type=int, default=90)
    args = ap.parse_args()
    ref = frames(args.ref, 10**9)
    test = frames(args.test, 10**9)
    print(f"ref frames={len(ref)} test frames={len(test)}")
    for ri in range(0, len(ref), args.step):
        d = np.abs(test - ref[ri]).mean(axis=(1, 2, 3))
        lo, hi = max(0, ri - args.window), min(len(test), ri + args.window)
        best = lo + int(np.argmin(d[lo:hi]))
        print(f"ref f{ri:5d} ({ri/30:6.2f}s) -> test f{best:5d} "
              f"({best/30:6.2f}s) off={best-ri:+4d} mae={d[best]:5.1f} "
              f"same={d[ri] if ri < len(d) else float('nan'):5.1f}")


if __name__ == "__main__":
    sys.exit(main())
