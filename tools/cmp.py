#!/usr/bin/env python3
"""Build comparison images for eyeballing: ref vs test stacked, optional crop.

Usage:
  cmp.py REF.png TEST.png OUT.png [--crop x,y,w,h] [--scale S] [--gap G]
         [--diff]     # add a third panel: amplified abs-diff heat
  cmp.py --video REF.mp4 TEST.mp4 OUT.png --frame N [--crop ...] [--scale S]
"""
import argparse
import subprocess

import numpy as np
from PIL import Image


def grab(video, n, w=None, h=None):
    # extract single frame as png bytes
    out = subprocess.check_output(
        ["ffmpeg", "-v", "error", "-i", video, "-vf", f"select=eq(n\\,{n})",
         "-vframes", "1", "-f", "image2pipe", "-vcodec", "png", "-"]
    )
    import io
    return Image.open(io.BytesIO(out)).convert("RGB")


def heat(dm, amp=8.0):
    hi = np.clip(dm.astype(np.float32) * amp, 0, 255).astype(np.uint8)
    rgb = np.zeros(dm.shape + (3,), np.uint8)
    rgb[..., 0] = hi
    rgb[..., 1] = np.clip(dm.astype(np.float32) * 2.0, 0, 255).astype(np.uint8)
    return Image.fromarray(rgb)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref")
    ap.add_argument("test")
    ap.add_argument("out")
    ap.add_argument("--frame", type=int, default=None)
    ap.add_argument("--crop", default="", help="x,y,w,h")
    ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--gap", type=int, default=6)
    ap.add_argument("--diff", action="store_true")
    a = ap.parse_args()

    if a.frame is not None or a.ref.endswith(".mp4"):
        ref = grab(a.ref, a.frame or 0)
        test = grab(a.test, a.frame or 0)
    else:
        ref = Image.open(a.ref).convert("RGB")
        test = Image.open(a.test).convert("RGB")

    if a.crop:
        x, y, w, h = (int(v) for v in a.crop.split(","))
        ref = ref.crop((x, y, x + w, y + h))
        test = test.crop((x, y, x + w, y + h))

    panels = [ref, test]
    if a.diff:
        d = np.abs(np.asarray(ref).astype(np.int16) - np.asarray(test).astype(np.int16)).max(2)
        panels.append(heat(d))

    if a.scale != 1.0:
        panels = [p.resize((int(p.width * a.scale), int(p.height * a.scale))) for p in panels]
        a.gap = int(a.gap * a.scale)

    W = max(p.width for p in panels)
    H = sum(p.height for p in panels) + a.gap * (len(panels) - 1)
    canvas = Image.new("RGB", (W, H), (40, 0, 60))
    yy = 0
    for p in panels:
        canvas.paste(p, (0, yy))
        yy += p.height + a.gap
    canvas.save(a.out)
    print(f"{a.out}  {W}x{H}")


if __name__ == "__main__":
    main()
