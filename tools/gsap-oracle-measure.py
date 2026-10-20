import sys
# 同帧色块中心位置对比：把「同一时刻每个 ease 把方块推到哪」变成可读的数，
# 比像素 diff 更直接地判断动画数值是否一致。

from PIL import Image
import numpy as np

COLORS = [
 ('linear', (59,130,246)), ('customease',(34,197,94)), ('wiggle',(234,179,8)),
 ('bounce',(249,115,22)), ('slowmo',(168,85,247)), ('expo',(6,182,212)), ('rough',(236,72,153)),
 ('phys2d',(56,189,248)), ('physprops',(244,114,182)),
]

def centers(path):
    im = np.asarray(Image.open(path).convert('RGB')).astype(np.int16)
    out = {}
    for name, rgb in COLORS:
        d = np.abs(im - np.array(rgb, dtype=np.int16)).sum(axis=2)
        mask = d < 30
        if mask.sum() < 100:
            out[name] = None
            continue
        ys, xs = np.nonzero(mask)
        out[name] = (float(xs.mean()), float(ys.mean()), int(mask.sum()))
    return out

a = centers(sys.argv[1]); b = centers(sys.argv[2])
print(f"{'name':12s} {'opencat':>22s} {'chrome':>22s} {'d':>8s}")
maxd = 0
for name, _ in COLORS:
    pa, pb = a.get(name), b.get(name)
    if pa is None or pb is None:
        print(f"{name:12s} {'MISSING' if pa is None else f'{pa[0]:.1f},{pa[1]:.1f}':>22s} {'MISSING' if pb is None else f'{pb[0]:.1f},{pb[1]:.1f}':>22s}")
        continue
    d = ((pa[0]-pb[0])**2 + (pa[1]-pb[1])**2) ** 0.5
    maxd = max(maxd, d)
    print(f"{name:12s} {f'{pa[0]:.1f},{pa[1]:.1f}':>22s} {f'{pb[0]:.1f},{pb[1]:.1f}':>22s} {d:8.2f}")
print(f"max center delta = {maxd:.2f}px")
