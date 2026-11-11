#!/usr/bin/env python3
"""Local shift field: split frames into windows, NCC-shift each window of
TEST relative to REF. Print grid. Usage: shift_field.py REF TEST FRAME [--win 320]"""
import sys
import numpy as np
from PIL import Image

def local_shift(ref, tst, search=10):
    g0 = ref.mean(axis=2); g1 = tst.mean(axis=2)
    g0 = g0 - g0.mean(); g1 = g1 - g1.mean()
    best = (1e18, 0, 0)
    H, W = g0.shape
    for dy in range(-search, search+1):
        for dx in range(-search, search+1):
            y0a, y1a = max(0,dy), min(H, H+dy)
            x0a, x1a = max(0,dx), min(W, W+dx)
            a = g0[y0a:y1a, x0a:x1a]
            b = g1[y0a-dy:y1a-dy, x0a-dx:x1a-dx]
            d = float(((a-b)**2).mean())
            if d < best[0]:
                best = (d, dx, dy)
    return best[1], best[2]

def main():
    ref_dir, tst_dir, frame = sys.argv[1], sys.argv[2], int(sys.argv[3])
    win = int(sys.argv[4]) if len(sys.argv) > 4 else 320
    a = np.asarray(Image.open(f"{ref_dir}/frame_{frame:04d}.png").convert("RGB"), np.float32)
    b = np.asarray(Image.open(f"{tst_dir}/frame_{frame:04d}.png").convert("RGB"), np.float32)
    H, W = a.shape[:2]
    ny, nx = 3, 6
    hy, hx = H // ny, W // nx
    print("shift of TEST vs REF per window (dx,dy,px):")
    for iy in range(ny):
        row = []
        for ix in range(nx):
            y0, x0 = iy*hy, ix*hx
            ra = a[y0:y0+hy, x0:x0+hx]
            ba = b[y0:y0+hy, x0:x0+hx]
            dx, dy = local_shift(ra, ba)
            row.append(f"({dx:+3d},{dy:+3d})")
        print(" ".join(row))

main()
