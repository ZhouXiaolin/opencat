# 排版

我们skia支持的字体。

## 禁用字体

每个 LLM 都会默认使用的训练数据字体。这些会导致各合成之间出现同质化。

Inter、Roboto、Open Sans、Noto Sans、Arimo、Lato、Source Sans、PT Sans、Nunito、Poppins、Outfit、Sora、Playfair Display、Cormorant Garamond、Bodoni Moda、EB Garamond、Cinzel、Prata、Syne

**特别是 Syne** 是最被滥用的"特色"展示字体。它是一眼就能识别的 AI 设计标志。

## 护栏

你知道这些规则但你在违反它们。停下来。

- **不要配对两种无衬线字体。** 你一直这样做——一种用于标题，一种用于正文。跨越边界：衬线 + 无衬线，或无衬线 + 等宽。
- **每个场景一种表现性字体。** 你选择两种有趣的字体试图让它"更好"。一种表现，一种退居幕后。
- **粗细对比必须极端。** 你默认使用 400 对比 700。视频需要 300 对比 900。差异必须在运动中一目了然。
- **使用视频尺寸，而非网页尺寸。** 正文：最小 20px。标题：60px+。数据标签：16px。你会试图使用 14px。不要。

## 你不被告知就不会做的事

- **张力应有含义。** 不要模式匹配配对。问为什么这两种字体不协调。配对应体现内容的矛盾——机械 vs 人性、公共 vs 私人、机构 vs 个人。如果你说不清张力在哪里，那就是武断的。
- **语域切换。** 为不同的传达模式分配不同的字体——一种声音用于陈述，另一种用于数据，另一种用于署名。不是页面上的层次结构。而是对话中的声音。
- **张力可以存在于单一字体内部。** 一种看起来熟悉但暗藏怪异的字体会与观众的期望产生张力，而不是与另一种字体。
- **一个变量变化 = 戏剧性对比。** 相同字形，等宽 vs 比例。相同家族的不同光学大小。只改变节奏而其他一切保持不变。
- **双重个性可行。** 两种表现性字体可以共存，如果它们共享一种态度（都玩世不恭，都精确），即使它们的形式完全不同。
- **时间即层次。** 最先出现的元素最重要。在视频中，序列取代了位置。
- **运动即排版。** 一个词如何进入携带的意义与字体本身一样多。0.1s 的猛击 vs 2s 的淡入——相同字体，完全不同的信息。
- **固定的阅读时间。** 屏幕上停留 3 秒 = 必须在 2 秒内可读。更少的词，更大的字号。
- **字距比网页更紧密。** 展示尺寸上 -0.03em 到 -0.05em。视频编码会压缩文字细节。

## 查找字体

不要默认使用你熟悉的字体。如果内容是奢侈品，怪诞无衬线可能比预期的迪多衬线创造更多张力。先决定语域，然后搜索。

将此脚本保存到 `/tmp/fontquery.py` 并使用 `curl -s 'https://fonts.google.com/metadata/fonts' > /tmp/gfonts.json && python3 /tmp/fontquery.py /tmp/gfonts.json` 运行：

```python
import json, sys, random
from collections import OrderedDict

random.seed()  # 每次运行真随机

with open(sys.argv[1]) as f:
    data = json.load(f)
fonts = data.get("familyMetadataList", [])

ban = {"Inter","Roboto","Open Sans","Noto Sans","Lato","Poppins","Source Sans 3",
       "PT Sans","Nunito","Outfit","Sora","Playfair Display","Cormorant Garamond",
       "Bodoni Moda","EB Garamond","Cinzel","Prata","Arimo","Source Sans Pro","Syne"}
skip_pfx = ("Roboto","Noto ","Google Sans","Bpmf","Playwrite","Anek","BIZ ",
            "Nanum","Shippori","Sawarabi","Zen ","Kaisei","Kiwi ","Yuji ","Radio ")

def ok(f):
    if f["family"] in ban: return False
    if any(f["family"].startswith(b) for b in skip_pfx): return False
    if "latin" not in (f.get("subsets") or []): return False
    return True

seen = set()
R = OrderedDict()

# 流行无衬线 — 近期 (2022+)，流行 (<300)
R["流行无衬线"] = []
for f in fonts:
    if not ok(f) or f["family"] in seen: continue
    if f.get("category") in ("Sans Serif","Display") and f.get("dateAdded","") >= "2022-01-01" and f.get("popularity",9999) < 300:
        R["流行无衬线"].append(f); seen.add(f["family"])

# 流行衬线 — 近期 (2018+)，流行 (<600)
R["流行衬线"] = []
for f in fonts:
    if not ok(f) or f["family"] in seen: continue
    if f.get("category") == "Serif" and f.get("dateAdded","") >= "2018-01-01" and f.get("popularity",9999) < 600:
        R["流行衬线"].append(f); seen.add(f["family"])

# 等宽 — 近期 (2018+)，流行 (<600)
R["等宽"] = []
for f in fonts:
    if not ok(f) or f["family"] in seen: continue
    if f.get("category") == "Monospace" and f.get("dateAdded","") >= "2018-01-01" and f.get("popularity",9999) < 600:
        R["等宽"].append(f); seen.add(f["family"])

# 冲击与窄体 — 800+ 粗细的重型展示字体
R["冲击与窄体"] = []
for f in fonts:
    if not ok(f) or f["family"] in seen: continue
    has_heavy = any(k in list(f.get("fonts",{}).keys()) for k in ("800","900"))
    is_display = f.get("category") in ("Sans Serif","Display")
    if has_heavy and is_display and f.get("popularity",9999) < 400:
        R["冲击与窄体"].append(f); seen.add(f["family"])

# 手写体 — 流行 (<300)
R["手写体"] = []
for f in fonts:
    if not ok(f) or f["family"] in seen: continue
    if f.get("category") == "Handwriting" and f.get("popularity",9999) < 300:
        R["手写体"].append(f); seen.add(f["family"])

# 随机化每个类别的前 5 名，使 LLM 不会总是选择相同的第一个结果
for cat in R:
    R[cat].sort(key=lambda x: x.get("popularity",9999))
    top5 = R[cat][:5]
    rest = R[cat][5:]
    random.shuffle(top5)
    R[cat] = top5 + rest
limits = {"流行无衬线":15,"流行衬线":12,"等宽":8,
          "冲击与窄体":12,"手写体":10}
for cat in R:
    items = R[cat][:limits.get(cat,10)]
    if not items: continue
    print(f"--- {cat} ({len(items)}) ---")
    for ff in items:
        var = "VAR" if ff.get("axes") else "   "
        print(f'  {ff.get("popularity"):4d} | {var} | {ff["family"]}')
    print()
```

五个类别：流行无衬线、流行衬线、等宽、冲击/窄体、手写体。全部从 Google Fonts 元数据动态过滤——没有硬编码的字体名称。配对时跨分类边界。

## 选择思维

不要通过类别条件反射地选择字体（编辑→衬线，科技→等宽，现代→几何无衬线）。那是模式匹配，不是设计。

1. **命名语域。** 内容使用什么声音？机构权威？个人表白？技术精确？随意不敬？语域比类别更能缩小选择范围。
2. **物理化思考。** 把字体想象成品牌可以出货的物理对象——博物馆展品说明、手绘店招、1970 年代大型机终端手册、外套内的织物标签、廉价新闻纸上的儿童书、税务表格。适合语域的物理对象指向了正确的字体_类型_。
3. **拒绝你的第一直觉。** 第一感觉正确的字体通常是你对该语域的训练数据默认值。如果你上次也选了它，找别的。
4. **交叉检查假设。** 编辑简报**不是**必须用衬线。技术简报**不是**必须用无衬线。儿童产品**不是**必须用圆润展示字体。最独特的选择往往与类别期望相矛盾。

## 相似字体配对

永远不要配对两种相似但不相同的字体——两种几何无衬线、两种过渡衬线、两种人文无衬线。它们会产生视觉摩擦而没有清晰的层次。观众感觉到有些"不对劲"但说不出来。要么使用一种字体的两个粗细，要么配对在多个轴上对比的字体：衬线 + 无衬线、窄体 + 宽体、几何 + 人文。

## 深色背景

深色背景上的浅色文字会产生两种需要补偿的光学错觉：

- **感知重量增加。** 浅色在深色上读起来比相同 `font-weight` 的深色在浅色上更重。正文使用 350 而不是 400。标题受影响较小，因为尺寸会补偿。
- **感知间距变紧。** 字形的浅色光晕减少了感知间隔。将 `line-height` 比你浅色背景的值增加 0.05-0.1。对于展示尺寸，增加 0.01em 的 `letter-spacing` 来抵消。
