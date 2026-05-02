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
- **平台** — 在哪儿播放？社交媒体、网站主页、产品演示？
- **优先级** — 什么最重要？动效质量？内容准确性？速度？
- **变体** — 用户想要多个选项还是一个最佳方案？

对于具体请求（"加一个标题卡片"），跳过探索阶段。

### 步骤 1：设计系统（不可跳过，必须先创建或者读取design.md）

如果存在 `design.md`，先读取。它是品牌色、字体和约束的事实来源。

如果没有 `design.md`：
1. 用户提到了风格或情绪？→ 阅读 [references/visual-styles.md](references/visual-styles.md) 匹配 8 个命名预设
2. 想要完整视觉身份？→ 阅读 [references/design-create.md](references/design-create.md) 执行对话式设计流程
3. 想跳过？→ 阅读 [references/house-style.md](references/house-style.md) 选色板，问：情绪、亮/暗、品牌色/字体？

### 步骤 2：Prompt 扩展

阅读 [references/prompt-expansion.md](references/prompt-expansion.md) 获取完整流程。将用户意图锚定到设计系统，产生一致的中间产物。写入 `expanded-prompt.md`。

### 步骤 3：规划

写 JSONL 之前思考：

1. **什么** — 叙事弧线、关键时刻、情感节奏
2. **结构** — plain tree（单场景）还是 timeline（多场景+转场）？
3. **节奏** — 声明节奏模式（如 `fast-fast-SLOW-fast-TRANSITION-hold`）。多场景时阅读 [references/beat-direction.md](references/beat-direction.md)。
4. **时间** — 总帧数、fps、转场位置
5. **布局** — 先构建 end-state
6. **动效** — 阅读 [references/motion-principles.md](references/motion-principles.md)

### 步骤 4：生成 JSONL

构建被请求的内容。每个场景、每个元素、每个 tween 应赢得它的位置。


## 核心实例

### 科学事实准确性
涉及科学/数学等事实内容时，我们的节点以及动画或者CanvasKit子集绘制的所有几何参数、坐标、比例、运动轨迹必须经过数学验证：
- **坐标计算**：椭圆焦点、轨道参数等用公式精确计算，不得目测估算
- **比例关系**：相对大小、距离、速度必须符合客观事实
- **运动轨迹**：路径动画的 SVG path 与 SVG 元素 `d` 使用完全一致的坐标字符串
- **优先使用 `path` 元素 + 路径动画**：用 SVG 椭圆弧（`A rx ry`）而非 `drawCircle` + 手动矩阵变换
- **验证方法**：在 JSONL 旁注明关键数学参数（a, b, e, focus offset），便于审查
---

**转场：** [references/transitions.md](references/transitions.md) — 内置转场、GL 转场、情绪匹配、参数表达

**文字动效：** [references/text-animations.md](references/text-animations.md) — 打字机、splitText、高亮着色、流光效果

**普通动效：** [references/animations.md](references/animations.md) — 节点变换、颜色、路径动画、morphSVG

**CanvasKit：** [references/canvaskit.md](references/canvaskit.md) — Canvas API、Paint、Path、程序化绘制

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