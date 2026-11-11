#!/usr/bin/env python3
"""Dual-metric alignment report: perceptual + strict, per frame.

Metrics per frame (test vs ref):
  p8      strict: fraction of pixels with max-channel |diff| > 8 (k3diff 口径)
  d4      perceptual: same threshold after 4x4 BOX downsample (感知代理)
  ssim    perceptual: mean 8x8-window SSIM on grayscale
  shift   best integer (dx,dy) in [-N,N] minimizing p8 — 结构位置差;
          shift=(0,0) 且 d4 低 = 位置形态一致（肉眼一致判据）

Usage:
  align_metrics.py REF REF_STYLE TEST TEST_STYLE --start A --end B [--size 1920x1080] [--shift 8]
  REF/TEST each is a PNG dir (frame_%04d.png) or a video path.

PNG dirs use frame_%04d.png naming (opencat --frames output).
"""
import argparse, os, subprocess
import numpy as np


def load_idx(path, idx, w, h):
    if os.path.isdir(path):
        p = os.path.join(path, f"frame_{idx:04d}.png")
        from PIL import Image
        return np.asarray(Image.open(p).convert("RGB").resize((w, h), Image.BOX), np.float32)
    expr = f"eq(n\\,{idx})"
    r = subprocess.run(["ffmpeg", "-v", "error", "-i", path, "-vf", f"select='{expr}',scale={w}:{h}",
                        "-fps_mode", "passthrough", "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
                       stdout=subprocess.PIPE, check=True)
    a = np.frombuffer(r.stdout, np.uint8)
    return a.reshape(h, w, 3).astype(np.float32)


def down(a, bs):
    h, w = a.shape[:2]
    return a.reshape(h // bs, bs, w // bs, bs, a.shape[2]).mean(axis=(1, 3))


def p8(a, b):
    d = np.abs(a - b).max(axis=2)
    return float((d > 8).mean())


def ssim_gray(a, b, win=8):
    ga = a.mean(axis=2); gb = b.mean(axis=2)
    H, W = ga.shape
    c1 = (0.01 * 255) ** 2; c2 = (0.03 * 255) ** 2
    vals = []
    for y in range(0, H - win + 1, win):
        for x in range(0, W - win + 1, win):
            pa = ga[y:y + win, x:x + win]; pb = gb[y:y + win, x:x + win]
            ma = pa.mean(); mb = pb.mean()
            va = pa.var(); vb = pb.var()
            cov = ((pa - ma) * (pb - mb)).mean()
            vals.append(((2 * ma * mb + c1) * (2 * cov + c2)) /
                        ((ma * ma + mb * mb + c1) * (va + vb + c2)))
    return float(np.mean(vals))


def best_shift(a, b, search=8):
    best = (0, 0, p8(a, b))
    if best[2] == 0.0:
        return best
    H, W = a.shape[:2]
    for dy in range(-search, search + 1):
        for dx in range(-search, search + 1):
            if dx == 0 and dy == 0:
                continue
            ya0, ya1 = max(0, dy), min(H, H + dy)
            xa0, xa1 = max(0, dx), min(W, W + dx)
            yb0, yb1 = max(0, -dy), min(H, H - dy)
            xb0, xb1 = max(0, -dx), min(W, W - dx)
            v = p8(a[ya0:ya1, xa0:xa1], b[yb0:yb1, xb0:xb1])
            if v < best[2]:
                best = (dx, dy, v)
            if v == 0.0:
                return best
    return best


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref"); ap.add_argument("test")
    ap.add_argument("--start", type=int, required=True)
    ap.add_argument("--end", type=int, required=True)
    ap.add_argument("--size", default="1920x1080")
    ap.add_argument("--shift", type=int, default=8)
    args = ap.parse_args()
    w, h = map(int, args.size.split("x"))
    print("frame |   p8%%   d4%%   ssim  shift(dx,dy,p8%%)")
    rows = []
    for i in range(args.start, args.end):
        fr = load_idx(args.ref, i, w, h)
        ft = load_idx(args.test, i, w, h)
        v8 = p8(fr, ft)
        d4 = p8(down(fr, 4), down(ft, 4))
        ss = ssim_gray(fr, ft)
        bx, by, bv = best_shift(fr, ft, args.shift)
        rows.append((i, v8, d4, ss, bx, by, bv))
        print(f"f{i:04d} | {v8*100:6.2f} {d4*100:6.2f} {ss:7.4f}  ({bx:+3d},{by:+3d},{bv*100:6.2f})")
    n = len(rows)
    if n:
        print(f"AVG   | {sum(r[1] for r in rows)/n*100:6.2f} {sum(r[2] for r in rows)/n*100:6.2f} "
              f"{sum(r[3] for r in rows)/n:7.4f}")


if __name__ == "__main__":
    main()
