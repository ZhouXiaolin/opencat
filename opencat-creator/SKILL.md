---
name: opencat-creator
description: 用 OpenCat JSONL 格式创建视频合成、动画、标题卡片、叠加层、字幕、配音、音频响应式视觉和场景转场。
---

# OpenCat Creator

JSONL 是视频的事实来源。运行时解析 JSONL，构建场景树，使用 Skia + Taffy + QuickJS 渲染帧。

**核心事实来源：** [references/opencat.md](references/opencat.md)

---

## 方法

### 探索阶段

对于开放性请求，了解意图：

- **受众** — 谁看？开发者？高管？普通消费者？
- **平台** — 在哪儿播放？社交媒体、网站主页、产品演示？尺寸？
- **优先级** — 什么最重要？动效质量？内容准确性？速度？
- **变体** — 用户想要多个选项还是一个最佳方案？

对于具体请求（"加一个标题卡片"），跳过探索阶段。
对于探索性请求，考虑提供2-3个有意义的变体——不仅仅是颜色替换，而是不同的节奏、能量水平或结构方法。一个安全/常规的，一个大胆的。不要强制要求——这只是适当时可用的工具。

### 步骤 1：设计系统

如果项目中有 `design.md` 或 `DESIGN.md`，先读取它（检查两种大小写——在 Linux 上是不同的文件）。它是品牌颜色、字体和约束的源数据。使用其确切值——不要发明颜色或替换字体。任何格式都可以（YAML frontmatter、散文、表格——只需提取值）。

如果没有 `design.md`：
1. 用户提到了风格或情绪？→ 阅读 [references/visual-styles.md](references/visual-styles.md) 了解8个命名预设。选择最接近的匹配。
2. 想要完整视觉身份？→ 阅读 [references/design-create.md](references/design-create.md) 执行对话式设计流程
3. 想跳过？→ 询问：氛围、浅色或深色、任何品牌颜色/字体？然后阅读 [references/house-style.md](references/house-style.md) 选色板

### 步骤 2：Prompt 扩展

阅读 [references/prompt-expansion.md](references/prompt-expansion.md) 获取完整流程。将用户意图锚定到设计系统，产生一致的中间产物。写入 `expanded-prompt.md`。

### 步骤 3：规划

写 JSONL 之前思考：

1. **什么** — 观众应该体验到什么？确定叙事弧、关键时刻和情感节奏。
2. **结构** — plain tree（单场景）还是 timeline（多场景+转场）？有多少个合成，哪些是子合成 vs 内联，哪些轨道承载什么（视频、音频、叠加层、字幕）。
3. **节奏** — 声明节奏模式（如 `fast-fast-SLOW-fast-TRANSITION-hold`）。多场景时阅读 [references/beat-direction.md](references/beat-direction.md)。
4. **时间** — 哪些片段驱动持续时间，过渡落在哪里，节奏是什么。
5. **布局** — 先构建 end-state
6. **动效** — 阅读 [references/motion-principles.md](references/motion-principles.md)

### 步骤 4：生成 JSONL

构建被请求的内容。每个场景、每个元素、每个 tween 应赢得它的位置。
将每个元素放置在其**最可见的时刻**应该处于的位置——即它完全进入、正确放置且尚未退出的帧。

**为什么这很重要：** 如果你将元素定位在它们的动画起始状态（屏幕外、缩放到0、不透明度为0）然后补间到你认为它们应该落地的位置，你是在猜测最终布局。重叠在视频渲染之前是不可见的。通过首先构建最终状态，你可以在添加任何动效之前看到并修复布局问题。

## 核心实例

### 科学事实准确性
涉及科学/数学等事实内容时，我们的节点以及动画或者CanvasKit子集绘制的所有几何参数、坐标、比例、运动轨迹必须经过数学验证：
- **坐标计算**：椭圆焦点、轨道参数等用公式精确计算，不得目测估算
- **比例关系**：相对大小、距离、速度必须符合客观事实
- **运动轨迹**：路径动画的 SVG path 与 SVG 元素 `d` 使用完全一致的坐标字符串
- **公式方向验证**：任何数学/几何公式推导出的位置、轨迹、形状，先用边界条件（最简输入）验证方向正确性。例如极坐标 `r = a(1-e²)/(1+e·cosθ)` 以右焦点为原点、近日点在 θ=0 指向右侧，如果太阳在左焦点则需 `fx - r·cosθ`。始终用 θ=0（近日点）和 θ=π（远日点）做端点测试，确认结果与视觉元素（`addOval`、SVG path、参考线）边界一致。这条适用于一切有方向/符号的数学公式。
- **验证方法**：可以使用python验证
---

**转场：** [references/transitions.md](references/transitions.md) — 内置转场、GL 转场、情绪匹配、参数表达

**文字动效：** [references/text-animations.md](references/text-animations.md) — 打字机、splitText、高亮着色、流光效果

**普通动效：** [references/animations.md](references/animations.md) — 节点变换、颜色、路径动画、morphSVG

**CanvasKit：** [references/canvaskit.md](references/canvaskit.md) — Canvas API、Paint、Path、程序化绘制

## 编辑现有合成

- **读取实际文件，不要猜测。** 在编辑、扩展或创建配套合成时，读取现有的源代码。不要从记忆中重建十六进制代码。不要猜测动效缓动模式。合成即是规范——从中提取确切的值。
- 匹配你从读取内容中发现的现有字体、颜色、动画模式
- 只更改被要求的内容
- 保留无关片段的时间
---

## 输出检查清单

**快速检查：**
- [ ] JSONL 语法有效
- [ ] 遵守 design.md 约束
- [ ] 每对相邻场景有转场
- [ ] 每场景有入场动画
- [ ] 末场景外无退场动画
- [ ] `composition.frames` = `sum(scene.duration) + sum(transition.duration)`
- [ ] `text` 节点必须有 `text` 字段，且不用 `\uXXXX` unicode 转义，一律用实际 UTF-8 字符
- [ ] className 无 CSS 动画/transform 类
- [ ] tween 颜色用显式字面量
- [ ] 确定性随机

**慢速检查：**
- [ ] 字号符视频缩放要求（60px+ 标题、20px+ 正文、16px+ 标签）
- [ ] 每场景 8-10 元素
- [ ] 对比度问题已处理
- [ ] 动画编排已验证
- [ ] 科学事实准确性：涉及科学/历史/事实内容时，视觉呈现（位置、比例、运动轨迹、数值）必须经数学验证，符合客观事实
- [ ] 公式方向验证：所有数学/几何公式计算的位置、轨迹、形状都用边界条件做端点测试，确认方向、符号与视觉元素一致


### 设计遵循性

如果存在 `design.md`，在创作后验证合成是否遵循它。读取jsonl并检查：

1. **颜色** — 合成中的每个十六进制值都出现在 design.md 的调色板部分（无论用户如何标记：Colors、Palette、Theme 等）。标记任何发明的颜色。
2. **排版** — 字体系列和粗细匹配 design.md 的类型规范。没有替换。
3. **圆角** — border-radius 值与声明的圆角风格匹配（如果指定）。
4. **间距** — padding 和 gap 值在声明的密度范围内（如果指定）。
5. **深度** — 阴影使用与声明的深度级别匹配（如果指定）（扁平 = 无，微妙 = 浅，分层 = 发光）。
6. **避免规则** — 如果 design.md 有列出要避免的事项的部分（通常是 "What NOT to Do"、"Don'ts"、"Anti-patterns" 或 "Do's and Don'ts"），验证没有违反。

以检查清单形式报告违规。在交付前修复每项。

如果不存在 `design.md`（仅使用 house-style），验证：

1. **调色板一致性** — 相同的背景、前景和强调色在所有场景中使用。没有按场景发明颜色。
2. **没有惰性默认值** — 对照 house-style.md 的"需要质疑的惰性默认值"列表检查合成。如果出现任何内容，必须是针对内容的刻意选择，而不是默认值。