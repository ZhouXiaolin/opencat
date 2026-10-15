#!/usr/bin/env python3
"""字体发现：打破"每次都伸手拿同样那几个字体"的惯性。

为什么需要它：模型选字体时会条件反射地回到训练数据里最常见的那几个
（Inter / Poppins / Playfair / Space Grotesk ...）。这个脚本从 Google Fonts
全量目录里按类别取样本，并打乱头部，逼出一个真正贴合内容的选择。

用法：

    # 列出候选（默认每类 8 个，已打乱头部）
    python3 font-discover.py

    # 只要某一类，要 12 个
    python3 font-discover.py --category serif --count 12

    # 选定字体后，取它的 TTF 直链（写进 JSX 的 fonts 导出）
    python3 font-discover.py --url "Fraunces" --weight 400
    python3 font-discover.py --url "Space Grotesk" --weight 700 --out ./assets/fonts

    # 一次拿多个字重
    python3 font-discover.py --url "Inter" --weight 400 --weight 700 --weight 900

数据源：fontsource API（分类/字重/子集），Google Fonts CSS API（TTF 直链）。
两者都不需要 API key。

注意：OpenCat 的字体加载支持 TTF / OTF / TTC / OTC，不支持 WOFF2。
本脚本只取 .ttf 直链，不要改用 woff2。
"""

import argparse
import json
import random
import re
import sys
import urllib.error
import urllib.request
from collections import OrderedDict

FONTSOURCE = "https://api.fontsource.org/v1/fonts"
CSS_API = "https://fonts.googleapis.com/css2?family={query}"
UA = "Mozilla/5.0"  # 不带现代 UA 时 Google 会返回 woff2

# 训练数据里被用滥的字体。它们能用，但读起来是默认味，不是选择。
BAN = {
    "Inter", "Roboto", "Open Sans", "Noto Sans", "Lato", "Poppins",
    "Source Sans 3", "Source Sans Pro", "PT Sans", "Nunito", "Outfit",
    "Sora", "Playfair Display", "Cormorant Garamond", "Bodoni Moda",
    "EB Garamond", "Cinzel", "Prata", "Arimo", "Syne", "Montserrat",
    "Raleway", "Work Sans", "DM Sans", "Manrope", "Rubik", "Karla",
}

# 非拉丁族、装饰性花体、以及 Google 的杂项系列，噪音太大。
SKIP_PREFIX = (
    "Noto ", "Bpmf", "Playwrite", "Anek", "BIZ ", "Nanum", "Shippori",
    "Sawarabi", "Zen ", "Kaisei", "Kiwi ", "Yuji ", "Radio ", "M PLUS ",
    "IBM Plex Sans JP", "IBM Plex Sans KR", "IBM Plex Sans Thai",
    "Material Symbols", "Material Icons",
)

# fontsource 的 category 字段 -> 展示用分组
CATEGORIES = OrderedDict([
    ("sans-serif", "Sans"),
    ("serif", "Serif"),
    ("monospace", "Mono"),
    ("display", "Display"),
    ("handwriting", "Script"),
])


def fetch_json(url, timeout=40):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return json.loads(r.read().decode("utf-8"))


def fetch_text(url, timeout=40):
    req = urllib.request.Request(url, headers={"User-Agent": UA})
    with urllib.request.urlopen(req, timeout=timeout) as r:
        return r.read().decode("utf-8")


def is_usable(f):
    """只要拉丁主体字体：defSubset 是 latin，且带 latin 子集。"""
    if f.get("family") in BAN:
        return False
    fam = f.get("family", "")
    if any(fam.startswith(p) for p in SKIP_PREFIX):
        return False
    # defSubset 是字体"主打"的语言。许多 CJK / Indic 字体也带 latin 子集，
    # 只看 subsets 会放进一堆非拉丁主体字体。
    if f.get("defSubset") != "latin":
        return False
    if "latin" not in (f.get("subsets") or []):
        return False
    # CJK / 其他文字体系的变体常以拉丁字体名 + 地区后缀出现
    if re.search(r"\b(TC|SC|HK|JP|KR|Thai|Arabic|Hebrew|Devanagari|Bengali)\b", fam):
        return False
    if not f.get("weights"):
        return False
    return True


def discover(count, only=None):
    """按类别列出候选，头部打乱。"""
    try:
        data = fetch_json(FONTSOURCE)
    except urllib.error.URLError as e:
        sys.exit(f"取字体目录失败：{e}\n（需要能访问 api.fontsource.org）")

    usable = [f for f in data if is_usable(f)]
    if not usable:
        sys.exit("字体目录为空或全部被过滤。")

    rng = random.Random()
    for cat, label in CATEGORIES.items():
        if only and cat != only:
            continue
        pool = [f for f in usable if f.get("category") == cat]
        if not pool:
            continue
        # 打乱后取前 N：避免每次都看到字母序最靠前的同一批
        rng.shuffle(pool)
        sample = pool[:count]
        print(f"--- {label} ({len(pool)} 可用，随机取 {len(sample)}) ---")
        for f in sorted(sample, key=lambda x: x["family"]):
            w = f.get("weights") or []
            wstr = f"{min(w)}-{max(w)}" if len(w) > 1 else str(w[0])
            var = " VAR" if f.get("variable") else ""
            print(f"  {f['family']:<28} {wstr:<10}{var}")
        print()


def resolve_url(family, weights):
    """把 family + 字重解析成 TTF 直链。"""
    out = []
    for w in weights:
        query = urllib.parse.quote(family) + f":wght@{w}"
        try:
            css = fetch_text(CSS_API.format(query=query))
        except urllib.error.URLError as e:
            sys.exit(f"查询 {family} {w} 失败：{e}")
        # 只取 truetype 行；woff2 行在带现代 UA 时才会出现，这里显式过滤
        urls = re.findall(r"url\((https://[^)]+\.ttf)\)", css)
        if not urls:
            # 该字重不存在时 CSS 会返回注释或空；回退到不带字重查询
            try:
                css = fetch_text(CSS_API.format(query=urllib.parse.quote(family)))
                urls = re.findall(r"url\((https://[^)]+\.ttf)\)", css)
            except urllib.error.URLError:
                pass
        if not urls:
            print(f"  ! {family} {w}：没有可用的 TTF（可能该字重不存在）", file=sys.stderr)
            continue
        out.append((w, urls[0]))
    return out


def download(family, weights, out_dir):
    import os
    resolved = resolve_url(family, weights)
    if not resolved:
        sys.exit("没有解析到任何 TTF。")
    slug = re.sub(r"[^a-z0-9]+", "-", family.lower()).strip("-")
    if out_dir:
        os.makedirs(out_dir, exist_ok=True)
    print(f"# {family} —— 写进 JSX 的 fonts 导出：\n")
    print("export const fonts = {")
    print("  faces: [")
    lines = []
    for w, url in resolved:
        fname = f"{slug}-{w}.ttf"
        if out_dir:
            path = os.path.join(out_dir, fname)
            try:
                req = urllib.request.Request(url, headers={"User-Agent": UA})
                with urllib.request.urlopen(req, timeout=60) as r, open(path, "wb") as fh:
                    fh.write(r.read())
                print(f"    下载 {path}", file=sys.stderr)
                lines.append(f"    {{ id: '{slug}{w}', family: '{family}', path: '{path}' }}")
            except urllib.error.URLError as e:
                print(f"    下载失败 {fname}：{e}", file=sys.stderr)
                lines.append(f"    {{ id: '{slug}{w}', family: '{family}', url: '{url}' }}")
        else:
            lines.append(f"    {{ id: '{slug}{w}', family: '{family}', url: '{url}' }}")
    for i, line in enumerate(lines):
        print(line + ("," if i < len(lines) - 1 else ","))
    print("  ],")
    print("};")


def main():
    ap = argparse.ArgumentParser(description="字体发现 / TTF 直链解析")
    ap.add_argument("--category", choices=list(CATEGORIES), help="只看某一类")
    ap.add_argument("--count", type=int, default=8, help="每类样本数（默认 8）")
    ap.add_argument("--url", metavar="FAMILY", help="解析这个 family 的 TTF 直链")
    ap.add_argument("--weight", type=int, action="append", help="字重，可重复")
    ap.add_argument("--out", metavar="DIR", help="下载到目录（省略则只打印 url）")
    args = ap.parse_args()

    if args.url:
        download(args.url, args.weight or [400], args.out)
        return

    if args.category:
        discover(args.count, only=args.category)
    else:
        discover(args.count)


if __name__ == "__main__":
    import urllib.parse  # noqa: E402  (resolve_url 用到)
    main()
