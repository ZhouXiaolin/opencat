#!/usr/bin/env python3
"""Pixel-level frame differ for two videos.

Streams both videos as raw rgb24 via ffmpeg (no disk), compares every pixel,
and reports hard pixel metrics (no SSIM):

  mae      mean absolute error across R,G,B (0..255)
  maxd     max per-pixel channel delta in the frame
  p<T>     fraction of pixels whose max-channel delta exceeds T
            (T in 0,2,4,8,16,32,64,128)
  npx<T>   absolute pixel count above T
  bbox     bounding box (x0,y0,x1,y1) of pixels above --bbox-thresh

Usage:
  k3diff.py REF TEST [--out DIR] [--dump f1,f2,...] [--sheet STEP]
"""
import argparse
import json
import os
import subprocess
import sys

import numpy as np
from PIL import Image

THRESHOLDS = [0, 2, 4, 8, 16, 32, 64, 128]
GRID = (16, 9)  # cols, rows for region localization


def decode(path, w, h):
    p = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-i", path, "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        stdout=subprocess.PIPE,
    )
    n = w * h * 3
    try:
        while True:
            buf = p.stdout.read(n)
            if len(buf) < n:
                break
            yield np.frombuffer(buf, np.uint8).reshape(h, w, 3)
    finally:
        p.stdout.close()
        p.wait()


def probe_nframes(path):
    out = subprocess.check_output(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-count_frames",
         "-show_entries", "stream=nb_read_frames,width,height", "-of", "json", path],
        text=True,
    )
    s = json.loads(out)["streams"][0]
    return int(s["nb_read_frames"]), int(s["width"]), int(s["height"])


def heat(dm):
    """Amplified grayscale heatmap: 0 -> black, 255 -> red-ish."""
    hi = np.clip(dm.astype(np.float32) * 8.0, 0, 255).astype(np.uint8)
    rgb = np.zeros(dm.shape + (3,), np.uint8)
    rgb[..., 0] = hi
    rgb[..., 1] = np.clip(dm.astype(np.float32) * 2.0, 0, 255).astype(np.uint8)
    return rgb


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("ref")
    ap.add_argument("test")
    ap.add_argument("--out", required=True)
    ap.add_argument("--dump", default="")
    ap.add_argument("--sheet", type=int, default=0, help="contact-sheet every N frames")
    ap.add_argument("--bbox-thresh", type=int, default=16)
    ap.add_argument("--size", default="", help="WxH override")
    args = ap.parse_args()

    os.makedirs(args.out, exist_ok=True)
    nref, w, h = probe_nframes(args.ref)
    ntest, w2, h2 = probe_nframes(args.test)
    if args.size:
        w, h = (int(v) for v in args.size.lower().split("x"))
    elif (w, h) != (w2, h2):
        print(f"size mismatch ref {w}x{h} test {w2}x{h2}", file=sys.stderr)
        return 2
    n = min(nref, ntest)
    print(f"ref frames={nref} test frames={ntest} comparing={n} @ {w}x{h}")

    dump = {int(x) for x in args.dump.split(",") if x != ""}
    gh, gw = GRID[1], GRID[0]
    ch, cw = h // gh, w // gw

    rows = []
    sheet = []
    for i, (a, b) in enumerate(zip(decode(args.ref, w, h), decode(args.test, w, h))):
        d = np.abs(a.astype(np.int16) - b.astype(np.int16))
        dm = d.max(2)
        rec = {
            "frame": i,
            "mae": float(d.mean()),
            "maxd": int(dm.max()),
        }
        for t in THRESHOLDS:
            rec[f"p{t}"] = float((dm > t).mean()) if t > 0 else float((dm > 0).mean())
        m = dm > args.bbox_thresh
        if m.any():
            ys, xs = np.where(m)
            rec["bbox"] = [int(xs.min()), int(ys.min()), int(xs.max()), int(ys.max())]
            # coarse per-cell error to localize
            grid = dm.reshape(gh, ch, gw, cw).mean((1, 3))
            rec["grid_max_cell"] = [int(grid.argmax() % gw), int(grid.argmax() // gw)]
        else:
            rec["bbox"] = None
        rows.append(rec)

        if i in dump:
            Image.fromarray(a).save(f"{args.out}/f{i:04d}-ref.png")
            Image.fromarray(b).save(f"{args.out}/f{i:04d}-test.png")
            Image.fromarray(heat(dm)).save(f"{args.out}/f{i:04d}-diff.png")
        if args.sheet and i % args.sheet == 0:
            sheet.append(Image.fromarray(heat(dm)).resize((w // 4, h // 4)))

    # csv
    keys = ["frame", "mae", "maxd"] + [f"p{t}" for t in THRESHOLDS[1:]]
    with open(f"{args.out}/pixel.csv", "w") as f:
        f.write(",".join(keys) + ",bbox\n")
        for r in rows:
            f.write(",".join(str(r.get(k, "")) for k in keys) + f",{r['bbox']}\n")

    order = sorted(rows, key=lambda r: r["p8"], reverse=True)
    print("\n=== worst 25 frames by p8 (pixels changed > 8) ===")
    print(f"{'frame':>6} {'mae':>7} {'maxd':>5} {'p0':>7} {'p8':>7} {'p16':>7} {'p32':>7} {'bbox'}")
    for r in order[:25]:
        print(f"{r['frame']:>6} {r['mae']:>7.3f} {r['maxd']:>5} {r['p0']:>7.4f} "
              f"{r['p8']:>7.4f} {r['p16']:>7.4f} {r['p32']:>7.4f} {r['bbox']}")

    agg = {
        "frames": n,
        "mae_mean": float(np.mean([r["mae"] for r in rows])),
        "maxd_max": int(max(r["maxd"] for r in rows)),
    }
    for t in THRESHOLDS[1:]:
        agg[f"avg_p{t}"] = float(np.mean([r[f"p{t}"] for r in rows]))
    print("\n=== aggregate ===")
    print(json.dumps(agg, indent=2))
    with open(f"{args.out}/summary.json", "w") as f:
        json.dump({"agg": agg, "rows": rows}, f, indent=1)

    if sheet:
        cols = 4
        rows_n = (len(sheet) + cols - 1) // cols
        sw, sh = sheet[0].size
        canvas = Image.new("RGB", (cols * sw, rows_n * sh))
        for k, im in enumerate(sheet):
            canvas.paste(Image.fromarray(im), ((k % cols) * sw, (k // cols) * sh))
        canvas.save(f"{args.out}/sheet.png")
        print(f"sheet -> {args.out}/sheet.png")

    print(f"\nartifacts -> {args.out}/pixel.csv, summary.json")


if __name__ == "__main__":
    sys.exit(main())
