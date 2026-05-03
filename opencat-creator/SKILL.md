---
name: opencat-creator
description: 用 OpenCat JSONL 格式创建视频合成、动画、标题卡片、叠加层、字幕、配音、音频响应式视觉和场景转场。
---

# OpenCat Creator

JSONL 是视频的事实来源。运行时解析 JSONL，构建场景树，使用 Skia + Taffy + QuickJS 渲染帧。

## 流程概览与审批门禁

```
意图 → [步骤1: 设计系统 → design.md] → 审批 → [步骤2: 扩展 → expanded-prompt.md] → 审批 → [步骤3: 生成 JSONL]
                                                                                           ↑ 规划内化于此
```

**铁律：**
- 步骤 1 产出 `design.md` → **用户审批后** 才能进步骤 2
- 步骤 2 产出 `expanded-prompt.md` → **用户审批后** 才能进步骤 3
- 设计阶段（步骤 1-2）只读设计文件；实现阶段（步骤 3）才读实现文件

### 文件分组（渐进式加载）

| 阶段 | 可读文件 | 加载时机 |
|------|---------|---------|
| **设计阶段** | `visual-styles.md`, `design-create.md`, `house-style.md`, `typography.md` | 步骤 1 按需 |
| | `prompt-expansion.md`, `beat-direction.md` | 步骤 2 |
| | `motion-principles.md` 仅「设计原则」部分 | 步骤 2 可选 |
| **实现阶段** | `opencat.md`, `transitions.md`, `animations.md`, `canvaskit.md`, `text-animations.md` | 步骤 3 按需 |
| | `motion-principles.md` 完整（含「实现护栏」） | 步骤 3 |

**规则：** 每个步骤只读该步骤需要的文件。不提前加载。如果某个文件被引用了但当前步骤不需要，跳过它。

---

## 方法

### 探索阶段

了解意图：

- **受众** — 谁看？开发者？高管？普通消费者？
- **平台** — 在哪儿播放？社交媒体、网站主页、产品演示？尺寸？
- **优先级** — 什么最重要？动效质量？内容准确性？速度？
- **变体** — 用户想要多个选项还是一个最佳方案？

对于具体请求（"加一个标题卡片"），跳过探索阶段。
对于探索性请求，考虑提供 2-3 个有意义的变体——不同的节奏、能量水平或结构方法。

### 步骤 1：设计系统 → 产出 `design.md`

**产出：** `design.md`（项目根目录）

**需要审批才能进入步骤 2。**

如果项目已有 `design.md` 或 `DESIGN.md`，先读取它（检查两种大小写）。使用其确切值——不要发明颜色或替换字体。

如果没有，阅读 [references/design-create.md](references/design-create.md) 执行完整的 6 步对话式设计流程，产出 `design.md`。

`design.md` 必须使用 design-create.md 第 6 步规定的格式：YAML frontmatter（colors/typography/rounded/spacing/motion token 块）+ Markdown 正文（Overview/Colors/Typography/Layout/Elevation/Components/Do's and Don'ts）。不按此格式输出的设计文件会被拒绝。

### 步骤 2：Prompt 扩展 → 产出 `expanded-prompt.md`

**产出：** `expanded-prompt.md`（项目根目录）

**需要审批才能进入步骤 3。**

阅读 [references/prompt-expansion.md](references/prompt-expansion.md) 获取完整流程。将用户意图锚定到 `design.md`，产生完整的逐场景生产规范。

`expanded-prompt.md` 应包含：
1. **标题 + 风格块** — 引用 design.md 的精确 token
2. **节奏声明** — 命名节奏模式（如 `hook-PUNCH-hold-CTA`）
3. **全局规则** — 视差、微动效、转场风格
4. **逐场景拍点** — 每个场景的 Concept / Depth layers / 动效编排 / 退场转场
5. **转场方案** — 按情绪匹配选择具体 effect 和 duration

阅读 [references/beat-direction.md](references/beat-direction.md) 辅助多场景节奏设计。

### 步骤 3：生成 JSONL

**实现阶段。此时才读实现文件。** 设计阶段确定了"做什么"，现在关心"怎么写"。

先做脑力规划（不需要用户审批）：
1. **结构** — plain tree 还是 timeline？哪些轨道？
2. **时间** — 每个场景和转场的精确帧数
3. **布局** — 先构建 end-state（元素在其最可见时刻的位置）
4. **动效** — 缓动选择、入场方向变化、交错节奏

然后按需读取实现文件：
- [references/opencat.md](references/opencat.md) — JSONL 语法、节点类型、动画 API
- [references/transitions.md](references/transitions.md) — 转场效果参数
- [references/animations.md](references/animations.md) — 节点变换、颜色、路径动画、morphSVG
- [references/canvaskit.md](references/canvaskit.md) — Canvas API、Paint、Path
- [references/text-animations.md](references/text-animations.md) — 打字机、splitText、高亮
- [references/motion-principles.md](references/motion-principles.md) — 动效护栏规则、编排原则

最后构建 JSONL。每个场景、每个元素、每个 tween 应赢得它的位置。

#### 布局约束（Tailwind 对齐）

OpenCat 渲染器使用 Taffy 布局引擎，行为与 HTML/Tailwind 对齐：

- **div 默认 `display: block`**。className 写了 `flex` / `flex-row` / `flex-col` 才切换为 Flex；写 `grid` 才切换为 Grid。
- **优先使用 flex 布局**。Root节点应以 `flex` 起手（`flex flex-col` / `flex items-center justify-center` 等），通过 gap / items / justify 决定子元素位置，尽可能避免散落的 `absolute` 坐标。flex 布局可读、可维护、屏幕适配性更好。
- **`absolute` 元素必须显式给定位**：至少声明 `top` / `left` / `right` / `bottom` / `inset-X` 之一（或显式 `top-0 left-0` / `inset-0`）。**不允许**写裸 `absolute` 而不带任何坐标。Taffy 不实现 CSS 标准的 absolute static position fallback，inset 全 auto 的 absolute 元素会塞到容器内容区左上 `(0, 0)`，多个会完全重叠。
- 规模化使用 `absolute` 仅限三类场景：(1) `inset-0` 充满父级的画布/叠加层；(2) 钉在容器四角的标签（`top-[N] left-[N]` 等显式坐标）；(3) 与 Canvas 绘制坐标系手动对齐的标签（必须算清楚像素坐标）。
- `absolute`不会是root节点

#### 科学事实准确性

涉及科学/数学等事实内容时，所有几何参数、坐标、比例、运动轨迹必须经过数学验证：
- **坐标计算**：椭圆焦点、轨道参数等用公式精确计算，不得目测估算
- **比例关系**：相对大小、距离、速度必须符合客观事实
- **运动轨迹**：路径动画的 SVG path 与元素 `d` 使用完全一致的坐标字符串
- **公式方向验证**：任何数学/几何公式推导出的位置，先用边界条件做端点测试。例如极坐标 `r = a(1-e²)/(1+e·cosθ)` 以右焦点为原点、近日点在 θ=0 指向右侧，如果太阳在左焦点则需 `fx - r·cosθ`。始终用 θ=0（近日点）和 θ=π（远日点）验证方向正确性
- **验证方法**：可以使用 Python 验证

#### 编辑现有合成

- **读取实际文件，不要猜测。** 不要从记忆中重建十六进制代码或缓动模式。合成即是规范。
- 匹配现有字体、颜色、动画模式
- 只更改被要求的内容
- 保留无关片段的时间

---

## 输出检查清单

### 快速检查
- [ ] JSONL 语法有效
- [ ] 遵守 design.md 约束
- [ ] 每对相邻场景有转场
- [ ] 每场景有入场动画
- [ ] 末场景外无退场动画
- [ ] `composition.frames` = `sum(scene.duration) + sum(transition.duration)`
- [ ] `text` 节点有 `text` 字段，且不用 `\uXXXX` 转义，一律用实际 UTF-8 字符
- [ ] className 无 CSS 动画/transform 类
- [ ] tween 颜色用显式字面量
- [ ] 确定性随机
- [ ] root节点优先以 `flex` / `flex-col` / `flex-row` 起手而非裸 div + absolute 坐标
- [ ] 每一个 `absolute` 元素至少有 `top` / `left` / `right` / `bottom` / `inset-X` 之一（含 `inset-0`）。裸 `absolute` 不带坐标 = bug

### 慢速检查
- [ ] 字号符视频缩放要求（60px+ 标题、20px+ 正文、16px+ 标签）
- [ ] 每场景 8-10 元素
- [ ] 对比度问题已处理
- [ ] 动画编排已验证
- [ ] 科学事实准确性：视觉呈现（位置、比例、运动轨迹、数值）必须经数学验证，符合客观事实
- [ ] 公式方向验证：所有数学/几何公式用边界条件做端点测试

### 设计遵循性

如果存在 `design.md`，检查：
1. **颜色** — 合成中的每个十六进制值都出现在 design.md 的调色板中
2. **排版** — 字体系列和粗细匹配 design.md
3. **圆角** — border-radius 值与声明的风格匹配
4. **间距** — padding 和 gap 值在声明的密度范围内
5. **深度** — 阴影使用与声明的深度级别匹配
6. **避免规则** — 无违反

以检查清单形式报告违规，交付前修复每项。

如果不存在 `design.md`（仅使用 house-style），检查：
1. **调色板一致性** — 相同 bg/fg/accent 跨场景使用，不按场景发明颜色
2. **没有惰性默认值** — 对照 house-style.md 的"需要质疑的默认项"列表检查
