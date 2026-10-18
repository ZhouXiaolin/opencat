#!/usr/bin/env python3
"""SVG alignment harness: resvg PNG vs opencat jsx PNG, k3diff-style pixel diff.

The single test method for the svg port (docs/svg-align/PLAN.md §5). No unit
tests are written for the svg backend; this report drives all fixes.

Stages: ref (resvg renders corpus) -> gen (emit one jsx case per svg, meta size
= reference PNG size) -> test (opencat renders each case) -> diff (pixel.csv +
summary.json + category table + worst list). `--probe` self-checks the harness
itself on tools/svg-align/fixture.

  python tools/svg-align/align.py --stage all [--filter shapes/rect] [--limit 50] [--force]
  python tools/svg-align/align.py --stage diff          # re-aggregate only
"""
import argparse
import csv
import json
import os
import subprocess
import sys
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
RESVG = Path(os.environ.get("RESVG_EXE", "F:/resvg/target/release/resvg.exe"))
OPENCAT = Path(os.environ.get("OPENCAT_EXE", str(REPO / "target" / "release" / "opencat.exe")))
CORPUS = Path(os.environ.get("RESVG_TESTS", "F:/resvg/crates/resvg/tests/tests"))
OUT = REPO / "out" / "svg-align"

THRESHOLDS = [0, 2, 4, 8, 16, 32, 64, 128]
BBOX_THRESH = 16
PASS_P8 = 0.005  # pass line: <=0.5% pixels with max-channel delta > 8
RENDER_TIMEOUT = 90

CASE_TEMPLATE = """export const meta = {{ width: {w}, height: {h}, fps: 30, duration: 0.04 }};
export default function SvgCase() {{
  return (
    <div id="root" class="c-{flat}">
      <svg src="{src}" width="{w}" height="{h}" class="s" />
    </div>
  );
}}
"""


def case_css(cases):
    # One shared <cases-dir>/tailwind.css (the engine's css sidecar convention).
    # Stick to the engine's CSS subset: simple properties, background-color,
    # no shorthand.
    rules = []
    for c in cases:
        meta = CASES_META.get(c["flat"])
        if not meta:
            continue
        w, h = meta
        rules.append(
            f".c-{c['flat']} {{ position: relative; width: {w}px; height: {h}px; "
            f"overflow: hidden; }}\n"
            f".c-{c['flat']} .s {{ position: absolute; left: 0; top: 0; }}"
        )
    return "\n".join(rules) + "\n"


CASES_META = {}  # flat -> (w, h), filled by stage_gen


def log(msg):
    print(msg, flush=True)


def find_ffmpeg_dll_dir():
    """Directory holding swscale-10.dll. The shell PATH can be stale (points
    at a removed C:/ffmpeg), so candidates come from FFMPEG_DIR, the registry
    PATH and a bounded glob over C:/ffmpeg*, each verified before use."""
    import glob
    import winreg

    cands = []
    ff = os.environ.get("FFMPEG_DIR")
    if ff:
        cands += [Path(ff) / "bin", Path(ff) / "lib", Path(ff)]
    for root, key in [(winreg.HKEY_CURRENT_USER, "Environment"),
                      (winreg.HKEY_LOCAL_MACHINE,
                       r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment")]:
        try:
            with winreg.OpenKey(root, key) as k:
                v, _ = winreg.QueryValueEx(k, "PATH")
                for p in v.split(";"):
                    if p.strip():
                        cands += [Path(p), Path(p) / "bin"]
        except OSError:
            pass
    for pat in ("C:/ffmpeg*/*/bin", "C:/ffmpeg*/bin"):
        cands += [Path(g) for g in glob.glob(pat)]
    for c in cands:
        if (c / "swscale-10.dll").exists():
            return c
    return None


_FFMPEG_DIR_FOUND = None


def spawn_env():
    # opencat.exe needs ffmpeg DLLs (swscale-10.dll etc.); spawned processes
    # don't inherit a working PATH entry, so inject the verified dir.
    global _FFMPEG_DIR_FOUND
    if _FFMPEG_DIR_FOUND is None:
        _FFMPEG_DIR_FOUND = find_ffmpeg_dll_dir()
    env = os.environ.copy()
    if _FFMPEG_DIR_FOUND:
        env["PATH"] = f"{_FFMPEG_DIR_FOUND};{env.get('PATH', '')}"
    return env


def run(cmd, timeout):
    return subprocess.run(
        [str(c) for c in cmd],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
        timeout=timeout,
        env=spawn_env(),
    )


def discover(filt, limit):
    svgs = sorted(CORPUS.rglob("*.svg"))
    cases = []
    for svg in svgs:
        rel = svg.relative_to(CORPUS).as_posix()
        cid = rel[:-4]  # strip .svg
        if filt and not cid.startswith(filt):
            continue
        cases.append({
            "id": cid,                       # shapes/rect/cap-values
            "flat": cid.replace("/", "__"),  # flat file stem
            "category": rel.split("/")[0],
            "svg": svg,
        })
    if limit:
        cases = cases[:limit]
    return cases


def stage_ref(cases, force):
    (OUT / "ref").mkdir(parents=True, exist_ok=True)
    (OUT / "errors").mkdir(parents=True, exist_ok=True)
    ok = 0
    for c in cases:
        png = OUT / "ref" / f"{c['flat']}.png"
        if png.exists() and not force:
            ok += 1
            continue
        try:
            r = run([RESVG, c["svg"], png], RENDER_TIMEOUT)
        except subprocess.TimeoutExpired:
            (OUT / "errors" / f"ref__{c['flat']}.txt").write_text("timeout")
            continue
        if r.returncode != 0:
            (OUT / "errors" / f"ref__{c['flat']}.txt").write_text(r.stderr[-4000:])
        else:
            ok += 1
    log(f"[ref] {ok}/{len(cases)} reference PNGs ready")


def stage_gen(cases, force):
    from PIL import Image
    (OUT / "cases").mkdir(parents=True, exist_ok=True)
    n = 0
    for c in cases:
        png = OUT / "ref" / f"{c['flat']}.png"
        if not png.exists():
            continue
        jsx = OUT / "cases" / f"{c['flat']}.jsx"
        w, h = Image.open(png).size
        CASES_META[c["flat"]] = (w, h)
        if not (jsx.exists() and not force):
            jsx.write_text(CASE_TEMPLATE.format(
                w=w, h=h, src=c["svg"].as_posix(), flat=c["flat"]))
        n += 1
    (OUT / "cases" / "tailwind.css").write_text(case_css(cases))
    log(f"[gen] {n} case jsx files + shared tailwind.css ready")


def stage_test(cases, force):
    (OUT / "test").mkdir(parents=True, exist_ok=True)
    ok = fail = 0
    for c in cases:
        jsx = OUT / "cases" / f"{c['flat']}.jsx"
        if not jsx.exists():
            continue
        png = OUT / "test" / f"{c['flat']}.png"
        if png.exists() and not force:
            ok += 1
            continue
        try:
            r = run([OPENCAT, "render", jsx, png], RENDER_TIMEOUT)
        except subprocess.TimeoutExpired:
            fail += 1
            (OUT / "errors" / f"{c['flat']}.txt").write_text("timeout")
            continue
        if r.returncode != 0 or not png.exists():
            fail += 1
            (OUT / "errors" / f"{c['flat']}.txt").write_text(r.stderr[-4000:])
        else:
            ok += 1
    log(f"[test] {ok} rendered, {fail} failed (see out/svg-align/errors)")


def diff_pair(ref_png, test_png):
    from PIL import Image
    import numpy as np
    a = Image.open(ref_png).convert("RGBA")
    b = Image.open(test_png).convert("RGBA")
    if a.size != b.size:
        return None
    black = Image.new("RGBA", a.size, (0, 0, 0, 255))
    ar = np.asarray(Image.alpha_composite(black, a).convert("RGB"), dtype=np.int16)
    br = np.asarray(Image.alpha_composite(black, b).convert("RGB"), dtype=np.int16)
    d = np.abs(ar - br).max(axis=2)
    row = {"w": a.size[0], "h": a.size[1],
           "mae": round(float(d.mean()), 4), "maxd": int(d.max())}
    for t in THRESHOLDS:
        row[f"p{t}"] = round(float((d > t).mean()), 6)
    ys, xs = np.nonzero(d > BBOX_THRESH)
    if len(xs):
        row["bbox"] = f"{int(xs.min())},{int(ys.min())},{int(xs.max())},{int(ys.max())}"
    else:
        row["bbox"] = ""
    return row


def stage_diff(cases):
    rows, n_ref_fail = [], 0
    for c in cases:
        ref = OUT / "ref" / f"{c['flat']}.png"
        test = OUT / "test" / f"{c['flat']}.png"
        if not ref.exists():
            n_ref_fail += 1
            continue
        row = {"id": c["id"], "category": c["category"], "status": "ok"}
        if not test.exists():
            row["status"] = "render-error"
            rows.append(row)
            continue
        m = diff_pair(ref, test)
        if m is None:
            row["status"] = "size-mismatch"
            rows.append(row)
            continue
        row.update(m)
        row["status"] = "pass" if row["p8"] <= PASS_P8 else "fail"
        rows.append(row)

    with open(OUT / "pixel.csv", "w", newline="", encoding="utf-8") as f:
        w = csv.DictWriter(f, fieldnames=list(rows[0].keys()) if rows else ["id"])
        w.writeheader()
        w.writerows(rows)

    cats = {}
    for r in rows:
        s = cats.setdefault(r["category"], {"n": 0, "pass": 0, "err": 0})
        s["n"] += 1
        if r["status"] == "pass":
            s["pass"] += 1
        elif r["status"] in ("render-error", "size-mismatch"):
            s["err"] += 1
    summary = {
        "pass_p8": PASS_P8,
        "total": len(rows),
        "pass": sum(1 for r in rows if r["status"] == "pass"),
        "render_error": sum(1 for r in rows if r["status"] == "render-error"),
        "size_mismatch": sum(1 for r in rows if r["status"] == "size-mismatch"),
        "ref_fail": n_ref_fail,
        "categories": cats,
    }
    worst = sorted((r for r in rows if r["status"] == "fail"),
                   key=lambda r: r.get("p8", 0), reverse=True)[:20]
    summary["worst"] = [{"id": r["id"], "p8": r["p8"], "bbox": r.get("bbox", "")} for r in worst]
    with open(OUT / "summary.json", "w", encoding="utf-8") as f:
        json.dump(summary, f, indent=2)

    log(f"\n[diff] {summary['pass']}/{summary['total']} pass (p8<={PASS_P8}); "
        f"render-error {summary['render_error']}, size-mismatch {summary['size_mismatch']}, "
        f"ref-fail {summary['ref_fail']}")
    for cat in sorted(cats):
        s = cats[cat]
        rate = s["pass"] / s["n"] * 100 if s["n"] else 0
        log(f"  {cat:<14} {s['pass']:>4}/{s['n']:<4} {rate:5.1f}%  err {s['err']}")
    if worst:
        log("  worst:")
        for r in worst[:10]:
            log(f"    {r['id']}  p8={r['p8']}")
    log(f"\nreport: {OUT / 'pixel.csv'}")
    return 0


def probe():
    fx = Path(__file__).resolve().parent / "fixture"
    (OUT / "probe").mkdir(parents=True, exist_ok=True)
    ref = OUT / "probe" / "ref.png"
    test = OUT / "probe" / "test.png"
    r = run([RESVG, fx / "rect.svg", ref], RENDER_TIMEOUT)
    assert r.returncode == 0, r.stderr
    r = run([OPENCAT, "render", fx / "rect.jsx", test], RENDER_TIMEOUT)
    if r.returncode != 0:
        log("probe FAIL: opencat render failed\n" + r.stderr[-2000:])
        return 1
    m = diff_pair(ref, test)
    if m is None:
        log("probe FAIL: size mismatch")
        return 1
    log(f"probe: {m}")
    if m["p8"] > 0:
        log("probe FAIL: pixels differ")
        return 1
    log("probe PASS: harness mechanics verified (resvg render, jsx render, diff)")
    return 0


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--stage", default="all",
                    choices=["all", "ref", "gen", "test", "diff"])
    ap.add_argument("--filter", default="", help="case id prefix, e.g. shapes/rect")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--force", action="store_true", help="re-render existing PNGs")
    ap.add_argument("--probe", action="store_true", help="harness self-check only")
    args = ap.parse_args()

    if args.probe:
        sys.exit(probe())
    if not CORPUS.exists():
        sys.exit(f"corpus not found: {CORPUS} (override with RESVG_TESTS)")
    cases = discover(args.filter, args.limit)
    log(f"{len(cases)} cases (filter={args.filter!r})")
    stages = ["ref", "gen", "test", "diff"] if args.stage == "all" else [args.stage]
    for st in stages:
        if st == "diff":
            sys.exit(stage_diff(cases))
        {"ref": stage_ref, "gen": stage_gen, "test": stage_test}[st](cases, args.force)


if __name__ == "__main__":
    main()
