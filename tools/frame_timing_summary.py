"""Aggregate [timing-script] / [timing-vk] lines from a full-render stderr log.

Usage: python tools/frame_timing_summary.py out/inspector-hf-profile.log
"""
import re
import sys
import statistics as st

script_re = re.compile(r"\[timing-script\] f(\d+) script=([\d.]+)ms")
vk_re = re.compile(
    r"\[timing-vk\] f(\d+) script=([\d.]+)ms gl-stream=([\d.]+)ms draw=([\d.]+)ms "
    r"readback=([\d.]+)ms total=([\d.]+)ms"
)

script = {}  # frame -> ms
vk = {}      # frame -> (gl_stream, draw, readback, total)

for path in sys.argv[1:]:
    with open(path, encoding="utf-8", errors="replace") as fh:
        for line in fh:
            m = script_re.search(line)
            if m:
                script[int(m.group(1))] = float(m.group(2))
                continue
            m = vk_re.search(line)
            if m:
                vk[int(m.group(1))] = tuple(float(x) for x in m.groups()[1:])


def dist(name, values):
    if not values:
        print(f"{name}: no data")
        return
    values = sorted(values)
    n = len(values)
    print(
        f"{name:<12} n={n:<5} avg={st.mean(values):7.1f}  "
        f"p50={st.median(values):7.1f}  p90={values[int(n*0.9)]:7.1f}  "
        f"p95={values[int(n*0.95)]:7.1f}  p99={values[int(n*0.99)]:7.1f}  "
        f"max={values[-1]:7.1f}  sum={sum(values)/1000:7.1f}s"
    )


print("=== per-stage per-frame ms ===")
dist("script", list(script.values()))
# vk tuple = groups()[1:] = (script, gl-stream, draw, readback, total); vk-side script is always 0
for i, name in enumerate(("vk-script", "gl-stream", "draw", "readback", "vk-total")):
    dist(name, [v[i] for v in vk.values()])

# top-N frames by each stage
print("\n=== top 12 frames by stage ===")
for name, table, idx in (
    ("script", script, None),
    ("draw", vk, 2),
    ("readback", vk, 3),
    ("vk-total", vk, 4),
):
    rows = (
        ((f, v) for f, v in script.items() if f in vk)
        if idx is None
        else ((f, v[idx]) for f, v in vk.items())
    )
    top = sorted(rows, key=lambda kv: -kv[1])[:12]
    heads = ", ".join(f"f{f}:{v:.0f}" for f, v in top)
    print(f"{name:<9} {heads}")

# draw spike segmentation: contiguous frame ranges with draw > threshold
if vk:
    thr = sorted(v[2] for v in vk.values())[int(len(vk) * 0.9)]
    print(f"\n=== draw spike segments (>{thr:.0f}ms) ===")
    segs = []
    start = None
    prev = None
    for f in sorted(vk):
        hot = vk[f][2] > thr
        if hot and start is None:
            start = f
        if not hot and start is not None:
            segs.append((start, prev))
            start = None
        prev = f
    if start is not None:
        segs.append((start, prev))
    for a, b in segs:
        if b - a >= 4:
            avg_draw = st.mean(vk[f][2] for f in range(a, b + 1) if f in vk)
            avg_rb = st.mean(vk[f][3] for f in range(a, b + 1) if f in vk)
            avg_sc = st.mean(script[f] for f in range(a, b + 1) if f in script)
            print(
                f"  f{a}-{b} ({b-a+1}f): draw={avg_draw:.0f}ms readback={avg_rb:.0f}ms script={avg_sc:.0f}ms"
            )
