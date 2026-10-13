#!/usr/bin/env python3
"""Build comparison images for eyeballing: ref vs test stacked, optional crop.

Usage:
  cmp.py REF.png TEST.png OUT.png [--crop x,y,w,h] [--scale S] [--gap G]
         [--diff]     # add a third panel: amplified abs-diff heat
  cmp.py --video REF.mp4 TEST.mp4 OUT.png --frame N [--crop ...] [--scale S]
  cmp.py REF.mp4 TEST.mp4 OUT.png --frames 60,150,168
         # contact sheet: one row per frame, columns ref | test | diff-over-ref
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


def overlay(ref, dm, thresh=16):
    """Reference frame with mismatching pixels flagged in red.

    Red where the max-channel delta exceeds thresh. Yellow where it does not.
    Use for a contact sheet: one look tells where the two videos disagree.
    """
    rgb = np.asarray(ref).astype(np.float32).copy()
    m = dm > thresh
    rgb[m] = 0.25 * rgb[m] + 0.75 * np.array([255, 0, 0], np.float32)
    return Image.fromarray(rgb.astype(np.uint8))


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref")
    ap.add_argument("test")
    ap.add_argument("out")
    ap.add_argument("--frame", type=int, default=None)
    ap.add_argument("--frames", default="", help="comma list -> contact sheet")
    ap.add_argument("--thresh", type=int, default=16, help="red flag threshold")
    ap.add_argument("--crop", default="", help="x,y,w,h")
    ap.add_argument("--scale", type=float, default=1.0)
    ap.add_argument("--gap", type=int, default=6)
    ap.add_argument("--diff", action="store_true")
    a = ap.parse_args()

    if a.frames:
        frames = [int(x) for x in a.frames.split(",") if x != ""]
        pngs = None
        if not a.test.endswith(".mp4"):
            cand = a.test.split(",")
            pngs = cand if len(cand) == len(frames) else None
        cells = []
        for fi, n in enumerate(frames):
            ref = grab(a.ref, n)
            if a.test.endswith(".mp4"):
                test = grab(a.test, n)
            else:
                test = Image.open(pngs[fi] if pngs else a.test).convert("RGB")
            if a.crop:
                x, y, w, h = (int(v) for v in a.crop.split(","))
                box = (x, y, x + w, y + h)
                ref, test = ref.crop(box), test.crop(box)
            d = np.abs(np.asarray(ref).astype(np.int16) - np.asarray(test).astype(np.int16)).max(2)
            th = a.thresh
            tag = (f"f{n}  p8={float((d > 8).mean()):.4f} p16={float((d > 16).mean()):.4f} "
                   f"p>{th}={float((d > th).mean()):.3f} mae={float(d.mean()):.2f}")
            print(tag)
            cells.append([(ref, tag), (test, ""), (overlay(ref, d, th), "")])
        if a.scale != 1.0:
            for row in cells:
                for k, (p, _) in enumerate(row):
                    row[k] = (p.resize((int(p.width * a.scale), int(p.height * a.scale))), row[k][1])
        cw = max(p.width for row in cells for p, _ in row)
        chh = max(p.height for row in cells for p, _ in row)
        g = a.gap
        canvas = Image.new("RGB", (3 * cw + 2 * g, len(cells) * (chh + 22) + (len(cells) - 1) * g), (40, 0, 60))
        from PIL import ImageDraw
        dr = ImageDraw.Draw(canvas)
        for r, row in enumerate(cells):
            y = r * (chh + 22 + g)
            for c, (p, tag) in enumerate(row):
                canvas.paste(p, (c * (cw + g), y + 22))
            dr.text((4, y + 5), f"ref | test | diff-over-ref    {row[0][1]}", fill=(255, 255, 255))
        canvas.save(a.out)
        print(f"{a.out}  {canvas.width}x{canvas.height}  frames={frames}")
        return

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
