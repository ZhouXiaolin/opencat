# OpenCat JSONL 格式参考

> **格式规则**
> - **每行一个 JSON 对象。** 不要将单个 JSON 对象拆分为多行。
> - **脚本内容中无注释。** 脚本代码必须保持干净。

OpenCat JSONL 是一种 JSON Lines 格式，用于描述动态图形合成。每行是一个节点声明、脚本附件或元数据记录。运行时解析文件，构建场景树，并使用 Skia + Taffy + QuickJS 渲染帧。

---

## 1. Composition 头部

第一行必须是 `composition` 记录。

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 180}
```

| 字段 | 类型 | 说明 |
|------|------|------|
| `width` | `i32` | 画布宽度（像素） |
| `height` | `i32` | 画布高度（像素） |
| `fps` | `i32` | 每秒帧数 |
| `frames` | `i32` | 总帧数。`frames / fps` = 时长（秒） |

---

## 2. 节点树

### 2.1 父子关系

每个节点（除 `composition` 和 `script`/`transition`）有 `id` 和 `parentId`。树通过这些链接构建。

- 恰好一个根节点的 `parentId` 为 `null`。
- `parentId` 必须引用先前声明的 `id`。
- `script` 和 `transition` 记录没有 `id`；它们附加到 `parentId`。

### 2.2 普通树（单场景）

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 60}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold", "text": "Hello"}
```

### 2.3 Timeline（多场景 + 转场）

```json
{"type":"composition","width":390,"height":844,"fps":30,"frames":162}
{"id":"root","parentId":null,"type":"div","className":"relative w-[390px] h-[844px]"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene1","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-white","duration":60}
{"id":"scene2","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-slate-900","duration":90}
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"fade","duration":12}
```

规则：
- `tl` 必须是树中的显式节点。不支持根级多场景推断。
- `tl` 遵循 `NodeStyle` — `tl` 节点本身的布局和脚本被保留。
- `tl` 必须至少有两个直接子场景，每对相邻场景必须有匹配的 `transition`。
- `tl` 没有 `duration` 字段。总长推导：`sum(scene.duration) + sum(transition.duration)`。
- `transition.parentId` 必须引用拥有它的 `tl` 节点。
- 将 `tl` 和持久叠加层（如 `caption`）放在共享父 `div` 下作为兄弟节点以进行 z 序合成。
- 保持 `composition.frames` 与推导总长对齐。

---

## 3. 节点类型

每个元素是一行 JSON。`className` 使用 Tailwind 风格类（见 §5 样式）。

### 3.1 `div`

容器。等价于 HTML `<div>`，**默认 `display: block`**。className 含 `flex` / `flex-row` / `flex-col` 时切换为 Flex；含 `grid` 时切换为 Grid。

布局硬性规则（Tailwind 对齐）：

- **优先使用 flex**。容器应以 `flex flex-col` / `flex items-center justify-center` 等起手，通过 gap / items / justify 排版子元素。
- **`absolute` 必须显式坐标**。任何 `absolute` 元素必须至少包含 `top` / `left` / `right` / `bottom` / `inset-X` 之一（含 `inset-0`）。Taffy 不实现 CSS 的 absolute static position fallback——inset 全 auto 的 `absolute` 元素会塞到容器内容区左上 `(0, 0)`，多个 absolute 元素会完全重叠。

```json
{"id": "box", "parentId": "root", "type": "div", "className": "flex flex-col items-center gap-4 p-6"}
```

### 3.2 `text`

文本内容节点。等价于 `<span>` / `<p>`。

```json
{"id": "title", "parentId": "box", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Hello"}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `text` | 是 | 文本内容字符串 |

### 3.3 `image`

图像节点。等价于 `<img>`。

```json
{"id": "hero", "parentId": "scene1", "type": "image", "className": "w-[300px] h-[200px] object-cover rounded-lg", "query": "mountain landscape"}
```

指定恰好一个图像源：

| 字段 | 说明 |
|------|------|
| `path` | 本地文件路径 |
| `url` | 远程 URL |
| `query` | Openverse 搜索查询（1-4 个名词） |

### 3.4 `icon`

Lucide 图标节点。使用 kebab-case 图标名。

```json
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] stroke-slate-400", "icon": "search"}
```

### 3.5 `path`

SVG 路径节点。渲染一个或多个 SVG 路径数据字符串。

```json
{"id": "triangle", "parentId": "scene1", "type": "path", "className": "w-[100px] h-[100px] fill-red-500 stroke-blue stroke-2", "d": "M0 0 L100 0 L50 100 Z"}
```

### 3.6 `canvas`

Canvas 绘制表面。需要子 `script` 进行绘制命令。

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

### 3.7 `audio`

音频播放节点。等价于 `<audio>`。

```json
{"id": "bgm", "parentId": "root", "type": "audio", "path": "/tmp/bgm.mp3"}
```

`parentId` 控制播放时机：
- 附加到场景节点下 → 在该场景期间播放
- `parentId: null` → 在整个合成期间播放

### 3.8 `video`

视频播放节点。等价于 `<video>`。

```json
{"id": "clip", "parentId": "scene1", "type": "video", "className": "w-full h-full object-cover", "path": "clip.mp4"}
```

### 3.9 `caption`

SRT 驱动的文本节点。

```json
{"id": "subs", "parentId": "root", "type": "caption", "className": "absolute inset-x-[48px] bottom-[32px] text-center text-white", "path": "subtitles.utf8.srt"}
```

### 3.10 `tl`

Timeline 容器。见 §2.3 完整规范。

### 3.11 `transition`

两个相邻场景之间的转场。见 §4 完整规范。

---

## 4. 转场

转场描述 `tl` 节点内两个相邻场景之间的交接。它们消耗额外帧。

```json
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

| 字段 | 必填 | 说明 |
|------|------|------|
| `parentId` | 是 | 必须引用拥有它的 `tl` 节点 |
| `from` | 是 | 源场景 id |
| `to` | 是 | 目标场景 id |
| `effect` | 是 | 效果名称 |
| `duration` | 是 | 转场帧数 |
| `direction` | 否 | `slide`/`wipe` 的方向 |
| `timing` | 否 | 缓动名称（默认 `"linear"`） |

### 效果类型

| effect | 说明 | direction（可选） |
|--------|------|-------------------|
| `fade` | 交叉淡入淡出 | — |
| `slide` | 滑动 | `from_left`/`from_right`/`from_top`/`from_bottom` |
| `wipe` | 擦除 | 8 个方向 |
| `clock_wipe` | 时钟擦除 | — |
| `iris` | 虹膜开合 | — |
| `light_leak` | 漏光 | — |
| `gl_transition` | GLSL 运行时着色器 | — |

未识别的 `effect` 名回退到 `gl_transition` 模式。

---

## 5. 样式（Tailwind）

`className` 使用 Tailwind 风格类进行布局、颜色、间距、圆角和相关视觉属性。

**限制：**
- 不要使用 CSS 动画类（`transition-*`、`animate-*`、`duration-*`、`ease-*`、`delay-*`）。
- 不要在 `className` 中使用 transform 类（`transform`、`translate-*`、`rotate-*`、`scale-*`、`skew-*`）。

### 5.1 缓动参考

| 预设 | 效果 |
|------|------|
| `'linear'` | 匀速 |
| `'ease'`/`'ease-in'`/`'ease-out'`/`'ease-in-out'` | 标准 CSS 三次曲线 |
| `'back-in'`/`'back-out'`/`'back-in-out'` | 轻微回弹 |
| `'elastic-in'`/`'elastic-out'`/`'elastic-in-out'` | 阻尼振荡 |
| `'bounce-in'`/`'bounce-out'`/`'bounce-in-out'` | 地面弹跳 |
| `'steps(N)'` | N 步离散 |
| `'spring.default'`/`'spring.gentle'`/`'spring.stiff'`/`'spring.slow'`/`'spring.wobbly'` | 弹簧 |

自定义弹簧：
```js
ease: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

三次贝塞尔：
```js
ease: [0.25, 0.1, 0.25, 1.0]
```

---

## 6. 动画系统

> 动画模式、示例和速查表见 [ctx-animation.md](ctx-animation.md)。

动画脚本通过 `script` 记录附加到节点，使用 QuickJS 在每帧运行。

```json
{"type": "script", "parentId": "scene1", "src": "ctx.fromTo('title',{opacity:0},{opacity:1,duration:20,ease:'spring.gentle'});"}
```

执行上下文：

| 字段 | 说明 |
|------|------|
| `ctx.frame` | 全局帧索引 |
| `ctx.totalFrames` | 总帧数 |
| `ctx.currentFrame` | 当前场景内的帧索引 |
| `ctx.sceneFrames` | 当前场景的帧数 |

**使用指南：**
- **循环动画**（呼吸、闪烁、持续旋转）：使用 `ctx.frame`
- **场景内进度**（路径绘制、淡入淡出、场景内动画）：使用 `ctx.currentFrame / ctx.sceneFrames`

### 6.1 设计哲学

OpenCat 的动画系统是**函数式纯的**：每个动画值通过精确数学公式计算为 `value = f(current_frame)`。

- **插值**：`from + (to - from) * easing(progress)`
- **弹簧**：从物理参数精确求解
- **颜色**：HSLA 空间最短路径色相旋转
- **路径**：Skia `ContourMeasure` 亚像素精度弧长采样

### 6.2 Tween API

```js
ctx.set(targets, vars);                // 立即设置
ctx.to(targets, vars);                 // 当前 → 目标
ctx.from(targets, vars);               // 初始 → 当前
ctx.fromTo(targets, fromVars, toVars); // 完整控制
```

**属性别名：**

| 属性 | 说明 |
|------|------|
| `opacity` | 视觉透明度 |
| `x`, `y` | translateX/Y |
| `scale`, `scaleX`, `scaleY` | 缩放 |
| `rotate`, `rotation` | 旋转 |
| `skewX`, `skewY` | 倾斜 |
| `path` | 路径动画 |
| `morphSVG`, `d` | SVG 路径变形 |
| `left`, `top`, `width`, `height` | 布局 |
| `backgroundColor`, `bg` | 背景色 |
| `color`, `textColor` | 文字色 |
| `borderColor`, `borderRadius`, `borderWidth` | 边框 |
| `fillColor`, `strokeColor`, `strokeWidth` | SVG 绘制 |
| `text` | 文字内容层（打字机） |

**时间字段：**

| 字段 | 默认 | 说明 |
|------|------|------|
| `duration` | 非弹簧必填 | 帧数 |
| `delay` | `0` | 起始偏移 |
| `ease`/`easing` | `'linear'` | 缓动 |
| `repeat` | `0` | 循环。`-1` = 无限 |
| `yoyo` | `false` | 交替反向 |
| `stagger` | `0` | 数组目标延迟 |

### 6.3 Timeline

```js
ctx.timeline({ defaults: { duration: 18, ease: 'spring.gentle' } })
  .from('title', { opacity: 0, y: 30 })
  .from('subtitle', { opacity: 0, y: 18 }, '-=8')
  .fromTo('cta', { scale: 0.8 }, { scale: 1, duration: 24 }, '+=6');
```

**Position 参数：**

| Position | 含义 |
|----------|------|
| 省略 | 从当前游标开始 |
| 数字 | 绝对帧 |
| `'+=N'` | 游标后 N 帧 |
| `'-=N'` | 游标前 N 帧 |
| `'<'`/`'>'` | 前一个子项的开始/结束 |

### 6.4 路径动画

```js
ctx.to('rocket', {
  path: 'M100 360 C400 80 880 640 1180 360',
  orient: -90,
  duration: 120,
  ease: 'ease-in-out',
});
```

### 6.5 变形路径

```js
ctx.fromTo('blob',
  { d: 'M55 0 L110 95 L0 95 Z' },
  { d: 'M55 95 L110 0 L0 0 Z', duration: 45, ease: 'ease-in-out' }
);
```

### 6.6 文字动画

**打字机：**
```js
ctx.to('title', { text: 'Hello OpenCat', duration: 30, ease: 'linear' });
```

**splitText：**
```js
ctx.from(ctx.splitText('title', { type: 'chars' }), {
  opacity: 0, y: 38, scale: 0.86,
  duration: 22, stagger: 2, ease: 'spring.wobbly',
});
```

### 6.7 Node API

```js
ctx.getNode('id')
  .opacity(0.5).translateX(100).translateY(50)
  .scale(1.5).rotate(45)
  .bg('blue-500').borderRadius(16)
  .textColor('white').textSize(24).fontWeight('bold');
```

---

## 7. Canvas API

`canvas` 节点提供 CanvasKit 风格的绘制表面。

### 入口点

| 对象 | 用途 |
|------|------|
| `ctx.CanvasKit` | 辅助函数、构造函数、枚举 |
| `ctx.getCanvas()` | 绘制接口 |
| `ctx.getImage(assetId)` | 图像句柄 |

### 常用方法

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();

// 状态
canvas.clear(color?);
canvas.save(); canvas.restore();
canvas.translate(dx, dy);
canvas.scale(sx, sy?);
canvas.rotate(degrees, rx?, ry?);

// 形状
canvas.drawRect(rect, paint);
canvas.drawCircle(cx, cy, radius, paint);
canvas.drawPath(path, paint);
canvas.drawText(text, x, y, paint, font);
canvas.drawImage(image, x, y, paint?);
```

### Paint

```js
var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Fill);
paint.setColor(CK.parseColorString('#ff0000'));
paint.setAlphaf(0.8);
paint.setStrokeWidth(2);
paint.setStrokeCap(CK.StrokeCap.Round);
```

### Path

```js
var path = new CK.Path();
path.moveTo(x, y);
path.lineTo(x, y);
path.cubicTo(x1, y1, x2, y2, x3, y3);
path.close();
```

### 限制

- `clipRect()`/`clipPath()`/`clipRRect()` — 仅 `CK.ClipOp.Intersect`
- `PathEffect` — 仅 `MakeDash()`
- 文字 — 仅系统默认字体
- `ctx.getImage()` — 仅资源 id 句柄

---

## 附录：常见错误

| 错误 | 正确 |
|------|------|
| `type: "div"` 带 `text` 字段 | 仅 `type: "text"` 接受 `text` |
| 用 `bg-{color}` 给图标/路径着色 | 用 `fill-{color}` / `stroke-{color}` |
| className 中放 transform 类 | 用 Node API |
| `parentId` 指向无效 id | 必须引用已存在节点 |
| `tl` 缺转场或场景少于 2 | 添加缺失的 `transition` |
| 帧数不匹配 | `frames = sum(scene.duration) + sum(transition.duration)` |
