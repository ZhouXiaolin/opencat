# Kepler's Three Laws — Production Spec

## Style Identity

```
Composition: 1920 × 1080 (16:9 landscape), 30fps

Palette:
  bg-primary:     slate-950  (#020617)  — deep space
  bg-surface:     slate-900  (#0f172a)  — card/surface
  text-primary:   white      (#ffffff)
  text-secondary: slate-300  (#cbd5e1)
  accent-orbit:   amber-400  (#fbbf24)  — sun, orbital glow
  accent-science: cyan-400   (#22d3ee)  — data, formulas

Typography:
  title:    font-bold     text-[80px]  — main title
  scene:    font-bold     text-[56px]  — per-scene heading
  body:     font-normal   text-[30px]  — explanation
  label:    font-normal   text-[22px]  — footnotes, tag
  data:     font-mono     text-[24px]  — formulas, tabular data

  Note: no paired fonts. Single sans-serif family, mono reserved for formulas.

Rounded: rounded-none throughout (precision theme)
Spacing: sm=16px, md=32px, lg=64px

Motion:
  energy: moderate
  entrance: ease-out, 15-24f
  ambient: ease-in-out, slow drift (sceneFrames range)
  transition: 12-18f

Atmosphere:
  - star field (tiny dots, random but seeded, slow twinkle)
  - orbital rings (thin elliptical paths, rotated, slow spin)
  - radial glow (accent-orbit / amber, 5-15% opacity, centered)
  - hairline rule (border-t border-slate-700/50, scaleX entrance)

Transition palette:
  - primary: fade (60%) — calm, educational
  - emphasis: light_leak (20%) — cosmic feel
  - accent: slide (20%) — direction change
```

## Rhythm Declaration

```
Title — Law-1 — Law-2 — Law-3 — Outro
calm   build  build  build  resolve
fade   light  slide  fade
      leak
```

Rhythm pattern: **hook → teach → teach → teach → resolve**

Each educational scene: 3f pause → title enters (9-15f) → visualization appears (12-18f) → hold for reading (120-150f) → ambient to end.

## Global Rules

- Every element enters via `ctx.from()` — no pop-in
- First tween offset 3-9f (never t=0)
- 3+ easing varieties per scene
- Vary entrance direction between scenes (not all `{y:30, opacity:0}`)
- No exit animations except final scene (fade to black)
- All ambient animations mounted on `ctx.timeline()`
- Deterministic random for star positions (`ctx.utils.random` with seed)
- Explicit color literals in tweens (no Tailwind tokens in animation calls)
- `tabular-nums` for all data/period values

## Scene Beats

---

### Scene 1: Title — "Kepler's Laws of Planetary Motion"

**Duration:** 210 frames (7s)

**Layout:** 16:9 full canvas. Title centered. Decorative orbit ring frames the lower third.

**Concept:** 镜头从深空深处缓缓推进。星光暗淡闪烁，一个金色的椭圆轨道环在画面下半部缓慢旋转。标题居中浮现——"开普勒行星三定律"——每一个字像星辰被引力捕获一样就位。

**Mood direction:** 星空电影开场。哈勃深场的敬畏感。缓慢、庄严、神秘。

**Depth layers:**
- BG: 星空背景（40+ 随机星点覆盖全画幅，带缓慢呼吸 opacity）
- BG: 径向金色辉光（从画面中心下方散发，5% opacity，呼吸缩放）
- BG: 大尺寸幽灵文字 "KEPLER"（slate-700/5 opacity，超大 font-bold，漂浮）
- MG: 椭圆轨道环（canvas 绘制椭圆形，slate-600, 1px stroke，缓慢旋转）
- MG: 标题 "开普勒行星三定律"（white font-bold text-[80px]）
- MG: 小标题 "Johannes Kepler · 1571-1630"（slate-300 text-[24px]）
- MG: 分割线（hairline rule，w-[60%] mx-auto border-t border-slate-700，scaleX: 0 → 1）

**Motion choreography:**
- Stars: opacity breath (0.2↔0.7), stagger per star, ease-in-out
- Orbit ring: rotation from -5° to 355°, sceneFrames, linear
- Title: stagger chars from {opacity:0, y:50, scale:0.95}, stagger 3f, spring.gentle
- Subtitle: fade from {opacity:0, y:20}, delay after title, ease-out
- Hairline: scaleX 0→1, duration 20f, back-out
- Radial glow: slow breath scale 1↔1.06, sceneFrames

**Exit transition:** `fade`, 15f, ease-in-out → Scene 2

---

### Scene 2: First Law — The Law of Elliptical Orbits

**Duration:** 270 frames (9s)

**Layout:** Left half: orbit canvas (800×600). Right half: text column (title + explanation).

**Concept:** 画面左侧，椭圆轨道在星空中显现。太阳位于焦点，行星沿轨道运行。右侧文字同步解说。

**Mood direction:** NASA 轨道力学图。冷背景 + 金色焦点。严谨、清晰。

**Depth layers:**
- BG: 星空延续
- BG: 深蓝径向辉光（轨道区域后方）
- MG: 椭圆轨道 canvas（800×600，cyan-400/50 2px stroke）
- MG: 太阳（canvas 绘制，amber-400 fill + halo 辉光）
- MG: 行星（canvas 绘制 10px 白色小圆，沿轨道运动）
- MG: 焦点标记（canvas 绘制十字线，slate-300）
- MG: 右栏标题 "第一定律：椭圆轨道"（white font-bold text-[56px]）
- MG: 右栏说明文字（slate-300 text-[30px]，max-w-[700px]）
- MG: 标签 "太阳 · 焦点"（amber-400 text-[22px]）

**Motion choreography:**
- Canvas orbit: ellipse rx=300 ry=200, focus offset c=√(300²-200²)=224px
- Planet: θ 0→2π over sceneFrames, linear (constant speed = contrast for Law 2)
- Title: from {opacity:0, x:40}, ease-out, 18f
- Body text: fade in staggered after title, from {opacity:0, y:20}
- Sun: golden pulse scale 1↔1.05
- Labels: fade in with slight delay

**Exit transition:** `light_leak`, 18f, ease-in-out → Scene 3

---

### Scene 3: Second Law — The Law of Equal Areas

**Duration:** 270 frames (9s)

**Layout:** Left: orbit canvas (800×600) with sector fills. Right: explanation.

**Concept:** 同一轨道，行星变速运动。两片扇形区域交替高亮——近日点小弧长但半径大，远日点弧长但半径小——面积相等。

**Mood direction:** 数据可视化。扇形填充动画让抽象概念可见。

**Depth layers:**
- BG: 星空延续
- BG: 冷蓝径向辉光（orbit 区域右下方）
- MG: 椭圆轨道 canvas（复用场景 2 配置）
- MG: 行星（变速运动：ω ∝ 1/r²）
- MG: 面积扇形（canvas 填充三角形，cyan-400/20 fill，cyan-400/50 stroke）
- MG: 标签 "近日点 ▲ 快"（amber-400 text-[22px]）
- MG: 标签 "远日点 ▼ 慢"（slate-300 text-[22px]）
- MG: 右栏标题 "第二定律：面积定律"（white font-bold text-[56px]）
- MG: 右栏说明文字（slate-300 text-[30px]）
- MG: 强调文字 "面积₁ = 面积₂"（cyan-400 font-mono text-[24px]）

**Motion choreography:**
- Planet: variable ω, faster at perihelion (θ near 0), slower at aphelion (θ near π)
- Sector A (near sun): draw filled wedge from focus to planet positions near θ=0
- Sector B (far from sun): draw filled wedge from focus to planet positions near θ=π
- Two sectors alternate highlight: one fades in while other fades out
- Title: from {opacity:0, y:-20}, ease-out, 15f
- Equal-area label: scale from 0.85, spring.gentle

**Exit transition:** `slide` `from_left`, 12f, ease-out → Scene 4

---

### Scene 4: Third Law — The Law of Periods

**Duration:** 270 frames (9s)

**Layout:** Formula dominates left-center. Data table on right.

**Concept:** 公式 T² ∝ a³ 大字浮现。右侧行星数据表展示比值恒定。

**Mood direction:** 数学的优雅。简约、精确。

**Depth layers:**
- BG: 星空延续
- BG: 紫蓝径向辉光（从画面左侧）
- MG: 公式 "T² ∝ a³"（white font-bold font-mono text-[96px]）
- MG: 副文字 "周期² ∝ 半长轴³"（slate-300 text-[28px]）
- MG: 标题 "第三定律：周期定律"（white font-bold text-[56px]）
- MG: 数据表（4 行 × 4 列：行星 | a(AU) | T(年) | T²/a³）
- MG: 表格底行 "≈ 1" 高亮（cyan-400 font-mono text-[24px] tabular-nums）
- MG: 说明文字（slate-300 text-[26px]，表格下方）

**Motion choreography:**
- Formula: chars stagger from {opacity:0, scale:0.7}, spring.gentle, stagger 2f
- Data rows: cascade from bottom, stagger 5f, each from {opacity:0, y:30}
- Constant value: color pulse on ratio column
- Title: from {opacity:0, x:40}, ease-out, 18f

**Exit transition:** `fade`, 18f, ease-in-out → Scene 5

---

### Scene 5: Outro — Summary & Conclusion

**Duration:** 180 frames (6s)

**Layout:** Horizontal row of three law summaries across screen. Conclusion below.

**Concept:** 三条定律横向排列。结论性文字浮现在下方。

**Mood direction:** 庄严收束。

**Depth layers:**
- BG: 星空
- BG: 中心径向金色辉光，缓慢放大
- MG: 三定律卡片（横向等宽排列）
  - "① 椭圆轨道" | "② 面积相等" | "③ T² ∝ a³"
- MG: 结论 "牛顿万有引力的基石"（white font-bold text-[48px]）
- MG: 署名 "Kepler · 1609-1619"（slate-400 text-[22px]）

**Motion choreography:**
- Three cards: cascade from left, stagger 8f, each from {opacity:0, x:-60}
- Conclusion: fade from {opacity:0, y:30}, after cards settle
- Last 24f: fade entire scene to black (only exit animation, final scene exception)
- Stars slowly dim in last 30f

**End:** Black frame.

---

## Recurring Visual Themes

1. **Star field** — persists across all scenes. Same stars, deterministic positions.
2. **Orbit motif** — elliptical path appears/scales across scenes. Scene 1 (decorative ring), Scene 2-3 (functional orbit), Scene 5 (subtle background ring).
3. **Color language** — amber/gold = celestial bodies & gravity. cyan/blue = mathematics & data. slate = structure & space.
4. **Hairline rules** — horizontal thin lines as scene dividers and entrance markers.

## Negative Checklist

- No pure black (#000) — shift to slate-950 for deep space
- No neon colors — this is natural cosmos, not cyberpunk
- No gradient text (`bg-gradient-to-r` on text) — avoid house-style warning
- No flickering/dizzying animations — educational content needs readability
- No <br> tags — use max-width for text wrapping
- No CSS animation/transform classes in className — all motion via ctx.*
- No Math.random() — use ctx.utils.random with seed
- No exit animations except scene 5
