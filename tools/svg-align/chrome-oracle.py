#!/usr/bin/env python3
"""Chrome-driver oracle: headless Chromium/Edge render vs resvg vs opencat.

把浏览器渲染接入 svg 对齐链路作为 oracle（docs/svg-align 的验收基准从
resvg 升级为 Chrome）。三方出图后两两做 k3diff 风格像素 diff：

    chrome  — headless Edge/Chrome，SVG 以 <img> 嵌透明底 HTML
    resvg   — usvg 分支（feat/lightningcss-css）的 resvg.exe
    opencat — align.py 既有链路（svg → jsx → opencat.exe，tiny 光栅化）

  python tools/svg-align/chrome-oracle.py [--filter structure/style] [--limit 40] [--all]
环境变量：
  BROWSER_EXE  Edge/Chrome 可执行（默认 x86 Edge 稳定版路径）
  RESVG_EXE    resvg exe（默认 vendor 分支构建产物）
"""
import argparse
import importlib.util
import json
import os
import subprocess
import sys
import tempfile
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
spec = importlib.util.spec_from_file_location("align", Path(__file__).parent / "align.py")
align = importlib.util.module_from_spec(spec)
spec.loader.exec_module(align)

BROWSER = Path(os.environ.get(
    "BROWSER_EXE",
    "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe"))
OUT = REPO / "out" / "svg-align" / "chrome-oracle"
# CSS 相关语料子集：oracle 的首轮观察面（--all 跑全量）。
CSS_FILTERS = [
    "structure/style/",
    "structure/style-attribute/",
    "structure/use/cSS-rules",
    "structure/svg/background-color",
    "painting/mix-blend-mode/",
    "painting/isolation/",
    "painting/marker/the-marker-property-in-CSS",
    "text/font",
]
RENDER_TIMEOUT = 90
CHROME_TIMEOUT = 60


def log(msg):
    print(msg, flush=True)


def chrome_render(svg, png, w, h):
    """Edge/Chrome headless 截图：SVG 以 <img> 嵌透明底 HTML（图像模式下
    Chrome 禁脚本与外部引用，与 resvg 的静态渲染语义对齐）。"""
    case_dir = png.parent
    html = case_dir / (png.stem + ".html")
    html.write_text(
        "<!DOCTYPE html><html><head><style>"
        "html,body{margin:0;padding:0;background:transparent}img{display:block}"
        f"</style></head><body><img src=\"file:///{svg.as_posix()}\" "
        f"width=\"{w}\" height=\"{h}\"></body></html>",
        encoding="utf-8")
    profile = case_dir / "profile"
    cmd = [
        str(BROWSER),
        "--headless=new", "--disable-gpu", "--hide-scrollbars",
        "--default-background-color=00000000",
        f"--user-data-dir={profile}",
        "--virtual-time-budget=3000",
        f"--window-size={w},{h}",
        f"--screenshot={png}",
        html.as_uri(),
    ]
    try:
        r = subprocess.run(cmd, capture_output=True, text=True,
                           encoding="utf-8", errors="replace",
                           timeout=CHROME_TIMEOUT)
    except subprocess.TimeoutExpired:
        r = None
    # Edge 并发多实例即使独立 profile 也常以 rc=21 秒退（crashpad/服务
    # 竞争），串行 + 一次重试是实测稳定面。
    if r is None or r.returncode != 0 or not png.exists():
        import time
        time.sleep(0.5)
        try:
            r = subprocess.run(cmd, capture_output=True, text=True,
                               encoding="utf-8", errors="replace",
                               timeout=CHROME_TIMEOUT)
        except subprocess.TimeoutExpired:
            r = None
    if r is None or r.returncode != 0 or not png.exists():
        (OUT / "errors").mkdir(parents=True, exist_ok=True)
        detail = f"rc={r.returncode}\n{r.stderr[-2000:]}" if r else "timeout"
        (OUT / "errors" / f"{png.stem}.txt").write_text(detail, encoding="utf-8")
        return False
    return True


def one_case(c):
    from PIL import Image
    # ref 由 align.stage_ref 写在 align.OUT（out/svg-align/ref），与 align.py
    # 主流程共享缓存。
    ref = align.OUT / "ref" / f"{c['flat']}.png"
    shot = OUT / "chrome" / f"{c['flat']}.png"
    test = OUT / "test" / f"{c['flat']}.png"
    if not ref.exists():
        return {"id": c["id"], "category": c["category"], "status": "resvg-error"}

    w, h = Image.open(ref).size
    if not chrome_render(c["svg"], shot, w, h):
        return {"id": c["id"], "category": c["category"], "status": "chrome-error"}

    # opencat：复用 align 的 jsx 生成与渲染（尺寸基准 = resvg PNG）。
    jsx = OUT / "cases" / f"{c['flat']}.jsx"
    jsx.parent.mkdir(parents=True, exist_ok=True)
    css = json.dumps(align.case_css(c["flat"], w, h))
    jsx.write_text(align.CASE_TEMPLATE.format(
        w=w, h=h, src=c["svg"].as_posix(), cls=align.css_class(c["flat"]), css=css),
        encoding="utf-8")
    test.parent.mkdir(parents=True, exist_ok=True)
    try:
        r = align.run([align.OPENCAT, "render", jsx, test], align.RENDER_TIMEOUT)
    except subprocess.TimeoutExpired:
        return {"id": c["id"], "category": c["category"], "status": "opencat-error"}
    if r.returncode != 0 or not test.exists():
        (OUT / "errors").mkdir(parents=True, exist_ok=True)
        (OUT / "errors" / f"{c['flat']}__opencat.txt").write_text(
            r.stderr[-4000:], encoding="utf-8")
        return {"id": c["id"], "category": c["category"], "status": "opencat-error"}

    row = {"id": c["id"], "category": c["category"], "status": "ok"}
    for pair, key in (((shot, ref), "chrome_vs_resvg"),
                      ((shot, test), "chrome_vs_opencat"),
                      ((ref, test), "resvg_vs_opencat")):
        m = align.diff_pair(*pair)
        if m is None:
            row[key] = {"p8": None, "note": "size-mismatch"}
            continue
        row[key] = {"p8": m["p8"], "mae": m["mae"], "maxd": m["maxd"],
                    "bbox": m["bbox"]}
    return row


def main():
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--filter", default="", help="case id 前缀过滤（覆盖 CSS 子集）")
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--all", action="store_true", help="全量语料（默认 CSS 子集）")
    ap.add_argument("--jobs", type=int, default=1,
                    help=">1 时 Edge 实例竞争可能导致 rc=21 秒退，串行最稳")
    ap.add_argument("--force", action="store_true", help="重渲已存在的 resvg PNG")
    args = ap.parse_args()

    for d in ("ref", "chrome", "test", "cases", "errors"):
        (OUT / d).mkdir(parents=True, exist_ok=True)

    if args.all:
        cases = align.discover("", args.limit)
    elif args.filter:
        cases = align.discover(args.filter, args.limit)
    else:
        seen, cases = set(), []
        for f in CSS_FILTERS:
            for c in align.discover(f, 0):
                if c["id"] not in seen:
                    seen.add(c["id"])
                    cases.append(c)
        if args.limit:
            cases = cases[:args.limit]

    log(f"{len(cases)} cases (filters={args.filter or 'CSS subset'})")
    align.stage_ref(cases, args.force)

    with ThreadPoolExecutor(args.jobs) as pool:
        rows = list(pool.map(one_case, cases))

    # 汇总：三方两两 p8 的分布 + 最差清单。
    def pct(rows, key):
        vals = sorted(r[key]["p8"] for r in rows
                      if r["status"] == "ok" and r.get(key, {}).get("p8") is not None)
        if not vals:
            return {}
        import statistics
        return {
            "n": len(vals),
            "median_p8": round(statistics.median(vals), 4),
            "mean_p8": round(sum(vals) / len(vals), 4),
            "p90_p8": round(vals[int(len(vals) * 0.9)], 4),
            "max_p8": round(vals[-1], 4),
            "pass_p8_le_0.005": sum(1 for v in vals if v <= align.PASS_P8),
        }

    summary = {
        "total": len(rows),
        "status": {},
        "chrome_vs_resvg": pct(rows, "chrome_vs_resvg"),
        "chrome_vs_opencat": pct(rows, "chrome_vs_opencat"),
        "resvg_vs_opencat": pct(rows, "resvg_vs_opencat"),
    }
    for r in rows:
        summary["status"][r["status"]] = summary["status"].get(r["status"], 0) + 1

    worst = sorted((r for r in rows if r["status"] == "ok"),
                   key=lambda r: max(
                       (r[k]["p8"] or 0 for k in
                        ("chrome_vs_resvg", "chrome_vs_opencat", "resvg_vs_opencat")
                        if k in r and r[k].get("p8") is not None),
                       default=0), reverse=True)[:15]
    summary["worst"] = [
        {"id": r["id"],
         "chrome_vs_resvg": r.get("chrome_vs_resvg"),
         "chrome_vs_opencat": r.get("chrome_vs_opencat"),
         "resvg_vs_opencat": r.get("resvg_vs_opencat")}
        for r in worst]

    out_json = OUT / "report.json"
    out_json.write_text(json.dumps(summary, indent=2, ensure_ascii=False),
                        encoding="utf-8")
    log(json.dumps(summary["status"], ensure_ascii=False))
    for k in ("chrome_vs_resvg", "chrome_vs_opencat", "resvg_vs_opencat"):
        log(f"  {k}: {summary[k]}")
    log(f"report: {out_json}")


if __name__ == "__main__":
    main()
