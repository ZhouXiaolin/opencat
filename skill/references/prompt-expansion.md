# Prompt 扩展

每个多场景 OpenCat XML 视频都运行。扩展将用户意图锚定到 `design.md`，产生一致的中间产物。

---

## 为什么总是运行

**扩展从不是直通。** 每个用户 prompt — 无论多详细 — 都是一个**种子**。扩展的工作是将其丰富为完整的逐场景生产规范。

即使详细的 brief 也缺少扩展才能添加的东西：

- **每场景的氛围层** — 径向发光、幽灵文字、强调线、杂色、主题装饰
- **每个装饰的次级动效** — breath、drift、pulse、orbit
- **让场景感觉真实的微细节** — 刻度标记、标签、排版强调
- **对象级别的转场编排** — "crossfade" → "X 向外展开变成 Y"
- **每个场景内的节奏拍点** — 紧张感建立、停留、强调词位置
- **来自 design.md 的精确 easing 选择**

单场景 XML 和简单修改是唯一的例外。

---

## 前置条件

生成前读取：

- `design.md`（必须存在）— 提取品牌色、字体、情绪、约束
- [beat-direction.md](beat-direction.md) — 场景/镜头规划格式
- [video-composition.md](video-composition.md) — 视频媒介规则：密度、尺度、色彩存在感、构图

扩展的目标不是直接写 XML，而是为后续 `STORYBOARD.md` 和 XML 实现提供足够具体的生产说明。

---

## 生成什么

扩展为完整的 production spec：

### 1. 标题 + 风格块

引用 design.md 的精确 Tailwind token 和 mood。不发明颜色。

### 2. 节奏声明

在场景细节之前命名节奏：

- `hook-PUNCH-hold-CTA`（社交广告 15s）
- `slow-build-BUILD-PEAK-breathe-CTA`（产品演示 30-60s）
- `SLAM-proof-SLAM-hold`（发布预告 10-20s）
- `drift-build-PEAK-drift-resolve`（品牌短片 20-45s）

### 3. 全局规则

- 视差层
- 微动效要求
- 转场风格（只需声明意图，如"这里需要高能量的转场"，具体 effect 到实现阶段选）
- 能量匹配到 mood

### 4. 逐场景拍点

对每个场景：

- **Concept** — 2-3 句大想法。什么视觉世界？什么隐喻？观众应该**感觉**什么？
- **Mood direction** — 文化/设计参考，不是色值
- **Shot** — close-up / medium / wide / extreme close-up / over-the-shoulder / dutch angle
- **Depth layers** — BG（2-5 个装饰元素带环境动效）、MG（内容）、FG（强调元素）
- **Camera move** — dolly in / pull-back / parallax pan / orbit / rack focus
- **动效编排** — 每个元素的具体动词：
  - High：SLAMS、CRASHES、PUNCHES
  - Medium：CASCADE、SLIDES、DROPS
  - Low：floats、types on、COUNTS UP
- **退场转场意图** — 如"快速硬切"或"缓慢溶解"，具体 effect 到实现阶段选
- **XML implementation note** — 这个场景在 OpenCat XML 中最容易出错的点，例如 timeline timing、canvas subtree、字幕、素材路径或 absolute 坐标

### 5. 复用视觉主题

跨场景的品牌色、字体、形状、纹理和运动线索。所有颜色都来自 `design.md`。

### 6. 负面清单

避免什么，由 design.md 的约束决定。

---

## 输出

将扩展后的 prompt 写入 `expanded-prompt.md`。

告知用户：

> "我已将你的需求扩展为完整的制作方案。查看：`expanded-prompt.md`
>
> 包含 [N] 个场景，共 [X] 秒及具体视觉元素、转场和节奏。如有需要可修改，然后告诉我继续。"

在用户批准之前，不要进入构建阶段。
