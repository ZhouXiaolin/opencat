# OpenCat JSONL

> **⚠️ 重要格式要求**
> - **每行一个 JSON 对象**，禁止把单个 JSON 对象拆成多行
> - **script 内容禁止注释**，代码必须保持纯净

OpenCat JSONL 是用于描述 composition、场景节点、脚本和转场的 JSON Lines 格式。

---

## 1. 文件结构

### 1.1 Composition（第 1 行，必填）

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 180}
```

`frames / fps` 表示总时长（秒）。

### 1.2 两种模式：单场景 vs 多场景

#### 单场景

只有一个 `parentId: null` 的根节点，没有转场。`composition.frames` 必须等于该场景的 `duration`。

```text
时间线：[   scene1: 60 帧   ]
约束：composition.frames = scene1.duration
```

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 60}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold", "text": "Hello"}
```

#### 多场景 + 转场

可以有多个 `parentId: null` 的根节点。场景之间通过 `transition` 记录连接。每个场景都有自己独立的节点树。转场会额外消耗帧数，并在切换期间让两个场景发生重叠。

```text
时间线：[ scene1: 60 帧 ] [ fade: 12 帧 ] [ scene2: 90 帧 ]
约束：composition.frames = 60 + 12 + 90 = 162
```

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 162}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Part 1"}
{"id": "scene2", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-slate-900", "duration": 90}
{"id": "subtitle", "parentId": "scene2", "type": "text", "className": "text-[20px] font-semibold text-white", "text": "Part 2"}
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

**关键规则**：

- 每个场景有独立的节点树，节点不会跨场景共享
- `composition.frames = sum(所有 scene.duration) + sum(所有 transition.duration)`
- 转场按顺序连接：`scene1 -> transition(scene1->scene2) -> scene2 -> ...`

### 1.3 元素节点

每个元素占一行 JSON，通过 `parentId` 形成父子关系。

`className` 使用的是 Tailwind 风格的类名，用来描述布局和视觉属性，方式上类似给 HTML 节点写 Tailwind。

```json
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Hello"}
{"id": "hero", "parentId": "scene1", "type": "image", "className": "w-[300px] h-[200px] object-cover rounded-lg", "query": "mountain landscape"}
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] text-slate-400", "icon": "search"}
```

**类型对照**：

| type | 对应 HTML | 特有字段 |
|------|-----------|----------|
| `div` | `<div>` | — |
| `text` | `<span>` / `<p>` | `text`: 文本内容 |
| `image` | `<img>` | `query`: 图片搜索词（1-4 个名词） |
| `icon` | Lucide 图标 | `icon`: kebab-case 图标名 |
| `canvas` | `<canvas>` | 需要配套 script |
| `audio` | `<audio>` | `path` 或 `url` |
| `video` | `<video>` | — |

### 1.4 Script

> **⚠️ `script.src` 中禁止注释**

脚本挂载在节点上，并且会在每一帧执行。

```json
{"type": "script", "parentId": "scene1", "src": "var node = ctx.getNode('title');\nvar anim = ctx.animate({from:{opacity:0},to:{opacity:1},duration:20,easing:'spring-gentle'});\nnode.opacity(anim.opacity);"}
{"type": "script", "parentId": "scene1", "path": "scene1.js"}
```

### 1.5 Transition

转场只在多场景模式下使用。它描述两个场景之间的切换，并额外占用帧数。

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

**effect 类型**（`effect` 和 `direction` 是两个独立字段）：

| effect | 说明 | direction（可选） |
|--------|------|-------------------|
| `fade` | 淡入淡出 | — |
| `slide` | 滑动切换 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` |
| `wipe` | 擦除切换 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` / `from_top_left` / `from_top_right` / `from_bottom_left` / `from_bottom_right` |
| `clock_wipe` | 时钟擦除 | — |
| `iris` | 光圈开合 | — |
| `light_leak` | 光泄漏 | —（支持 `seed`、`hueShift`、`maskScale`） |

**timing 控制**（所有 effect 通用）：

`timing` 使用和 `ctx.animate()` 一样的缓动名称，默认值是 `"linear"`。

| timing | 说明 |
|--------|------|
| `"linear"`（默认） | 匀速 |
| `"ease"` | CSS ease |
| `"ease-in"` | 渐入 |
| `"ease-out"` | 渐出 |
| `"ease-in-out"` | 渐入渐出 |
| `"spring-default"` / `"spring-gentle"` / … | 弹簧预设 |
| `"bezier:x1,y1,x2,y2"` | 三次贝塞尔 |

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 20, "timing": "ease-out"}
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "slide", "direction": "from_right", "duration": 15, "timing": "bezier:0.4,0,0.2,1"}
```

也可以直接使用自定义弹簧参数：

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "wipe", "direction": "from_right", "duration": 12, "damping": 10, "stiffness": 100, "mass": 1}
```

---

## 2. 样式（Tailwind）

大多数 Tailwind 类名都可以直接用于布局、颜色、间距、圆角等静态样式。

**主要限制**：

- 不要使用 CSS 动画类
- 不要在 `className` 中生成任何 transform 相关的 Tailwind 类
  - 包括 `transform`、`translate-*`、`rotate-*`、`scale-*`、`skew-*`
  - 这类变换统一通过脚本节点 API 处理

| 不要用 | 替代方案 |
|------|----------|
| `transition-*` `animate-*` `duration-*` `ease-*` `delay-*` | `ctx.animate()` / `ctx.stagger()` |
| `transform` `translate-*` `rotate-*` `scale-*` `skew-*` | `ctx.getNode(...).translateX()` / `translateY()` / `scale()` / `rotate()` / `skew()` |

> Tailwind 负责静态样式，脚本负责动态效果。

---

## 3. 动画系统

动画通过 JavaScript 声明。脚本每帧执行，并读取插值后的动画值来驱动节点属性。

### Context

| 字段 | 说明 |
|------|------|
| `ctx.frame` | 全局帧号 |
| `ctx.totalFrames` | 全局总帧数 |
| `ctx.currentFrame` | 当前场景内的帧号（`0 -> sceneFrames - 1`） |
| `ctx.sceneFrames` | 当前场景的总帧数 |

场景内动画优先使用 `ctx.currentFrame` 和 `ctx.sceneFrames`。

### ctx.animate(opts)

声明一个 `from -> to` 动画。返回对象中的属性通过 getter 暴露当前插值结果。

```js
var anim = ctx.animate({
  from: { opacity: 0, translateY: 40, scale: 0.95 },
  to:   { opacity: 1, translateY: 0,  scale: 1 },
  duration: 30,
  delay: 0,
  easing: 'spring-gentle',
  clamp: false,
});

node.opacity(anim.opacity).translateY(anim.translateY).scale(anim.scale);
```

附加字段：

- `anim.progress`：从 `0` 到 `1`
- `anim.settled`：弹簧是否已经稳定
- `anim.settleFrame`：弹簧稳定的帧号

### Keyframes（单动画多关键帧）

当一个动画需要多于两个关键点时，用 `keyframes` 替代 `from`/`to`：

```js
// 简写：数值均匀分布在 [0, 1]
var a = ctx.animate({
  keyframes: { scale: [1, 1.4, 0.8, 1] },
  duration: 60,
});
ctx.getNode('card').scale(a.scale);

// 全写：显式 `at`（归一化到 [0, 1]）+ 可选的每段 easing
var b = ctx.animate({
  keyframes: {
    rotate: [
      { at: 0,   value: 0 },
      { at: 0.5, value: 360, easing: 'back-out' },
      { at: 1,   value: 0 }
    ],
  },
  duration: 60,
});
ctx.getNode('logo').rotate(b.rotate);
```

注意事项：

- keyframes 只支持**数值**（颜色 keyframes 暂不支持——颜色动画请用 `from`/`to`）。
- `at` 归一化到 `[0, 1]`；`ctx.animate` 上的**外层** `easing`（以及 `repeat`/`yoyo`）先作用于 progress，然后 progress 再映射到每段的 easing。
- `keyframes` 和 `from`/`to` 可以在同一个动画中共存，但两者都定义的 key 以 `keyframes` 为准。

### ctx.stagger(count, opts)

和 `animate` 类似，但会为多个元素生成带交错延迟的动画。

```js
var anims = ctx.stagger(4, {
  from: { opacity: 0, translateY: 30 },
  to:   { opacity: 1, translateY: 0 },
  gap: 4,
  duration: 20,
  easing: 'spring-gentle',
});
```

### ctx.alongPath(svgPath)

将 SVG path 字符串采样为运动路径。返回一个包含 `getLength()`、`at(t)` 和 `dispose()` 的对象。`at(t)` 接受 `t in [0, 1]` 并返回 `{ x, y, angle }`——`angle` 是路径切线角度，单位为**度**，可直接传给 `node.rotate()`。

SVG 字符串在创建时解析一次；采样通过 Rust 侧 Skia 的 `ContourMeasure` 计算。

```js
// 重要：缓存 measurer；不要每帧重建
if (!ctx.__along) {
  ctx.__along = ctx.alongPath('M100 360 C400 80 880 640 1180 360');
}

var a = ctx.animate({
  from: { t: 0 }, to: { t: 1 },
  duration: 120, easing: 'ease-in-out',
  repeat: -1, yoyo: true,
});
var pos = ctx.__along.at(a.t);
ctx.getNode('ball')
  .translateX(pos.x - 24)
  .translateY(pos.y - 24)
  .rotate(pos.angle);
```

**支持的 SVG path 命令**（大写 = 绝对，小写 = 相对）：

| 命令 | 含义 |
|---|---|
| `M x y` / `m dx dy` | 移动到 |
| `L x y` / `l dx dy` | 直线到 |
| `H x` / `h dx` | 水平直线到 |
| `V y` / `v dy` | 垂直直线到 |
| `C x1 y1 x2 y2 x y` | 三次贝塞尔 |
| `S x2 y2 x y` | 平滑三次贝塞尔 |
| `Q x1 y1 x y` | 二次贝塞尔 |
| `T x y` | 平滑二次贝塞尔 |
| `A rx ry x-axis-rot large sweep x y` | 椭圆弧 |
| `Z` / `z` | 闭合路径 |

**限制：**

- 只采样**第一条 contour**。如果路径包含多个 `M` 命令（多个子路径），后续的会被忽略。
- 路径只在 `ctx.alongPath()` 调用时解析一次。要使用不同路径，需要创建新实例。
- 始终将 `alongPath` 实例缓存到 `ctx.__yourKey`——每帧重建会泄漏 Rust 侧的 `ContourMeasure`，直到脚本上下文销毁。
- `dispose()` 是可选的，但在长时间运行的 composition 或切换多条路径时推荐使用。

### Easing

| 预设 | 效果 |
|------|------|
| `'linear'` | 匀速 |
| `'spring-default'` | 通用弹簧 |
| `'spring-gentle'` | 柔和弹簧 |
| `'spring-stiff'` | 更硬的弹簧 |
| `'spring-slow'` | 更慢的弹簧 |
| `'spring-wobbly'` | 摇摆感更强 |

自定义弹簧：

```js
easing: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

三次贝塞尔：

```js
easing: [0.25, 0.1, 0.25, 1.0]
```

### 颜色动画

`ctx.animate()` 会在 `from` 或 `to` 的值为字符串时自动对颜色进行插值。颜色会被转换为 HSLA，色相沿最短弧线插值（处理 360->0 环绕），结果以 `rgba(...)` 字符串返回，兼容 `node.bg()`、`node.textColor()`、`node.borderColor()` 等。

```js
var a = ctx.animate({
  from: { bg: '#ef4444' },
  to:   { bg: 'hsl(220, 90%, 55%)' },
  duration: 60,
  repeat: -1,
  yoyo: true,
});
ctx.getNode('card').bg(a.bg);
```

支持的颜色字面量（用于 `from` / `to`）：

- `#rgb` / `#rrggbb` / `#rrggbbaa`
- `rgb(r, g, b)` / `rgba(r, g, b, a)`
- `hsl(h, s%, l%)` / `hsla(h, s%, l%, a)`

颜色总是会被 clamp（spring 超出 `[0, 1]` 的 progress 会被归一化回范围内），因此弹簧缓动不会把值推到可见色域之外。

> Tailwind token 如 `'blue-500'` 仍然作为离散的 `node.bg(token)` 调用——它们**不会**被插值。要做颜色动画，请在 `from` / `to` 中用 hex/rgb/hsl 格式书写颜色。

### 节点 API

`ctx.getNode('id')` 会返回一个可链式调用的代理对象。

```js
// Transform
node.opacity(0.5).translateX(100).translateY(50).translate(100, 50);
node.scale(1.5).scaleX(1.2).scaleY(0.8);
node.rotate(45).skewX(10).skewY(10).skew(10, 10);

// Layout
node.position('absolute').left(100).top(50).right(20).bottom(20);
node.width(200).height(100);

// Spacing
node.padding(16).paddingX(24).paddingY(12);
node.margin(8).marginX(16).marginY(8);

// Flex
node.flexDirection('col').justifyContent('center').alignItems('center').gap(12).flexGrow(1);

// Style
node.bg('blue-500').borderRadius(16).borderWidth(2).borderColor('gray-300');
node.objectFit('cover').textColor('white').textSize(24).fontWeight('bold');
node.textAlign('center').lineHeight(1.5).letterSpacing(1).shadow('lg');
node.strokeWidth(2).strokeColor('gray-300').fillColor('blue-500');
```

### 常用模式

**交错入场**：

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

**联动效果**：

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

**循环脉冲**：

```js
var icons = ['icon-a', 'icon-b', 'icon-c'];
var frame = ctx.frame;
var cycleLen = 30;
var activeIndex = Math.floor((frame % (icons.length * cycleLen)) / cycleLen);
var cycleStart = frame - (frame % cycleLen);

var entrance = ctx.stagger(icons.length, {
  from: { scale: 0.85, translateY: 18 }, to: { scale: 1, translateY: 0 },
  gap: 4, easing: 'spring-default',
});

icons.forEach(function(id, i) {
  var s = entrance[i].scale;
  if (i === activeIndex) {
    var pulse = ctx.animate({
      from: { scale: 1 }, to: { scale: 1.08 },
      duration: cycleLen, delay: cycleStart, easing: 'spring-wobbly',
    });
    s = pulse.scale;
  }
  ctx.getNode(id).scale(s);
});
```

### 限制

- 不要使用 `document`、`window`、`requestAnimationFrame`、`element.style`
- 只能通过 `ctx.getNode()` 获取节点
- 非弹簧缓动必须提供 `duration`

---

## 4. Canvas（CanvasKit 风格子集）

`type: "canvas"` 节点相当于一个画布表面，但只支持 OpenCat 当前暴露出来的 CanvasKit 子集。绘图脚本必须作为该 canvas 节点的子 script 挂载，并且会在每一帧重新执行。

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

### 入口对象

| 对象 | 作用 |
|------|------|
| `ctx.CanvasKit` / `globalThis.CanvasKit` | CanvasKit 风格的辅助函数、构造器和枚举 |
| `ctx.getCanvas()` | 返回当前 canvas 节点的绘图接口 |
| `ctx.getImage(assetId)` | 返回宿主侧提供的 asset id 对应图片句柄 |

### 当前支持的 CanvasKit 辅助能力

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

### `ctx.getCanvas()` 当前支持的方法

除特殊说明外，方法都支持链式调用。

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

// 图形
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

// 文字
canvas.drawText(text, x, y, paint, font);
```

### `Paint` 支持范围

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

目前只支持 dash 路径效果。

### `Path` 支持范围

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

### 文字 API 支持范围

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

当前约束：

- `typeface` 目前必须为 `null`，表示系统默认字体
- 不支持自定义字体对象、`Typeface`、`FontMgr` 和字体 asset
- 不支持 `TextBlob` 和 `Paragraph`

### 图片资源规则

Canvas 脚本中的图片必须通过 `ctx.getImage(assetId)` 获取，不能直接传 URL、文件路径或任意原生图片对象。

```js
var img = ctx.getImage('hero-asset');
canvas.drawImage(img, 40, 40);
canvas.drawImageRect(
  img,
  CK.XYWHRect(0, 0, 320, 180),
  CK.XYWHRect(40, 40, 160, 90)
);
```

### 当前明确限制

- 这里只是 CanvasKit 子集，不是完整 CanvasKit
- `clipRect()`、`clipPath()`、`clipRRect()` 目前只支持 `CK.ClipOp.Intersect`
- `drawColor()`、`drawColorInt()`、`drawColorComponents()` 目前只支持 `CK.BlendMode.SrcOver`
- `PathEffect` 目前只支持 `MakeDash()`
- 文字绘制仅支持系统默认字体
- `ctx.getImage()` 只接受 asset id 句柄

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

## 5. 常见错误

| 错误 | 正确 |
|------|------|
| `type: "div"` 却带有 `text` 字段 | 只有 `type: "text"` 才能带 `text` |
| 用 `bg-{color}` 给图标着色 | 图标应该使用 `text-{color}` |
| `id` 里有 "icon" 但 `type: "div"` | 应该使用 `type: "icon"` 并提供 Lucide 图标名 |
| 图片 `query` 里带形容词 | 只使用 1-4 个名词 |
| 默认依赖 `absolute` 做布局 | 优先使用 flex；`absolute` 只用于重叠或贴边 |
| 在 `className` 中写 transform 相关 Tailwind 类 | 使用节点变换 API，如 `translateX()`、`translateY()`、`scale()`、`rotate()`、`skew()` |
| `parentId` 指向不存在的 id | `parentId` 必须引用已存在节点 |
| 帧数不匹配 | `composition.frames == sum(scene.duration) + sum(transition.duration)` |
| `"effect": "slide-left"` | 应拆成两个字段：`"effect": "slide", "direction": "from_left"` |
