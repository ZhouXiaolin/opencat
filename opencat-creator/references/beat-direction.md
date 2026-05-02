# Beat Direction

如何规划和导演多场景 composition 中的单个场景（拍点）。在写任何多场景视频之前阅读。

---

## 每拍点方向

每个拍点是一个**世界**，不是一个布局。在写 JSONL className 和 `ctx.*()` 指令之前，描述观众**体验**什么。一个优秀分镜和一个平庸分镜的差别：

**平庸：** "深蓝背景。'$1.9T' 白色 80px。logo 左上。波浪图右下。"
**优秀：** "镜头已经在一片广阔的深色画布上飞行。渐变波像极光一样扫过画面 — 活着的、变化的。'$1.9T' 以如此大的力量撞击进画面，连波浪都在响应。这不是一张幻灯片 — 这是一个时刻。"

第一个描述像素。第二个描述体验。写第二个，然后算出像素。

每个拍点应该有：

### Concept

这个场景的 2-3 句大想法。什么视觉**世界**？什么隐喻驱动它？观众应该**感觉**什么？这是最重要的部分 — 一切从它展开。

### Mood direction

文化和设计参考，不是色值：

- "几何、节奏、精确。想到 Josef Albers 或 Bauhaus 色彩研究。"
- "温暖工作空间。好的笔记本能量，不是技术蓝图。"
- "电影开场序列。那种让你向前倾身的开场。"

### 动效编排

每个元素的具体动词 — 不是"它动效入场"而是**如何**：

| Energy | Verbs | 示例 |
|--------|-------|------|
| High impact | SLAMS, CRASHES, PUNCHES, STAMPS, SHATTERS | "$1.9T" 从左侧 SLAMS 进画面 |
| Medium energy | CASCADE, SLIDES, DROPS, FILLS, DRAWS | 三张卡片 CASCADE 入场 |
| Low energy | types on, FLOATS, morphs, COUNTS UP, fades in | 计数器从 0 COUNTS UP 到 135K |

每个元素得到一个动词。如果你不能命名动词，这个元素还没设计好。

### Transition

这个场景如何交接到下一个。

**选择规则：**
- 场景是**核心**（产品揭示、CTA）→ 用 `clock_wipe` / `iris` / `light_leak` 等戏剧性效果
- 场景是**过渡连接** → 用 `fade` / `slide` / `wipe`
- 节奏感场景（能量高、速度快）→ `slide` 更有方向性

### Depth layers

前景、中景、背景有什么。每个拍点至少应有 2 层：

- "BG: 深蓝填充 + 柔和的径向发光。MG: 需阴影的 stat cards。FG: logo 右下。"

---

## 节奏规划

在写 JSONL 之前，声明你的场景节奏：哪些是快节奏、哪些是停留、转场落在哪里、能量峰值在哪。在实现之前命名模式 — fast-fast-SLOW-fast-TRANSITION-hold。

| 视频类型 | 典型节奏模式 |
|---------|-------------|
| 社交广告（15s） | hook-PUNCH-hold-CTA |
| 产品演示（30-60s） | slow-build-BUILD-PEAK-breathe-CTA |
| 发布预告（10-20s） | SLAM-proof-SLAM-hold |
| 品牌短片（20-45s） | drift-build-PEAK-drift-resolve |

## 动效参数参考

OpenCat ctx API 参数速查：

| 参数 | 说明 |
|------|------|
| `duration` | 帧数。@30fps 时 0.5s = 15f |
| `ease` | `'ease-out'`、`'ease-in'`、`'ease-in-out'`、`'back-out'`、`'elastic-out'`、`'linear'` 等 |
| `stagger` | 帧数。@30fps 时 0.15s = 4f |
| `x` / `y` | 像素值 |
| `scale` | 缩放倍数 |
| `opacity` | 0-1 |
