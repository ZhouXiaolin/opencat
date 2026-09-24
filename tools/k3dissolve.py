#!/usr/bin/env python3
"""k3-promo scene-H scramble dissolve — offline exact replay.

Transcribes the reference's drawDissolve() (hyperframes-launches/k3-promo/
index.html:1240-1523) plus the raster builder rebuildDissolveField()
(:1254-1314) into Python/numpy, and emits one RGBA PNG per frame
(411..455) to assets/k3dis/fNNN.png. The XML then blits the right PNG per
frame via canvas drawImage — no engine pixel readback needed.

Everything here is a pure deterministic function of the frame index:
  - card raster mask ("K3. Now Open" + braces) rendered with the SAME
    Inter fonts the reference renderer aliases (assets/Inter-*.ttf are
    instanced from the reference project's own Inter-Variable.woff2),
  - chamfer 3-4 distance transform,
  - bandP lit-probability table (index.html:1323-1331),
  - h01 pixel hash with 7-frame temporal lerp (:1316-1321, :1457),
  - scanline banding, dust phase, additive y=540 glow band.

Usage:
  python3 tools/k3dissolve.py --out assets/k3dis      # all frames
  python3 tools/k3dissolve.py --frame 411 --preview   # single frame + stats
"""
import argparse
import math
import os

import numpy as np
from PIL import Image, ImageDraw, ImageFont
from fontTools.ttLib import TTFont

# ---------------------------------------------------------------- constants
DIS_X0, DIS_Y0, DIS_W, DIS_H = 232, 430, 1460, 222
CARD_CAP, CARD_TOP = 108, 478          # index.html:1177
FGB_H, FGB_TOP = 119, 480              # index.html:1175
GAP1, WORD_NOW_W, GAP2 = 55, 298, 49   # index.html:1421 G run gaps
G_CENTER = 962.5                       # index.html:1420
OLD_W = {"K3.": 260, "Open": 245}      # index.html:1417-1418 swapW targets
OLD_STR = {"K3.": "JTX.", "Open": "Live"}
LS = -2.5                              # letter-spacing px (index.html:1263)
REG_TTF = "assets/Inter-Regular.ttf"
LGT_TTF = "assets/Inter-Light.ttf"
UPM = 2048

# bandP table (index.html:1323-1331): frame -> [d<1, <4, <8, <16, <24, bg]
DIS_TAB = [
    (412, [0.97, 0.88, 0.74, 0.34, 0.06, 0.010]),
    (414, [0.97, 0.88, 0.84, 0.56, 0.19, 0.033]),
    (425, [0.94, 0.83, 0.80, 0.54, 0.21, 0.033]),
    (435, [0.87, 0.77, 0.74, 0.51, 0.23, 0.036]),
    (441, [0.83, 0.735, 0.70, 0.49, 0.24, 0.036]),
    (442, [0.76, 0.66, 0.61, 0.41, 0.19, 0.036]),
    (443, [0.60, 0.47, 0.37, 0.23, 0.10, 0.030]),
]


def band_p(f):
    if f <= DIS_TAB[0][0]:
        return np.array(DIS_TAB[0][1])
    for i in range(len(DIS_TAB) - 1):
        if f <= DIS_TAB[i + 1][0]:
            a, b = DIS_TAB[i], DIS_TAB[i + 1]
            w = (f - a[0]) / (b[0] - a[0])
            return np.array([v + (bv - v) * w for v, bv in zip(a[1], b[1])])
    return np.array(DIS_TAB[-1][1])


def streak_env(f):
    if f < 416 or f > 439:
        return 0.0
    if f < 420:
        return 0.42 + 0.145 * (f - 416)
    if f <= 436:
        return 1.0
    return {437: 0.58, 438: 0.33}.get(f, 0.125)


# JS Math.imul / h01 (index.html:1316-1321) in exact 32-bit semantics.
# uint32 values are held in uint64 lanes so XOR / logical >> are exact.
M32 = 0xFFFFFFFF


def imul(a, b):
    return (np.asarray(a, dtype=np.uint64) * np.asarray(b, dtype=np.uint64)) & M32


def h01(a, b, c):
    ua = (np.asarray(a, dtype=np.uint64) + np.uint64(374761)) & M32
    ub = (np.asarray(b, dtype=np.uint64) + np.uint64(668265)) & M32
    uc = (np.asarray(c, dtype=np.uint64) + np.uint64(951274)) & M32
    h = (imul(ua, 0x9E3779B1) ^ imul(ub, 0x85EBCA6B) ^ imul(uc, 0xC2B2AE35)) & M32
    h = imul(h ^ (h >> np.uint64(16)), 0x27D4EB2F) & M32
    h = (h ^ (h >> np.uint64(15))) & M32
    return h.astype(np.float64) / 4294967296.0


# ---------------------------------------------------------------- raster
def _advances(font, text):
    cmap = font.getBestCmap()
    hmtx = font["hmtx"]
    return [hmtx[cmap[ord(ch)]][0] for ch in text]


def build_mask(ls_trailing=True):
    """Render the card lockup alpha into a (DIS_H, DIS_W) uint8 array.

    Mirrors rebuildDissolveField (index.html:1254-1277). Word ink boxes are
    BAKED from the reference video's f411 crisp frame (the raster's own
    development; measured with tools/k3dissolve.py --probe) instead of the
    swapW formula — the browser's measureText advance/letter-spacing
    semantics are not exactly reproducible from font metrics, and f411 is
    the ground truth the dissolve must match anyway.
    """
    ft = TTFont(REG_TTF)
    fl = TTFont(LGT_TTF)
    cap_asc = ft["OS/2"].sCapHeight / UPM              # 1490/2048
    gs = fl.getGlyphSet()
    from fontTools.pens.boundsPen import BoundsPen
    bg = fl.getBestCmap()[ord("{")]
    bp = BoundsPen(gs)
    gs[bg].draw(bp)
    (bx0, by0, bx1, by1) = bp.bounds
    b_asc, b_desc = by1 / UPM, -by0 / UPM

    fs_t = CARD_CAP / cap_asc
    base = CARD_TOP + CARD_CAP                          # 586, alphabetic baseline
    fs_b = FGB_H / (b_asc + b_desc)
    b_base = FGB_TOP + fs_b * b_asc

    def nat(s, fs):
        adv_px = sum(_advances(ft, s)) * fs / UPM
        n = len(s)
        return adv_px + (n if ls_trailing else n - 1) * LS

    fs_c = CARD_CAP / cap_asc
    w_k3 = OLD_W["K3."] * nat("K3.", fs_c) / nat(OLD_STR["K3."], fs_c)
    w_open = OLD_W["Open"] * nat("Open", fs_c) / nat(OLD_STR["Open"], fs_c)
    g_start = G_CENTER - (w_k3 + GAP1 + WORD_NOW_W + GAP2 + w_open) / 2
    # ink boxes measured off reference f411 (screen coords, threshold 90)
    words = [
        ("K3.", 516.0, 167.0),
        ("Now", 758.0, 283.0),
        ("Open", 1100.0, 393.0),
    ]

    pad = 32
    img = Image.new("L", (DIS_W, DIS_H), 0)
    draw = ImageDraw.Draw(img)

    font_t = ImageFont.truetype(REG_TTF, fs_t)
    for word, ink_left, ink_w in words:
        n = nat(word, fs_t)
        tmp_w = int(math.ceil(n)) + 2 * pad
        tmp_h = int(math.ceil(fs_t * 2.2)) + 2 * pad
        yb = tmp_h - int(math.ceil(fs_t * 1.2))         # baseline row in tmp
        tmp = Image.new("L", (tmp_w, tmp_h), 0)
        td = ImageDraw.Draw(tmp)
        x = float(pad)
        for ch in word:
            td.text((x, yb), ch, font=font_t, fill=255, anchor="ls")
            x += _advances(ft, ch)[0] * fs_t / UPM + LS
        # crop to ink (threshold 8 keeps the AA skirt), squeeze to ink_w
        ta = np.asarray(tmp)
        iys, ixs = np.where(ta > 8)
        x0, x1 = int(ixs.min()), int(ixs.max()) + 1
        y0, y1 = int(iys.min()), int(iys.max()) + 1
        ink = tmp.crop((x0, y0, x1, y1))
        scale = ink_w / (x1 - x0)
        new_w = max(1, int(round((x1 - x0) * scale)))
        ink = ink.resize((new_w, y1 - y0), Image.BILINEAR)
        px = int(round(ink_left - DIS_X0))
        py = int(round(base - DIS_Y0)) - (yb - y0)
        img.paste(ink, (px, py))

    font_b = ImageFont.truetype(LGT_TTF, fs_b)
    for ch, ink_left in (("{", 400.0), ("}", 1469.0)):
        tmp = Image.new("L", (int(math.ceil(fs_b)) + 2 * pad, int(math.ceil(fs_b * 2.2))), 0)
        td = ImageDraw.Draw(tmp)
        td.text((pad, tmp.height - pad), ch, font=font_b, fill=255, anchor="ls")
        ta = np.asarray(tmp)
        iys, ixs = np.where(ta > 8)
        ink = tmp.crop((int(ixs.min()), int(iys.min()), int(ixs.max()) + 1, int(iys.max()) + 1))
        px = int(round(ink_left - DIS_X0))
        # ink top = FGB_TOP (= b_base - asc_px); ref f411 measures y 479..598
        py = int(round(b_base - DIS_Y0 - fs_b * b_asc))
        img.paste(ink, (px, py))

    return np.asarray(img, dtype=np.uint8), (w_k3, w_open, g_start)


def pad_scaled(scale, pad):
    return int(round(pad * scale))


def chamfer(mask):
    """3-4 chamfer distance transform (index.html:1278-1313). Units: px*3."""
    h, w = mask.shape
    big = np.int32(3000)
    dist = np.where(mask > 120, 0, big).astype(np.int32)
    # forward pass
    for y in range(h):
        row = dist[y]
        if y > 0:
            up = dist[y - 1]
            row[:] = np.minimum(row, up + 3)
            left_up = np.empty_like(up)
            left_up[0] = up[0]
            left_up[1:] = up[:-1]
            row[:] = np.minimum(row, left_up + 4)
            right_up = np.empty_like(up)
            right_up[-1] = up[-1]
            right_up[:-1] = up[1:]
            row[:] = np.minimum(row, right_up + 4)
        left = np.empty_like(row)
        left[0] = row[0]
        left[1:] = row[:-1]
        row[:] = np.minimum(row, left + 3)
    # backward pass
    for y in range(h - 1, -1, -1):
        row = dist[y]
        if y < h - 1:
            dn = dist[y + 1]
            row[:] = np.minimum(row, dn + 3)
            left_dn = np.empty_like(dn)
            left_dn[0] = dn[0]
            left_dn[1:] = dn[:-1]
            row[:] = np.minimum(row, left_dn + 4)
            right_dn = np.empty_like(dn)
            right_dn[-1] = dn[-1]
            right_dn[:-1] = dn[1:]
            row[:] = np.minimum(row, right_dn + 4)
        right = np.empty_like(row)
        right[-1] = row[-1]
        right[:-1] = row[1:]
        row[:] = np.minimum(row, right + 3)
    return dist


def render_frame(f, mask, dist):
    """Exact replay of drawDissolve (index.html:1448-1523) for frame f."""
    ep = f // 7
    fr = (f % 7) / 7.0
    scramble = f >= 412
    dust = f >= 444
    dust_base = 0.40 if (dust and f == 444) else (0.32 * max(0, 455 - f) / 10.0 if dust else 0.0)
    s_env = streak_env(f)
    sx_l = 240 + (f - 436) * 18 if f >= 437 else 240
    sx_r = 1686 - (f - 436) * 18 if f >= 437 else 1686

    ys = np.arange(DIS_Y0, DIS_Y0 + DIS_H, dtype=np.int64)
    xs = np.arange(DIS_X0, DIS_X0 + DIS_W, dtype=np.int64)
    py = ys[:, None] * np.ones(DIS_W, dtype=np.int64)
    pxx = xs[None, :] * np.ones(DIS_H, dtype=np.int64)[:, None]
    cx = (pxx // 3).astype(np.uint64)

    d = dist.astype(np.float64) / 3.0
    v = np.zeros((DIS_H, DIS_W), dtype=np.float64)

    if not scramble:
        a = mask.astype(np.float64)
        v = np.where(a > 10, 214.0 * a / 255.0, 0.0)
    else:
        nz = h01(cx, py, ep) * (1 - fr) + h01(cx, py, ep + 1) * fr
        if dust:
            with np.errstate(invalid="ignore"):
                p = np.where(d < 24, dust_base * (1 - 0.28 * d / 24.0), 0.03 * max(0, 455 - f) / 10.0)
        else:
            bi = np.digitize(d, [1, 4, 8, 16, 24])      # 0..5
            p = band_p(f)[bi]
            p = np.where((bi == 5) & ((pxx < 402) | (pxx > 1548)), 0.0, p)
            rb = py % 3
            # scanline banding (index.html:1489-1490): core vs fringe rows
            mul = np.where(
                bi < 2,
                np.where(rb == 0, 1.1, np.where(rb == 1, 1.0, 0.8)),
                np.where(rb == 0, 1.4, np.where(rb == 1, 1.05, 0.58)),
            )
            p = np.minimum(p * mul, 0.985)
        lit = nz < p
        nb = h01(cx * 3 + 911, py + 377, ep)
        v = np.where(lit, np.where(d <= 1.6, 255.0, 128.0 + 96.0 * nb),
                     np.where((d <= 10) & (not dust), 44.0 + 12.0 * nz, 0.0))

    # additive grainy glow band at y=540 (index.html:1501-1511)
    if s_env > 0:
        dys = (ys - 540).astype(np.float64)
        gy = np.where((dys > -46) & (dys < 46), np.exp(-dys * dys / 441.6), 0.0)
        xp = np.ones(DIS_W, dtype=np.float64)
        if sx_l is not None:
            for j, x in enumerate(xs):
                xv = float(x)
                if xv < sx_l + 60:
                    xp[j] = max(0.0, (xv - sx_l + 8) / 68.0)
                elif xv > sx_r - 36:
                    xp[j] = max(0.0, (sx_r + 8 - xv) / 44.0)
                if not (sx_l - 8 < xv < sx_r + 8):
                    xp[j] = 0.0
        cs = cx + 4096
        ng = h01(cs, py, ep) * (1 - fr) + h01(cs, py, ep + 1) * fr
        gm = np.where(ng < 0.91, 0.85 + 1.15 * ng, 3.2 + 2.2 * (ng - 0.91) / 0.09)
        v = v + 20.0 * s_env * gy[:, None] * xp[None, :] * gm
    v = np.minimum(v, 255.0)

    rgba = np.zeros((DIS_H, DIS_W, 4), dtype=np.uint8)
    bright = v >= 40
    faint = (~bright) & (v > 0.8)
    gray = np.where(bright, v, 40.0)
    alpha = np.where(bright, 255.0, np.where(faint, v * 6.375, 0.0))
    rgba[..., 0] = gray.astype(np.uint8)
    rgba[..., 1] = gray.astype(np.uint8)
    rgba[..., 2] = gray.astype(np.uint8)
    rgba[..., 3] = alpha.astype(np.uint8)
    return rgba


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out", default="assets/k3dis")
    ap.add_argument("--frame", type=int, default=None)
    ap.add_argument("--preview", action="store_true")
    ap.add_argument("--no-trailing-ls", action="store_true",
                    help="letter-spacing consumed between chars only (n-1)")
    args = ap.parse_args()

    mask, geom = build_mask(ls_trailing=not args.no_trailing_ls)
    w_k3, w_open, g_start = geom
    print(f"geom: wK3={w_k3:.4f} wOpen={w_open:.4f} gStart={g_start:.4f}")
    print(f"mask ink px: {(mask > 120).sum()}  (alpha>10: {(mask > 10).sum()})")
    dist = chamfer(mask)
    print(f"dist range: {dist.min()}..{dist.max()}")

    frames = [args.frame] if args.frame else list(range(411, 456))
    os.makedirs(args.out, exist_ok=True)
    for f in frames:
        rgba = render_frame(f, mask, dist)
        im = Image.fromarray(rgba, "RGBA")
        path = os.path.join(args.out, f"f{f}.png")
        im.save(path, optimize=True)
        if args.preview:
            lit = (rgba[..., 3] == 255).sum()
            print(f"f{f}: opaque px={lit} -> {path}")
    if args.preview and args.frame:
        rgba = render_frame(args.frame, mask, dist)
        Image.fromarray(rgba, "RGBA").resize((DIS_W * 2, DIS_H * 2), Image.NEAREST).save("/tmp/k3dis-preview.png")


if __name__ == "__main__":
    main()
