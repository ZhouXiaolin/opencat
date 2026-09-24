#!/usr/bin/env python3
"""Offline replay of the reference k3-promo PRNG streams.

Ground truth: /home/solaren/Projects/hyperframes-launches/k3-promo/index.html
  mulberry32 definition  : lines 410-417
  global rnd = m32(1337) : line 418
  ghost columns          : lines 974-992  (42 cols; per col: range(0.3,1) colOp,
                           y=-floor(range(0,26)); per cell: pick([18,22,26,30]),
                           rnd()<0.6 existence, range(0.3,1) cellOp when kept)
  barInit                : line 1033      (18x range(0.42,1))
  BAR_EV shimmer         : lines 1663-1668 (for t=6.16; t<7.32; t+=0.1333 ->
                           rounds of [floor(rnd()*18), range(0.4,1)])

Validation anchor: BAR_INIT baked into examples/k3-promo.xml (verified <=1px
in round 1). barInit is consumed from the SAME global stream strictly AFTER
the ghost columns, so a 6-decimal match proves the ghost draw count/order.

Also replays the scene-A field streams (independent seeds, NOT the global one):
  buildField  : mulberry32(4242)  (lines 609-726; ringSnap consumers)
  RING_CELLS  : mulberry32(3131)  (lines 728-745)
  births      : mulberry32(777)   (lines 766-777, over fieldW.born order)
  ringSched   : mulberry32(9091)  (lines 780-820)

Usage: k3replay.py [--emit]   (--emit writes /tmp/k3-ghost.json /tmp/k3-field.json)
"""
import json
import math
import sys


def js_imul(a, b):
    return (a * b) & 0xFFFFFFFF


def m32(a):
    """Exact transliteration of index.html:410-417."""
    s = a & 0xFFFFFFFF

    def rnd():
        nonlocal s
        s = (s + 0x6D2B79F5) & 0xFFFFFFFF
        t = s
        t = js_imul(t ^ (t >> 15), (1 | t) & 0xFFFFFFFF)
        t = ((t + js_imul(t ^ (t >> 7), (61 | t) & 0xFFFFFFFF)) & 0xFFFFFFFF) ^ t
        t = t ^ (t >> 14)
        return t / 4294967296.0

    return rnd


GS = 94


def snap(v):
    # JS Math.round (values here are non-negative)
    return math.floor(v / GS + 0.5) * GS


def make_ring_snap(rnd):
    def ring_snap():
        for _ in range(60):
            x = snap(430 + rnd() * 1070)
            y = snap(rnd() * 940)
            cx, cy = x + 47, y + 47
            if cx < 455 or cx > 1560:
                continue
            if cx <= 690 or cx >= 1140 or cy <= 265 or cy >= 845:
                return [x, y]
        return [564, 94]

    return ring_snap


def replay_ghost(rnd):
    cols = []
    for c in range(42):
        col_op = 0.3 + rnd() * 0.7
        y = -math.floor(rnd() * 26)
        cells = []
        while y < 1080:
            h = [18, 22, 26, 30][math.floor(rnd() * 4)]
            if rnd() < 0.6:
                cell_op = 0.3 + rnd() * 0.7
                cells.append((y, h, cell_op))
            y += h + 6
        cols.append({"c": c, "colOp": col_op, "cells": cells})
    return cols


def replay_bar_init(rnd):
    return [0.42 + rnd() * 0.58 for _ in range(18)]


def replay_bar_ev(rnd):
    ev = []
    t = 6.16
    while t < 7.32:
        for _ in range(5):
            bi = math.floor(rnd() * 18)
            ev.append((round(t, 4), bi, 0.4 + rnd() * 0.6))
        t += 0.1333
    return ev


def replay_field():
    out = {}
    r = m32(4242)
    ring_snap = make_ring_snap(r)
    born = []
    for _ in range(3):
        born.append({"kind": "obox", "xy": ring_snap(), "slot": "early"})
    for _ in range(4):
        xy = ring_snap()
        born.append({"kind": "pinwheel", "xy": xy, "slot": "mid",
                     "flip": r() < 0.5})
    for _ in range(14):
        born.append({"kind": "plus", "xy": ring_snap(), "slot": "mid"})
    for _ in range(12):
        born.append({"kind": "dot", "xy": ring_snap(), "slot": "mid"})
    for _ in range(6):
        xy = ring_snap()
        left = xy[0] + 47 + (-20 + r() * 40)
        top = xy[1] + 47 + (-16 + r() * 32)
        size = 5 + r() * 5
        born.append({"kind": "fleck", "xy": xy, "slot": "mid",
                     "left": left, "top": top, "size": size})
    for i in range(4):
        xy = ring_snap()
        born.append({"kind": "ocell", "xy": [xy[0] + 4, xy[1] + 4],
                     "slot": "early"})
        if i == 0:
            born.append({"kind": "ocell", "xy": [xy[0] + GS + 4, xy[1] + 4],
                         "slot": "early"})
    out["born"] = born

    rj = m32(3131)
    cells = []

    def put(x, y, px, py):
        if rj() < 0.15:
            return
        if rj() < 0.4:
            x += px * 94 * (1 if rj() < 0.5 else -1)
            y += py * 94 * (1 if rj() < 0.5 else -1)
        cells.append([x, y])

    x = 564
    while x <= 1410:
        put(x, 94, 0, 1)
        put(x, 940, 0, 1)
        x += 94
    y = 188
    while y <= 846:
        put(564, y, 1, 0)
        put(1410, y, 1, 0)
        y += 94
    for p in [[658, 188], [1316, 188], [658, 846], [1316, 846],
              [470, 470], [1504, 376], [470, 752], [1504, 658]]:
        cells.append(p)
    out["ring_cells"] = cells

    rb = m32(777)
    births = []
    for g in born:
        # birth reads the element's style.left/top (adjusted per kind)
        if g["kind"] == "obox":
            ex, ey = g["xy"]          # first-3 boxes: left=rc0[0], no inset
        elif g["kind"] == "pinwheel":
            ex, ey = g["xy"][0] + 10, g["xy"][1] + 10
        elif g["kind"] == "plus":
            ex, ey = g["xy"][0] + 47 - 4.5, g["xy"][1] + 47 - 4.5
        elif g["kind"] == "dot":
            ex, ey = g["xy"][0] + 45.5, g["xy"][1] + 45.5
        elif g["kind"] == "fleck":
            ex, ey = g["left"], g["top"]
        else:  # ocell
            ex, ey = g["xy"]
        near_tr = ex > 1000 and ey < 500
        if g["slot"] == "early":
            t = (0.78 + rb() * 0.17) if near_tr else (0.95 + rb() * 0.5)
        else:
            t = 1.0 + rb() * 0.42
        births.append(t)
    out["births"] = births

    rr = m32(9091)
    T_REV = 0.9333
    SPURTS = [[1.0, 2.2], [2.633, 3.2]]
    KINDS_MID = ["solid", "pin", "outline", "solid", "scan", "hatch"]
    KINDS_HOLD = ["hatch", "hatch", "outline", "solid", "pin", "hatch"]

    def pick(arr):
        return arr[math.floor(rr() * len(arr))]

    sched = []
    for rc in cells:
        cx, cy = rc[0] + 47, rc[1] + 47
        prog = math.atan2(cy - 540, cx - 960) / (2 * math.pi)
        prog = (prog + 1) % 1
        ev = []
        ev.append([0.9 + rr() * 0.6,
                   "empty" if rr() < 0.5 else pick(KINDS_HOLD),
                   0.55 + rr() * 0.45])
        passes = []
        for m in range(10):
            tp = 0.62 + (prog + m * 0.5) * T_REV
            if any(s[0] <= tp <= s[1] for s in SPURTS):
                passes.append(tp)
        for pi, tp in enumerate(passes):
            if rr() < 0.35 and pi != len(passes) - 1:
                continue
            t = tp + rr() * 0.04
            n_ch = 2 + math.floor(rr() * 3)
            for k in range(n_ch):
                last = k == n_ch - 1
                if last:
                    last_pass = pi == len(passes) - 1
                    kind = ("empty" if rr() < (0.55 if last_pass else 0.72)
                            else pick(KINDS_HOLD))
                else:
                    kind = pick(KINDS_MID)
                ev.append([t, kind, 0.55 + rr() * 0.45])
                t += 0.0333 * (1 + math.floor(rr() * 2))
        ev.sort(key=lambda e: e[0])
        sched.append(ev)
    out["ring_sched"] = sched
    return out


def main():
    rnd = m32(1337)
    cols = replay_ghost(rnd)
    bar_init = replay_bar_init(rnd)
    bar_ev = replay_bar_ev(rnd)

    XML_BAR_INIT = [0.755120, 0.474258, 0.932753, 0.888181, 0.765575, 0.509834,
                    0.759621, 0.960859, 0.792899, 0.453528, 0.626782, 0.908583,
                    0.488705, 0.528622, 0.573839, 0.905405, 0.535246, 0.920957]
    ok = len(bar_init) == len(XML_BAR_INIT) and all(
        abs(a - b) < 5e-7 for a, b in zip(bar_init, XML_BAR_INIT))
    print("BAR_INIT match:", ok)
    if not ok:
        print("replayed:", [round(v, 6) for v in bar_init])
        print("xml     :", XML_BAR_INIT)
        sys.exit(1)

    XML_BAR_EV_HEAD = [(6.16, 5, 0.523743), (6.16, 3, 0.995485),
                       (6.16, 14, 0.904621), (6.16, 10, 0.454545),
                       (6.16, 2, 0.428828)]
    for (t0, bi0, v0), (t1, bi1, v1) in zip(bar_ev, XML_BAR_EV_HEAD):
        assert abs(t0 - t1) < 1e-3 and bi0 == bi1 and abs(v0 - v1) < 5e-7, \
            (t0, bi0, v0, t1, bi1, v1)
    print("BAR_EV head match: True  (entries:", len(bar_ev), ")")

    total_cells = sum(len(c["cells"]) for c in cols)
    print("ghost cols:", len(cols), "cells:", total_cells)
    field = replay_field()
    print("field born:", len(field["born"]),
          "ring cells:", len(field["ring_cells"]),
          "sched events:", sum(len(e) for e in field["ring_sched"]))

    if len(sys.argv) > 1 and sys.argv[1] == "--emit":
        with open("/tmp/k3-ghost.json", "w") as f:
            json.dump(cols, f)
        with open("/tmp/k3-field.json", "w") as f:
            json.dump(field, f)
        print("wrote /tmp/k3-ghost.json /tmp/k3-field.json")


if __name__ == "__main__":
    main()
