# 文字高亮动效模式

五种强调动画模式的高亮实现（highlight / circle / burst / scribble / sketchout），全部使用 OpenCat JSONL + Tailwind + ctx.* API。适合在字幕或标题中为重点词添加视觉强调。

## 目录

- [1. 高亮模式](#1-高亮模式highlight) — 黄色马克笔扫过文字
- [2. 圆圈模式](#2-圆圈模式circle) — 手绘椭圆环绕文字
- [3. 爆发模式](#3-爆发模式burst) — 从文字中心辐射线条
- [4. 涂鸦模式](#4-涂鸦模式scribble) — SVG 波浪下划线
- [5. 划掉模式](#5-划掉模式sketchout) — 交叉线划掉文字
- [6. 模式组合与轮换](#6-模式组合与轮换)

## 1. 高亮模式 (highlight)

黄色半透明条从左到右扫过文字，模拟马克笔高亮。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"hl-scene","parentId":"root","type":"div","className":"absolute inset-0 flex items-center justify-center"}
{"id":"hl-wrap","parentId":"hl-scene","type":"div","className":"relative inline"}
{"id":"hl-bar","parentId":"hl-wrap","type":"div","className":"absolute inset-0 -left-[6px] -right-[6px] bg-yellow-400 opacity-35 scale-x-0 origin-left rounded-[3px] z-0"}
{"id":"hl-text","parentId":"hl-wrap","type":"text","className":"relative z-1 text-white text-[48px] font-bold","text":"高亮文字"}
{"type":"script","parentId":"root","src":"var tl = ctx.timeline();\ntl.to('hl-bar', { scaleX: 1, duration: 15, ease: 'ease-out' }, 18);"}
```

说明：
- `hl-bar` 初始 `scale-x-0`（不显示），动画展开到 `scaleX: 1`。
- `origin-left` 确保缩放从左向右。
- 0.6s（18f）延迟后动画开始，避免零延迟跳切感。
- 可选增加 `skewX: -2` 取得手绘倾斜效果。

### 多行高亮

多段文字使用交错：

```jsonl
{"type":"script","parentId":"root","src":"var tl = ctx.timeline();\ntl.to('hl-bar', { scaleX: 1, duration: 15, ease: 'ease-out', stagger: 9 }, 18);"}
```

每行交错 0.3s（9f）。

---

## 2. 圆圈模式 (circle)

红色圆环从中心放大包住文字，带弹性效果。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"circle-scene","parentId":"root","type":"div","className":"absolute inset-0 flex items-center justify-center"}
{"id":"circle-wrap","parentId":"circle-scene","type":"div","className":"relative inline"}
{"id":"circle-text","parentId":"circle-wrap","type":"text","className":"relative z-1 text-white text-[48px] font-bold","text":"重点"}
{"id":"circle-ring","parentId":"circle-wrap","type":"div","className":"absolute top-1/2 left-1/2 w-[130%] h-[160%] border-2 border-red-500 rounded-full pointer-events-none z-0"}
{"type":"script","parentId":"root","src":"ctx.set('circle-ring', { x: '-50%', y: '-50%', rotate: -3, scale: 0 });\nvar tl = ctx.timeline();\ntl.to('circle-ring', { scale: 1, rotation: -3, duration: 18, ease: 'back-out' }, 21);"}
```

说明：
- 初始 `scale-0` 隐藏，动画用 `ease: 'back-out'` 产生弹性放大效果。
- `-translate-x-1/2 -translate-y-1/2` 定位中心。
- `rotation: -3` 保持手绘的微微旋转感。
- 0.7s（21f）延迟入场。

### 变体

| 变体 | 调整 |
|------|------|
| 紧凑圈（短词） | `w-[150%] h-[180%]` |
| 圆角矩形 | `rounded-[30%] w-[120%] h-[140%]` |
| 椭圆（宽>高） | `w-[150%] h-[130%] rounded-full` |

---

## 3. 爆发模式 (burst)

从文字中心向外辐射颜色线，每条线不同长度产生有机感。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1920px] h-[1920px]"}
{"id":"burst-scene","parentId":"root","type":"div","className":"absolute inset-0 flex items-center justify-center"}
{"id":"burst-wrap","parentId":"burst-scene","type":"div","className":"relative inline"}
{"id":"burst-text","parentId":"burst-wrap","type":"text","className":"relative z-2 text-white text-[48px] font-bold","text":"惊叹"}
{"id":"burst-container","parentId":"burst-wrap","type":"div","className":"absolute top-1/2 left-1/2 w-0 h-0 z-1"}
{"id":"line-0","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[70px] bg-blue-500 -left-[1.5px]"}
{"id":"line-1","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[55px] bg-blue-500 -left-[1.5px]"}
{"id":"line-2","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[80px] bg-blue-500 -left-[1.5px]"}
{"id":"line-3","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[45px] bg-blue-500 -left-[1.5px]"}
{"id":"line-4","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[65px] bg-blue-500 -left-[1.5px]"}
{"id":"line-5","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[75px] bg-blue-500 -left-[1.5px]"}
{"id":"line-6","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[50px] bg-blue-500 -left-[1.5px]"}
{"id":"line-7","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[60px] bg-blue-500 -left-[1.5px]"}
{"id":"line-8","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[80px] bg-blue-500 -left-[1.5px]"}
{"id":"line-9","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[40px] bg-blue-500 -left-[1.5px]"}
{"id":"line-10","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[70px] bg-blue-500 -left-[1.5px]"}
{"id":"line-11","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[55px] bg-blue-500 -left-[1.5px]"}
{"type":"script","parentId":"root","src":"var lines = ['line-0','line-1','line-2','line-3','line-4','line-5','line-6','line-7','line-8','line-9','line-10','line-11'];\nvar tl = ctx.timeline();\ntl.fromTo(lines, { scaleY: 0, opacity: 0 }, { scaleY: 1, opacity: 1, duration: 12, ease: 'ease-out', stagger: 1 }, 21);"}
```

说明：
- 12 条辐射线均匀分布（每 30° 一条），长度 40-80px 不等防机械感。
- `className` 中的 `-left-[1.5px]` 作为宽度补偿（3px 宽的线条居中）。
- 每条线的旋转角度和高度在初始 JSONL 中不声明，通过 `ctx.set()` 在首帧设置（避免 style 属性）。
- 用数组列出所有线条 ID，交错 1f 造成辐射扩散效果。
- 初始 `scaleY: 0`，展开到 `scaleY: 1`，模拟线条从中心向外生长。

---

## 4. 涂鸦模式 (scribble)

SVG 波浪路径从文字下方划过，利用 `ctx.set()` 设置 `strokeDasharray` / `strokeDashoffset` 并通过 `ctx.to()` 动画实现自绘效果。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"scribble-scene","parentId":"root","type":"div","className":"absolute inset-0 flex items-center justify-center"}
{"id":"scribble-wrap","parentId":"scribble-scene","type":"div","className":"relative inline"}
{"id":"scribble-text","parentId":"scribble-wrap","type":"text","className":"relative z-1 text-white text-[48px] font-bold","text":"下划线文字"}
{"id":"scribble-svg","parentId":"scribble-wrap","type":"div","className":"absolute left-0 -bottom-[6px] w-full h-[24px] z-0"}
{"id":"scribble-path","parentId":"scribble-svg","type":"path","className":"fill-none stroke-[#FDD835] stroke-[3px]","d":"M0,12 Q31,0 62,12 Q93,24 125,12 Q156,0 187,12 Q218,24 250,12 Q281,0 312,12 Q343,24 375,12 Q406,0 437,12 Q468,24 500,12"}
{"type":"script","parentId":"root","src":"ctx.set('scribble-path', { strokeDasharray: 500, strokeDashoffset: 500 });\nvar tl = ctx.timeline();\ntl.to('scribble-path', { strokeDashoffset: 0, duration: 24, ease: 'ease-in-out' }, 21);"}
```

说明：
- `type: "path"` 节点定义 SVG path，`d` 为路径数据，样式通过 className 的 Tailwind 类控制（`stroke-*`、`fill-*`）。
- 用 SVG 路径包围盒渲染，无需 `<svg>` 标签包裹。
- 脚本设置 `strokeDasharray` / `strokeDashoffset`，动画使 offset → 0 实现自绘。路径总长度用固定估算值（或预计算）。
- 0.8s（24f）匀速绘制，0.7s（21f）延迟。

### 删除线变体

将 SVG 定位改为垂直居中：

```
className="absolute left-0 top-1/2 w-full h-[24px] z-0"
```

### 波浪路径参数

| 波浪密度 | x 增量（每半波） |
|----------|-----------------|
| 紧凑 | 25px |
| 标准 | 31px |
| 宽松 | 50px |

振幅通过改变 `y` 范围控制：`0-24` 标准，`0-16` 温和。

---

## 5. 划掉模式 (sketchout)

两条交叉红线从中间向两侧划过文字，模拟"划掉"效果。

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":180}
{"id":"root","parentId":null,"type":"div","className":"relative w-[1920px] h-[1080px]"}
{"id":"sketchout-scene","parentId":"root","type":"div","className":"absolute inset-0 flex items-center justify-center"}
{"id":"sketchout-wrap","parentId":"sketchout-scene","type":"div","className":"relative inline"}
{"id":"sketchout-text","parentId":"sketchout-wrap","type":"text","className":"relative z-0 text-gray-400 text-[48px] font-bold line-through","text":"旧价格"}
{"id":"sketchout-lines","parentId":"sketchout-wrap","type":"div","className":"absolute inset-0 -left-[4px] -right-[4px] overflow-hidden z-1"}
{"id":"line-fwd","parentId":"sketchout-lines","type":"div","className":"absolute block top-1/2 left-0 w-full h-[2px] bg-red-500 origin-left"}
{"id":"line-bwd","parentId":"sketchout-lines","type":"div","className":"absolute block top-1/2 left-0 w-full h-[2px] bg-red-500 origin-left"}
{"type":"script","parentId":"root","src":"ctx.set('line-fwd', { scaleX: 0, rotate: -12 });\nctx.set('line-bwd', { scaleX: 0, rotate: 12 });\nvar tl = ctx.timeline();\ntl.to('line-fwd', { scaleX: 1, duration: 9, ease: 'ease-out' }, 30);\ntl.to('line-bwd', { scaleX: 1, duration: 9, ease: 'ease-out' }, 35);"}
```

说明：
- `line-fwd` 旋转 -12°（前斜线），`line-bwd` 旋转 12°（后斜线）。
- 前斜线在 1.0s（30f）开始，0.3s（9f）画出；后斜线在 1.15s（35f）开始（错开 5f）。
- 初始 `scale-x-0`，动画展开到 `scaleX: 1`。
- `sketchout-text` 上的 `.line-through` 作为备用样式，但动效由覆盖线实现。

---

## 6. 模式组合与轮换

在字幕场景中轮换模式，避免视觉单调：

```jsonl
{"type":"script","parentId":"root","src":"var MODES = ['highlight', 'circle', 'burst', 'scribble'];\nGROUPS.forEach(function(group, gi) {\n  var mode = MODES[gi % MODES.length];\n  group.emphasisWords.forEach(function(word) {\n    applyMode(word.el, mode, ctx, word.start);\n  });\n});"}
```

轮换节奏建议：
- 高能量：每 2-3 组轮换
- 中等能量：每 3-4 组轮换
- 低能量：每 4-5 组轮换
