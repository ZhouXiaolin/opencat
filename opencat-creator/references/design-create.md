# Design Create

当项目没有 `design.md` 时，通过对话引导用户确定视觉身份，最终生成 `design.md`。替代 hyperframes 的浏览器 Design Picker，改为 AI 对话流程。

---

## 触发条件

在 SKILL.md 步骤 1 中发现没有 `design.md` 时启动本流程。用户也可以直接说"帮我创建 design.md"。

---

## 流程

### 第 1 步：理解意图

在生成任何选项之前，先理解用户的内容：

- **产品/品牌** — 这是什么？SaaS？餐饮？科技？个人？
- **受众** — 谁看？开发者？高管？普通消费者？年轻人？
- **平台** — 在哪儿播放？社交媒体（15s）、网站主页、产品演示？
- **情绪关键词** — 用户自然语言描述的情绪（"高级感"、"活泼"、"冷静专业"）

从用户 prompt 中提取这些信息，不要逐一盘问。如果 prompt 已经足够清晰（如"给我的 AI 编程助手做一个产品发布视频"），直接进入下一步。

---

### 第 2 步：Mood Board（方向选择）

根据用户意图，生成 **4-6 个方向**。每个方向是一个完整的叙事，不是换字体换色。

**生成规则：**

- 每个方向必须讲不同的故事，问"这个产品的真正不同定位方式是什么？"
- 每个方向包含：名称、一句话描述、情绪关键词、推荐风格（从 visual-styles.md 选）
- 不要只是排列组合 — 每个方向应该是不同叙事

**示例（AI 编程助手）：**

| # | 方向 | 描述 | 风格 |
|---|------|------|------|
| 1 | Terminal Precision | CLI 能量、数据密度、开发者原生 | Swiss Pulse |
| 2 | Quiet Confidence | 克制、高级、不说教 | Velvet Standard |
| 3 | Electric Build | 发布能量、大字体、冲击力 | Maximalist Type |
| 4 | Warm Companion | 友好、平易近人、有温度 | Soft Signal |
| 5 | Dark Intelligence | 神秘、深度、技术优越感 | Shadow Cut |

**输出格式：** 简洁的表格或编号列表，每项 2-3 行。不要长篇大论。

用户选择一个方向（或说"X 的感觉但更 Y"进行混合）。

---

### 第 3 步：色板

基于选定方向，推荐 **3-4 套色板**。

**生成规则：**

- 从 [house-style.md](../house-style.md) 的 9 个类别中选最匹配的 2-3 个，再根据方向微调 1 套
- 每套色板包含：背景色、前景色、强调色、中性色
- 暗/亮必须混合 — 即使用户说"暗色"也给一个亮色选项作对比
- 色板命名用品牌世界的语言，不用通用情绪词（不用"活力蓝"、"沉稳灰"）

**每套色板格式：**

```
色板名
  bg:    #0a0a0a   (slate-950)
  fg:    #e2e8f0   (slate-200)
  accent:#3b82f6   (blue-500)
  muted: #64748b   (slate-500)
```

用户选择一套，或指定"用 B 但把强调色换成绿色"。

---

### 第 4 步：字体配对

**前置步骤：** 阅读 [typography.md](typography.md) 的禁用列表和护栏规则。

**生成规则：**

- 推荐 **3-4 套字体配对**
- 每套包含：标题字体 + 正文字体 + 可选的数据/标签字体
- **必须跨类别配对** — 不要两个无衬线（typography.md 规则）
- **必须避开禁用列表** — Inter、Roboto、Playfair Display 等全部禁用
- 字重对比必须极端 — 标题 800-900，正文 300-400
- 说明每套配对的张力是什么（"机械 vs 人文"、"公共 vs 私人"等）

**每套格式：**

```
配对名
  标题: Instrument Serif 800   → font-[Instrument_Serif] font-[800]
  正文: Space Mono 400          → font-[Space_Mono] font-[400]
  张力: 有机衬线的温度 × 等宽的精确感
```

用户选择一套。

---

### 第 5 步：细节确认

快速确认以下细节（可以合并为一轮对话）：

| 维度 | 选项 | 默认 |
|------|------|------|
| **圆角** | 无 (0px) / 细微 (4-8px) / 圆润 (12-16px) / 胶囊 (999px) | 根据方向推断 |
| **密度** | 紧凑 / 标准 / 宽松 | 根据平台推断（社交媒体=紧凑，演示=宽松） |
| **深度** | 平面（无阴影）/ 微妙（轻阴影）/ 分层（发光+阴影） | 根据方向推断 |
| **动效能量** | 低（沉思）/ 中（专业）/ 高（冲击） | 根据方向推断 |
| **首选转场** | fade / slide / wipe / gl_transition | 根据风格推荐 |

如果用户不想纠结，直接用默认值跳过。

---

### 第 6 步：生成 design.md

将以上所有选择合并为一个 `design.md` 文件。

**格式：** YAML frontmatter + Markdown 正文。

```yaml
---
name: [项目名]
colors:
  primary: "#0a0a0a"       # → slate-950
  on-primary: "#e2e8f0"    # → slate-200
  accent: "#3b82f6"        # → blue-500
  muted: "#64748b"         # → slate-500
typography:
  headline:
    fontFamily: "Instrument Serif"
    fontSize: 80px          # → text-[80px]
    fontWeight: 800         # → font-[800]
  body:
    fontFamily: "Space Mono"
    fontSize: 24px          # → text-[24px]
    fontWeight: 400         # → font-[400]
  label:
    fontFamily: "Space Mono"
    fontSize: 16px          # → text-[16px]
    fontWeight: 400         # → font-[400]
rounded:
  sm: 4px                   # → rounded
  md: 8px                   # → rounded-lg
  lg: 16px                  # → rounded-2xl
spacing:
  sm: 8px                   # → gap-2 / p-2
  md: 16px                  # → gap-4 / p-4
  lg: 32px                  # → gap-8 / p-8
motion:
  energy: high              # low / medium / high
  easing:
    entry: "ease-out"
    exit: "ease-in"
    ambient: "sine.inOut"
  duration:
    entrance: 12            # 帧
    hold: 45
    transition: 18
  atmosphere:
    - radial-glow
    - ghost-text
  transition: fade
---

## Overview

[2-3 句话描述整体视觉方向和设计意图]

## Colors

[色板说明和使用规则]

## Typography

[字体选择理由、使用规则、尺寸层级]

## Layout

[布局约束、间距规则、圆角风格]

## Elevation

[阴影和深度规则]

## Components

[组件样式规则：卡片、按钮、标签等]

## Do's and Don'ts

[这个项目的具体规则 — 不是通用规则，是针对这个品牌/内容的]
```

---

## 注意事项

- **不要跳步骤。** 每一步的选择都影响下一步的选项。
- **不要生成通用选项。** "科技蓝"、"活力橙"是失败。选项必须来自用户的具体内容。
- **不要长篇大论。** 每步的选项用表格或编号列表，2-3 行一个选项。
- **允许混合。** 用户说"A 的配色但 B 的排版"是有效选择。
- **design.md 完成后继续 SKILL.md 步骤 2（Prompt 扩展）。** design.md 是视觉身份的事实来源，后续所有步骤都引用它。
