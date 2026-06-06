# Design Create

当前 OpenCat skill 默认项目已经提供 `design.md`。只有用户明确要求“创建 design.md”或项目确实缺失设计文件时，才使用本文件。

不要使用本地预设配色或风格表推导颜色。设计值必须来自用户提供的品牌信息、现有产品视觉、或用户明确确认的选择。

## 最小目标

生成项目根目录 `design.md`，作为后续 XML 的品牌事实来源。它至少要包含：

- 颜色 token：背景、前景、强调色、弱化文本/线条
- 字体 token：标题、正文、标签/数据
- 形状 token：圆角、边框、阴影/发光
- 间距 token：紧凑/标准/宽松的 padding 和 gap
- 动效 token：能量、入场缓动、环境动效、转场倾向
- Do's and Don'ts：这个品牌/视频明确要做和避免的事

## 缺失信息时怎么问

一次只问必要问题：

1. **品牌颜色/字体是否已有？**
2. **画面应该偏明亮还是偏暗？**
3. **动效气质是什么：克制、专业、高能、电影感，还是其他？**

如果用户给了品牌色或截图描述，就围绕它生成 token。不要提供多套配色让用户挑。

## 输出格式

```markdown
---
name: [项目名]
description: [设计风格描述]
keywords: [关键词]
colors:
  background: "#000000"
  foreground: "#ffffff"
  accent: "#00C3FF"
  muted: "#94A3B8"
typography:
  headline:
    fontFamily: "[标题字体]"
    fontSize: 96px
    fontWeight: 800
  body:
    fontFamily: "[正文字体]"
    fontSize: 32px
    fontWeight: 400
  label:
    fontFamily: "[标签字体]"
    fontSize: 20px
    fontWeight: 500
rounded:
  sm: 4px
  md: 8px
  lg: 16px
spacing:
  sm: 12px
  md: 32px
  lg: 80px
motion:
  energy: medium
  easing:
    entry: "ease-out"
    ambient: "sine.inOut"
  duration:
    entrance: 0.5
    transition: 0.45
  atmosphere:
    - [背景/纹理/结构线规则]
  transition: fade
---

## Overview

[2-3 句话描述整体视觉方向]

## Colors

[每个 token 怎么用，哪些场景禁止发明新色]

## Typography

[字号层级、字重关系、适用场景]

## Layout

[边距、网格、对齐、密度]

## Motion

[入场、环境动效、转场气质]

## Components

[字幕、标签、卡片、强调词、边角 metadata 等]

## Do's and Don'ts

[具体规则]
```

## 规则

- `design.md` 完成后，后续所有 XML 颜色、字体、圆角、间距和动效都引用它。
- 如果某个值用户没有给，生成一个最小可用 token，但要在正文说明“需用户确认”。
- 不要维护或引用本地预设配色文件。
