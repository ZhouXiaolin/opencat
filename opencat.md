# OpenCat JSONL

> **⚠️ 重要格式要求**
> - **每行一个 JSON 对象**，禁止换行分隔多行 JSON
> - **script 内容禁止注释**，代码必须纯净

OpenCat JSONL 是 HTML+CSS+JS 的 JSON Lines 序列化。每一行是一个 JSON 对象，描述 DOM 节点、脚本或转场。

**心智映射**：

| Web | OpenCat JSONL |
|-----|---------------|
| `<html>` 属性 | `composition` 行 |
| 页面 / `<body>` | `parentId: null` 的场景根节点 |
| DOM 树嵌套 | `parentId` 引用 |
| CSS class | `className`（Tailwind） |
| `<script>` | `type: "script"` 行 |
| CSS 动画 | `ctx.animate()` / `ctx.stagger()` |
| `<canvas>` | `type: "canvas"` + CanvasKit API |

---

## 1. 文件结构

### 1.1 Composition（第 1 行，必填）

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 180}
```

`frames / fps` = 视频总时长（秒）。

### 1.2 两种模式：单场景 vs 多场景

#### 单场景

一个 `parentId: null` 的根节点，无转场。`composition.frames` 等于该场景的 `duration`。

```
时间线：[   scene1: 60帧   ]
约束：composition.frames = scene1.duration
```

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 60}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold", "text": "Hello"}
```

#### 多场景 + 转场

多个 `parentId: null` 的根节点，场景间通过 `transition` 衔接。每个场景是独立的元素树（类比多页网站，各页 DOM 互不相通）。转场占用额外帧数，两个场景之间有重叠过渡。

```
时间线：[ scene1: 60帧 ] [ fade: 12帧 ] [ scene2: 90帧 ]
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
- 每个场景有独立的元素树，场景间节点互不可见
- `composition.frames = sum(所有 scene.duration) + sum(所有 transition.duration)`
- 转场按出现顺序衔接：scene1 → transition(scene1→scene2) → scene2 → ...

### 1.3 元素节点

一个元素 = 一个 DOM 节点，一行 JSON。通过 `parentId` 形成父子树：

```json
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Hello"}
{"id": "hero", "parentId": "scene1", "type": "image", "className": "w-[300px] h-[200px] object-cover rounded-lg", "query": "mountain landscape"}
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] text-slate-400", "icon": "search"}
```

**类型对照**：

| type | 等价 HTML | 特有字段 |
|------|-----------|----------|
| `div` | `<div>` | — |
| `text` | `<span>` / `<p>` | `text`: 文本内容 |
| `image` | `<img>` | `query`: 图片搜索词（1-4 名词） |
| `icon` | Lucide 图标 | `icon`: 图标名（kebab-case） |
| `canvas` | `<canvas>` | 需配套 script |
| `audio` | `<audio>` | `path` 或 `url` |
| `video` | `<video>` | — |

### 1.4 Script

> **⚠️ script.src 禁止注释**，代码必须纯净

挂载在节点上，每帧执行（类比 `requestAnimationFrame` 循环）：

```json
{"type": "script", "parentId": "scene1", "src": "var node = ctx.getNode('title');\nvar anim = ctx.animate({from:{opacity:0},to:{opacity:1},duration:20,easing:'spring-gentle'});\nnode.opacity(anim.opacity);"}
{"type": "script", "parentId": "scene1", "path": "scene1.js"}
```

### 1.5 Transition

仅在多场景模式下使用。转场是两个场景之间的过渡效果，占用额外帧数：

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

**effect 类型**（`effect` 只填类型名，方向由 `direction` 字段单独指定）：

| effect | 说明 | direction（可选） |
|--------|------|-------------------|
| `fade` | 淡入淡出 | — |
| `slide` | 滑动切换 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` |
| `wipe` | 擦除切换 | `from_left`（默认）/ `from_right` / `from_top` / `from_bottom` / `from_top_left` / `from_top_right` / `from_bottom_left` / `from_bottom_right` |
| `clock_wipe` | 时钟擦除 | — |
| `iris` | 光圈开合 | — |
| `light_leak` | 光泄漏 | —（特有参数：`seed`, `hueShift`, `maskScale`） |

**timing 控制**（所有 effect 通用）：

`timing` 字段使用与 `ctx.animate()` 相同的缓动名称，默认 `"linear"`：

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

自定义弹簧（通过 `damping`/`stiffness`/`mass` 字段，此时 `timing` 可省略）：

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "wipe", "direction": "from_right", "duration": 12, "damping": 10, "stiffness": 100, "mass": 1}
```

---

## 2. 样式（Tailwind）

大部分 Tailwind class 直接可用，布局、颜色、间距、圆角等和写 React/Vue 一样。

**唯一限制**：禁止所有 CSS 动画 class：

| 禁止 | 替代 |
|------|------|
| `transition-*` `animate-*` `duration-*` `ease-*` `delay-*` | `ctx.animate()` / `ctx.stagger()` |

> Tailwind 管静态，脚本管动态。

---

## 3. 动画系统

类比 CSS `transition` / `animation`，但通过 JS 声明。脚本每帧执行，读取动画插值驱动节点属性。

### Context

| 属性 | 说明 |
|------|------|
| `ctx.frame` | 全局帧号 |
| `ctx.totalFrames` | 全局总帧数 |
| `ctx.currentFrame` | 当前场景帧号（0 → sceneFrames-1） |
| `ctx.sceneFrames` | 当前场景总帧数 |

场景内动画优先用 `ctx.currentFrame` / `ctx.sceneFrames`。

### ctx.animate(opts)

声明 from → to，返回响应式对象，属性为 getter，读取时返回当前帧插值：

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

额外属性：`anim.progress`（0→1）、`anim.settled`（spring 是否稳定）、`anim.settleFrame`。

### ctx.stagger(count, opts)

同 `animate` 但为 N 个元素依次延迟，返回数组：

```js
var anims = ctx.stagger(4, {
  from: { opacity: 0, translateY: 30 },
  to:   { opacity: 1, translateY: 0 },
  gap: 4,          // 元素间延迟帧数
  duration: 20,
  easing: 'spring-gentle',
});
// anims[0]..anims[3] 各自独立插值
```

### Easing

| 预设 | 效果 |
|------|------|
| `'linear'` | 匀速 |
| `'spring-default'` | 通用弹簧 |
| `'spring-gentle'` | 柔和 |
| `'spring-stiff'` | 硬弹 |
| `'spring-slow'` | 慢弹簧 |
| `'spring-wobbly'` | 摇摆 |

自定义弹簧（可省略 `duration`）：

```js
easing: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

三次贝塞尔：`easing: [0.25, 0.1, 0.25, 1.0]`

### 节点操作 API

`ctx.getNode('id')` 返回代理对象，链式调用。等价于直接操作 CSS 属性：

```js
// Transform（对应 CSS transform）
node.opacity(0.5).translateX(100).translateY(50).translate(100, 50);
node.scale(1.5).scaleX(1.2).scaleY(0.8);
node.rotate(45).skewX(10).skewY(10).skew(10, 10);

// Layout（对应 CSS position / width / height）
node.position('absolute').left(100).top(50).right(20).bottom(20);
node.width(200).height(100);

// Spacing（对应 padding / margin）
node.padding(16).paddingX(24).paddingY(12);
node.margin(8).marginX(16).marginY(8);

// Flex（对应 flex 属性）
node.flexDirection('col').justifyContent('center').alignItems('center').gap(12).flexGrow(1);

// Style（对应各种 CSS 属性）
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

**联动效果**（动画值可做数学运算）：

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

**循环脉冲**（用 `ctx.frame` 计算周期）：

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

- 禁止 `document`、`window`、`requestAnimationFrame`、`element.style`
- 仅用 `ctx.getNode()` 获取节点
- 非弹簧缓动时 `duration` 必填

---

## 4. Canvas（CanvasKit 脚本子集）

`type: "canvas"` 节点类似 HTML `<canvas>`，但只支持 OpenCat 当前暴露的 CanvasKit 子集。脚本必须作为 `canvas` 节点的子 `script` 挂载，并会在每一帧重新执行。

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

### 入口对象

| 对象 | 作用 |
|------|------|
| `ctx.CanvasKit` / `globalThis.CanvasKit` | CanvasKit 风格工具函数、构造器、枚举 |
| `ctx.getCanvas()` | 取得当前 `canvas` 节点对应的绘图接口 |
| `ctx.getImage(assetId)` | 取得图片句柄，只接受宿主侧已接入的 asset id |

### 当前支持的 CanvasKit 工具

```js
var CK = ctx.CanvasKit;

// 颜色
CK.Color(r, g, b, a?)            // r/g/b: 0-255, a: 0-1
CK.Color4f(r, g, b, a?)          // 全部 0-1
CK.ColorAsInt(r, g, b, a?)       // 打包成 ARGB int
CK.parseColorString('#ff0000')   // 支持 black/white/#rgb/#rrggbb/#rrggbbaa/rgb()/rgba()
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

所有方法都支持链式调用；只有 `save()` / `saveLayer()` 返回 save count。

```js
var canvas = ctx.getCanvas();

// 状态与变换
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

// 文字
canvas.drawText(text, x, y, paint, font);
```

### `Paint` 支持范围

```js
var paint = new CK.Paint();

paint.setStyle(CK.PaintStyle.Fill);     // Fill | Stroke
paint.setColor(CK.parseColorString('#ff0000'));
paint.setColorComponents(1, 0, 0, 1);
paint.setColorInt(CK.ColorAsInt(255, 0, 0, 1));
paint.setAlphaf(0.8);
paint.setStrokeWidth(2);
paint.setStrokeCap(CK.StrokeCap.Round);
paint.setStrokeJoin(CK.StrokeJoin.Round);
paint.setAntiAlias(true);
paint.setStrokeDash([10, 5], 0);        // 兼容层，底层等价于 PathEffect.MakeDash()
paint.setPathEffect(CK.PathEffect.MakeDash([10, 5], 0));
```

当前 `PathEffect` 只支持 dash。

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

当前是“基础可用版”，对齐最小 CanvasKit 风格接口：

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

约束：
- `typeface` 当前必须传 `null`，表示系统默认字体
- 不支持自定义字体对象、`Typeface`、`FontMgr`、字体 asset
- 不支持 `TextBlob`、`Paragraph`

### 图片资源约束

Canvas 脚本里的图片必须通过 `ctx.getImage(assetId)` 获取，不能直接传 URL、文件路径或宿主原生图片对象：

```js
var img = ctx.getImage('hero-asset');
canvas.drawImage(img, 40, 40);
canvas.drawImageRect(
  img,
  CK.XYWHRect(0, 0, 320, 180),
  CK.XYWHRect(40, 40, 160, 90)
);
```

`drawImageRect()` 当前已经支持 `srcRect` 裁切，不再是整图强制缩放。

### 当前明确限制

- 这是 CanvasKit 子集，不是完整 CanvasKit。
- `clipRect()` / `clipPath()` / `clipRRect()` 目前只支持 `CK.ClipOp.Intersect`，传 `Difference` 会抛错。
- `drawColor()` / `drawColorInt()` / `drawColorComponents()` 目前只支持 `CK.BlendMode.SrcOver`。
- `PathEffect` 目前只支持 `MakeDash()`。
- 文字绘制只支持系统默认字体，不支持外部字体资源。
- `ctx.getImage()` 只接受 asset id 句柄，不接受任意外部图片对象。

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
| `type: "div"` 含 `text` 字段 | 仅 `type: "text"` 有 `text` |
| 图标用 `bg-{color}` 着色 | 图标用 `text-{color}` |
| `id` 含 "icon" 但用 `type: "div"` | 必须用 `type: "icon"` + Lucide 名 |
| 图片 `query` 含形容词 | 仅用 1-4 个名词 |
| 布局用 `absolute` 定位 | 默认用 `flex`，`absolute` 仅用于重叠/固定边缘 |
| `parentId` 引用无效 id | 必须引用已存在的节点 |
| 帧数不匹配 | `composition.frames == sum(scene.duration) + sum(transition.duration)` |
| `"effect": "slide-left"` | effect 和方向是两个字段：`"effect": "slide", "direction": "from_left"` |
