#!/usr/bin/env python3
"""Compare OpenCat text metrics against the Chrome/Edge dump.

Arguments: engine.json (JSON lines from `text_metric_parity`), browser.json
(the JSON array the harness page writes into #result).

Prints the worst offenders per field so a mis-metric can be fixed in
`text/surface.rs` with the real delta instead of a guess. Exit code 1 when any
field exceeds its tolerance — this is the Phase 0.1 gate.

Tolerances (px): text layout must be sub-pixel, so `width` gets 0.02
(Skia grayscales hinting is off; Chrome's `ctx.measureText` is float). Ink box
edges are shape outlines, and fontBoundingBox comes from OS/2 typo metrics —
both 0.02. When the pair uses different fonts entirely, `width` blows past
these immediately, which is exactly the signal Phase 0.1 is looking for.
"""
import json
import re
import sys

sys.stdout.reconfigure(encoding="utf-8", errors="replace")

WIDTH_TOL = 0.02
INK_TOL = 0.02
FONT_TOL = 0.02
FIELDS = ["width", "ink_left", "ink_right", "ink_ascent", "ink_descent",
          "font_ascent", "font_descent"]
TOL = {"width": WIDTH_TOL, "ink_left": INK_TOL, "ink_right": INK_TOL,
       "ink_ascent": INK_TOL, "ink_descent": INK_TOL,
       "font_ascent": FONT_TOL, "font_descent": FONT_TOL}


def load_engine(path):
    rows = []
    for line in open(path, encoding="utf-8"):
        line = line.strip()
        if not line.startswith("{"):
            continue
        rows.append(json.loads(line))
    return rows


def load_browser(path):
    raw = open(path, encoding="utf-8").read()
    # The harness writes the JSON into a hidden div; tolerate either the bare
    # JSON or a DOM dump that wraps it.
    # Prefer the harness's own container: a DOM dump can hold more than one
    # JSON blob, and a greedy scan picks the wrong one.
    m = re.search(r'<div id="result"[^>]*>(.*?)</div>', raw, re.S)
    blob = m.group(1) if m else raw
    rows = json.loads(blob.replace("&quot;", '"').replace("&amp;", "&"))
    # `--dump-dom` can serialize after the page script ran twice; the first
    # pass measured with the embedded faces, the second fell back to the
    # system font. Keep the first row per key.
    seen, out = set(), []
    for r in rows:
        key = (r["weight"], round(r["size"], 4), round(r["ls"], 4), r["text"])
        if key in seen:
            continue
        seen.add(key)
        out.append(r)
    return out


def main():
    engine = load_engine(sys.argv[1])
    browser = load_browser(sys.argv[2])
    # The family name is whatever the harness page declares, so key on the
    # metric-relevant fields and treat family as a free label.
    index = {
        (r["weight"], round(r["size"], 4), round(r["ls"], 4), r["text"]): r
        for r in browser
    }
    missing, worst = [], []
    for e in engine:
        key = (e["weight"], round(e["size"], 4), round(e["ls"], 4), e["text"])
        b = index.get(key)
        if b is None:
            missing.append(key)
            continue
        deltas = {f: (e[f], b[f], abs(e[f] - b[f])) for f in FIELDS}
        worst.append((max(d for _, _, d in deltas.values()),
                      e["text"], e["weight"], e["size"], e["ls"], deltas))

    if missing:
        print(f"MISSING {len(missing)} browser rows (first 5): {missing[:5]}")

    worst.sort(key=lambda r: -r[0])
    failures = 0
    print(f"{'text':<12}{'w':>4}{'size':>6}{'ls':>6}  {'maxΔ':>8}  per-field")
    for maxd, text, weight, size, ls, deltas in worst[:40]:
        bad = [f for f in FIELDS if deltas[f][2] > TOL[f]]
        flag = "  <<< " + ",".join(bad) if bad else ""
        print(f"{text[:12]:<12}{weight:>4}{size:>6}{ls:>6}  {maxd:>8.4f}"
              f"  " + " ".join(f"{f}={deltas[f][2]:.4f}" for f in bad) + flag)
        if bad:
            failures += 1

    n = len(worst)
    for f in FIELDS:
        ds = [d[f][2] for _, _, _, _, _, d in worst]
        over = sum(1 for d in ds if d > TOL[f])
        print(f"{f:<14} max={max(ds):7.4f} mean={sum(ds)/len(ds):7.5f} "
              f"over-tol={over}/{n}")
    print(f"\nrows compared: {n}, rows over tolerance: {failures}")
    return 1 if failures or missing else 0


if __name__ == "__main__":
    sys.exit(main())
