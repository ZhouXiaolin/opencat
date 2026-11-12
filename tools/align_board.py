#!/usr/bin/env python3
"""多指标对齐打分板：严格像素 + 感知 + 位置 + 形态。

k3diff 的 avg_p8 是严格口径（逐像素 max-channel |diff|>8），它对色度噪声和
逐像素相位极敏感，但对**整体位置错位**和**形态（形状）差异**并不敏感——
一个整屏平移 8px 的帧，p8 可能只有 2%，肉眼却一眼看出错位。

本工具按用户口径补齐多组指标：

  p8     严格：max-channel |diff|>8 的像素占比（与 tools/k3diff.py 同口径）
  d4     感知代理：4x4 BOX 下采样后同阈值占比（滤掉逐像素相位/色度噪声）
  ssim   灰度 8x8 窗 SSIM（结构相似度）
  ncc    全图归一化互相关峰值（整体结构一致性）
  shift  最佳整数位移 (dx,dy)，1/4 分辨率 FFT NCC —— **位置判据**
  shape  边缘图 IoU（Sobel 幅值分位阈值，二值边缘）—— **形态判据**（与颜色无关）

聚合判定：
  POS_FAIL   shift0 < 0.9 或 shift_max 超限  → 位置不一致（肉眼可见错位）
  SHAPE_FAIL avg_shape < 0.5                → 形态不一致（形状对不上）
  P8_FAIL    avg_p8 > 门限（默认 0.02）      → 严格像素门禁
  PASS       以上都不触发

用法：
  align_board.py case  REF TEST [--out DIR] [--shift 12] [--p8-max 0.02]
  align_board.py board --manifest MANIFEST --render-dir DIR [--out JSON]
                       [--only slug1,slug2] [--p8-max 0.02]

board 模式读 out/sweep/manifest.txt（slug|input|ref），在 --render-dir 里找
<slug>.mp4 与 <slug>.render.log，从日志的 [guard] 行取耗时与峰值内存。
"""
import argparse
import json
import os
import re
import subprocess
import sys

import numpy as np

# ---------------------------------------------------------------- 解码

def probe(path):
    out = subprocess.check_output(
        ["ffprobe", "-v", "error", "-select_streams", "v:0", "-count_frames",
         "-show_entries", "stream=nb_read_frames,width,height", "-of", "json", path],
        text=True,
    )
    s = json.loads(out)["streams"][0]
    return int(s["nb_read_frames"]), int(s["width"]), int(s["height"])


def decode(path, w, h, scale=False):
    """逐帧产出 (h,w,3) uint8。scale=True 时把画面缩放到 w x h。"""
    vf = f"scale={w}:{h}" if scale else "null"
    p = subprocess.Popen(
        ["ffmpeg", "-v", "error", "-i", path, "-vf", vf,
         "-f", "rawvideo", "-pix_fmt", "rgb24", "-"],
        stdout=subprocess.PIPE,
    )
    n = w * h * 3
    try:
        while True:
            buf = p.stdout.read(n)
            if len(buf) < n:
                break
            yield np.frombuffer(buf, np.uint8).reshape(h, w, 3)
    finally:
        p.stdout.close()
        p.wait()


# ---------------------------------------------------------------- 指标

def box_mean(a, r):
    """(2r+1)^2 盒滤波均值，纯 numpy 二维积分图实现（无 scipy）。

    积分图 I = 逐行逐列前缀和；盒和 = 四项容斥（I[y2][x2] - I[y1-1][x2]
    - I[y2][x1-1] + I[y1-1][x1-1]）。窗口以原图 (y,x) 为中心。
    """
    H, W = a.shape
    p = np.pad(a.astype(np.float64), r + 1, mode="reflect")
    I = p.cumsum(0).cumsum(1)
    y2 = slice(2 * r + 1, 2 * r + 1 + H)
    x2 = slice(2 * r + 1, 2 * r + 1 + W)
    y1 = slice(0, H)
    x1 = slice(0, W)
    win = I[y2, x2] - I[y1, x2] - I[y2, x1] + I[y1, x1]
    return win / float((2 * r + 1) ** 2)


def ssim_gray(a, b, r=4):
    """8x8 窗 SSIM（均值/方差/协方差全部盒滤波）。"""
    ga = a.mean(axis=2)
    gb = b.mean(axis=2)
    c1 = (0.01 * 255) ** 2
    c2 = (0.03 * 255) ** 2
    mx = box_mean(ga, r)
    my = box_mean(gb, r)
    vx = np.maximum(box_mean(ga * ga, r) - mx * mx, 0)
    vy = np.maximum(box_mean(gb * gb, r) - my * my, 0)
    cov = box_mean(ga * gb, r) - mx * my
    s = ((2 * mx * my + c1) * (2 * cov + c2)) / ((mx * mx + my * my + c1) * (vx + vy + c2))
    return float(s.mean())


def down4(a):
    h, w = a.shape[:2]
    h -= h % 4
    w -= w % 4
    a = a[:h, :w].astype(np.float32)
    return a.reshape(h // 4, 4, w // 4, 4, a.shape[2]).mean(axis=(1, 3))


def p8(a, b):
    return float((np.abs(a.astype(np.int16) - b.astype(np.int16)).max(axis=2) > 8).mean())


def p8_u8(a, b):
    """uint8 快路径：一次 subtract 求差，避免 int16 中间量的多次分配。"""
    d = np.subtract(a, b, dtype=np.int16)
    np.abs(d, out=d)
    return float((d.max(axis=2) > 8).mean())


def down4(a):
    """4x4 BOX 平均（整数求和，结果与 float mean 逐位一致）。"""
    h, w = a.shape[:2]
    h -= h % 4
    w -= w % 4
    s = a[:h, :w].reshape(h // 4, 4, w // 4, 4, a.shape[2]).sum(axis=(1, 3), dtype=np.uint32)
    return (s // 16).astype(np.uint8)


def down2(a):
    """2x2 BOX 平均（整数）。"""
    h, w = a.shape[:2]
    h -= h % 2
    w -= w % 2
    s = a[:h, :w].reshape(h // 2, 2, w // 2, 2, a.shape[2]).sum(axis=(1, 3), dtype=np.uint32)
    return (s // 4).astype(np.uint8)


def edges(gray):
    gy, gx = np.gradient(gray)
    return np.hypot(gx, gy)


def _thr_top(mag, pct):
    """分位阈值：用 partition（O(n)）代替 percentile（O(n log n)）。"""
    k = max(1, int(mag.size * (100.0 - pct) / 100.0))
    return np.partition(mag.ravel(), k)[k]


def shape_iou(a, b, pct=92):
    """边缘图 IoU：分位阈值二值化后求交并比 —— 只看形状，不管颜色。"""
    ea = edges(a.mean(axis=2))
    eb = edges(b.mean(axis=2))
    ta = ea >= _thr_top(ea, pct)
    tb = eb >= _thr_top(eb, pct)
    union = int((ta | tb).sum())
    if union == 0:
        return 1.0
    return float(int((ta & tb).sum()) / union)


def _ncc_peak(ref_gray, tst_gray, search):
    """Hann 窗加权的 FFT 互相关峰值位置（整数像素）。

    返回 (dx, dy, peak)。dx>0 表示 test 内容相对 ref 整体右移 dx 像素。
    """
    H, W = ref_gray.shape
    a = ref_gray - ref_gray.mean()
    b = tst_gray - tst_gray.mean()
    win = np.hanning(H)[:, None] * np.hanning(W)[None, :]
    a = a * win
    b = b * win
    cc = np.fft.irfft2(np.fft.rfft2(b) * np.conj(np.fft.rfft2(a)), s=(H, W))
    iy, ix = np.unravel_index(int(np.argmax(cc)), cc.shape)
    if iy > H // 2:
        iy -= H
    if ix > W // 2:
        ix -= W
    denom = np.sqrt(float((a * a).sum()) * float((b * b).sum())) + 1e-9
    return int(ix), int(iy), float(cc.max()) / denom


def best_shift(ref, tst, q=4):
    """test 相对 ref 的整数位移（原生像素）。q = 内部降采样倍数。

    降采样用 BOX 平均抑制色度噪声；位移分辨力 = q 像素。
    """
    H, W = ref.shape[:2]
    h4, w4 = H // q, W // q
    if h4 < 8 or w4 < 8:
        return 0, 0, 0.0
    a = ref[:h4 * q, :w4 * q].reshape(h4, q, w4, q, 3).mean(axis=(1, 3, 4)).astype(np.float32)
    b = tst[:h4 * q, :w4 * q].reshape(h4, q, w4, q, 3).mean(axis=(1, 3, 4)).astype(np.float32)
    ix, iy, peak = _ncc_peak(a, b, 0)
    return ix * q, iy * q, peak


def local_shift(ref, tst, nx=6, ny=3, search=8, min_ncc=0.5):
    """元素级位置判据：分窗 NCC 位移。

    返回 (moved_ratio, max_shift, hot_window)。moved_ratio = 位移非零的窗占比。
    平坦窗（相关峰值低）的位移无意义——按 min_ncc 跳过，不计入。
    """
    H, W = ref.shape[:2]
    ys = np.linspace(0, H, ny + 1).astype(int)
    xs = np.linspace(0, W, nx + 1).astype(int)
    moved = 0
    total = 0
    worst = (0, 0, 0)
    for iy in range(ny):
        for ix in range(nx):
            y0, y1 = ys[iy], ys[iy + 1]
            x0, x1 = xs[ix], xs[ix + 1]
            if y1 - y0 < 16 or x1 - x0 < 16:
                continue
            a = ref[y0:y1, x0:x1].mean(axis=2).astype(np.float32)
            b = tst[y0:y1, x0:x1].mean(axis=2).astype(np.float32)
            dx, dy, peak = _ncc_peak(a, b, search)
            if peak < min_ncc:
                # 平坦窗：相关峰无意义（噪声位置），不计。
                continue
            m = int(np.hypot(dx, dy))
            total += 1
            if m > 0:
                moved += 1
            if m > worst[2]:
                worst = (ix, iy, m)
    if total == 0:
        return 0.0, 0, (0, 0, 0)
    return moved / total, worst[2], (worst[0], worst[1])


# ---------------------------------------------------------------- 单案

def analyze(ref_path, test_path, search=12, half=True, min_ncc=0.5):
    """逐帧多指标。

    p8 在原生分辨率算（与 tools/k3diff.py 同口径）；其余感知/结构指标在
    半分辨率算（结构信息不丢，成本降 4 倍）。返回 rows。
    """
    nref, w, h = probe(ref_path)
    _, w2, h2 = probe(test_path)
    scale = (w, h) != (w2, h2)
    rows = []
    for i, (a, b) in enumerate(zip(decode(ref_path, w, h),
                                   decode(test_path, w, h, scale=scale))):
        r = {
            "frame": i,
            "p8": p8_u8(a, b),
            "d4": 0.0,
            "ssim": 1.0,
            "shape": 1.0,
            "ncc": 1.0,
            "dx": 0, "dy": 0, "shift": 0,
            "local_moved": 0.0, "local_max": 0,
        }
        if half and w >= 640 and h >= 360:
            ah = down2(a)
            bh = down2(b)
            r["d4"] = p8_u8(down4(ah), down4(bh))
            r["ssim"] = ssim_gray(ah, bh)
            r["shape"] = shape_iou(ah, bh)
            dx, dy, ncc = best_shift(ah, bh, q=2)
            r["dx"], r["dy"], r["ncc"] = dx * 2, dy * 2, ncc
            moved, lmax, _ = local_shift(ah, bh, min_ncc=min_ncc)
            r["local_moved"] = moved
            r["local_max"] = lmax * 2
        else:
            r["d4"] = p8_u8(down4(a), down4(b))
            r["ssim"] = ssim_gray(a, b)
            r["shape"] = shape_iou(a, b)
            dx, dy, ncc = best_shift(a, b, q=2)
            r["dx"], r["dy"], r["ncc"] = dx, dy, ncc
            moved, lmax, _ = local_shift(a, b, min_ncc=min_ncc)
            r["local_moved"] = moved
            r["local_max"] = lmax
        r["shift"] = int(round(np.hypot(r["dx"], r["dy"])))
        rows.append(r)
    return rows


def aggregate(rows, min_ncc=0.5):
    """聚合。位移类指标只在**可信帧**上统计（相关峰值够高 = 画面有纹理，
    位移有意义）；平坦帧的互相关峰是噪声，报出去会假报位置不一致。"""
    n = len(rows) or 1
    trusted = [r for r in rows if r["ncc"] >= min_ncc]
    shifts = [r["shift"] for r in trusted]
    zero = sum(1 for r in trusted if r["dx"] == 0 and r["dy"] == 0)
    agg = {
        "trusted_ratio": len(trusted) / n,
        "frames": len(rows),
        "avg_p8": float(np.mean([r["p8"] for r in rows])) if rows else 0.0,
        "avg_d4": float(np.mean([r["d4"] for r in rows])) if rows else 0.0,
        "avg_ssim": float(np.mean([r["ssim"] for r in rows])) if rows else 0.0,
        "avg_shape": float(np.mean([r["shape"] for r in rows])) if rows else 0.0,
        "avg_ncc": float(np.mean([r["ncc"] for r in rows])) if rows else 0.0,
        "shift0_ratio": (zero / len(trusted)) if trusted else 1.0,
        "shift_max": int(max(shifts)) if shifts else 0,
        "shift_p95": float(np.percentile(shifts, 95)) if shifts else 0.0,
        "local_moved_ratio": float(np.mean([r["local_moved"] for r in rows])) if rows else 0.0,
        "local_max": int(max([r["local_max"] for r in rows])) if rows else 0,
    }
    return agg


def verdict(agg, p8_max=0.02):
    """三道独立判据：位置 / 形态 / 严格像素。任一不过即不通过。

    位置判据分两层：整帧全局位移（shift0 / shift_max）与元素级局部位移
    （local_moved_ratio / local_max）——后者抓「某个元素位置不对」。
    """
    fails = []
    # 可信帧太少 = 画面整体太平，位移无从谈起（不算位置失败）。
    if agg["trusted_ratio"] >= 0.30:
        if agg["shift0_ratio"] < 0.90:
            fails.append(f"POS(shift0={agg['shift0_ratio']:.3f})")
        if agg["shift_max"] > 16:
            fails.append(f"POS(max={agg['shift_max']}px)")
        if agg["local_max"] > 24:
            fails.append(f"POS(local={agg['local_max']}px)")
    if agg["avg_shape"] < 0.50:
        fails.append(f"SHAPE(iou={agg['avg_shape']:.3f})")
    if agg["avg_p8"] > p8_max:
        fails.append(f"P8({agg['avg_p8'] * 100:.2f}%>{p8_max * 100:.0f}%)")
    return ("FAIL:" + ",".join(fails)) if fails else "PASS"


# ---------------------------------------------------------------- board

GUARD_RE = re.compile(r"\[guard\]\s+rc=(\d+)\s+elapsed=(\d+)s\s+peak_mem=(\d+)MB")


def read_manifest(path):
    cases = []
    with open(path, encoding="utf-8") as f:
        for line in f:
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split("|")
            if len(parts) < 3:
                continue
            slug, inp, ref = parts[0], parts[1], parts[2]
            cases.append({"slug": slug, "input": inp, "ref": ref})
    return cases


def read_guard(log_path):
    if not os.path.isfile(log_path):
        return {}
    with open(log_path, encoding="utf-8", errors="replace") as f:
        txt = f.read()
    m = GUARD_RE.findall(txt)
    if not m:
        return {}
    rc, el, mem = m[-1]
    return {"rc": int(rc), "elapsed_s": int(el), "peak_mem_mb": int(mem)}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=["case", "board"])
    ap.add_argument("ref", nargs="?")
    ap.add_argument("test", nargs="?")
    ap.add_argument("--out", default="")
    ap.add_argument("--shift", type=int, default=12, help="位移搜索半径(1/4 分辨率像素)")
    ap.add_argument("--p8-max", type=float, default=0.02)
    ap.add_argument("--manifest", default="")
    ap.add_argument("--render-dir", default="")
    ap.add_argument("--only", default="")
    ap.add_argument("--save-frames", default="", help="导出逐帧 JSON 的目录")
    args = ap.parse_args()

    if args.mode == "case":
        if not args.ref or not args.test:
            ap.error("case 模式需要 REF TEST")
        rows = analyze(args.ref, args.test, args.shift)
        agg = aggregate(rows)
        agg["verdict"] = verdict(agg, args.p8_max)
        print(json.dumps(agg, indent=2))
        if args.out:
            os.makedirs(args.out, exist_ok=True)
            with open(os.path.join(args.out, "align.json"), "w") as f:
                json.dump({"agg": agg, "rows": rows}, f, indent=1)
        return 0

    # board 模式
    cases = read_manifest(args.manifest)
    only = {s for s in args.only.split(",") if s}
    out = []
    print(f"{'case':<24} {'p8%':>7} {'d4%':>6} {'ssim':>7} {'shape':>6} "
          f"{'shift0':>7} {'maxpx':>6} {'lmove':>6} {'lmax':>5} "
          f"{'elapsed':>8} {'memMB':>7}  verdict")
    for c in cases:
        if only and c["slug"] not in only:
            continue
        test = os.path.join(args.render_dir, f"{c['slug']}.mp4")
        ref = c["ref"]
        if not (ref and os.path.isfile(ref) and os.path.isfile(test)):
            print(f"{c['slug']:<24} {'-':>7} {'-':>6} {'-':>7} {'-':>6} "
                  f"{'-':>7} {'-':>6} {'-':>6} {'-':>5} {'-':>8} {'-':>7}  SKIP(no ref/test)")
            continue
        rows = analyze(ref, test, args.shift)
        agg = aggregate(rows)
        agg["verdict"] = verdict(agg, args.p8_max)
        g = read_guard(os.path.join(args.render_dir, f"{c['slug']}.render.log"))
        agg.update(g)
        agg["slug"] = c["slug"]
        agg["ref"] = ref
        agg["test"] = test
        out.append(agg)
        print(f"{c['slug']:<24} {agg['avg_p8'] * 100:7.3f} {agg['avg_d4'] * 100:6.3f} "
              f"{agg['avg_ssim']:7.4f} {agg['avg_shape']:6.3f} {agg['shift0_ratio']:7.3f} "
              f"{agg['shift_max']:6d} {agg['local_moved_ratio']:7.3f} {agg['local_max']:6d} "
              f"{g.get('elapsed_s', -1):8d} {g.get('peak_mem_mb', -1):7d}"
              f"  {agg['verdict']}")
        if args.save_frames:
            os.makedirs(args.save_frames, exist_ok=True)
            with open(os.path.join(args.save_frames, f"{c['slug']}-align.json"), "w") as f:
                json.dump({"agg": agg, "rows": rows}, f, indent=1)
    if args.out:
        os.makedirs(os.path.dirname(os.path.abspath(args.out)), exist_ok=True)
        with open(args.out, "w", encoding="utf-8") as f:
            json.dump(out, f, indent=1, ensure_ascii=False)
        print(f"\nboard -> {args.out}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
