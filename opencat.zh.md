# OpenCat JSONL

> **格式规则**
> - **每行一个 JSON 对象。** 不要将单个 JSON 对象拆分为多行。
> - **脚本内容中不要包含注释。** 脚本代码必须保持干净。

OpenCat JSONL 是一种用于描述动态图形合成（motion graphics composition）的 JSON Lines 格式。每一行是一个节点声明、脚本附件或元数据记录。运行时解析文件，构建场景树，并使用 Skia + Taffy + QuickJS 渲染帧。

---

## 1. 合成头部（Composition Header）

第一行必须是 `composition` 记录。

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 180}
```

| 字段 | 类型 | 描述 |
|-------|------|-------------|
| `width` | `i32` | 画布宽度（像素） |
| `height` | `i32` | 画布高度（像素） |
| `fps` | `i32` | 每秒帧数 |
| `frames` | `i32` | 总帧数。`frames / fps` = 持续时间（秒） |

---

## 2. 节点树（Node Tree）

### 2.1 父子关系

每个节点（`composition` 和 `script`/`transition` 除外）都有一个 `id` 和一个 `parentId`。树通过这些链接构建。

- 必须恰好有一个根节点具有 `parentId: null`。
- `parentId` 必须引用先前声明的 `id`。
- `script` 和 `transition` 记录没有 `id`；它们附加到其 `parentId`。

### 2.2 纯树（单场景）

适用于单个场景、静态叠加或任何没有场景间过渡的合成。

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 60}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold", "text": "你好"}
```

### 2.3 时间线（多场景带过渡）

适用于两个或更多场景，场景之间有过渡。

```json
{"type":"composition","width":390,"height":844,"fps":30,"frames":162}
{"id":"root","parentId":null,"type":"div","className":"relative w-[390px] h-[844px]"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene1","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-white","duration":60}
{"id":"scene2","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-slate-900","duration":90}
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"fade","duration":12}
```

规则：

- `tl` 必须是树中的显式节点。不支持根级别的多场景推断。
- `tl` 遵循 `NodeStyle` — `tl` 节点本身的布局和脚本会被保留。
- `tl` 必须至少有两个直接子场景，并且每对相邻场景必须有一个匹配的 `transition`。
- `tl` 没有 `duration` 字段。其总时长由 `sum(scene.duration) + sum(transition.duration)` 推导得出。
- `transition.parentId` 是必需的，并且必须引用所属的 `tl` 节点。
- 将 `tl` 和持久化叠加层（如 `caption`）作为兄弟节点放在共享父 `div` 下，以实现 Z 顺序合成。
- 保持 `composition.frames` 与推导出的总帧数一致。

---

## 3. 节点类型（Node Types）

每个元素都是一行 JSON。`className` 使用 Tailwind 风格的类（见 §5 样式）。

### 3.1 `div`

带弹性布局的容器。等价于 `<div>`。

```json
{"id": "box", "parentId": "root", "type": "div", "className": "flex flex-col items-center gap-4 p-6"}
```

除 `id`、`parentId`、`className`、`duration` 外，没有特殊字段。

### 3.2 `text`

文本内容节点。等价于 `<span>` / `<p>`。

```json
{"id": "title", "parentId": "box", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "你好"}
```

| 字段 | 必需 | 描述 |
|-------|----------|-------------|
| `text` | 是 | 文本内容字符串 |

### 3.3 `image`

图片节点。等价于 `<img>`。

```json
{"id": "hero", "parentId": "scene1", "type": "image", "className": "w-[300px] h-[200px] object-cover rounded-lg", "query": "mountain landscape"}
```

指定恰好一个图片来源：

| 字段 | 描述 |
|-------|-------------|
| `path` | 本地文件路径 |
| `url` | 远程 URL |
| `query` | Openverse 搜索查询（1-4 个名词） |

使用 `query` 时，可选字段：

| 字段 | 默认值 | 描述 |
|-------|---------|-------------|
| `queryCount` | `1` | 要获取的图片数量 |
| `aspectRatio` | — | 宽高比过滤器（例如 `"square"`） |

### 3.4 `icon`

Lucide 图标节点。使用 kebab-case 图标名称。

```json
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] stroke-slate-400", "icon": "search"}
```

| 字段 | 必需 | 描述 |
|-------|----------|-------------|
| `icon` | 是 | Lucide 图标名称，kebab-case 格式 |

使用标准的 SVG Tailwind 工具类：
- `stroke-{color}` / `stroke-[#hex]` — 图标描边颜色（默认 Black）
- `stroke-0` / `stroke-1` / `stroke-2` — 图标描边宽度（默认 2）
- `stroke-[n]` — 任意描边宽度
- `fill-{color}` / `fill-[#hex]` — 图标填充（默认 none）

### 3.5 `path`

SVG 路径节点。使用专门的填充/描边样式渲染一个或多个 SVG 路径数据字符串。

```json
{"id": "triangle", "parentId": "scene1", "type": "path", "className": "w-[100px] h-[100px] fill-red-500 stroke-blue stroke-2", "d": "M0 0 L100 0 L50 100 Z"}
```

| 字段 | 必需 | 描述 |
|-------|----------|-------------|
| `d` | 是 | SVG 路径数据字符串 |

使用与 `icon` 相同的 SVG Tailwind 工具类进行样式设置：
- `fill-{color}` / `fill-[#hex]` — 填充颜色（默认 none）
- `stroke-{color}` / `stroke-[#hex]` — 描边颜色（默认 none）
- `stroke-0` / `stroke-1` / `stroke-2` / `stroke-[n]` — 描边宽度

与 `icon` 不同，`path` 没有默认的固有大小 — 通过 `className` 设置 `w`/`h` 或使用布局。

### 3.6 `canvas`

画布绘制面。需要一个子 `script` 来执行绘制命令。

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

完整的绘制参考见 §6 Canvas API。

### 3.7 `audio`

音频播放节点。等价于 `<audio>`。

```json
{"id": "bgm", "parentId": "root", "type": "audio", "path": "/tmp/bgm.mp3"}
{"id": "sfx", "parentId": "root", "type": "audio", "url": "https://example.com/sfx.mp3"}
```

指定恰好一个来源：`path`（本地）或 `url`（远程）。

`parentId` 控制音频播放时机：
- 附加到场景节点下 → 在该场景期间播放。
- `parentId: null` → 在整个合成期间播放（时间线级别）。

### 3.8 `video`

视频播放节点。等价于 `<video>`。

```json
{"id": "clip", "parentId": "scene1", "type": "video", "className": "w-full h-full object-cover", "path": "clip.mp4"}
```

| 字段 | 必需 | 描述 |
|-------|----------|-------------|
| `path` | 是 | 本地视频文件路径 |

### 3.9 `caption`

SRT 驱动的文本节点。显示的内容通过最近的继承时间上下文从字幕条目中选择。

```json
{"id": "subs", "parentId": "root", "type": "caption", "className": "absolute inset-x-[48px] bottom-[32px] text-center text-white", "path": "subtitles.utf8.srt"}
```

| 字段 | 必需 | 描述 |
|-------|----------|-------------|
| `path` | 是 | 本地 SRT 文件路径 |
| `duration` | 否 | 通常省略；可见性由 SRT 时间戳驱动 |

### 3.10 `tl`

时间线容器。完整规范见 §2.3。

| 字段 | 描述 |
|-------|-------------|
| `id` | 必需 |
| `parentId` | 父节点 |
| `className` | Tailwind 样式 |

没有 `duration` 字段 — 总时长由子场景和过渡推导得出。

### 3.11 `transition`

`tl` 内两个相邻场景之间的过渡。完整规范见 §4。

---

## 4. 过渡（Transitions）

过渡描述了 `tl` 节点内两个相邻场景之间的切换。它们会消耗额外的帧数。

```json
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

| 字段 | 必需 | 描述 |
|-------|----------|-------------|
| `parentId` | 是 | 必须引用所属的 `tl` 节点 |
| `from` | 是 | 源场景 id（必须是 `tl` 的直接子节点） |
| `to` | 是 | 目标场景 id（必须与 `from` 相邻） |
| `effect` | 是 | 效果名称（见下文） |
| `duration` | 是 | 过渡持续帧数 |
| `direction` | 否 | `slide` / `wipe` 效果的方向 |
| `timing` | 否 | 缓动名称（默认 `"linear"`）。见 §5.1。 |
| `damping` | 否 | 自定义弹簧阻尼 |
| `stiffness` | 否 | 自定义弹簧刚度 |
| `mass` | 否 | 自定义弹簧质量 |

### 效果类型

| effect | 描述 | direction（可选） |
|--------|-------------|----------------------|
| `fade` | 交叉淡入淡出 | — |
| `slide` | 滑动过渡 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` |
| `wipe` | 擦拭过渡 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` / `from_top_left` / `from_top_right` / `from_bottom_left` / `from_bottom_right` |
| `clock_wipe` | 时钟擦拭 | — |
| `iris` | 光圈开合 | — |
| `light_leak` | 漏光 | — |

`light_leak` 额外字段：`seed`（`f32`）、`hueShift`（`f32`）、`maskScale`（`f32`，范围 `0.03125`–`1.0`）。

### 示例

```json
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 20, "timing": "ease-out"}
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "slide", "direction": "from_right", "duration": 15, "timing": "bezier:0.4,0,0.2,1"}
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "wipe", "direction": "from_right", "duration": 12, "damping": 10, "stiffness": 100, "mass": 1}
```

---

## 5. 样式（Styling / Tailwind）

`className` 使用 Tailwind 风格的类来处理布局、颜色、间距、圆角和相关视觉属性。

**限制：**

- 不要使用 CSS 动画类（`transition-*`、`animate-*`、`duration-*`、`ease-*`、`delay-*`）。
- 不要在 `className` 中使用 transform 类（`transform`、`translate-*`、`rotate-*`、`scale-*`、`skew-*`）。请改用脚本 Node API。

| 避免使用 | 改用 |
|------|-------------|
| `transition-*` `animate-*` `duration-*` `ease-*` `delay-*` | `ctx.to()` / `ctx.from()` / `ctx.fromTo()` / `ctx.timeline()` |
| `transform` `translate-*` `rotate-*` `scale-*` `skew-*` | `ctx.getNode(...).translateX()` / `translateY()` / `scale()` / `rotate()` / `skew()` |

> Tailwind 处理静态样式。脚本处理动画。

### 5.1 缓动参考（Easing Reference）

缓动名称在 `ctx.to()` / `ctx.from()` / `ctx.fromTo()` / `ctx.timeline()` 和 `transition.timing` 中共享。

| 预设 | 效果 |
|--------|--------|
| `'linear'` | 匀速 |
| `'ease'` / `'ease-in'` / `'ease-out'` / `'ease-in-out'` | 标准的 CSS 类三次贝塞尔曲线 |
| `'back-in'` / `'back-out'` / `'back-in-out'` | 轻微过冲（UI 回弹） |
| `'elastic-in'` / `'elastic-out'` / `'elastic-in-out'` | 阻尼振荡 |
| `'bounce-in'` / `'bounce-out'` / `'bounce-in-out'` | 落地弹跳风格 |
| `'steps(N)'` | 量化为 N 个离散步骤 |
| `'spring.default'` / `'spring-default'` | 通用弹簧 |
| `'spring.gentle'` / `'spring-gentle'` | 柔和弹簧 |
| `'spring.stiff'` / `'spring-stiff'` | 较硬弹簧 |
| `'spring.slow'` / `'spring-slow'` | 较慢弹簧 |
| `'spring.wobbly'` / `'spring-wobbly'` | 摇晃弹簧 |

自定义弹簧（JS）：

```js
ease: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

三次贝塞尔曲线（JS）：

```js
ease: [0.25, 0.1, 0.25, 1.0]
```

Transition 的 `timing` 字段也接受字符串形式：`"bezier:0.4,0,0.2,1"`。

---

## 6. 动画系统

动画脚本通过 `script` 记录附加到节点，使用 QuickJS 在每一帧上执行。

```json
{"type": "script", "parentId": "scene1", "src": "ctx.fromTo('title',{opacity:0},{opacity:1,duration:20,ease:'spring.gentle'});"}
{"type": "script", "parentId": "scene1", "path": "scene1.js"}
```

| 字段 | 描述 |
|-------|-------------|
| `parentId` | 可选。附加到节点作用域，或省略以使用全局作用域。 |
| `src` | 内联 JavaScript 代码 |
| `path` | 外部 `.js` 文件路径（相对于 JSONL 文件解析） |

`src` 和 `path` 互斥。必须恰好指定一个。

执行上下文：

| 字段 | 描述 |
|------|-------------|
| `ctx.frame` | 全局帧索引 |
| `ctx.totalFrames` | 总帧数 |
| `ctx.currentFrame` | 当前场景内的帧索引（`0 → sceneFrames - 1`） |
| `ctx.sceneFrames` | 当前场景的帧数 |

### 6.1 设计理念

OpenCat 的动画系统是**函数纯粹**的：每个动画值都通过精确的数学公式计算为 `value = f(current_frame)`。没有内部 tick 循环，没有累积状态，也没有非确定性漂移。

- **插值**：线性 `from + (to - from) * easing(progress)`
- **弹簧**：从物理参数（`stiffness`、`damping`、`mass`）求解，具有精确的稳定时间检测
- **颜色**：HSLA 空间，使用最短路径色相旋转（处理 360° 环绕）
- **路径**：Skia `ContourMeasure`，用于亚像素精度的弧长采样

脚本在每一帧重新执行。类似 GSAP 的 API 声明补间和时间线，但运行时仍然将它们作为当前帧的纯函数进行采样。

---

### 6.2 语法规则：Tween API

```js
ctx.set(targets, vars);                // 立即设置，不产生动画
ctx.to(targets, vars);                 // 从当前值动画到目标值
ctx.from(targets, vars);               // 从初始值动画到当前值
ctx.fromTo(targets, fromVars, toVars); // 完全控制起始值和结束值
```

`targets` 接受节点 id、节点 id 数组或 `ctx.splitText(...)` 部分。

```js
ctx.fromTo('hero',
  { opacity: 0, y: 40, scale: 0.95 },
  { opacity: 1, y: 0, scale: 1, duration: 30, ease: 'spring.gentle' }
);
```

**属性别名：**

| 属性 | 应用于 |
|----------|------------|
| `opacity` | 视觉透明度 |
| `x`, `y` | `translateX`, `translateY` |
| `scale`, `scaleX`, `scaleY` | 变换缩放 |
| `rotate`, `rotation` | 旋转角度（度） |
| `skewX`, `skewY` | 倾斜角度（度） |
| `path` | 运动路径动画通道；将 SVG 路径数据采样为 `x`、`y` 和 `rotation` |
| `orient` | `path` 动画的旋转偏移（度） |
| `morphSVG`, `d` | SVG 路径变形通道；重写 `path` 节点的路径数据 |
| `left`, `top`, `right`, `bottom`, `width`, `height` | 布局尺寸 |
| `backgroundColor`, `bg` | 背景颜色 |
| `color`, `textColor` | 文本颜色 |
| `borderColor`, `borderRadius`, `borderWidth` | 边框样式 |
| `fillColor`, `strokeColor`, `strokeWidth` | SVG/图标/路径绘制 |
| `text` | 文本内容层，以字素安全打字机语义显示 |

**时间参数：**

| 字段 | 默认值 | 描述 |
|-------|---------|-------------|
| `duration` | 非弹簧缓动必需 | 持续时间（帧） |
| `delay` | `0` | 起始偏移（帧） |
| `ease` / `easing` | `'linear'` | 缓动名称、贝塞尔数组或弹簧对象 |
| `repeat` | `0` | 额外循环次数。`-1` = 无限 |
| `yoyo` | `false` | 反向交替循环 |
| `repeatDelay` | `0` | 重复循环之间的保持时间 |
| `stagger` | `0` | 数组或分割文本部分中每个目标的延迟偏移 |

**返回值：** 补间对象暴露 `progress`、`settled`、`settleFrame`、`values` 以及每个采样的属性：

```js
var hero = ctx.fromTo('title', { opacity: 0, y: 40 }, { opacity: 1, y: 0, duration: 20 });
ctx.getNode('subtitle').opacity(hero.opacity * 0.8).translateY(hero.y * 0.5);
```

---

### 6.3 语法规则：ctx.timeline

`ctx.timeline()` 提供 GSAP 风格的编排：

```js
ctx.timeline({ defaults: { duration: 18, ease: 'spring.gentle' } })
  .from('title', { opacity: 0, y: 30 })
  .from('subtitle', { opacity: 0, y: 18 }, '-=8')
  .fromTo('cta', { scale: 0.8 }, { scale: 1, duration: 24 }, '+=6');
```

**位置参数：**

| 位置 | 含义 |
|----------|---------|
| 省略 | 从当前时间线光标开始 |
| 数字 | 时间线中的绝对帧数 |
| `'+=N'` | 光标之后 N 帧 |
| `'-=N'` | 光标之前 N 帧 |
| `'<'` / `'>'` | 上一个 child 的开始 / 结束 |
| `'<N'` / `'>-N'` | 相对上一个 child 开始 / 结束的偏移 |
| label | 由 `addLabel(name, position)` 注册的标签 |

时间线语义：

- 光标表示当前时间线末尾。每插入一个 child，都会更新为 `max(cursor, child_end)`。
- 省略位置参数的 tween 从当前光标开始。
- `'+=N'` 和 `'-=N'` 相对当前时间线末尾，和 GSAP 的 position parameter 模型一致。
- 未来的 `from` / `fromTo` tween 在开始前保持 `from` 值，除非同一目标属性已经被更早的活跃 tween 接管。
- 如果声明出的时间线总长超过 `ctx.sceneFrames`，OpenCat 会把该 timeline 的采样缩放进当前场景，降低 JSONL 帧数生成偏差导致尾部动画被截断的风险。
- 重叠 tween 按声明顺序应用；后声明的 tween 可能覆盖同一目标的同一属性。

---

### 6.4 缓动系统

缓动名称由 Tween API 的所有方法和 `transition.timing` 共享。详见 §5.1 的缓动参考表。

自定义弹簧：

```js
ease: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

三次贝塞尔曲线：

```js
ease: [0.25, 0.1, 0.25, 1.0]
```

Transition 的 `timing` 字段也接受字符串形式：`"bezier:0.4,0,0.2,1"`。

---

### 6.5 插件：颜色插值

颜色属性在 HSLA 空间中使用最短路径色相旋转进行插值：

```js
ctx.fromTo('card',
  { backgroundColor: '#ef4444' },
  { backgroundColor: 'hsl(220, 90%, 55%)', duration: 60, repeat: -1, yoyo: true }
);
```

支持的颜色字面量：`#rgb` / `#rrggbb` / `#rrggbbaa`、`rgb(...)` / `rgba(...)`、`hsl(...)` / `hsla(...)`。

> Tailwind 颜色令牌不可插值；请在补间中使用显式的颜色字面量。

---

### 6.6 插件：关键帧

简写形式（均匀分布）：

```js
ctx.to('card', {
  keyframes: { scale: [1, 1.4, 0.8, 1] },
  duration: 60,
});
```

完整形式（每帧指定缓动）：

```js
ctx.to('logo', {
  keyframes: {
    rotate: [
      { at: 0, value: 0 },
      { at: 0.5, value: 360, easing: 'back-out' },
      { at: 1, value: 0 }
    ],
  },
  duration: 60,
});
```

仅支持数值关键帧。对于颜色关键帧，请链接单独的颜色补间或使用 `fromTo`。

---

### 6.7 插件：路径动画

运动路径动画通过 `path` 选项内建于 `ctx.to()` / `ctx.from()` / `ctx.fromTo()` 中。运行时会解析 SVG 路径，缓存测量器，逐帧采样位置/旋转。

```js
ctx.to('rocket', {
  path: 'M100 360 C400 80 880 640 1180 360',
  orient: -90,
  duration: 120,
  ease: 'ease-in-out',
  repeat: -1,
  yoyo: true,
});
```

语义：

- `path` 接受 SVG path data 字符串。
- 进度 `0 → 1` 映射为从路径起点到终点的弧长。
- 目标节点接收 `x`、`y` 和 `rotation`。
- `rotation` 跟随路径切线角度（度）；`orient` 添加固定角度偏移。
- 多个 `M` 子路径会首尾连接成一条统一的采样路径。

---

### 6.8 插件：SVG 路径变形

SVG 路径变形通过插值 SVG path data 来改变 `type: "path"` 节点的几何形状。不同于路径动画（沿路径移动），此插件重写节点本身的形状数据。

```js
ctx.fromTo('blob',
  { d: 'M55 0 L110 95 L0 95 Z' },
  { d: 'M55 95 L110 0 L0 0 Z', duration: 45, ease: 'ease-in-out' }
);
```

- `morphSVG` 是标准属性名，`d` 是其别名。
- 目标必须为 `type: "path"` 节点。
- `from` 和 `to` 必须是 Skia 接受的合法 SVG path data。
- 中间帧通过弧长重采样和点对应关系生成。
- 开放路径保持开放；闭合路径保持闭合。
- 多个子路径按几何形状匹配。
- 最佳效果用于连贯的剪影、图标、blob、描边等矢量形状。

---

### 6.9 插件：文本内容动画

文本内容通过常规的补间 API 进行动画，按字素簇（grapheme cluster）逐字显示：

```js
ctx.to('title', {
  text: 'Hello OpenCat',
  duration: 30,
  delay: 6,
  ease: 'linear',
});
```

ZWJ 表情符号和组合标记不会在簇中间分割。

---

### 6.10 插件：文本单元动画（splitText）

`ctx.splitText(id, { type })` 读取帧的已解析文本源，返回可动画的视觉单元：

```js
var chars = ctx.splitText('title', { type: 'chars' });
ctx.from(chars, {
  opacity: 0,
  y: 38,
  scale: 0.86,
  duration: 22,
  stagger: 2,
  ease: 'spring.wobbly',
});
```

支持的类型：

| 类型 | 含义 |
|------|---------|
| `'chars'` | 字素簇 |
| `'words'` | 基于 Unicode 词边界的单元；CJK 回退到 `chars` |
| `'lines'` | 保留用于布局推导的行范围 |

每个部分暴露 `index`、`text`、`start`、`end` 和 `part.set({ opacity, x, y, scale, rotate })`。

文本动画有两个独立的层：

1. **内容层**：`ctx.to('title', { text: ... })` 更改要布局的字符串。
2. **单元样式层**：`ctx.splitText(...); ctx.from(parts, ...)` 更改已布局单元的视觉属性。

它们可以在同一帧中共存：

```js
ctx.set('title', { text: '你好' });
ctx.from(ctx.splitText('title', { type: 'chars' }), {
  opacity: 0,
  y: 12,
  duration: 12,
  stagger: 1,
});
```

---

### 6.11 ctx.utils

数值辅助工具和**确定性**随机数：

```js
ctx.utils.clamp(value, min, max);
ctx.utils.snap(value, step);
ctx.utils.wrap(value, min, max);
ctx.utils.mapRange(value, inMin, inMax, outMin, outMax);

ctx.utils.random(min, max, seed?);
ctx.utils.randomInt(min, max, seed?);
```

> 省略 `seed` 时回退到 `Math.random()`。**视频渲染必须始终传递 seed。**

---

### 6.12 Node API

`ctx.getNode('id')` 返回一个可链式调用的代理对象：

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

// 弹性布局
node.flexDirection('col').justifyContent('center').alignItems('center').gap(12).flexGrow(1);

// 样式
node.bg('blue-500').borderRadius(16).borderWidth(2).borderColor('gray-300');
node.objectFit('cover').textColor('white').textSize(24).fontWeight('bold');
node.textAlign('center').lineHeight(1.5).letterSpacing(1).shadow('lg');
node.strokeWidth(2).strokeColor('gray-300').fillColor('blue-500');
node.morphSVG('M0 0 L100 0 L50 100 Z');

// 内容（仅文本节点 — 覆盖 JSONL `text` 字段）
node.text('Hello world');
```

---

### 6.13 常用模式

**交错入场：**

```js
ctx.fromTo(
  ['card-1', 'card-2', 'card-3'],
  { opacity: 0, y: 30, scale: 0.9 },
  {
    opacity: 1, y: 0, scale: 1,
    stagger: 4,
    ease: { spring: { stiffness: 80, damping: 14, mass: 1 } },
  }
);
```

**逐节点手动控制：**

```js
var items = ['card-1', 'card-2', 'card-3'];
var anims = ctx.fromTo(items,
  { opacity: 0, y: 30, scale: 0.9 },
  { opacity: 1, y: 0, scale: 1, stagger: 4, ease: 'spring.gentle' }
);
items.forEach(function(id, i) {
  ctx.getNode(id).opacity(anims[i].opacity).translateY(anims[i].y).scale(anims[i].scale);
});
```

**联动动画：**

```js
var hero = ctx.fromTo('title',
  { opacity: 0, y: 40 },
  { opacity: 1, y: 0, duration: 20, ease: 'spring.gentle' }
);
ctx.getNode('subtitle')
  .opacity(Math.min(0.85, hero.opacity * 0.85))
  .translateY(hero.y * 0.6);
```

**循环脉冲：**

```js
var icons = ['icon-a', 'icon-b', 'icon-c'];
var frame = ctx.frame;
var cycleLen = 30;
var activeIndex = Math.floor((frame % (icons.length * cycleLen)) / cycleLen);
var cycleStart = frame - (frame % cycleLen);

ctx.fromTo(icons,
  { scale: 0.85, y: 18 },
  { scale: 1, y: 0, stagger: 4, ease: 'spring.default' }
);

ctx.fromTo(icons[activeIndex],
  { scale: 1 },
  { scale: 1.08, duration: cycleLen, delay: cycleStart, ease: 'spring.wobbly' }
);
```

---

### 6.14 限制

- 不要使用 `document`、`window`、`requestAnimationFrame` 或 `element.style`。
- 只能通过 `ctx.getNode()` 访问节点。
- 非弹簧缓动必须提供 `duration`。
- 不要在 `className` 中使用 CSS 动画类（`transition-*`、`animate-*`、`duration-*`、`ease-*`、`delay-*`）或 transform 类（`transform`、`translate-*`、`rotate-*`、`scale-*`、`skew-*`）。

---

## 7. Canvas API

`canvas` 节点提供 CanvasKit 风格的绘制面。绘制脚本必须是 canvas 节点的子 `script`，并在每一帧重新执行。

### 入口点

| 对象 | 用途 |
|--------|---------|
| `ctx.CanvasKit` / `globalThis.CanvasKit` | 辅助工具、构造函数、枚举 |
| `ctx.getCanvas()` | 当前 canvas 节点的绘制接口 |
| `ctx.getImage(assetId)` | 主机提供的资源 id 的图片句柄 |

### CanvasKit 辅助工具

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

// 构造函数
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

// 图片
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

目前仅支持虚线路径效果。

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

### 文本（Text）

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

### 图片资源（Image Resources）

Canvas 脚本必须通过 `ctx.getImage(assetId)` 获取图片。不接受 URL、文件路径和任意原生图片对象。

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

- 这是一个 CanvasKit 子集，不是完整的 CanvasKit。
- `clipRect()`、`clipPath()`、`clipRRect()` — 仅支持 `CK.ClipOp.Intersect`。
- `drawColor()`、`drawColorInt()`、`drawColorComponents()` — 仅支持 `CK.BlendMode.SrcOver`。
- `PathEffect` — 仅支持 `MakeDash()`。
- 文本绘制 — 仅支持系统默认字体。
- `ctx.getImage()` — 仅支持资源 id 句柄。

### 推荐模板（Recommended Template）

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

## 附录：常见错误（Common Errors）

| 错误 | 正确做法 |
|------|---------|
| `type: "div"` 带有 `text` 字段 | 只有 `type: "text"` 接受 `text` |
| 使用 `bg-{color}` 为图标/路径着色 | 使用 `fill-{color}` 进行 SVG 填充，`stroke-{color}` 进行 SVG 描边 |
| `id` 包含 "icon" 但 `type: "div"` | 使用 `type: "icon"` 并指定 Lucide 图标名称 |
| 图片 `query` 包含形容词 | 仅使用 1-4 个名词 |
| 默认依赖 `absolute` 布局 | 优先使用 flex 布局；仅在需要重叠或固定边缘时使用 `absolute` |
| 在 `className` 中使用 transform 相关的 Tailwind 类 | 使用节点变换 API，如 `translateX()`、`translateY()`、`scale()`、`rotate()` 和 `skew()` |
| `parentId` 指向无效的 id | `parentId` 必须引用一个已存在的节点 |
| 期望存在 `layer` 记录类型 | `layer` 类型已被移除；使用 `parentId: null` 的 `div`，并将子节点安排在 `tl` 节点下 |
| 多个根场景加上根级 `transition` | 显式声明一个 `tl` 节点，并将场景移到其下 |
| 为单个场景使用 `tl` | 使用简单的 `div` 树；仅在需要两个或更多场景带过渡时使用 `tl` |
| `tl` 有场景，但相邻对之间没有过渡 | 添加缺失的 `transition`，或移除 `tl` 并使用纯树 |
| 根级 `caption` 没有父 `div`，但期望它跨场景持续存在 | 将主要内容与根级 `caption` 放在共享的父 `div` 下 |
| `caption.path` 指向 UTF-16 字幕文件 | 先将 SRT 转换为 UTF-8；当前加载器只读取 UTF-8 文本 |
| 时间线模式中帧数不匹配 | 运行时总帧数由 `sum(scene.duration) + sum(transition.duration)` 推导 |
| `"effect": "slide-left"` | 使用独立字段：`"effect": "slide", "direction": "from_left"` |
