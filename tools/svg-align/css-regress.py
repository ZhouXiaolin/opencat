"""resvg CSS 引擎回归对比：simplecss 基线 vs lightningcss 分支。

对 resvg 官方测试集逐个 SVG 用两个 resvg.exe 渲染，字节相等直接过，
不等则解码像素算 p8 差值分级。产出一行一案的 JSONL 报告 + 控制台摘要。

用法:
  python tools/svg-align/css-regress.py [--limit N] [--filter substr]
环境变量:
  RESVG_BASE  基线 exe（默认 F:/resvg/target/release/resvg.exe）
  RESVG_BRANCH 分支 exe（默认 F:/OpenCat/vendor/resvg/target/release/resvg.exe）
  RESVG_TESTS 语料目录（默认 F:/resvg/crates/resvg/tests/tests）
"""

import argparse
import json
import os
import struct
import subprocess
import sys
import tempfile
import zlib
from concurrent.futures import ProcessPoolExecutor
from pathlib import Path

BASE = Path(os.environ.get("RESVG_BASE", "F:/resvg/target/release/resvg.exe"))
BRANCH = Path(os.environ.get("RESVG_BRANCH", "F:/OpenCat/vendor/resvg/target/release/resvg.exe"))
CORPUS = Path(os.environ.get("RESVG_TESTS", "F:/resvg/crates/resvg/tests/tests"))
RENDER_TIMEOUT = 30


def run(exe, svg, png):
    try:
        r = subprocess.run(
            [str(exe), str(svg), str(png)],
            capture_output=True, timeout=RENDER_TIMEOUT,
        )
        return r.returncode
    except subprocess.TimeoutExpired:
        return -1


def decode_png(path):
    """最小 PNG 解码：8-bit RGB/RGBA 非隔行，反 filter 后返回行列表。"""
    data = Path(path).read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", "not a png"
    pos, idat, w, h, depth, ctype = 8, b"", 0, 0, 0, 0
    while pos < len(data):
        ln, tag = struct.unpack(">I4s", data[pos:pos + 8])
        chunk = data[pos + 8:pos + 8 + ln]
        if tag == b"IHDR":
            w, h, depth, ctype = struct.unpack(">IIBB", chunk[:10])
        elif tag == b"IDAT":
            idat += chunk
        pos += 12 + ln
    assert depth == 8 and ctype in (2, 6), f"unsupported png {depth}bit ctype{ctype}"
    ch = 3 if ctype == 2 else 4
    raw = zlib.decompress(idat)
    stride = w * ch
    out = bytearray()
    prev = bytearray(stride)
    p = 0
    for _ in range(h):
        f = raw[p]
        line = bytearray(raw[p + 1:p + 1 + stride])
        p += 1 + stride
        for i in range(stride):
            a = line[i - ch] if i >= ch else 0
            b = prev[i]
            c = prev[i - ch] if i >= ch else 0
            if f == 1:
                line[i] = (line[i] + a) & 0xFF
            elif f == 2:
                line[i] = (line[i] + b) & 0xFF
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 0xFF
            elif f == 4:
                pp = a + b - c
                pa, pb, pc = abs(pp - a), abs(pp - b), abs(pp - c)
                pr = a if (pa <= pb and pa <= pc) else (b if pb <= pc else c)
                line[i] = (line[i] + pr) & 0xFF
        out += line
        prev = line
    return w, h, bytes(out)


def one_case(args):
    svg, out_dir = args
    tag = f"{hash(str(svg)) & 0xFFFFFFFF:08x}"
    case_dir = out_dir / str(os.getpid()) / tag
    case_dir.mkdir(parents=True, exist_ok=True)
    png_a = case_dir / "a.png"
    png_b = case_dir / "b.png"
    rc_a = run(BASE, svg, png_a)
    rc_b = run(BRANCH, svg, png_b)
    case = {
        "svg": str(svg).replace("\\", "/").split("/tests/tests/")[-1],
        "rc_base": rc_a, "rc_branch": rc_b,
    }
    if rc_a != 0 or rc_b != 0:
        case["status"] = "render-error"
        return case
    # 两构建的 flate2 输出字节可能不同（zlib 版本漂移），只比解码后的像素。
    try:
        w, h, pa = decode_png(png_a)
        _, _, pb = decode_png(png_b)
    except Exception as e:  # noqa: BLE001
        case["status"] = "png-decode-error"
        case["error"] = str(e)
        return case
    if len(pa) != len(pb):
        case["status"] = "size-mismatch"
        return case
    if pa == pb:
        case["status"] = "identical"
        return case
    diff = sum(1 for x, y in zip(pa, pb) if abs(x - y) > 8)
    case["status"] = "pixel-diff"
    case["p8"] = round(diff / len(pa) * 100, 4)
    return case


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--filter", default="")
    ap.add_argument("--jobs", type=int, default=8)
    ap.add_argument("--out", default="out/css-regress.jsonl")
    args = ap.parse_args()

    svgs = sorted(CORPUS.rglob("*.svg"))
    if args.filter:
        svgs = [s for s in svgs if args.filter in str(s)]
    if args.limit:
        svgs = svgs[:args.limit]
    if not svgs:
        sys.exit(f"no svg found under {CORPUS}")

    with tempfile.TemporaryDirectory() as td:
        tdp = Path(td)
        jobs = [(s, tdp) for s in svgs]
        with ProcessPoolExecutor(args.jobs) as pool:
            results = list(pool.map(one_case, jobs))

    out = Path(args.out)
    out.parent.mkdir(parents=True, exist_ok=True)
    with out.open("w", encoding="utf-8") as f:
        for r in results:
            f.write(json.dumps(r) + "\n")

    n = {}
    worst = []
    for r in results:
        n[r["status"]] = n.get(r["status"], 0) + 1
        if r["status"] == "pixel-diff":
            worst.append((r["p8"], r["svg"]))
    worst.sort(reverse=True)
    print(f"total {len(results)}: {json.dumps(n, ensure_ascii=False)}")
    for p8, svg in worst[:20]:
        print(f"  p8={p8:6.2f}%  {svg}")
    print(f"report: {out}")


if __name__ == "__main__":
    main()
