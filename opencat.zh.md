# OpenCat JSONL

> **格式规则**
> - **每行一个 JSON 对象。** 不要将单个 JSON 对象拆分到多行。
> - **脚本内容中不要写注释。** 脚本代码必须保持干净。

OpenCat JSONL 是一种 JSON Lines 格式，用于描述动态图形合成。每行是一个节点声明、脚本附着或元数据记录。运行时解析文件、构建场景树，并使用 Skia + Taffy + QuickJS 渲染帧。

---

## 1. 合成头

第一行必须是 `composition` 记录。

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 180}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `width` | `i32` | 画布宽度（像素） |
| `height` | `i32` | 画布高度（像素） |
| `fps` | `i32` | 帧率 |
| `frames` | `i32` | 总帧数。`frames / fps` = 时长（秒） |

---

## 2. 节点树

### 2.1 父子关系

每个节点（`composition` 和 `script`/`transition` 除外）都有 `id` 和 `parentId`。树通过这些链接构建。

- 有且仅有一个根节点的 `parentId` 为 `null`。
- `parentId` 必须引用已声明的 `id`。
- `script` 和 `transition` 记录没有 `id`，通过 `parentId` 附着到节点。

### 2.2 普通树（单场景）

适用于单场景、静态叠加层，或不需要场景间转场的合成。

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 60}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold", "text": "Hello"}
```

### 2.3 时间线（多场景 + 转场）

适用于两个或更多场景之间需要转场的情况。

```json
{"type":"composition","width":390,"height":844,"fps":30,"frames":162}
{"id":"root","parentId":null,"type":"div","className":"relative w-[390px] h-[844px]"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene1","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-white","duration":60}
{"id":"scene2","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-slate-900","duration":90}
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"fade","duration":12}
```

规则：

- `tl` 必须显式声明为树中的节点。不支持根级多场景推断。
- `tl` 遵循 `NodeStyle`——`tl` 节点本身的布局和脚本会被保留。
- `tl` 必须至少有两个直接子场景，每对相邻场景之间必须有对应的 `transition`。
- `tl` 没有 `duration` 字段。总时长由 `sum(scene.duration) + sum(transition.duration)` 推导。
- `transition.parentId` 为必填，必须引用所属的 `tl` 节点。
- 如需 z-order 合成，将 `tl` 和持久叠加层（如 `caption`）作为兄弟节点放在共享的父 `div` 下。
- `composition.frames` 应与推导出的总时长对齐。

---

## 3. 节点类型

每个元素是一行 JSON。`className` 使用 Tailwind 风格的类名（参见 §5 样式）。

### 3.1 `div`

容器节点，支持 flex 布局。等价于 `<div>`。

```json
{"id": "box", "parentId": "root", "type": "div", "className": "flex flex-col items-center gap-4 p-6"}
```

除 `id`、`parentId`、`className`、`duration` 外无特殊字段。

### 3.2 `text`

文本内容节点。等价于 `<span>` / `<p>`。

```json
{"id": "title", "parentId": "box", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Hello"}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `text` | 是 | 文本内容 |

### 3.3 `image`

图片节点。等价于 `<img>`。

```json
{"id": "hero", "parentId": "scene1", "type": "image", "className": "w-[300px] h-[200px] object-cover rounded-lg", "query": "mountain landscape"}
```

指定一种图片来源：

| 字段 | 说明 |
|------|------|
| `path` | 本地文件路径 |
| `url` | 远程 URL |
| `query` | Openverse 搜索关键词（1-4 个名词） |

使用 `query` 时的可选字段：

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `queryCount` | `1` | 获取的图片数量 |
| `aspectRatio` | — | 宽高比过滤器（如 `"square"`） |

### 3.4 `icon`

Lucide 图标节点。使用 kebab-case 图标名称。

```json
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] text-slate-400", "icon": "search"}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `icon` | 是 | Lucide 图标名称（kebab-case） |

图标用 `text-{color}` 着色，不要用 `bg-{color}`。

### 3.5 `canvas`

画布绘图表面。需要子 `script` 提供绘图命令。

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

完整绘图参考见 §7 Canvas API。

### 3.6 `audio`

音频播放节点。等价于 `<audio>`。

```json
{"id": "bgm", "parentId": "root", "type": "audio", "path": "/tmp/bgm.mp3"}
{"id": "sfx", "parentId": "root", "type": "audio", "url": "https://example.com/sfx.mp3"}
```

指定一种来源：`path`（本地）或 `url`（远程）。

`parentId` 控制音频播放时机：
- 附着在场景节点下 → 在该场景期间播放。

### 3.7 `video`

视频播放节点。等价于 `<video>`。

```json
{"id": "clip", "parentId": "scene1", "type": "video", "className": "w-full h-full object-cover", "path": "clip.mp4"}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `path` | 是 | 本地视频文件路径 |

### 3.8 `caption`

SRT 驱动的文本节点。显示内容通过最近继承的时间上下文从字幕条目中选取。

```json
{"id": "subs", "parentId": "root", "type": "caption", "className": "absolute inset-x-[48px] bottom-[32px] text-center text-white", "path": "subtitles.utf8.srt"}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `path` | 是 | 本地 SRT 文件路径 |
| `duration` | 否 | 通常省略；可见性由 SRT 时间戳驱动 |

### 3.9 `tl`

时间线容器。完整规范见 §2.3。

| 字段 | 说明 |
|------|------|
| `id` | 必填 |
| `parentId` | 父节点 |
| `className` | Tailwind 样式 |

没有 `duration` 字段——总时长由子场景和转场推导。

### 3.10 `transition`

`tl` 内两个相邻场景之间的转场。完整规范见 §4。

---

## 4. 转场

转场描述 `tl` 节点内两个相邻场景之间的切换。转场会消耗额外的帧。

```json
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `parentId` | 是 | 必须引用所属的 `tl` 节点 |
| `from` | 是 | 源场景 id（必须是 `tl` 的直接子节点） |
| `to` | 是 | 目标场景 id（必须与 `from` 相邻） |
| `effect` | 是 | 效果名称（见下表） |
| `duration` | 是 | 转场时长（帧） |
| `direction` | 否 | `slide` / `wipe` 的方向 |
| `timing` | 否 | 缓动名称（默认 `"linear"`），见 §5.1 |
| `damping` | 否 | 自定义弹簧阻尼 |
| `stiffness` | 否 | 自定义弹簧刚度 |
| `mass` | 否 | 自定义弹簧质量 |

### 效果类型

| effect | 说明 | direction（可选） |
|--------|------|-------------------|
| `fade` | 交叉淡化 | — |
| `slide` | 滑动转场 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` |
| `wipe` | 擦除转场 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` / `from_top_left` / `from_top_right` / `from_bottom_left` / `from_bottom_right` |
| `clock_wipe` | 时钟擦除 | — |
| `iris` | 虹膜开合 | — |
| `light_leak` | 光泄漏 | — |

`light_leak` 额外字段：`seed`（`f32`）、`hueShift`（`f32`）、`maskScale`（`f32`，范围 `0.03125`–`1.0`）。

### 示例

```json
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 20, "timing": "ease-out"}
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "slide", "direction": "from_right", "duration": 15, "timing": "bezier:0.4,0,0.2,1"}
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "wipe", "direction": "from_right", "duration": 12, "damping": 10, "stiffness": 100, "mass": 1}
```

---

## 5. 样式（Tailwind）

`className` 使用 Tailwind 风格的类名来设置布局、颜色、间距、圆角等视觉属性。

**限制：**

- 不要使用 CSS 动画类（`transition-*`、`animate-*`、`duration-*`、`ease-*`、`delay-*`）。
- 不要在 `className` 中使用变换类（`transform`、`translate-*`、`rotate-*`、`scale-*`、`skew-*`）。使用脚本 Node API 代替。

| 避免 | 替代方案 |
|------|----------|
| `transition-*` `animate-*` `duration-*` `ease-*` `delay-*` | `ctx.animate()` / `ctx.stagger()` / `ctx.sequence()` |
| `transform` `translate-*` `rotate-*` `scale-*` `skew-*` | `ctx.getNode(...).translateX()` / `translateY()` / `scale()` / `rotate()` / `skew()` |

> Tailwind 处理静态样式，脚本处理动画。

### 5.1 缓动参考

缓动名称在 `ctx.animate()`、`ctx.sequence()` 步骤和 `transition.timing` 中共享。

| 预设 | 效果 |
|------|------|
| `'linear'` | 匀速 |
| `'ease'` / `'ease-in'` / `'ease-out'` / `'ease-in-out'` | 标准 CSS 三次曲线 |
| `'back-in'` / `'back-out'` / `'back-in-out'` | 轻微过冲（UI 弹性） |
| `'elastic-in'` / `'elastic-out'` / `'elastic-in-out'` | 阻尼振荡 |
| `'bounce-in'` / `'bounce-out'` / `'bounce-in-out'` | 地面弹跳 |
| `'steps(N)'` | 量化为 N 个离散步进 |
| `'spring-default'` | 通用弹簧 |
| `'spring-gentle'` | 柔和弹簧 |
| `'spring-stiff'` | 硬弹簧 |
| `'spring-slow'` | 慢弹簧 |
| `'spring-wobbly'` | 摇晃弹簧 |

自定义弹簧（JS）：

```js
easing: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

三次贝塞尔（JS）：

```js
easing: [0.25, 0.1, 0.25, 1.0]
```

转场 `timing` 字段也接受字符串形式：`"bezier:0.4,0,0.2,1"`。

---

## 6. 脚本

脚本通过 `script` 记录附着到节点。

```json
{"type": "script", "parentId": "scene1", "src": "ctx.animate({targets:'title',from:{opacity:0},to:{opacity:1},duration:20,easing:'spring-gentle'});"}
{"type": "script", "parentId": "scene1", "path": "scene1.js"}
```

| 字段 | 说明 |
|------|------|
| `parentId` | 可选。附着到节点作用域，省略则为全局作用域 |
| `src` | 内联 JavaScript 代码 |
| `path` | 外部 `.js` 文件路径（相对于 JSONL 文件解析） |

`src` 和 `path` 互斥，必须指定其中一个。

### 6.1 执行上下文

| 字段 | 说明 |
|------|------|
| `ctx.frame` | 全局帧索引 |
| `ctx.totalFrames` | 总帧数 |
| `ctx.currentFrame` | 当前场景内的帧索引（`0 → sceneFrames - 1`） |
| `ctx.sceneFrames` | 当前场景的帧数 |

场景局部动画优先使用 `ctx.currentFrame` 和 `ctx.sceneFrames`。

### 6.2 设计：精确数学计算

OpenCat 的动画系统是**函数式纯的**：每个动画值通过精确数学公式 `value = f(current_frame)` 计算。没有内部 tick 循环，没有累积状态，没有非确定性漂移。

- **插值**：线性 `from + (to - from) * easing(progress)`
- **弹簧**：通过物理参数（`stiffness`、`damping`、`mass`）求解，带精确稳定时间检测
- **颜色**：HSLA 空间的插值，最短弧线旋转（处理 360° 环绕）
- **路径**：通过 Skia `ContourMeasure` 实现亚像素精确弧长采样

脚本每帧重新执行。模式是：声明动画 → 读取当前值 → 写入节点。声明式 `targets`（见下文）自动完成写入步骤，不改变底层数学计算。

---

### 6.3 ctx.animate(opts)

声明 `from → to` 动画。值通过 `targets` 自动应用到节点。

```js
ctx.animate({
  targets: 'hero',
  from: { opacity: 0, translateY: 40, scale: 0.95 },
  to:   { opacity: 1, translateY: 0,  scale: 1 },
  duration: 30,
  delay: 0,
  easing: 'spring-gentle',
  clamp: false,
});
```

返回对象的额外 getter：

- `anim.progress`：`0` → `1`
- `anim.settled`：弹簧是否已稳定
- `anim.settleFrame`：弹簧稳定的帧

`targets` 接受：
- 单个节点 id：`targets: 'hero'`
- id 数组：`targets: ['node1', 'node2', 'node3']`
- 分割文本部件：`targets: ctx.splitTextNode('title', { granularity: 'graphemes' })`

`targets` 是首选模式。需要完全手动控制时（联动运动、派生值），返回对象的 getter 仍可直接使用（见 §6.11）。

**仅 `from` 语义：**

如果只指定 `from`，动画从身份默认值推断 `to`（`opacity: 1`、`translateX/Y: 0`、`scale: 1`、`rotation: 0` 等）。匹配 GSAP 的 `gsap.from()` 行为。

```js
ctx.animate({
  targets: 'box',
  from: { opacity: 0, translateY: 24 },
  duration: 20,
});
// 隐式 to: { opacity: 1, translateY: 0 }
```

**通过 `ctx.animate` 交错：**

当 `targets` 是数组时，传入 `stagger` 偏移每个目标的延迟：

```js
ctx.animate({
  targets: ['a', 'b', 'c', 'd'],
  from: { opacity: 0, translateY: 20 },
  to:   { opacity: 1, translateY: 0 },
  duration: 18,
  stagger: 3,
  easing: 'spring-gentle',
});
```

**重复选项：**

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `repeat` | `0` | 额外循环次数。`0` = 播放一次，`N` = N+1 次，`-1` = 无限 |
| `yoyo` | `false` | 交替循环反向播放 |
| `repeatDelay` | `0` | 每次循环重启前保持的帧数 |

颜色属性（`bg`、`textColor`、`borderColor`）与数值属性的动画方式相同——自动进行 HSLA 最短路径插值：

```js
ctx.animate({
  targets: 'card',
  from: { bg: '#ef4444' },
  to:   { bg: 'hsl(220, 90%, 55%)' },
  duration: 60,
  repeat: -1,
  yoyo: true,
});
```

支持的颜色字面量：`#rgb` / `#rrggbb` / `#rrggbbaa`、`rgb(r,g,b)` / `rgba(r,g,b,a)`、`hsl(h,s%,l%)` / `hsla(h,s%,l%,a)`。Tailwind 标记如 `'blue-500'` **不会**被插值——请在 `from`/`to` 中使用 hex/rgb/hsl。

#### 路径动画

传入 `path`（SVG 路径字符串）代替 `from`/`to`，沿曲线动画。返回 `x`、`y`、`rotation` getter。

```js
var a = ctx.animate({
  path: 'M100 360 C400 80 880 640 1180 360',
  orient: -90,
  duration: 120,
  easing: 'ease-in-out',
  repeat: -1,
  yoyo: true,
});
ctx.getNode('ball')
  .position('absolute')
  .left(a.x - 24)
  .top(a.y - 24)
  .rotate(a.rotation);
```

`path` 和 `from`/`to` 可以在同一动画中共存——用 `from`/`to` 控制 `opacity` 等属性，用 `path` 驱动位置。

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `path` | — | SVG 路径字符串（支持的命令见下文） |
| `orient` | `0` | 相对于路径切线的旋转偏移（度）。向上朝向的形状使用 `-90` |

#### 关键帧（单动画多停驻点）

单个动画需要超过两个停驻点时，使用 `keyframes` 代替 `from`/`to`：

```js
// 简写：均匀分布的数值
ctx.animate({
  targets: 'card',
  keyframes: { scale: [1, 1.4, 0.8, 1] },
  duration: 60,
});

// 完整形式：显式 `at`（归一化时间 [0, 1]）+ 可选的分段缓动
ctx.animate({
  targets: 'logo',
  keyframes: {
    rotate: [
      { at: 0,   value: 0 },
      { at: 0.5, value: 360, easing: 'back-out' },
      { at: 1,   value: 0 }
    ],
  },
  duration: 60,
});
```

说明：

- 关键帧仅支持**数值**。不支持颜色关键帧——颜色动画请使用 `from`/`to`。
- `at` 归一化到 `[0, 1]`；外层 `easing`（和 `repeat`/`yoyo`）先生效，然后通过分段缓动映射结果。
- `keyframes` 和 `from`/`to` 可共存；同时定义的键以 `keyframes` 为准。

### 6.4 ctx.stagger(count, opts)

类似 `animate`，但创建多个交错动画。始终配合 `targets` 使用。

```js
ctx.stagger(0, {
  targets: ['a', 'b', 'c', 'd'],
  from: { opacity: 0, scale: 0.9 },
  to:   { opacity: 1, scale: 1 },
  gap: 3,
  duration: 18,
});
```

提供 `targets` 时，`count` 从目标列表长度推断，每个目标接收自己交错的动画。

对于需要逐节点手动控制的情况，`ctx.stagger(count, opts)` 返回值对象数组：

```js
var anims = ctx.stagger(4, {
  from: { opacity: 0, translateY: 30 },
  to:   { opacity: 1, translateY: 0 },
  gap: 4,
  duration: 20,
  easing: 'spring-gentle',
});
```

### 6.5 ctx.sequence(steps)

异构动画链。每个步骤推进内部光标，因此每个步骤的 `duration`、`easing`、`from`、`to` 可以不同。当 `ctx.stagger`（相同动画、统一间隔）不够灵活时使用——不规则计时、重叠或平行分支。

```js
var seq = ctx.sequence([
  { from: { opacity: 0, translateY: -20 }, to: { opacity: 1, translateY: 0 }, duration: 24, easing: 'spring-gentle' },
  { from: { opacity: 0 }, to: { opacity: 1 }, duration: 18, gap: -6 },
  { from: { scale: 0.8 }, to: { scale: 1 }, duration: 30, easing: 'spring-stiff' },
]);

ctx.getNode('title').opacity(seq[0].opacity).translateY(seq[0].translateY);
ctx.getNode('subtitle').opacity(seq[1].opacity);
ctx.getNode('cta').scale(seq[2].scale);
```

**逐步骤字段：**

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `from`、`to`、`duration`、`easing`、`clamp` | — | 同 `ctx.animate()` |
| `delay` | `0` | 步骤开始前的额外偏移 |
| `gap` | `0` | 步骤结束后推进光标的帧数。负值表示与下一步重叠。 |
| `at` | — | 绝对起始帧。设置后忽略光标且**不推进光标**。用于平行分支或固定锚点。 |

每个返回项暴露与 `ctx.animate()` 相同的 getter（`progress`、`settled`、`settleFrame`，以及每个动画键）。

**使用 `at` 的平行分支：**

```js
var seq = ctx.sequence([
  { to: { opacity: 1 }, duration: 20 },       // 运行 0..20，光标 → 20
  { to: { opacity: 1 }, duration: 30, at: 5 }, // 固定在帧 5，光标不动
  { to: { opacity: 1 }, duration: 10 },        // 从光标 20 开始，运行 20..30
]);
```

**如何选择：**

| 使用场景 | API |
|----------|-----|
| 单动画 | `ctx.animate({ targets, from, to })` |
| N 个相同动画，均匀间隔 | `ctx.stagger(0, { targets, from, to, gap })` |
| 文本单元（字素/词） | `ctx.splitTextNode(id, opts).animate({...})` |
| 异构步骤、不规则间隔、重叠、平行分支 | `ctx.sequence` |
| 逐节点手动控制（联动运动、派生值） | `ctx.animate({ from, to })` + `ctx.getNode()` |

### 6.6 ctx.typewriter(fullText, opts)

由动画曲线驱动，逐字符打出文本。返回一个对象，其 `text` getter 为当前帧产生正确的子字符串。

```js
var tw = ctx.typewriter('Hello OpenCat', {
  duration: 30,
  delay: 6,
  easing: 'linear',
  caret: '▍',
});

ctx.getNode('title').text(tw.text);
```

**选项：**

| 字段 | 默认值 | 说明 |
|------|--------|------|
| `duration` | — | 必填。从空字符串到完整字符串的帧数。 |
| `delay` | `0` | 开始打字前等待的帧数。 |
| `easing` | `'linear'` | `ctx.animate()` 支持的任何缓动。非线性可改变打字速度。 |
| `clamp` | `true` | 防止弹簧/贝塞尔过冲产生超出范围的字符数。 |
| `caret` | `''` | 打字过程中附加的字符串。完整文本显示后消失。 |

也暴露 `progress`、`settled` 和 `settleFrame`，同 `ctx.animate()`。

字符计数当前使用 `Array.from()`，因此效果基于 code point。
这对 ASCII、CJK 和许多单 emoji 情况适用良好，但不是 ZWJ emoji 或组合标记序列的完整字素簇分割器。

`ctx.typewriter()` 是一个内容替换辅助工具：为当前帧生成当前字符串，
通常通过 `ctx.getNode(id).text(tw.text)` 应用。

---

### 6.7 文本动画（`ctx.splitTextNode`）

OpenCat 通过 Rust 端的 `unicode-segmentation` 将文本分割为**字素簇**（而非 code point）。这意味着 ZWJ emoji（👨‍👩‍👧‍👦）和组合标记（é）被视为单个单元，匹配视觉感知。

`ctx.splitTextNode` 读取**解析后的文本源**——当前帧实际渲染的文本，在任何 `text_content` 变更已应用之后。这防止了双源漂移。

```js
var parts = ctx.splitTextNode('title', { granularity: 'graphemes' });
```

每个 `part` 暴露：

| 属性 | 说明 |
|------|------|
| `index` | 单元索引 |
| `text` | 单元字符串 |
| `start` / `end` | 源字符串中的字节偏移 |

和一个方法：

| 方法 | 说明 |
|------|------|
| `part.set({ opacity, translateX, translateY, scale, rotation })` | 批量写入此单元的视觉覆盖 |

#### 声明式动画：`parts.animate(opts)`

无需手动 `for` 循环，使用部件数组内置的 `animate` 方法：

```js
ctx.splitTextNode('title', { granularity: 'graphemes' }).animate({
  from: { opacity: 0, translateY: 38, scale: 0.86 },
  to:   { opacity: 1, translateY: 0,  scale: 1 },
  duration: 22,
  delay: 8,
  stagger: 2,
  easing: 'spring-wobbly',
});
```

等同于 `ctx.stagger(parts.length, { targets: parts, ... })`——相同的数学计算，零样板代码。

#### 恢复覆盖：`parts.revert()`

清除节点上的所有覆盖并恢复默认外观：

```js
var parts = ctx.splitTextNode('title', { granularity: 'graphemes' });
// ... 动画 ...
parts.revert();
```

#### `words` 粒度

```js
ctx.splitTextNode('title', { granularity: 'words' }).animate({
  from: { opacity: 0, translateX: 18 },
  to:   { opacity: 1, translateX: 0 },
  duration: 20,
  stagger: 5,
});
```

空白字符保留在源文本中；词单元包含空格，使布局节奏自然。

#### 文本动画 + 内容效果

`text_content` 变更（打字机、乱码）和 `text_unit_overrides`（分割文本）在**两个独立层**上操作：

1. **内容层**（`text_content`）：更改被布局的字符串
2. **单元样式层**（`text_unit_overrides`）：更改已布局单元的视觉属性

它们可以共存。先确定解析后的文本源，然后分割文本读取该源，然后应用覆盖。

```js
ctx.getNode('title').text('Hello');        // 内容层
ctx.splitTextNode('title', { granularity: 'graphemes' }).animate({
  from: { opacity: 0 },
  to:   { opacity: 1 },
  duration: 12,
  stagger: 1,
});                                         // 单元样式层
```

---

### 6.8 ctx.alongPath(svgPath)

底层路径采样器。大多数情况应使用 `ctx.animate({ path: ... })`（见 6.3），它会自动处理缓存和计时。

返回一个带有 `getLength()`、`at(t)` 和 `dispose()` 的小对象。`at(t)` 接受 `t in [0, 1]`，返回 `{ x, y, angle }`——`angle` 是路径切线角度（**度**）。

SVG 字符串在创建时解析一次；采样通过 Rust 的 Skia `ContourMeasure` 计算。

```js
// 手动使用（高级）：自行缓存测量器
if (!ctx.__along) {
  ctx.__along = ctx.alongPath('M100 360 C400 80 880 640 1180 360');
}
var pos = ctx.__along.at(0.5);
// pos = { x: ..., y: ..., angle: ... }
```

**支持的 SVG 路径命令**（大写 = 绝对，小写 = 相对）：

| 命令 | 含义 |
|------|------|
| `M x y` / `m dx dy` | 移动到 |
| `L x y` / `l dx dy` | 直线到 |
| `H x` / `h dx` | 水平线到 |
| `V y` / `v dy` | 垂直线到 |
| `C x1 y1 x2 y2 x y` | 三次贝塞尔 |
| `S x2 y2 x y` | 平滑三次贝塞尔 |
| `Q x1 y1 x y` | 二次贝塞尔 |
| `T x y` | 平滑二次贝塞尔 |
| `A rx ry x-axis-rot large sweep x y` | 椭圆弧 |
| `Z` / `z` | 闭合路径 |

仅采样**第一个轮廓**。多个 `M` 命令（子路径）的后续部分会被忽略。

### 6.9 ctx.utils

数值工具和**确定性**随机数。

```js
ctx.utils.clamp(value, min, max);
ctx.utils.snap(value, step);
ctx.utils.wrap(value, min, max);           // (value - min) 包裹到 [min, max)
ctx.utils.mapRange(value, inMin, inMax, outMin, outMax);

ctx.utils.random(min, max, seed?);         // [min, max)
ctx.utils.randomInt(min, max, seed?);      // 整数 [min, max]
```

> 省略 `seed` 时，`ctx.utils.random` 回退到 `Math.random()`，每次渲染产生**不同输出**。**视频渲染务必传入 seed。**

### 6.10 Node API

`ctx.getNode('id')` 返回可链式调用的代理对象。

```js
// 变换
node.opacity(0.5).translateX(100).translateY(50).translate(100, 50);
node.scale(1.5).scaleX(1.2).scaleY(0.8);
node.rotate(45).skewX(10).skewY(10).skew(10, 10);

// 布局
node.position('absolute').left(100).top(50).right(20).bottom(20);
node.width(200).height(100);

// 间距
node.padding(16).paddingX(24).paddingY(12);
node.margin(8).marginX(16).marginY(8);

// Flex
node.flexDirection('col').justifyContent('center').alignItems('center').gap(12).flexGrow(1);

// 样式
node.bg('blue-500').borderRadius(16).borderWidth(2).borderColor('gray-300');
node.objectFit('cover').textColor('white').textSize(24).fontWeight('bold');
node.textAlign('center').lineHeight(1.5).letterSpacing(1).shadow('lg');
node.strokeWidth(2).strokeColor('gray-300').fillColor('blue-500');

// 内容（仅 text 节点——覆盖当前帧的 JSONL `text` 字段）
node.text('Hello world');
```

### 6.11 常见模式

**交错入场（优先使用 `targets`）：**

```js
ctx.stagger(0, {
  targets: ['card-1', 'card-2', 'card-3'],
  from: { opacity: 0, translateY: 30, scale: 0.9 },
  to:   { opacity: 1, translateY: 0,  scale: 1 },
  gap: 4,
  easing: { spring: { stiffness: 80, damping: 14, mass: 1 } },
});
```

**逐节点手动控制**（每个项目自定义缓动）：

```js
var items = ['card-1', 'card-2', 'card-3'];
var anims = ctx.stagger(items.length, {
  from: { opacity: 0, translateY: 30, scale: 0.9 },
  to:   { opacity: 1, translateY: 0,  scale: 1 },
  gap: 4,
  easing: { spring: { stiffness: 80, damping: 14, mass: 1 } },
});
items.forEach(function(id, i) {
  ctx.getNode(id).opacity(anims[i].opacity).translateY(anims[i].translateY).scale(anims[i].scale);
});
```

**联动运动：**

```js
var hero = ctx.animate({
  from: { opacity: 0, translateY: 40 },
  to:   { opacity: 1, translateY: 0 },
  easing: 'spring-gentle',
});
ctx.getNode('subtitle')
  .opacity(Math.min(0.85, hero.opacity * 0.85))
  .translateY(hero.translateY * 0.6);
```

**循环脉冲（需要手动控制）：**

```js
var icons = ['icon-a', 'icon-b', 'icon-c'];
var frame = ctx.frame;
var cycleLen = 30;
var activeIndex = Math.floor((frame % (icons.length * cycleLen)) / cycleLen);
var cycleStart = frame - (frame % cycleLen);

var entrance = ctx.stagger(0, {
  targets: icons,
  from: { scale: 0.85, translateY: 18 },
  to: { scale: 1, translateY: 0 },
  gap: 4, easing: 'spring-default',
});

var pulse = ctx.animate({
  targets: icons[activeIndex],
  from: { scale: 1 }, to: { scale: 1.08 },
  duration: cycleLen, delay: cycleStart, easing: 'spring-wobbly',
});
```

### 6.12 限制

- 不要使用 `document`、`window`、`requestAnimationFrame` 或 `element.style`。
- 仅通过 `ctx.getNode()` 访问节点。
- 非弹簧缓动必须指定 `duration`。

---

## 7. Canvas API

`canvas` 节点提供 CanvasKit 风格的绘图表面。绘图脚本必须是 canvas 节点的子 `script`，每帧重新执行。

### 入口点

| 对象 | 用途 |
|------|------|
| `ctx.CanvasKit` / `globalThis.CanvasKit` | 辅助函数、构造器、枚举 |
| `ctx.getCanvas()` | 当前 canvas 节点的绘图接口 |
| `ctx.getImage(assetId)` | 宿主提供的资源 id 对应的图像句柄 |

### CanvasKit 辅助函数

```js
var CK = ctx.CanvasKit;

// 颜色
CK.Color(r, g, b, a?)
CK.Color4f(r, g, b, a?)
CK.ColorAsInt(r, g, b, a?)
CK.parseColorString('#ff0000')
CK.multiplyByAlpha(color, 0.5)

// 几何
CK.LTRBRect(l, t, r, b)
CK.XYWHRect(x, y, w, h)
CK.RRectXY(rect, rx, ry)

// 构造器
new CK.Paint()
new CK.Path()
new CK.Font(null, size?, scaleX?, skewX?)
CK.PathEffect.MakeDash(intervals, phase?)

// 枚举 / 常量
CK.BLACK / CK.WHITE
CK.PaintStyle.Fill / CK.PaintStyle.Stroke
CK.StrokeCap.Butt / Round / Square
CK.StrokeJoin.Miter / Round / Bevel
CK.FontEdging.Alias / AntiAlias / SubpixelAntiAlias
CK.BlendMode.SrcOver
CK.ClipOp.Intersect / Difference
CK.PointMode.Points / Lines / Polygon
```

### Canvas 方法

```js
var canvas = ctx.getCanvas();

// 状态和变换
canvas.clear(color?);
canvas.save();
canvas.saveLayer(paint?);
canvas.saveLayer(boundsRect);
canvas.saveLayer(paint, boundsRect);
canvas.restore();
canvas.restoreToCount(saveCount);
canvas.translate(dx, dy);
canvas.scale(sx, sy?);
canvas.rotate(degrees, rx?, ry?);
canvas.skew(sx, sy);
canvas.concat([m00, m01, m02, m10, m11, m12, m20, m21, m22]);
canvas.setAlphaf(alpha);

// 裁剪
canvas.clipRect(rect, CK.ClipOp.Intersect, doAntiAlias?);
canvas.clipPath(path, CK.ClipOp.Intersect, doAntiAlias?);
canvas.clipRRect(rrect, CK.ClipOp.Intersect, doAntiAlias?);

// 形状
canvas.drawPaint(paint);
canvas.drawColor(color, CK.BlendMode.SrcOver);
canvas.drawColorComponents(r, g, b, a?, CK.BlendMode.SrcOver);
canvas.drawColorInt(colorInt, CK.BlendMode.SrcOver);
canvas.drawRect(rect, paint);
canvas.drawRRect(rrect, paint);
canvas.drawDRRect(outerRRect, innerRRect, paint);
canvas.drawCircle(cx, cy, radius, paint);
canvas.drawOval(rect, paint);
canvas.drawArc(ovalRect, startDeg, sweepDeg, useCenter, paint);
canvas.drawLine(x0, y0, x1, y1, paint);
canvas.drawPath(path, paint);
canvas.drawPoints(CK.PointMode.Points, points, paint);
canvas.drawPoints(CK.PointMode.Lines, points, paint);
canvas.drawPoints(CK.PointMode.Polygon, points, paint);

// 图像
canvas.drawImage(image, x, y, paint?);
canvas.drawImageRect(image, srcRect, destRect, paint?, fastSample?);

// 文本
canvas.drawText(text, x, y, paint, font);
```

### Paint

```js
var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Fill);
paint.setColor(CK.parseColorString('#ff0000'));
paint.setColorComponents(1, 0, 0, 1);
paint.setColorInt(CK.ColorAsInt(255, 0, 0, 1));
paint.setAlphaf(0.8);
paint.setStrokeWidth(2);
paint.setStrokeCap(CK.StrokeCap.Round);
paint.setStrokeJoin(CK.StrokeJoin.Round);
paint.setAntiAlias(true);
paint.setStrokeDash([10, 5], 0);
paint.setPathEffect(CK.PathEffect.MakeDash([10, 5], 0));
```

目前仅支持 dash 路径效果。

### Path

```js
var path = new CK.Path();
path.moveTo(x, y);
path.lineTo(x, y);
path.quadTo(x1, y1, x2, y2);
path.cubicTo(x1, y1, x2, y2, x3, y3);
path.addRect(CK.XYWHRect(10, 10, 80, 40));
path.addRRect(CK.RRectXY(CK.XYWHRect(10, 10, 80, 40), 8, 8));
path.addOval(CK.XYWHRect(10, 10, 80, 40));
path.addArc(CK.XYWHRect(10, 10, 80, 40), 0, 180);
path.close();
path.reset();
path.rewind();
```

### Text

```js
var font = new CK.Font(null, 32);
font.setSize(36);
font.setScaleX(1);
font.setSkewX(0);
font.setSubpixel(true);
font.setEdging(CK.FontEdging.SubpixelAntiAlias);

var width = font.measureText('Hello OpenCat');
canvas.drawText('Hello OpenCat', 40, 80, paint, font);
```

当前限制：

- `typeface` 必须为 `null`（系统默认字体）。
- 不支持自定义字体对象、`Typeface`、`FontMgr` 和字体资源。
- 不支持 `TextBlob` 和 `Paragraph`。

### 图像资源

Canvas 脚本必须通过 `ctx.getImage(assetId)` 获取图像。不接受 URL、文件路径和任意原生图像对象。

```js
var img = ctx.getImage('hero-asset');
canvas.drawImage(img, 40, 40);
canvas.drawImageRect(
  img,
  CK.XYWHRect(0, 0, 320, 180),
  CK.XYWHRect(40, 40, 160, 90)
);
```

### 限制

- 这是 CanvasKit 子集，不是完整 CanvasKit。
- `clipRect()`、`clipPath()`、`clipRRect()` —— 仅支持 `CK.ClipOp.Intersect`。
- `drawColor()`、`drawColorInt()`、`drawColorComponents()` —— 仅支持 `CK.BlendMode.SrcOver`。
- `PathEffect` —— 仅支持 `MakeDash()`。
- 文本绘制 —— 仅支持系统默认字体。
- `ctx.getImage()` —— 仅接受资源 id 句柄。

### 推荐模板

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();

function fill(color) {
  var p = new CK.Paint();
  p.setStyle(CK.PaintStyle.Fill);
  p.setColor(CK.parseColorString(color));
  return p;
}

function stroke(color, width) {
  var p = new CK.Paint();
  p.setStyle(CK.PaintStyle.Stroke);
  p.setColor(CK.parseColorString(color));
  p.setStrokeWidth(width || 1);
  return p;
}

var font = new CK.Font(null, 24);
font.setEdging(CK.FontEdging.SubpixelAntiAlias);

canvas.clear(CK.WHITE);
canvas.drawRect(CK.XYWHRect(10, 10, 100, 60), fill('#0f172a'));
canvas.drawCircle(80, 40, 12, fill('#f8fafc'));

var path = new CK.Path();
path.moveTo(10, 10).lineTo(60, 10).lineTo(60, 40).close();
canvas.drawPath(path, stroke('#38bdf8', 2));
canvas.drawText('OpenCat', 16, 96, fill('#0f172a'), font);
```

---

## 附录：常见错误

| 错误写法 | 正确写法 |
|----------|----------|
| `type: "div"` 带 `text` 字段 | 仅 `type: "text"` 接受 `text` |
| 用 `bg-{color}` 给图标着色 | 图标用 `text-{color}` |
| `id` 含 "icon" 但 `type: "div"` | 使用 `type: "icon"` + Lucide 图标名称 |
| 图片 `query` 包含形容词 | 仅使用 1-4 个名词 |
| 默认依赖 `absolute` 布局 | 优先使用 flex 布局；`absolute` 仅用于重叠或固定边缘 |
| 在 `className` 中使用变换类 | 使用节点变换 API，如 `translateX()`、`translateY()`、`scale()`、`rotate()`、`skew()` |
| `parentId` 引用不存在的 id | `parentId` 必须引用已声明的节点 |
| 使用已移除的 `layer` 类型 | 使用 `parentId: null` 的 `div`，在 `tl` 节点下安排子节点 |
| 多个根场景 + 根级 `transition` | 显式声明 `tl` 节点，将场景移到其下 |
| 用 `tl` 包裹单个场景 | 使用普通 `div` 树；`tl` 仅用于两个及以上场景的转场 |
| `tl` 有场景但相邻对之间缺少转场 | 添加缺失的 `transition`，或移除 `tl` 改用普通树 |
| 根级 `caption` 没有父 `div`，但期望跨转场持续显示 | 将主视觉和根 `caption` 放在共享的父 `div` 下 |
| `caption.path` 指向 UTF-16 字幕文件 | 先将 SRT 转为 UTF-8；当前加载器仅读取 UTF-8 |
| 时间线模式帧数不匹配 | 运行时总时长由 `sum(scene.duration) + sum(transition.duration)` 推导 |
| `"effect": "slide-left"` | 使用分离字段：`"effect": "slide", "direction": "from_left"` |
