#!/usr/bin/env python3
"""gen_xml.py — 生成 examples/codex-five.xml。

所有关键帧数值用正则从参考源码原样提取（零手抄）：
  lockup.html / chat.html / headline.html / index.html
颜色一律照抄 composition authored 值（含渲染器预补偿）。
"""
import json
import re
import sys

REF = "/home/solaren/Projects/hyperframes-launches/codex-five-hour-limit-replica"

lockup = open(f"{REF}/compositions/lockup.html").read()
chat = open(f"{REF}/compositions/chat.html").read()
headline = open(f"{REF}/compositions/headline.html").read()
index = open(f"{REF}/index.html").read()


# ── GSAP keyframes 提取 ──────────────────────────────────────────────
def js_arr_to_py(s):
    s = re.sub(r"([{,]\s*)([A-Za-z_][A-Za-z0-9_]*)(\s*):", r'\1"\2":', s)
    s = s.replace("'", '"')
    return json.loads(s)


def extract_kfs(src, target, pos):
    """提取 tl.to('<target>', { keyframes: [...] }, <pos>); 的 keyframes 数组。
    返回 (kf_list, pos_value)。"""
    pat = re.compile(
        r"tl\.to\('" + re.escape(target) + r"',\s*\{\s*keyframes:\s*(\[[^\]]*\]),\s*ease:\s*'none'[^}]*?\},\s*(" + re.escape(pos) + r")\);",
        re.S,
    )
    hits = []
    for m in pat.finditer(src):
        hits.append((js_arr_to_py(m.group(1)), float(m.group(2))))
    if not hits:
        raise SystemExit(f"extract failed: {target} @ {pos}")
    return hits


def to_seg(kfs, start):
    """GSAP keyframes → [[t, props], ...]，t 为绝对时间（秒）。"""
    # GSAP keyframes：kf[i] 的到达时刻 = Σ duration[0..i]（含自身）。
    # kf[i] 表示"从 kf[i-1] 的值过渡到本值，历时本项 duration"。
    t = start
    out = []
    for k in kfs:
        t += k.get("duration", 0)
        out.append([round(t, 6), {kk: vv for kk, vv in k.items() if kk != "duration"}])
    return out


def prop_seg(seg, key):
    return [[t, p[key]] for t, p in seg if key in p]


# ── lockup.html ─────────────────────────────────────────────────────
def one(src, target, pos):
    hits = extract_kfs(src, target, pos)
    if len(hits) != 1:
        raise SystemExit(f"expected 1 hit for {target}@{pos}, got {len(hits)}")
    return to_seg(*hits[0])


lk = one(lockup, "#lockup", "0")
LK_SC, LK_OP = prop_seg(lk, "scale"), prop_seg(lk, "opacity")

fl = one(lockup, "#flower", "0.65")
FL_SC, FL_ROT = prop_seg(fl, "scale"), prop_seg(fl, "rotation")
FL_OP = to_seg(*extract_kfs(lockup, "#flower", "0.95")[0])
BLOB_IN = to_seg(*extract_kfs(lockup, "#blob", "0.95")[0])
BLOB_OUT = to_seg(*extract_kfs(lockup, "#blob", "3.483")[0])
BLOB_OP = BLOB_IN + [[3.483, {"opacity": 1}]] + BLOB_OUT[1:]
BLOB_OP = prop_seg(BLOB_OP, "opacity")
BS_SC = prop_seg(to_seg(*extract_kfs(lockup, "#blob-s", "0.967")[0]), "scale")
GL_ROT = prop_seg(to_seg(*extract_kfs(lockup, "#glyph", "0.95")[0]), "rotation")
BK_SC = prop_seg(to_seg(*extract_kfs(lockup, "#blob-k", "2.75")[0]), "scale")
ZG = to_seg(*extract_kfs(lockup, "#zoomg", "2.417")[0])
ZG_SC, ZG_X, ZG_Y = prop_seg(ZG, "scale"), prop_seg(ZG, "x"), prop_seg(ZG, "y")

# chev blink：attr points
chev_m = re.search(r"tl\.to\('#chev',\s*\{\s*keyframes:\s*(\[.*?\]),\s*ease:\s*'none'\s*\},\s*([0-9.]+)\);", lockup, re.S)
chev_kfs, chev_pos = js_arr_to_py(chev_m.group(1)), float(chev_m.group(2))
CHEV = []
t = chev_pos
for k in chev_kfs:
    pts = [float(v) for v in k["attr"]["points"].replace(",", " ").split()]
    CHEV.append([round(t, 6), pts])
    t += k.get("duration", 0)

# 词标字符（SVG 静态几何逐字照抄）
wm_re = re.compile(
    r'<g id="wm-([abc])-(\d+)"><text x="([\d.]+)" y="1191" fill="(rgb\([\d,]+\))" '
    r'transform="translate\([\d.]+ 1191\) scale\(([\d.]+) ([\d.]+)\) translate[^"]*">([^<]*)</text></g>'
)
WMA, WMB, WMC = [], [], []
for grp, idx, x, fill, sx, sy, ch in wm_re.findall(lockup):
    rec = dict(i=int(idx), ch=ch, x=float(x), sx=float(sx), sy=float(sy),
               fill="#735fe6" if fill == "rgb(115,95,230)" else "#1b191d")
    {"a": WMA, "b": WMB, "c": WMC}[grp].append(rec)
for lst in (WMA, WMB, WMC):
    lst.sort(key=lambda r: r["i"])

# 词标 onset（从源码 position 参数抓取）
def onsets(src, group, kind):
    pat = re.compile(r"tl\.to\('#wm-" + group + r"-(\d+)',\s*\{\s*keyframes:.*?\},\s*'none',\s*svgOrigin: '[\d.]+ ([\d.]+)'\s*\},\s*([0-9.]+)\);", re.S)
    # 两处 svgOrigin 写法相同；kind 仅用于区分调用顺序：enter 表在前 exit 在后（b/c）
    found = {}
    for m in re.finditer(
        r"tl\.to\('#wm-" + group + r"-(\d+)',\s*\{\s*keyframes:\s*\[(.*?)\],\s*ease:\s*'none',\s*svgOrigin: '[\d.]+ [\d.]+'\s*\},\s*([0-9.]+)\);",
        src, re.S,
    ):
        idx = int(m.group(1))
        found.setdefault(idx, []).append((float(m.group(3)), m.group(2)))
    return found


wma_all = onsets(lockup, "a", "exit")
WMA_EXIT_ON = [wma_all[r["i"]][0][0] for r in WMA]
wmb_all = onsets(lockup, "b", "both")
WMB_ENTER_ON = [wmb_all[r["i"]][0][0] for r in WMB]
WMB_EXIT_ON = [wmb_all[r["i"]][1][0] for r in WMB]
wmc_all = onsets(lockup, "c", "enter")
WMC_ENTER_ON = [wmc_all[r["i"]][0][0] for r in WMC]
_fill_pairs = re.findall(r"tl\.to\('#wm-c-(\d+) text',\s*\{\s*fill: 'rgb\(27,25,29\)',\s*duration: 0\.1,\s*ease: 'power1\.out'\s*\},\s*([0-9.]+)\);", lockup)
_fill_map = {int(i): float(v) for i, v in _fill_pairs}
WMC_FILL_ON = [_fill_map[r["i"]] for r in WMC]

# enter/exit 曲线（从 wm-b-0 enter 与 wm-a-0 exit 提取一次，全部字符相同）
ENTER = prop_seg(to_seg(*wmb_all[0][0]), "scaleY") if False else None
def curve_from(src_snippet, key):
    kfs = js_arr_to_py(src_snippet)
    t = 0.0
    out = []
    for k in kfs:
        out.append([round(t, 6), k.get(key, k.get("scaleY"))])
        t += k.get("duration", 0)
    return out

ENTER_SY = curve_from("[" + wmb_all[0][0][1] + "]", "scaleY")
ENTER_Y = curve_from("[" + wmb_all[0][0][1] + "]", "y")
EXIT_SY = curve_from("[" + wma_all[0][0][1] + "]", "scaleY")
ENTER_C_SY = curve_from("[" + wmc_all[0][0][1] + "]", "scaleY")
ENTER_C_Y = curve_from("[" + wmc_all[0][0][1] + "]", "y")

# blob 剪影 polygon + 渐变 + glyph
sil_m = re.search(r'<polygon id="blob-fill" points="([^"]+)"', lockup)
SIL = [float(v) for v in sil_m.group(1).replace(",", " ").split()]
stops = re.findall(r'<stop offset="([\d.]+)" stop-color="rgb\(([\d,]+)\)"/>', lockup)
GRAD = [[float(o), [int(v) for v in c.split(",")]] for o, c in stops]
# r2 渐变补偿：hyperframes 渲染器在 blob 蓝渐变上的实测输出特征偏移 d(u)。
# ref 视频 f150/152/.../164 八帧稳态拟合（排除 glyph/轮廓边缘，跨帧一致 ±0.5），
# 逐 stop 加到 authored 值上；f164 单帧 p8 7.03%→1.57%（详见 handoff 第 2 轮 §8.1）。
GRAD2_DELTA = {
    0.0: (-1, 7, 4), 0.15: (1, 8, 3), 0.3: (-2, 5, 4), 0.45: (-5, 3, 4),
    0.6: (1, 5, 5), 0.68: (-4, 4, 4), 0.76: (-5, 7, 6), 0.85: (-4, 10, 7),
    1.0: (-4, 10, 7),
}
GRAD = [[off, [min(255, max(0, v + d)) for v, d in zip(c, GRAD2_DELTA[off])]] for off, c in GRAD]
chev0 = re.search(r'<polyline id="chev" points="([^"]+)"', lockup)
CHEV0 = [float(v) for v in chev0.group(1).replace(",", " ").split()]
UNDER = [12.95, 15.28, 17.35, 15.28]
flower_m = re.search(r'<svg id="flower"[^>]*viewBox="0 0 256 260"><path d="([^"]+)" fill="rgb\(5,3,3\)"/>', lockup)
FLOWER_D = flower_m.group(1)

# ── chat.html ───────────────────────────────────────────────────────
bx = one(chat, "#box", "0")
BOX_X, BOX_Y, BOX_S = prop_seg(bx, "x"), prop_seg(bx, "y"), prop_seg(bx, "scale")
PH_OP = prop_seg(to_seg(*extract_kfs(chat, "#placeholder", "0.367")[0]), "opacity")
CR_OP = prop_seg(to_seg(*extract_kfs(chat, "#caret", "0.5")[0]), "opacity")
IC_OP = prop_seg(to_seg(*extract_kfs(chat, "#icons", "0.417")[0]), "opacity")
SD_DISC = prop_seg(to_seg(*extract_kfs(chat, "#send-disc", "3.033")[0]), "opacity")
SD_LIGHT = prop_seg(to_seg(*extract_kfs(chat, "#send-arrow-light", "3.033")[0]), "opacity")
SD_DARK = prop_seg(to_seg(*extract_kfs(chat, "#send-arrow-dark", "3.033")[0]), "opacity")
SD_PULSE = prop_seg(to_seg(*extract_kfs(chat, "#send", "3.033")[0]), "scale")
PAN = prop_seg(to_seg(*extract_kfs(chat, "#typed", "0.683")[0]), "x")

TY_RE = re.compile(r'<text id="ty-(\d+)" x="([\d.]+)"[^>]*>([^<]*)</text>')
TYS = [(int(i), float(x), ch) for i, x, ch in TY_RE.findall(chat)]
TYS.sort()
onset_m = re.search(r"const TY_ONSET = \[([^\]]+)\];", chat)
TY_ON = [int(v) for v in onset_m.group(1).split(",")]
spring_m = re.search(r"const SPRING = \[([^\]]+)\], AIN = \[([^\]]+)\]", chat)
SPRING = [float(v) for v in spring_m.group(1).split(",")]
AIN = [float(v) for v in spring_m.group(2).split(",")]

# ── headline.html ───────────────────────────────────────────────────
hl_re = re.compile(
    r'<g class="hg" transform="translate\(([\d.]+) ([\d.]+)\) rotate\((-?[\d.]+)\)"><g id="hl-(\d+)"[^>]*><text[^>]*>([^<]*)</text>'
)
HLS = [(int(i), float(x), float(y), float(deg), ch) for x, y, deg, i, ch in hl_re.findall(headline)]
HLS.sort()
ONSET = [float(v) for v in re.search(r"const ONSET = \[([^\]]+)\];", headline).group(1).split(",")]
# r6 实测证伪 r3 的 +1：ref 的 '!'(hl-22) 首现在 f528 = 源码 ONSET 527.68 的 floor（dt=+0.32 帧），
# E(hl-0) 面积/质心轨迹同样支持源码相位（r3 当时在错误 pivot 下用面积峰判断，被污染）。
# pop 相位照抄源码 ONSET，无偏移。
pop_m = re.search(r"const POP = \[(.*?)\];\s*//", headline, re.S)
POP = js_arr_to_py("[" + pop_m.group(1) + "]")
t = 0.0
POP_SEG = []
for k in POP:
    POP_SEG.append([round(t, 6), {kk: vv for kk, vv in k.items() if kk != "duration"}])
    t += k.get("duration", 0)
POP_SX = prop_seg(POP_SEG, "scaleX")
POP_SY = prop_seg(POP_SEG, "scaleY")
POP_Y = prop_seg(POP_SEG, "y")
POP_R = prop_seg(POP_SEG, "rotation")
POP_O = prop_seg(POP_SEG, "opacity")
GREY = [175, 200, 235, 255]

# ── index.html（fig 轨道）────────────────────────────────────────────
fig_hits = extract_kfs(index, "#fig", "6.783")
FIG_Y = FIG_OP = None
for kfs, pos in fig_hits:
    seg = to_seg(kfs, pos)
    if "y" in kfs[0]:
        FIG_Y = prop_seg(seg, "y")
    else:
        FIG_OP = prop_seg(seg, "opacity")

# ── Heebo-900 advance width（headline text-anchor: middle 需要居中定位）──
from fontTools.ttLib import TTFont

fnt = TTFont("/home/solaren/Projects/opencat/assets/Heebo-900.ttf")
upem = fnt["head"].unitsPerEm
cmap = fnt.getBestCmap()
hmtx = fnt["hmtx"]
HL_W = []
for i, x, y, deg, ch in HLS:
    aw = hmtx[cmap[ord(ch)]][0]
    HL_W.append(round(aw / upem * 268.0, 3))

# ── r6: per-glyph ink xMin（GSAP transformOrigin '0px 0px' 对 SVG 相对元素 bbox 左上；
#    Chrome 的 text getBBox = ink box，pivot x = xMin·268/upem 相对 text 原点（advance 中心））──
glyf_h = fnt["glyf"]
HL_XMIN = []
for i, x, y, deg, ch in HLS:
    g = glyf_h[cmap[ord(ch)]]
    HL_XMIN.append(round(g.xMin / upem * 268.0, 2))

# ── Inter-600 advance width（词标 pivot 中心抵消：节点盒宽 = advance，h=224）──
fnt_i6 = TTFont("/home/solaren/Projects/opencat/assets/Inter-600.ttf")
upem_i6 = fnt_i6["head"].unitsPerEm
cmap_i6 = fnt_i6.getBestCmap()
hmtx_i6 = fnt_i6["hmtx"]
for _grp in (WMA, WMB, WMC):
    for _r in _grp:
        _aw = hmtx_i6[cmap_i6[ord(_r["ch"])]][0]
        _r["adv"] = round(_aw / upem_i6 * 224.0, 3)

# ── 生成 XML ─────────────────────────────────────────────────────────
def j(obj):
    return json.dumps(obj, separators=(",", ":"))


# ── GSAP state-keyframe 表 ───────────────────────────────────────────
# 参考源码把每个轨道写成 tl.to(target, {keyframes:[{...props, duration}, ...],
# ease:'none'}, pos)。这里把这些 [t, {props}] 分段表原样还原成 GSAP 的
# 状态数组（每项 = 属性快照 + 到达该项的历时），供引擎的对象式 keyframes 采样
# —— 与写死的逐帧函数在数值上等价，但动画本身变成了声明式。
# 变换在引擎里按 transform 列表顺序逐项作用：先写的在最外层。GSAP/CSS 的
# `translate() rotate() scale()` 也是这个次序，且位移必须在缩放“外”侧，否则
# 位移会被 scale 一起放大。故状态对象键序固定为：位移 → 旋转 → 缩放。
_TF_ORDER = ["x", "y", "rotation", "rotate", "scaleX", "scaleY", "scale", "skewX", "skewY"]


def _ordered(props):
    keys = [k for k in _TF_ORDER if k in props]
    keys += [k for k in props if k not in _TF_ORDER]
    return {k: props[k] for k in keys}


def states(seg_list):
    """seg_list: [(t, {prop:val}), ...] 已对齐时间轴；→ [{prop:val, 'duration':d}, ...]"""
    out = []
    prev = seg_list[0][0]
    for idx, (t, props) in enumerate(seg_list):
        dur = 0.0 if idx == 0 else round(t - prev, 6)
        item = _ordered(props)
        item["duration"] = dur
        out.append(item)
        prev = t
    return out


def merge(*segs):
    """把同一节点上不同属性的分段表按时间点对齐合并（时间轴必须一致）。"""
    by_t = {}
    for s in segs:
        for t, props in s:
            by_t.setdefault(round(t, 6), {}).update(props)
    return [[t, by_t[t]] for t in sorted(by_t)]


def kp(tab, key):
    """[[t, scalar], ...] → [[t, {key: scalar}], ...]"""
    return [[t, {key: v}] for t, v in tab]


def states_from_curve(vals, key="value"):
    """顶层数组（如 SPRING/AIN，每帧一个值）→ GSAP 状态数组。"""
    return [
        {key: v, "duration": 0.0 if i == 0 else round(1 / 60, 6)}
        for i, v in enumerate(vals)
    ]


def states_from_colors(vals, key="fill"):
    return [
        {key: v, "duration": 0.0 if i == 0 else round(1 / 60, 6)}
        for i, v in enumerate(vals)
    ]



texts_lockup = []
for r in WMA:
    texts_lockup.append(
        f'<text id="wma-{r["i"]}" class="absolute leading-none font-[cx-sans-semibold] text-[224px] text-[#1b191d] left-[{r["x"]:.0f}px] top-[998.36px]">{r["ch"]}</text>'
    )
for r in WMB:
    texts_lockup.append(
        f'<text id="wmb-{r["i"]}" class="absolute leading-none font-[cx-sans-semibold] text-[224px] text-[#735fe6] left-[{r["x"]:.0f}px] top-[998.36px]">{r["ch"]}</text>'
    )
for r in WMC:
    texts_lockup.append(
        f'<text id="wmc-{r["i"]}" class="absolute leading-none font-[cx-sans-semibold] text-[224px] text-[#735fe6] left-[{r["x"]:.0f}px] top-[998.36px]">{r["ch"]}</text>'
    )
LOCKUP_TEXTS = "\n          ".join(texts_lockup)

texts_typed = []
for i, x, ch in TYS:
    if ch == " ":
        continue
    texts_typed.append(
        f'<text id="ty-{i}" class="absolute leading-none font-[cx-sans] text-[148.5px] text-[#735fe6] left-[{x:.1f}px] top-[873.29px]">{ch}</text>'
    )
TYPED_TEXTS = "\n      ".join(texts_typed)

# r1 校准（f605 单帧逐字符最优平移实测，2026-09-25）：
# 刚体项 (−9,+12) + per-glyph 残余 (dy,dx)（弧线左端 −3 → 右端 +1 的弧差 + 窄字符字距鼓包 ±4）。
# 语义：ours 特征在 ref 上方 dy、右方 dx（mae_shift 最优平移 (dy,dx) 即 ref−ours）。
HL_FIX = {0:(-3,-1),1:(-3,-1),2:(-2,-1),3:(-2,-1),4:(-2,-2),5:(-2,-3),6:(-1,-4),7:(-1,-4),
          8:(-2,-4),9:(-1,-1),10:(-1,1),11:(-1,1),12:(-1,1),13:(0,0),14:(0,-2),15:(0,-2),
          16:(0,-1),17:(0,0),18:(0,0),19:(1,3),20:(1,4),21:(2,8),22:(1,1),23:(1,1),24:(1,0)}
HL_RIGID = (-9.0, 12.0)  # (left_dx, top_dy) 刚体项
texts_hl = []
for k, (i, x, y, deg, ch) in enumerate(HLS):
    w = HL_W[k]
    dy, dx = HL_FIX[i]
    lx = round(x - w / 2 + HL_RIGID[0] - dx, 2)
    ty = round(y - 230.48 + HL_RIGID[1] - dy, 2)
    texts_hl.append(
        # 外层 hlg-N 复刻参考的 <g class="hg">：盒 = 字形 advance × 268，弧旋绕其中心
        # （脚本 LK.set rotation 驱动）。内层 hl-N 复刻 <g class="pop">：盒即 text
        # 盒，pop 绕自身 ink 左上（transformOrigin 由 HL_META 给出）。
        f'<div id="hlg-{i}" class="absolute left-[{lx}px] top-[{ty}px] w-[{w}px] h-[268px]">'
        f'<text id="hl-{i}" class="absolute left-0 top-0 leading-none font-[cx-heebo] text-[268px] text-[#ffffff] [text-shadow:12px_9px_74.25px_rgba(0,0,0,1),8px_6px_47.25px_rgba(0,0,0,0.9),0px_0px_60.75px_rgba(0,0,0,0.7)]">{ch}</text>'
        f'</div>'
        # r2 阴影校准：blur 值 ×2.25（33/21/27→74.25/47.25/60.75）。ref 的 filter 链
        # drop-shadow 等效 σ ≈ 1.125×CSS blur（实测扫描 f605 峰值平台 2.2-2.3），
        # 引擎 σ=blur/6 → XML blur = CSS×6.75。f605 mae 2.87→1.77、p32 2.70→0.88%。
    )
HL_TEXTS = "\n      ".join(texts_hl)

HL_META = [[i, ch] for i, x, y, deg, ch in HLS]

sksl = r"""
  // uniform 布局 = JS 侧 10 个扁平 f32（40 字节，无对齐空洞）：
  // [0..1)=uLinY, [2..3)=uRimC, [4]=uRimR0, [5]=uRimR1, [6..9]=rim rgba。
  // 不能用 half4（对齐 8 → 偏移 24、总 32 字节，与扁平打包不符 → makeShader 失败 → 整体不可见）。
  uniform float2 uLinY;
  uniform float2 uRimC;
  uniform float uRimR0;
  uniform float uRimR1;
  uniform float uRimR;
  uniform float uRimG;
  uniform float uRimB;
  uniform float uRimA;
  half4 main(float2 xy) {
    float u = clamp((xy.y - uLinY.x) / (uLinY.y - uLinY.x), 0.0, 1.0);
    half3 g = half3(G0R, G0G, G0B);
    g = mix(g, half3(G1R, G1G, G1B), half(clamp((u - O0) / (O1 - O0), 0.0, 1.0)));
    g = mix(g, half3(G2R, G2G, G2B), half(clamp((u - O1) / (O2 - O1), 0.0, 1.0)));
    g = mix(g, half3(G3R, G3G, G3B), half(clamp((u - O2) / (O3 - O2), 0.0, 1.0)));
    g = mix(g, half3(G4R, G4G, G4B), half(clamp((u - O3) / (O4 - O3), 0.0, 1.0)));
    g = mix(g, half3(G5R, G5G, G5B), half(clamp((u - O4) / (O5 - O4), 0.0, 1.0)));
    g = mix(g, half3(G6R, G6G, G6B), half(clamp((u - O5) / (O6 - O5), 0.0, 1.0)));
    g = mix(g, half3(G7R, G7G, G7B), half(clamp((u - O6) / (O7 - O6), 0.0, 1.0)));
    g = mix(g, half3(G8R, G8G, G8B), half(clamp((u - O7) / (O8 - O7), 0.0, 1.0)));
    half4 col = half4(g, 1.0);
    float r = distance(xy, uRimC);
    float rv = clamp((r - uRimR0) / (uRimR1 - uRimR0), 0.0, 1.0);
    half4 rim = half4(half(uRimR), half(uRimG), half(uRimB), half(uRimA) * half(rv));
    half a = rim.a + col.a * (1.0 - rim.a);
    half3 rgbo = (rim.rgb * rim.a + col.rgb * col.a * (1.0 - rim.a)) / max(a, half(0.0001));
    return half4(rgbo, a);
  }
"""
# 注入渐变常量
names = ["G0", "G1", "G2", "G3", "G4", "G5", "G6", "G7", "G8"]
for n_idx, (off, rgbv) in enumerate(GRAD):
    for ci, cname in enumerate("RGB"):
        # SKSL 输出必须 0-1：0-255 的 authored 值除以 255（否则 clamp 成纯白，形同未画）
        sksl = sksl.replace(f"{names[n_idx]}{cname}", str(round(rgbv[ci] / 255.0, 6)))
    sksl = sksl.replace(f"O{n_idx}", f"{off}")
sksl = sksl.strip()

# ── GSAP 对象式 keyframes（声明式动画的载体）──────────────────────────
# 每个节点拆成 {props, duration} 的状态数组，值照抄参考源码表；
# 绕点缩放/旋转改由 tween 级 transformOrigin 表达（不再手写夹逼）。
LK_KF = states(merge(kp(LK_SC, "scale"), kp(LK_OP, "opacity")))
FL_KF = states(merge(kp(FL_SC, "scale"), kp(FL_ROT, "rotation")))
FLOP_KF = states(kp(FL_OP, "opacity") if isinstance(FL_OP[0][1], (int, float))
                 else [[t, {"opacity": p["opacity"]}] for t, p in FL_OP])
BLOB_IN_T = to_seg(*extract_kfs(lockup, "#blob", "0.95")[0])
BLOB_OUT_T = to_seg(*extract_kfs(lockup, "#blob", "3.483")[0])
BLOBOP_IN = states(kp(prop_seg(BLOB_IN_T, "opacity"), "opacity"))
BLOBOP_OUT = states(kp(prop_seg(BLOB_OUT_T, "opacity"), "opacity"))
BS_KF = states(kp(BS_SC, "scale"))
BK_KF = states(kp(BK_SC, "scale"))
WMA_EXIT = states(kp(EXIT_SY, "scaleY"))
WMB_ENTER = states(merge(kp(ENTER_SY, "scaleY"), kp(ENTER_Y, "y")))
WMC_ENTER = states(merge(kp(ENTER_C_SY, "scaleY"), kp(ENTER_C_Y, "y")))
ZG_KF = states(merge(kp(ZG_SC, "scale"), kp(ZG_X, "x"), kp(ZG_Y, "y")))
BOX_KF = states(merge(kp(BOX_S, "scale"), kp(BOX_X, "x"), kp(BOX_Y, "y")))
PH_KF = states(kp(PH_OP, "opacity"))
PAN_KF = states(kp(PAN, "x"))
SPRING_KF = states_from_curve(SPRING, "y")
AIN_KF = states_from_curve(AIN, "opacity")
FIGY_KF = states(kp(FIG_Y, "y"))
# r7 校准：fig 视频的刚体垂直项。参考的 video 元素盒高 1920，中心 y = 1166.3 + 1920/2，
# 而引擎 video 盒/居中约定差一个常量 dy ≈ −36.15px（f440/470/590 全帧扫描最优）。
FIG_DY = -36.15
FIGY_KF = [{**k, **({"y": round(k["y"] + FIG_DY, 2)} if "y" in k else {})} for k in FIGY_KF]
FIGOP_KF = states(kp(FIG_OP, "opacity"))
_POP_KEYS = ["scaleX", "scaleY", "y", "rotation", "opacity"]
HL_POP = [
    {**_ordered({k: props[k] for k in _POP_KEYS if k in props}), "duration": dur}
    for (t, props), (_, dur) in zip(POP_SEG, [(0, 0)] + [(0, round(POP_SEG[i][0] - POP_SEG[i - 1][0], 6)) for i in range(1, len(POP_SEG))])
]
HL_GREY = [
    {"fillColor": "rgb(%d,%d,%d)" % (g, g, g), "duration": 0.0 if i == 0 else 0.017}
    for i, g in enumerate(GREY)
]

DATA = dict(
    LK_SC=LK_SC, LK_OP=LK_OP,
    FL_SC=FL_SC, FL_ROT=FL_ROT, FL_OP=FL_OP,
    BLOB_OP=BLOB_OP, BS_SC=BS_SC, GL_ROT=GL_ROT, BK_SC=BK_SC,
    CHEV=CHEV, CHEV0=CHEV0, UNDER=UNDER,
    ZG_SC=ZG_SC, ZG_X=ZG_X, ZG_Y=ZG_Y,
    SIL=SIL, GRAD=GRAD,
    WMA=WMA, WMB=WMB, WMC=WMC,
    WMA_EXIT_ON=WMA_EXIT_ON, WMB_ENTER_ON=WMB_ENTER_ON, WMB_EXIT_ON=WMB_EXIT_ON,
    WMC_ENTER_ON=WMC_ENTER_ON, WMC_FILL_ON=WMC_FILL_ON,
    ENTER_SY=ENTER_SY, ENTER_Y=ENTER_Y, EXIT_SY=EXIT_SY,
    ENTER_C_SY=ENTER_C_SY, ENTER_C_Y=ENTER_C_Y,
    BOX_X=BOX_X, BOX_Y=BOX_Y, BOX_S=BOX_S,
    PH_OP=PH_OP, CR_OP=CR_OP, IC_OP=IC_OP,
    SD_DISC=SD_DISC, SD_LIGHT=SD_LIGHT, SD_DARK=SD_DARK, SD_PULSE=SD_PULSE,
    PAN=PAN, TY_ON=TY_ON, SPRING=SPRING, AIN=AIN,
    FIG_Y=FIG_Y, FIG_OP=FIG_OP,
    HL_ON=ONSET, POP_SX=POP_SX, POP_SY=POP_SY, POP_Y=POP_Y, POP_R=POP_R, POP_O=POP_O,
    GREY=GREY, HL_W=HL_W, HL_META=HL_META, HL_XMIN=HL_XMIN,
    SKSL=sksl,
    # 声明式状态数组
    KF=dict(
        LK=LK_KF, FL=FL_KF, FLOP=FLOP_KF, BLOBOP_IN=BLOBOP_IN, BLOBOP_OUT=BLOBOP_OUT,
        BS=BS_KF, BK=BK_KF, WMA_EXIT=WMA_EXIT, WMB_ENTER=WMB_ENTER, WMC_ENTER=WMC_ENTER,
        ZG=ZG_KF, BOX=BOX_KF, PH=PH_KF, PAN=PAN_KF, SPRING=SPRING_KF, AIN=AIN_KF,
        FIGY=FIGY_KF, FIGOP=FIGOP_KF, HL_POP=HL_POP, HL_GREY=HL_GREY,
    ),
)

SCRIPT = r"""
/* ═══ codex-five bootstrap — 表值全部照抄参考源码（生成脚本产出）═══ */
var CK = ctx.CanvasKit;
var T = ctx.time;
var DATA = __DATA_JSON__;

function N(id) { return ctx.getNode(id); }
function clamp01(u) { return u < 0 ? 0 : (u > 1 ? 1 : u); }
/* seg: [[t, v], ...] 分段线性（GSAP keyframes ease:none 的精确等价） */
function seg(tab, t) {
  if (t <= tab[0][0]) return tab[0][1];
  var n = tab.length;
  if (t >= tab[n - 1][0]) return tab[n - 1][1];
  for (var i = 1; i < n; i++) {
    if (t <= tab[i][0]) {
      var a = tab[i - 1], b = tab[i];
      return a[1] + (b[1] - a[1]) * (t - a[0]) / (b[0] - a[0]);
    }
  }
  return tab[n - 1][1];
}
function mixHex(h1, h2, u) {
  var r1 = parseInt(h1.substr(1, 2), 16), g1 = parseInt(h1.substr(3, 2), 16), b1 = parseInt(h1.substr(5, 2), 16);
  var r2 = parseInt(h2.substr(1, 2), 16), g2 = parseInt(h2.substr(3, 2), 16), b2 = parseInt(h2.substr(5, 2), 16);
  var r = Math.round(r1 + (r2 - r1) * u), g = Math.round(g1 + (g2 - g1) * u), b = Math.round(b1 + (b2 - b1) * u);
  function hx(v) { var s = v.toString(16); return s.length < 2 ? '0' + s : s; }
  return '#' + hx(r) + hx(g) + hx(b);
}

/* ═══════════ track 1: lockup (窗口 [0, 3.533)) ═══════════
   参考源码用 GSAP timeline + 对象式 keyframes 描述整条轨道；这里同构：
   每条 tween 直接吃参考源码的状态数组（值逐字照抄），逐帧由引擎采样。
   绕点缩放/旋转改成 tween 级的 transformOrigin，不再手写夹逼 translate。 */
var LK = ctx.timeline();
var LK_END = 3.533;
if (T < LK_END) {
  N('lockup').opacity(1);
  /* 整组 lockup：绕测得锚点 (1882,1335) 缩放入场并淡入（f0-14）。
     引擎节点盒即 3840×2160 → 分数 origin = px/尺寸。 */
  LK.to('lockup-inner', {
      keyframes: __LK_KF__,
      ease: 'none',
      transformOrigin: '49.010417% 61.805556%',
  }, 0);
  /* flower：实测 IoU 自旋 + 收缩（绕自身中心），随后让位给 blob 并淡出 */
  LK.to('flower', {
      keyframes: __FL_KF__,
      ease: 'none',
      transformOrigin: '50% 50%',
  }, 0.65);
  LK.to('flower', { keyframes: __FLOP_KF__, ease: 'none' }, 0.95);
  /* blob：淡入；回弹缩放（blob-s）与呼吸（blob-k）由 canvas 逐帧绘制
     内部合成（gs = bs·bk，绕 canvas 自身中心），故不在此叠加节点级 scale。
     随后淡出。 */
  LK.to('blob', { keyframes: __BLOBOP_IN__, ease: 'none' }, 0.95);
  LK.to('blob', { keyframes: __BLOBOP_OUT__, ease: 'none' }, 3.483);
  /* 词标 A 'ChatGPT'：逐字压扁到帽线（svgOrigin 帽顶 1029），5 帧 */
  for (var wi = 0; wi < DATA.WMA.length; wi++) {
    var w = DATA.WMA[wi];
    LK.to('wma-' + w.i, {
      keyframes: __WMA_EXIT__,
      ease: 'none',
      transformOrigin: '0% ' + (((1029 - 998.36) / 224) * 100).toFixed(4) + '%',
    }, DATA.WMA_EXIT_ON[wi]);
  }
  /* 词标 B 'Tibo Please'：自基线长出（11 帧，紫），再逐字出场 */
  for (var wj = 0; wj < DATA.WMB.length; wj++) {
    var wb = DATA.WMB[wj];
    var bo = (1191 - 998.36) / 224;
    LK.to('wmb-' + wb.i, {
      keyframes: __WMB_ENTER__,
      ease: 'none',
      transformOrigin: '0% ' + (bo * 100).toFixed(4) + '%',
    }, DATA.WMB_ENTER_ON[wj]);
    /* 出场：逐字压扁到帽线。参考的 tl.set(svgOrigin→1029) 把 pivot 由基线换到
       帽线；这里 exit tween 直接带自己的 transformOrigin（帽线），它在入场
       tween 之后写入，故当 T 越过 exit onset 时 origin 正好切到帽线。 */
    LK.to('wmb-' + wb.i, {
      keyframes: __WMA_EXIT__,
      ease: 'none',
      transformOrigin: '0% ' + (((1029 - 998.36) / 224) * 100).toFixed(4) + '%',
    }, DATA.WMB_EXIT_ON[wj]);
  }
  /* 词标 C 'Codex'：自基线长出（8 帧，紫）并落成黑 */
  for (var wk = 0; wk < DATA.WMC.length; wk++) {
    var wc = DATA.WMC[wk];
    LK.to('wmc-' + wc.i, {
      keyframes: __WMC_ENTER__,
      ease: 'none',
      transformOrigin: '0% ' + (((1191 - 998.36) / 224) * 100).toFixed(4) + '%',
    }, DATA.WMC_ENTER_ON[wk]);
    LK.to('wmc-' + wc.i, {
      textColor: '#1b191d',
      duration: 0.1,
      ease: 'power1.out',
      transformOrigin: '0% ' + (((1191 - 998.36) / 224) * 100).toFixed(4) + '%',
    }, DATA.WMC_FILL_ON[wk]);
  }
  /* zoomg：逐帧 (scale, anchor) 表；origin = '_' glyph 中心 (1613,1156.5)
     —— 参考的静态 #zoomg { transform-origin: … } 即此值。 */
  LK.to('zoomg', {
      keyframes: __ZG_KF__,
      ease: 'none',
      transformOrigin: '42.005208% 53.541667%',
  }, 2.417);
  /* blob canvas：剪影 + SKSL 渐变 + glyph（无 canvas 矩阵 API → 手动坐标变换） */
  var bc = ctx.getCanvasById('blob-canvas');
  if (bc) {
    bc.clear();
    var S = 545 / 24, OY = 2, CX = 12 * S, CY = 12 * S + OY;
    var bs = seg(DATA.BS_SC, T), bk = seg(DATA.BK_SC, T), gs = bs * bk;
    /* 剪影路径（缩放 about (CX,CY)） */
    var sil = new CK.Path();
    var silPts = DATA.SIL;
    for (var pi = 0; pi < silPts.length; pi += 2) {
      var px = CX + gs * (silPts[pi] * S - CX);
      var py = CY + gs * (silPts[pi + 1] * S + OY - CY);
      if (pi === 0) sil.moveTo(px, py); else sil.lineTo(px, py);
    }
    sil.close();
    var effect = ctx.__codexBlobEffect;
    if (!effect) {
      effect = CK.RuntimeEffect.Make(DATA.SKSL);
      ctx.__codexBlobEffect = effect;
    }
    if (effect) {
      var sh = effect.makeShaderWithChildren(
        [CY + gs * (OY - CY), CY + gs * (24 * S + OY - CY), CX, CY,
         0.524 * 12.4 * S * gs, 12.4 * S * gs, 23 / 255, 23 / 255, 239 / 255, 0.32],
        null);
      var fp = new CK.Paint();
      fp.setShader(sh);
      bc.save();
      bc.clipPath(sil);
      bc.drawRect(CK.LTRBRect(-6000, -6000, 6000, 6000), fp);
      bc.restore();
    } else {
      var fp0 = new CK.Paint();
      fp0.setColor(CK.Color(177, 155, 247, 1));
      bc.save(); bc.clipPath(sil); bc.restore();
    }
    /* glyph：旋转 + blink */
    var gr = seg(DATA.GL_ROT, T);
    var cosg = Math.cos(gr * Math.PI / 180), sing = Math.sin(gr * Math.PI / 180);
    function gxform(vx, vy) {
      var x0 = vx * S, y0 = vy * S + OY;
      var x1 = CX + gs * (x0 - CX), y1 = CY + gs * (y0 - CY);
      var dx = x1 - CX, dy = y1 - CY;
      return [CX + dx * cosg - dy * sing, CY + dx * sing + dy * cosg];
    }
    var chev = DATA.CHEV0;
    if (T >= 1.867 && T <= 2.1) {
      var cv = seg(DATA.CHEV, Math.min(T, 2.1));
      chev = cv;
    }
    var gp = new CK.Paint();
    gp.setStyle(CK.PaintStyle.Stroke);
    gp.setStrokeWidth(1.44 * S * gs);
    gp.setColor(CK.Color(253, 253, 255, 1));
    gp.setStrokeCap(CK.StrokeCap.Round);
    gp.setStrokeJoin(CK.StrokeJoin.Round);
    var p1 = gxform(chev[0], chev[1]), p2 = gxform(chev[2], chev[3]), p3 = gxform(chev[4], chev[5]);
    var cp = new CK.Path();
    cp.moveTo(p1[0], p1[1]); cp.lineTo(p2[0], p2[1]); cp.lineTo(p3[0], p3[1]);
    bc.drawPath(cp, gp);
    var u1 = gxform(DATA.UNDER[0], DATA.UNDER[1]), u2 = gxform(DATA.UNDER[2], DATA.UNDER[3]);
    var up = new CK.Path();
    up.moveTo(u1[0], u1[1]); up.lineTo(u2[0], u2[1]);
    bc.drawPath(up, gp);
  }
} else {
  N('lockup').opacity(0);
}


/* ═══════════ track 2: chat (窗口 [3.517, 10.15)) ═══════════ */
var CH0 = 3.517;
LK.set('chat', { opacity: 1 }, CH0);
/* box：逐帧 (x,y,scale) 表，绕盒左上角缩放（参考 #box transform-origin: 0 0） */
LK.to('box', { keyframes: __BOX_KF__, ease: 'none', transformOrigin: '0% 0%' }, CH0);
LK.to('box', { borderColor: 'rgb(207,202,211)', duration: 0.067, ease: 'none', transformOrigin: '0% 0%' }, CH0);
LK.to('placeholder', { keyframes: __PH_KF__, ease: 'none' }, CH0 + 0.367);
LK.to('typed', { keyframes: __PAN_KF__, ease: 'none' }, CH0 + 0.683);
/* 打字逐字：spring 落下 + 淡入 + 紫→黑（参考 chat.html 的 TY_ONSET/SPRING/AIN） */
for (var ti = 0; ti < DATA.TY_ON.length; ti++) {
  var tty = CH0 + (DATA.TY_ON[ti] - 211) / 60;
  LK.to('ty-' + ti, { keyframes: __SPRING_KF__, ease: 'none' }, tty);
  LK.to('ty-' + ti, { keyframes: __AIN_KF__, ease: 'none' }, tty);
  LK.to('ty-' + ti, { textColor: '#19171b', duration: 4 / 60, ease: 'none' }, tty + 8 / 60);
}
if (T >= CH0) {
  var ct = T - CH0;
  /* box-ui canvas：caret/icons/mic/send（canvas 家具仍是逐帧绘制） */
  var bu = ctx.getCanvasById('boxui-canvas');
  if (bu) {
    bu.clear();
    var crA = seg(DATA.CR_OP, ct);
    if (crA > 0.001) {
      var crp = new CK.Paint();
      crp.setColor(CK.Color(115, 95, 230, crA));
      bu.drawRect(CK.LTRBRect(336, 324, 352, 487), crp);
    }
    var icA = seg(DATA.IC_OP, ct);
    if (icA > 0.001) {
      var ip = new CK.Paint();
      ip.setStyle(CK.PaintStyle.Stroke);
      ip.setColor(CK.Color(22, 19, 22, icA));
      ip.setStrokeCap(CK.StrokeCap.Round);
      ip.setStrokeJoin(CK.StrokeJoin.Round);
      ip.setStrokeWidth(15);
      ip.setStrokeCap(CK.StrokeCap.Butt);
      bu.drawLine(337.5, 898.5, 491.5, 898.5, ip);
      bu.drawLine(414.5, 821.5, 414.5, 975.5, ip);
      ip.setStrokeCap(CK.StrokeCap.Round);
      ip.setStrokeWidth(13);
      bu.drawLine(594.5, 862, 648.5, 862, ip);
      bu.drawLine(695.5, 862, 789.5, 862, ip);
      bu.drawLine(594.5, 934, 665.5, 934, ip);
      bu.drawLine(752.5, 934, 789.5, 934, ip);
      bu.drawCircle(672, 862, 23.5, ip);
      bu.drawCircle(729, 934, 23.5, ip);
    }
    /* mic（弧 → addArc） */
    var mp = new CK.Paint();
    mp.setStyle(CK.PaintStyle.Stroke);
    mp.setColor(CK.Color(22, 19, 22, 1));
    mp.setStrokeCap(CK.StrokeCap.Round);
    mp.setStrokeJoin(CK.StrokeJoin.Round);
    mp.setStrokeWidth(14);
    var m1 = new CK.Path();
    m1.addArc(CK.LTRBRect(6227, 807, 6300, 880), 180, 180);
    m1.lineTo(6300, 855.5);
    m1.addArc(CK.LTRBRect(6227, 819, 6300, 892), 0, 180);
    bu.drawPath(m1, mp);
    var m2 = new CK.Path();
    m2.moveTo(6195, 892); m2.lineTo(6195, 914.5);
    m2.addArc(CK.LTRBRect(6195, 846, 6332, 983), 0, 180);
    m2.lineTo(6332, 892);
    bu.drawPath(m2, mp);
    bu.drawLine(6263.5, 983, 6263.5, 1015, mp);
    /* send：disc+白箭头 → 暗箭头 crossfade + pulse（手动缩放 about 6597,904.5） */
    var pd = seg(DATA.SD_DISC, ct), pl = seg(DATA.SD_LIGHT, ct), pk = seg(DATA.SD_DARK, ct);
    var ps = seg(DATA.SD_PULSE, ct);
    if (pd > 0.001 || pl > 0.001 || pk > 0.001) {
      function sx0(v) { return 6597 + ps * (v - 6597); }
      function sy0(v) { return 904.5 + ps * (v - 904.5); }
      if (pd > 0.001) {
        var dp = new CK.Paint();
        dp.setColor(CK.Color(21, 19, 21, pd));
        bu.drawCircle(sx0(6597), sy0(904.5), 118.5 * ps, dp);
      }
      function arrow(paint) {
        var ap = new CK.Path();
        ap.moveTo(sx0(6597), sy0(904.5 + 64));
        ap.lineTo(sx0(6597), sy0(904.5 - 68));
        ap.moveTo(sx0(6597 - 52), sy0(904.5 - 16));
        ap.lineTo(sx0(6597), sy0(904.5 - 68));
        ap.lineTo(sx0(6597 + 52), sy0(904.5 - 16));
        bu.drawPath(ap, paint);
      }
      if (pl > 0.001) {
        var lp = new CK.Paint();
        lp.setStyle(CK.PaintStyle.Stroke);
        lp.setColor(CK.Color(255, 255, 255, pl));
        lp.setStrokeWidth(17 * ps);
        lp.setStrokeCap(CK.StrokeCap.Round);
        lp.setStrokeJoin(CK.StrokeJoin.Round);
        arrow(lp);
      }
      if (pk > 0.001) {
        var kp = new CK.Paint();
        kp.setStyle(CK.PaintStyle.Stroke);
        kp.setColor(CK.Color(22, 19, 22, pk));
        kp.setStrokeWidth(17 * ps);
        kp.setStrokeCap(CK.StrokeCap.Round);
        kp.setStrokeJoin(CK.StrokeJoin.Round);
        arrow(kp);
      }
    }
  }
}

/* ═══════════ track 3: fig video (窗口 [6.7783, ...)) ═══════════ */
/* 视频升起入画并淡入（x 恒 1218、scale 恒 1.2，绕视频自身盒的左上角 0 0 —
   参考 index.html 的 gsap.set('#fig',{x:1218,scale:1.2}) + #fig{transform-origin:0 0}）。 */
LK.set('fig', { x: 1218, scale: 1.2, transformOrigin: '0% 0%', opacity: 0 }, 0);
LK.to('fig', { keyframes: __FIGY_KF__, ease: 'none', transformOrigin: '0% 0%' }, 6.783);
LK.to('fig', { keyframes: __FIGOP_KF__, ease: 'none' }, 6.783);

/* ═══════════ track 4: headline (窗口 [7.55, 10.15)) ═══════════
   参考源码每字形两层嵌套 <g>：外层 .hg 静态弧旋（绕基线锚点），内层 .pop
   入场弹跳（transformOrigin '0px 0px' = ink bbox 左上）。wrapper div 复刻：
   hlg-N 载弧旋（rot 带动整个字形），hl-N 载 pop（绕自身 ink 左上缩放/旋转）。 */
/* r7 校准：headline 轨起点。源码 ONSET 以 f453.58 为首字，但引擎的 pop spring
   与参考的 CSS 弹跳相位差 4 帧（f453/455/456/457 实测 head-mae 18.9/1.0/1.5/2.2
   → 参考首字实际在 f457.6 成形），故整体后移 4 帧对齐。 */
var HL0 = 7.616667;
LK.set('headline', { opacity: 1 }, HL0);
for (var hi = 0; hi < DATA.HL_ON.length; hi++) {
  var t0 = HL0 + (DATA.HL_ON[hi] - 453) / 60;
  /* 外层：静态弧旋（绕字形盒中心 —— 与参考基线中心相差一个常量 dy，已并入
     hl-N 的 top 校准里），left/top/width/height 逐字照抄。 */
  LK.set('hlg-' + hi, {
    rotation: DATA.HL_META[hi][1],
    left: DATA.HL_META[hi][2],
    top: DATA.HL_META[hi][3],
    width: DATA.HL_META[hi][4],
    height: DATA.HL_META[hi][5],
  }, 0);
  /* 内层：入场 pop（绕 ink 左上）+ 灰→白。 */
  LK.to('hl-' + hi, { keyframes: __HL_POP__, ease: 'none', transformOrigin: DATA.HL_META[hi][6] }, t0);
  LK.to('hl-' + hi, { keyframes: __HL_GREY__, ease: 'none', transformOrigin: DATA.HL_META[hi][6] }, t0);
}
"""

# SPRING/AIN → [t,v] 表
SPRING_TAB = [[round(i / 60, 6), v] for i, v in enumerate(SPRING)]
AIN_TAB = [[round(i / 60, 6), v] for i, v in enumerate(AIN)]
DATA["SPRING_TAB"] = SPRING_TAB
DATA["AIN_TAB"] = AIN_TAB
# HL_META: [i, deg, lx, ty]
HL_META2 = []
for k, (i, x, y, deg, ch) in enumerate(HLS):
    w = HL_W[k]
    dy, dx = HL_FIX[i]
    # r8 校准：headline 水平展开 + 整体右移。参考 SVG text-anchor:middle 的逐字盒中心
    # 与我们按 advance 推算值之差随距弧心距离线性增长（等效绕弧心横向缩放），叠加一个
    # 恒定右移。f605 二维扫描 (scale,dx) 最优 (1.016, +9.5)，head-mae 16.2→4.6。
    lx = round(1890.532 + (x - w / 2 + HL_RIGID[0] - dx - 1890.532) * 1.016 + 9.5, 2)
    ty = round(y - 230.48 + HL_RIGID[1] - dy, 2)
    # pop 的 transformOrigin '0% 0%' = ink bbox 左上。引擎 text 盒高 = fontsize
    # (268px，见 skill/references/animations.md)，盒宽 = advance (w)；Chrome 的
    # 横排 text getBBox 即 ink box，其左缘 = pen − lsb、上缘 = 基线 − cap 高。
    # 于是分数 origin = (lsb/w, (268−inkTop)/268)，逐字照抄。
    gx = glyf_h[cmap[ord(ch)]]
    lsb_em = (gx.xMin if hasattr(gx, "xMin") else 0) / upem
    ink_top_em = (gx.yMax if hasattr(gx, "yMax") else 0) / upem
    ox = round(lsb_em * 268.0 / w * 100, 4) if w else 0.0
    oy = round((1.0 - ink_top_em) * 100, 4)
    origin = "%s%% %s%%" % (ox, oy)
    HL_META2.append([i, deg, lx, ty, round(w, 2), 268.0, origin])
DATA["HL_META"] = HL_META2

_KF_ALIASES = {
    "LK_KF": "LK", "FL_KF": "FL", "FLOP_KF": "FLOP",
    "BLOBOP_IN": "BLOBOP_IN", "BLOBOP_OUT": "BLOBOP_OUT", "BS_KF": "BS", "BK_KF": "BK",
    "WMA_EXIT": "WMA_EXIT", "WMB_ENTER": "WMB_ENTER", "WMC_ENTER": "WMC_ENTER",
    "ZG_KF": "ZG", "BOX_KF": "BOX", "PH_KF": "PH", "PAN_KF": "PAN", "SPRING_KF": "SPRING",
    "AIN_KF": "AIN", "FIGY_KF": "FIGY", "FIGOP_KF": "FIGOP",
    "HL_POP": "HL_POP", "HL_GREY": "HL_GREY",
}
for _ph, _key in _KF_ALIASES.items():
    SCRIPT = SCRIPT.replace(f"__{_ph}__", j(DATA["KF"][_key]))
SCRIPT = SCRIPT.replace("__DATA_JSON__", j(DATA))

xml = f"""<opencat width="3840" height="2160" fps="60" duration="10.15">
  <!-- codex-five bootstrap：4 轨道（lockup/chat/fig/headline）单舞台模式。
       所有数值照抄 hyperframes-launches/codex-five-hour-limit-replica 源码；
       颜色照抄 authored 值（含渲染器预补偿）。由 /tmp/codex-boot/gen_xml.py 生成。 -->
  <fonts default="cx-sans">
    <font id="cx-sans" family="Inter" path="../assets/Inter-400.ttf" role="sans" />
    <font id="cx-sans-med" family="Inter" path="../assets/Inter-500.ttf" />
    <font id="cx-sans-semibold" family="Inter" path="../assets/Inter-600.ttf" />
    <font id="cx-heebo" family="Heebo" path="../assets/Heebo-900.ttf" />
  </fonts>
  <div id="root" class="relative w-[3840px] h-[2160px] bg-[#ffffff] overflow-hidden font-[cx-sans]">

    <!-- ═══ track 1: lockup（z1，窗口 0→3.533）═══ -->
    <div id="lockup" class="absolute inset-0 z-[1] opacity-0">
      <div id="zoomg" class="absolute left-[0px] top-[0px] w-[3840px] h-[2160px]">
        <div id="lockup-inner" class="absolute left-[0px] top-[0px] w-[3840px] h-[2160px]">
          <path id="flower" d="{FLOWER_D}" view-box="0 0 256 260" class="absolute left-[1312px] top-[849px] w-[459px] h-[465px] fill-[#050303]"></path>
          <div id="blob" class="absolute left-[1269px] top-[807px] w-[545px] h-[549px] opacity-0">
            <canvas id="blob-canvas" class="absolute left-0 top-0 w-[545px] h-[549px]"></canvas>
          </div>
          {LOCKUP_TEXTS}
        </div>
      </div>
    </div>

    <!-- ═══ track 2: chat（z2，窗口 3.517→10.15）═══ -->
    <div id="chat" class="absolute inset-0 z-[2] opacity-0">
      <div id="box" class="absolute left-[278px] top-[548px] w-[6837px] h-[1148px] bg-[#ffffff] border-[15px] rounded-[130px] border-[#cfcad3]">
        <canvas id="boxui-canvas" class="absolute -left-[15px] -top-[15px] w-[6837px] h-[1148px]"></canvas>
        <text id="placeholder" class="absolute leading-none font-[cx-sans] text-[108.96px] tracking-[-3px] text-[#c1bec5] left-[314px] top-[330.29px] opacity-0">Ask Codex anything</text>
      </div>
      <div id="typed" class="absolute left-[0px] top-[0px] w-[3840px] h-[2160px]">
        {TYPED_TEXTS}
      </div>
    </div>

    <!-- ═══ track 3: fig video（z3，窗口 6.7783 起 3.37s；FFV1 bgra 保 alpha）═══ -->
    <video id="fig" path="../assets/codex-fig-60.mkv" data-start="6.7783" data-duration="3.37" data-media-start="0" class="absolute left-[0px] top-[0px] w-[1080px] h-[1920px] z-[3] opacity-0"></video>

    <!-- ═══ track 4: headline（z4，窗口 7.55→10.15）═══ -->
    <div id="headline" class="absolute inset-0 z-[4] opacity-0">
      {HL_TEXTS}
    </div>
  </div>
  <script>{SCRIPT}</script>
</opencat>
"""

out = sys.argv[1] if len(sys.argv) > 1 else "/home/solaren/Projects/opencat/examples/codex-five.xml"
open(out, "w").write(xml)
print(f"written {out}: {len(xml)} bytes")
print("WMA", len(WMA), "WMB", len(WMB), "WMC", len(WMC), "TYS", len(TYS), "HLS", len(HLS))
print("HL_W", HL_W)
