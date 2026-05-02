# Visual Style Library

命名视觉风格，用于 OpenCat compositions。每种风格对应一个真实的设计传统，以 design.md 兼容的 token block 表达。作为起点使用 — 将 YAML 复制到项目 `design.md` 的 front matter，然后将 hex 色值替换为 Tailwind token。

**如何选择：** 先匹配情绪，再匹配内容。问：*"观众应该感受到什么？"*

**如何适配 OpenCat：** YAML 中的 hex 色值转换为最接近的 Tailwind 色 token（如 `#0066FF` → `blue-600`）。时间单位为帧（@30fps，×30）。Easing 使用 opencat.md §5.1 预设名（`ease-out`、`ease-in-out`、`back-out` 等）。

## Quick Reference

| Style | Mood | Best for | 推荐转场 |
|-------|------|----------|---------|
| Swiss Pulse | Clinical, precise | SaaS, data, dev tools, metrics | fade / wipe |
| Velvet Standard | Premium, timeless | Luxury, enterprise, keynotes | fade |
| Deconstructed | Industrial, raw | Tech launches, security, punk | slide / gl_transition |
| Maximalist Type | Loud, kinetic | Big announcements, launches | clock_wipe / iris |
| Data Drift | Futuristic, immersive | AI, ML, cutting-edge tech | light_leak |
| Soft Signal | Intimate, warm | Wellness, personal stories, brand | fade |
| Folk Frequency | Cultural, vivid | Consumer apps, food, communities | slide / wipe |
| Shadow Cut | Dark, cinematic | Dramatic reveals, security, exposé | light_leak / iris |

---

## 1. Swiss Pulse — Josef Müller-Brockmann

**Mood:** Clinical, precise | **Best for:** SaaS dashboards, developer tools, APIs, metrics

```yaml
name: Swiss Pulse
colors:
  primary: "#1a1a1a"        # → slate-900
  on-primary: "#ffffff"      # → white
  accent: "#0066FF"          # → blue-600
typography:
  headline:
    fontSize: 5rem           # → text-[80px]
    fontWeight: 700          # → font-bold
  label:
    fontSize: 0.875rem       # → text-[14px]
    fontWeight: 400          # → font-normal
  stat:
    fontSize: 7rem           # → text-[112px]
    fontWeight: 700          # → font-bold
rounded:
  none: 0px                  # → rounded-none
  sm: 2px                    # → rounded-sm
spacing:
  sm: 8px                    # → gap-2 / p-2
  md: 16px                   # → gap-4 / p-4
  lg: 32px                   # → gap-8 / p-8
motion:
  energy: high
  easing:
    entry: "ease-out"        # 原 expo.out
    exit: "ease-in"          # 原 power4.in
    ambient: "linear"        # 原 none
  duration:
    entrance: 12             # 帧 (原 0.4s)
    hold: 45                 # 帧 (原 1.5s)
    transition: 18           # 帧 (原 0.6s)
  atmosphere:
    - grid-lines
    - registration-marks
  transition: wipe
```

网格锁定构图。每个元素对齐不可见的 12 列网格。数字主导画面（`text-[80px]`-`text-[120px]`）。动效计数从 0 滚动。无装饰转场。不漂浮。

---

## 2. Velvet Standard — Massimo Vignelli

**Mood:** Premium, timeless | **Best for:** Luxury products, enterprise software, keynotes, investor decks

```yaml
name: Velvet Standard
colors:
  primary: "#0a0a0a"         # → slate-950
  on-primary: "#ffffff"      # → white
  accent: "#1a237e"          # → indigo-900
typography:
  headline:
    fontSize: 3rem           # → text-[48px]
    fontWeight: 300          # → font-light
    letterSpacing: 0.15em    # → tracking-[0.15em]
    textTransform: uppercase # → uppercase
  body:
    fontSize: 1rem           # → text-[16px]
    fontWeight: 300          # → font-light
    lineHeight: 1.6          # → leading-relaxed
rounded:
  sm: 0px                    # → rounded-none
  md: 2px                    # → rounded-sm
spacing:
  sm: 16px                   # → gap-4 / p-4
  md: 32px                   # → gap-8 / p-8
  lg: 64px                   # → gap-16 / p-16
motion:
  energy: calm
  easing:
    entry: "ease-in-out"     # 原 sine.inOut
    exit: "ease-in"          # 原 power1.in
    ambient: "ease-in-out"   # 原 sine.inOut
  duration:
    entrance: 36             # 帧 (原 1.2s)
    hold: 90                 # 帧 (原 3.0s)
    transition: 45           # 帧 (原 1.5s)
  atmosphere:
    - subtle-grain
    - hairline-rules
  transition: fade
```

大量负空间。对称居中，建筑精确度。薄无衬线，全大写，宽字距。顺序揭示，长停留。一切有意图地滑动。奢华需要时间。

---

## 3. Deconstructed — Neville Brody

**Mood:** Industrial, raw | **Best for:** Tech news, developer launches, security products, punk-energy reveals

```yaml
name: Deconstructed
colors:
  primary: "#1a1a1a"         # → slate-900
  on-primary: "#f0f0f0"      # → slate-100
  accent: "#D4501E"          # → orange-600
typography:
  headline:
    fontSize: 4rem           # → text-[64px]
    fontWeight: 700          # → font-bold
  label:
    fontSize: 0.75rem        # → text-[12px]
    fontWeight: 700          # → font-bold
    textTransform: uppercase # → uppercase
rounded:
  none: 0px                  # → rounded-none
spacing:
  sm: 4px                    # → gap-1 / p-1
  md: 12px                   # → gap-3 / p-3
  lg: 24px                   # → gap-6 / p-6
motion:
  energy: high
  easing:
    entry: "back-out"        # 原 back.out(2.5)
    exit: "steps(8)"         # 原 steps(8)
    ambient: "elastic-out"   # 原 elastic.out(1.2, 0.4)
  duration:
    entrance: 9              # 帧 (原 0.3s)
    hold: 30                 # 帧 (原 1.0s)
    transition: 15           # 帧 (原 0.5s)
  atmosphere:
    - scan-lines
    - glitch-artifacts
    - grain-overlay
  transition: gl_transition
```

字体倾斜，边缘重叠，溢出画面。大胆工业感。扫描线效果、glitch 瑕疵作为设计元素。文字 SLAM 和 SHATTER。字母扰乱然后 snap 到最终位置。有意的不规则——不应感觉被抛光。

---

## 4. Maximalist Type — Paula Scher

**Mood:** Loud, kinetic | **Best for:** Big product launches, milestone announcements, high-energy hype videos

```yaml
name: Maximalist Type
colors:
  primary: "#0a0a0a"         # → slate-950
  on-primary: "#ffffff"      # → white
  accent-red: "#E63946"      # → red-500
  accent-yellow: "#FFD60A"   # → yellow-400
typography:
  headline:
    fontSize: 8rem           # → text-[128px]
    fontWeight: 400          # → font-normal
    textTransform: uppercase # → uppercase
  subhead:
    fontSize: 3rem           # → text-[48px]
    fontWeight: 700          # → font-bold
rounded:
  none: 0px                  # → rounded-none
spacing:
  sm: 0px                    # → gap-0
  md: 8px                    # → gap-2
motion:
  energy: high
  easing:
    entry: "ease-out"        # 原 expo.out
    exit: "back-out"         # 原 back.out(1.8)
    ambient: "ease-out"      # 原 power3.out
  duration:
    entrance: 9              # 帧 (原 0.3s)
    hold: 24                 # 帧 (原 0.8s)
    transition: 12           # 帧 (原 0.4s)
  atmosphere:
    - type-layers
    - color-blocks
  transition: iris
```

文字就是视觉。重叠字体层，不同尺度和角度，填充 50-80% 画面。大胆饱和色——最大对比度。一切动态：SLAM、SLIDE、SCALE。2-3 秒快节奏场景。无静态时刻。快速到达，硬停。

---

## 5. Data Drift — Refik Anadol

**Mood:** Futuristic, immersive | **Best for:** AI products, ML platforms, data companies, speculative tech

```yaml
name: Data Drift
colors:
  primary: "#0a0a0a"         # → slate-950
  on-primary: "#e0e0e0"      # → slate-300
  accent-purple: "#7c3aed"   # → purple-500
  accent-cyan: "#06b6d4"     # → cyan-500
typography:
  headline:
    fontSize: 2.5rem         # → text-[40px]
    fontWeight: 200          # → font-extralight
    letterSpacing: 0.05em    # → tracking-[0.05em]
  body:
    fontSize: 0.875rem       # → text-[14px]
    fontWeight: 300          # → font-light
rounded:
  sm: 4px                    # → rounded
  md: 12px                   # → rounded-xl
  full: 9999px               # → rounded-full
spacing:
  sm: 16px                   # → gap-4
  md: 32px                   # → gap-8
  lg: 64px                   # → gap-16
motion:
  energy: moderate
  easing:
    entry: "ease-in-out"     # 原 sine.inOut
    exit: "ease-out"         # 原 power2.out
    ambient: "ease-in-out"   # 原 sine.inOut
  duration:
    entrance: 30             # 帧 (原 1.0s)
    hold: 75                 # 帧 (原 2.5s)
    transition: 45           # 帧 (原 1.5s)
  atmosphere:
    - particle-field
    - light-traces
    - radial-glow
  transition: light_leak
```

薄未来感无衬线——漂浮、失重、极简。流畅变形构图。极端尺度切换（微→宏）。粒子聚合成数字。光线描绘数据路径。平滑、连续、有机。无硬边。

---

## 6. Soft Signal — Stefan Sagmeister

**Mood:** Intimate, warm | **Best for:** Wellness brands, personal stories, lifestyle products, human-centered apps

```yaml
name: Soft Signal
colors:
  primary: "#FFF8EC"         # → amber-50
  on-primary: "#2a2a2a"      # → neutral-800
  accent-amber: "#F5A623"    # → amber-500
  accent-rose: "#C4A3A3"     # → rose-300
  accent-sage: "#8FAF8C"     # → green-400
typography:
  headline:
    fontSize: 3rem           # → text-[48px]
    fontWeight: 400          # → font-normal
    fontStyle: italic        # → italic
  body:
    fontSize: 1rem           # → text-[16px]
    fontWeight: 300          # → font-light
    lineHeight: 1.7          # → leading-loose
rounded:
  sm: 8px                    # → rounded-lg
  md: 16px                   # → rounded-2xl
  lg: 24px                   # → rounded-3xl
  full: 9999px               # → rounded-full
spacing:
  sm: 12px                   # → gap-3
  md: 24px                   # → gap-6
  lg: 48px                   # → gap-12
motion:
  energy: calm
  easing:
    entry: "ease-in-out"     # 原 sine.inOut
    exit: "ease-in-out"      # 原 power1.inOut
    ambient: "ease-in-out"   # 原 sine.inOut
  duration:
    entrance: 30             # 帧 (原 1.0s)
    hold: 90                 # 帧 (原 3.0s)
    transition: 45           # 帧 (原 1.5s)
  atmosphere:
    - soft-gradient
    - warm-grain
  transition: fade
```

手写风格或人文衬线字体。个人化、小写、精致。近景构图：单个元素填充画面。缓慢漂浮，从不 snap。柔和有机动效。不应感觉匆忙或抛光。亲密，永不企业。

---

## 7. Folk Frequency — Eduardo Terrazas

**Mood:** Cultural, vivid | **Best for:** Consumer apps, food platforms, community products, festive launches

```yaml
name: Folk Frequency
colors:
  primary: "#ffffff"         # → white
  on-primary: "#1a1a1a"      # → slate-900
  accent-pink: "#FF1493"     # → pink-600
  accent-blue: "#0047AB"     # → blue-800
  accent-yellow: "#FFE000"   # → yellow-400
  accent-green: "#009B77"    # → teal-600
typography:
  headline:
    fontSize: 4rem           # → text-[64px]
    fontWeight: 400          # → font-normal
  body:
    fontSize: 1rem           # → text-[16px]
    fontWeight: 600          # → font-semibold
rounded:
  sm: 8px                    # → rounded-lg
  md: 16px                   # → rounded-2xl
  lg: 32px                   # → rounded-[32px]
  full: 9999px               # → rounded-full
spacing:
  sm: 8px                    # → gap-2
  md: 16px                   # → gap-4
  lg: 32px                   # → gap-8
motion:
  energy: high
  easing:
    entry: "back-out"        # 原 back.out(1.6)
    exit: "elastic-out"      # 原 elastic.out(1, 0.5)
    ambient: "ease-in-out"   # 原 sine.inOut
  duration:
    entrance: 15             # 帧 (原 0.5s)
    hold: 45                 # 帧 (原 1.5s)
    transition: 24           # 帧 (原 0.8s)
  atmosphere:
    - pattern-tiles
    - confetti-burst
    - color-blocks
  transition: slide
```

大胆温暖圆体。图案和重复——民间艺术节奏和密度。分层构图，丰富视觉纹理。每帧感觉手工制作。彩色动效：元素弹跳、弹出、旋转就位。overshoot 感觉有意。庆祝性能量。

---

## 8. Shadow Cut — Hans Hillmann

**Mood:** Dark, cinematic | **Best for:** Security products, dramatic reveals, investigative content, intense launches

```yaml
name: Shadow Cut
colors:
  primary: "#0a0a0a"         # → slate-950
  on-primary: "#f0f0f0"      # → slate-100
  surface: "#3a3a3a"         # → slate-700
  accent: "#C1121F"          # → red-800
typography:
  headline:
    fontSize: 4rem           # → text-[64px]
    fontWeight: 700          # → font-bold
    textTransform: uppercase # → uppercase
  body:
    fontSize: 0.875rem       # → text-[14px]
    fontWeight: 400          # → font-normal
rounded:
  none: 0px                  # → rounded-none
  sm: 2px                    # → rounded-sm
spacing:
  sm: 8px                    # → gap-2
  md: 16px                   # → gap-4
  lg: 48px                   # → gap-12
motion:
  energy: moderate
  easing:
    entry: "ease-out"        # 原 power3.out
    exit: "ease-in"          # 原 power4.in
    ambient: "ease-in-out"   # 原 sine.inOut
  duration:
    entrance: 24             # 帧 (原 0.8s)
    hold: 75                 # 帧 (原 2.5s)
    transition: 36           # 帧 (原 1.2s)
  atmosphere:
    - deep-shadow
    - vignette
    - grain-overlay
  transition: iris
```

近单色：深黑、冷灰、纯白 + 一个血色强调。锐利角形文字如黑色电影标题卡片。高对比度，无柔和感。元素从黑暗中浮现——揭示即叙事。缓慢推进、戏剧性尺度揭示。击打前的停顿很重要。

---

## Mood → Style Guide

| 如果内容感觉... | 使用... |
|---------------|---------|
| 数据驱动、分析、技术 | Swiss Pulse |
| 高端、企业、奢华 | Velvet Standard |
| 原始、朋克、激进、反叛 | Deconstructed |
| 炒作、大声、高能发布 | Maximalist Type |
| AI、ML、科幻、未来 | Data Drift |
| 人文、温暖、个人、健康 | Soft Signal |
| 文化、有趣、消费、节日 | Folk Frequency |
| 黑暗、戏剧、强烈、电影 | Shadow Cut |

---

## 创建自定义风格

这 8 种风格是起点，不是限制。创建自己的风格：

1. **命名** — 以设计师、艺术运动或文化参考为灵感
2. **写 YAML token** — `colors`（2-5 个 token）、`typography`（2-3 个尺度）、`rounded`、`spacing`、`motion`（energy + easing + duration + atmosphere + transition）
3. **写 prose** — 一段描述感觉、该做什么、避免什么
4. **转换为 Tailwind token** — 将 hex 映射到 Tailwind 色，将 easing 映射到 opencat.md §5.1 预设

模式：**YAML tokens（什么）→ prose rationale（为什么）→ components（如何组合）。**
