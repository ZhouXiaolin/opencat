#!/usr/bin/env python3
"""全语料 Vulkan 路径扫描：串行渲染 + k3diff。带超时门禁。"""
import os, subprocess, sys, time, json, glob

ROOT = r"F:\hyperframes-launches"
OUT = r"F:\OpenCat\out\sweep\e5-scan"
OPENCAT = r"F:\OpenCat\target\release\opencat.exe"
K3DIFF = r"F:\OpenCat\tools\k3diff.py"
ENV = {**os.environ,
       "OC_ANGLE_DIR": r"F:\OpenCat\out\swangle",
       "OC_ANGLE_GPU": "1",
       "PATH": r"C:\ffmpeg-9.0.2\ffmpeg-9.0.2-full_build-shared\bin;" + os.environ["PATH"]}
RENDER_TIMEOUT = 900   # 门禁：单工程渲染 15 分钟
DIFF_TIMEOUT = 600

def run(cmd, timeout, log):
    with open(log, "w", encoding="utf-8", errors="replace") as f:
        p = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT,
                           timeout=timeout, cwd=r"F:\OpenCat")
    return p.returncode

def main(names):
    os.makedirs(OUT, exist_ok=True)
    results = {}
    summary_path = os.path.join(OUT, "summary.json")
    if os.path.exists(summary_path):
        results = json.load(open(summary_path))
    for name in names:
        if name in results and results[name].get("p8") is not None:
            print(f"skip {name} (done)", flush=True)
            continue
        ref = glob.glob(os.path.join(ROOT, name, "renders", "*swgl*.mp4"))
        if not ref:
            results[name] = {"error": "no ref"}
            continue
        ref = ref[0]
        mp4 = os.path.join(OUT, f"{name}.mp4")
        t0 = time.time()
        try:
            rc = run([OPENCAT, "render", os.path.join(ROOT, name, "index.html"), mp4],
                     RENDER_TIMEOUT, os.path.join(OUT, f"{name}.render.log"))
        except subprocess.TimeoutExpired:
            results[name] = {"error": "render timeout", "secs": RENDER_TIMEOUT}
            json.dump(results, open(summary_path, "w"), indent=1)
            print(f"{name}: RENDER TIMEOUT", flush=True)
            continue
        secs = round(time.time() - t0, 1)
        if rc != 0:
            results[name] = {"error": f"render rc={rc}", "secs": secs}
            json.dump(results, open(summary_path, "w"), indent=1)
            print(f"{name}: RENDER FAIL rc={rc} {secs}s", flush=True)
            continue
        try:
            r = run([sys.executable, K3DIFF, ref, mp4, "--out",
                     os.path.join(OUT, f"{name}-diff")], DIFF_TIMEOUT,
                    os.path.join(OUT, f"{name}.diff.log"))
        except subprocess.TimeoutExpired:
            results[name] = {"error": "diff timeout", "secs": secs}
            json.dump(results, open(summary_path, "w"), indent=1)
            print(f"{name}: DIFF TIMEOUT", flush=True)
            continue
        sj = os.path.join(OUT, f"{name}-diff", "summary.json")
        m = json.load(open(sj)) if os.path.exists(sj) else {}
        entry = {"secs": secs, "mae": m.get("mae_mean"), "p8": m.get("avg_p8"),
                 "frames": m.get("frames")}
        results[name] = entry
        json.dump(results, open(summary_path, "w"), indent=1)
        print(f"{name}: p8={entry['p8']} mae={entry['mae']} {secs}s "
              f"{entry['frames']}f", flush=True)

if __name__ == "__main__":
    main(sys.argv[1:])
