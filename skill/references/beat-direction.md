# Beat Direction

如何规划和导演多场景 OpenCat XML 视频中的单个场景（拍点）。在写任何多场景视频之前阅读。

---

## 每拍点方向

每个拍点是一个**世界**，不是一个布局。在写 XML `class` 和 `ctx.*()` 指令之前，描述观众**体验**什么。一个优秀分镜和一个平庸分镜的差别：

**平庸：** "深蓝背景。'$1.9T' 白色 80px。logo 左上。波浪图右下。"
**优秀：** "镜头已经在一片广阔的深色画布上飞行。渐变波像极光一样扫过画面 — 活着的、变化的。'$1.9T' 以如此大的力量撞击进画面，连波浪都在响应。这不是一张幻灯片 — 这是一个时刻。"

第一个描述像素。第二个描述体验。写第二个，然后算出像素。

每个拍点应该有：

### Shot

每个拍点第一行声明镜头类型：

| Shot | 适用 | 画面含义 |
|------|------|----------|
| Extreme close-up | 单个字符、按钮、数字、卡片、光标 | 主体占画面 60-90% |
| Close-up | UI 小区域、单列卡片、代码块、图表局部 | 主体占画面 40-60%，背景有上下文 |
| Medium | 2-3 个面板、看得清的产品片段 | 主体占画面 60-80% |
| Wide | 全局结构或建立场景 | 只在“全貌就是信息”时使用 |
| Over-the-shoulder | 观看者在操作背后 | 前景遮挡 + 中景 UI + 背景视差 |
| Dutch angle | 紧张、不稳定、冲击 | 4-8 度倾斜，慎用 |

产品演示不应每个 beat 都是 wide shot。多数时候 close-up / medium 更像视频，wide 更像截图。

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

### Camera move

视频不是网页。每个拍点至少需要一个相机式运动：

- **Dolly in** — 组合根缓慢 scale 1.00 -> 1.06
- **Pull-back** — 从局部 scale 1.12 拉回 1.00，揭示上下文
- **Parallax pan** — BG/MG/FG 用不同速度横移
- **Orbit** — 3D/伪 3D 元素绕主体移动
- **Rack focus** — blur 从背景切到前景或反过来
- **Push** — 关键瞬间短促 scale/position 推进

如果一个 beat 只有开头元素入场，然后静止超过 1.5 秒，它不是视频拍点。

### Transition

这个场景如何交接到下一个。

**选择规则：**
- 场景是**核心**（产品揭示、CTA）→ 用 `clock_wipe` / `iris` / `light_leak` 等戏剧性效果
- 场景是**过渡连接** → 用 `fade` / `slide` / `wipe`
- 节奏感场景（能量高、速度快）→ `slide` 更有方向性

### Depth layers

前景、中景、背景有什么。每个拍点至少应有 2 层：

- "BG: 深蓝填充 + 柔和的径向发光。MG: 需阴影的 stat cards。FG: logo 右下。"

### Forbidden web patterns

避免这些网页化失败：

- 浏览器 chrome、URL bar、macOS traffic lights，除非这个 chrome 本身是主题
- 侧边栏、导航栏、页脚，除非 beat 讲的是导航
- 居中的单张卡片/窗口，四周 60-120px 留白
- tooltip/modal 用来解释上下文
- hover 状态演示
- “呼吸”只有 y: 1px 或 scale: 1.01 的假运动
- 产品截图满屏静态贴图，没有重构、裁切、相机运动或 shader/canvas 处理

---

## 节奏规划

在写 XML 之前，声明你的场景节奏：哪些是快节奏、哪些是停留、转场落在哪里、能量峰值在哪。在实现之前命名模式 — fast-fast-SLOW-fast-TRANSITION-hold。

| 视频类型 | 典型节奏模式 |
|---------|-------------|
| 社交广告（15s） | hook-PUNCH-hold-CTA |
| 产品演示（30-60s） | slow-build-BUILD-PEAK-breathe-CTA |
| 发布预告（10-20s） | SLAM-proof-SLAM-hold |
| 品牌短片（20-45s） | drift-build-PEAK-drift-resolve |

## 转场时长参考

按场景在叙事中的位置选择转场秒数：

| 位置 | Duration（秒） |
|------|---------------|
| 开场 | 0.4-0.6 |
| 相关点之间 | 0.3 |
| 主题变化 | 0.3-0.4 |
| 高潮/揭示 | 0.17-0.3 |
| 放松 | 0.5-0.7 |
| 结尾 | 0.6-1 |

## 能量 → Timing

转场缓动与能量的匹配：

| 能量 | Duration（秒） | Timing |
|------|---------------|--------|
| **平静** | 0.5-0.8 | `'ease-in-out'` |
| **中等** | 0.3-0.5 | `'ease-out'` |
| **高能** | 0.17-0.3 | `'linear'` |
