# CanvasKit 子集

`<canvas>` 节点提供 CanvasKit 风格的即时绘制表面。OpenCat 不是浏览器 Canvas API 的完整实现；这里记录的是当前 JS runtime 暴露的 CanvasKit 子集。

绘制脚本通常写在 `<opencat>` 的全局 `<script>` 中，通过 `ctx.getCanvasById(id)` 指定目标 canvas。每个输出采样点都会重新执行脚本，所以绘制逻辑必须是确定性的时间函数。

---

## 入口点

| 对象 | 用途 |
| --- | --- |
| `ctx.CanvasKit` / `CanvasKit` | CanvasKit 辅助函数、构造函数、枚举 |
| `ctx.getCanvasById(id)` | 获取指定 `<canvas>` 的绘制接口 |
| `ctx.getImage(assetId)` | 获取图像句柄，可用于 `drawImage` / `drawImageRect` / shader child |

不要使用 `ctx.getCanvas()`。当前实现会直接抛错：必须传入 canvas id。

```xml
<opencat width="1280" height="720" fps="30" duration="3">
  <script>
    var CK = ctx.CanvasKit;
    var canvas = ctx.getCanvasById('stage');

    var paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);
    paint.setColor(CK.parseColorString('#0f172a'));

    canvas.clear(CK.WHITE);
    canvas.drawRect(CK.XYWHRect(40, 40, 240, 120), paint);
  </script>

  <canvas id="stage" class="w-[1280px] h-[720px]" />
</opencat>
```

---

## Canvas Hidden Subtree

`<canvas>` 在 markup 中允许子元素。这些子元素不会自动像普通 DOM 一样直接显示在 canvas 上，而是作为 hidden subtree 被记录，供 canvas 脚本显式采样或重放。

这用于模拟 “HTML in Canvas”：

```xml
<canvas id="surface" class="w-[360px] h-[480px] rounded-[32px] overflow-hidden">
  <image id="hero" class="w-[360px] h-[480px] object-cover" url="./hero.png" />
  <div id="badge" class="absolute left-[18px] top-[18px] px-[12px] py-[8px] rounded-full bg-black/60">
    <text id="badge-text" class="text-[12px] font-semibold tracking-[2px] text-white">LIVE SUBTREE</text>
  </div>
</canvas>
```

在脚本中：

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvasById('surface');
var picture = canvas.getSubTree();

canvas.clear();
canvas.drawPicture(picture, 0, 0);
```

规则：

- `canvas.getSubTree()` 只对 `<canvas>` 节点有效。
- `canvas.drawPicture(handle, x?, y?)` 会把该 canvas 的 hidden subtree 作为 picture 重放到当前 canvas。
- 子元素也可以通过 `ctx.getNode(id)` / 动画脚本参与变更；变更会影响 subtree 采样。
- `<canvas>` 子树里不要放 `<audio>`。

---

## CanvasKit 辅助函数

```js
var CK = ctx.CanvasKit;

// 颜色
CK.Color(r, g, b, a?)
CK.Color4f(r, g, b, a?)
CK.ColorAsInt(r, g, b, a?)
CK.parseColorString('#ff0000')
CK.parseColorString('#ff000080')
CK.parseColorString('rgb(255,0,0)')
CK.parseColorString('rgba(255,0,0,0.5)')
CK.multiplyByAlpha(color, 0.5)

// 几何
CK.LTRBRect(left, top, right, bottom)
CK.XYWHRect(x, y, width, height)
CK.RRectXY(rect, rx, ry)

// 构造函数
new CK.Paint()
new CK.Path()
new CK.Font(null, size?, scaleX?, skewX?)
CK.PathEffect.MakeDash(intervals, phase?)
CK.RuntimeEffect.Make(sksl)
```

常量和枚举：

```js
CK.BLACK
CK.WHITE

CK.PaintStyle.Fill
CK.PaintStyle.Stroke

CK.StrokeCap.Butt
CK.StrokeCap.Round
CK.StrokeCap.Square

CK.StrokeJoin.Miter
CK.StrokeJoin.Round
CK.StrokeJoin.Bevel

CK.FontEdging.Alias
CK.FontEdging.AntiAlias
CK.FontEdging.SubpixelAntiAlias

CK.BlendMode.SrcOver

CK.ClipOp.Intersect
CK.ClipOp.Difference // 常量存在，但 clip 当前只支持 Intersect

CK.PointMode.Points
CK.PointMode.Lines
CK.PointMode.Polygon

CK.TileMode.Clamp
CK.TileMode.Repeat
CK.TileMode.Mirror
CK.TileMode.Decal
```

---

## Canvas 方法

```js
var canvas = ctx.getCanvasById('stage');
```

### 状态与变换

```js
canvas.clear(color?);
canvas.save();
canvas.saveLayer(paintOrBounds?, bounds?);
canvas.restore();
canvas.restoreToCount(saveCount);

canvas.translate(dx, dy);
canvas.scale(sx, sy?);
canvas.rotate(degrees, rx?, ry?);
canvas.skew(sx, sy);
canvas.concat([m00, m01, m02, m10, m11, m12, m20, m21, m22]);
canvas.setAlphaf(alpha);
```

### 裁剪

```js
canvas.clipRect(rect, CK.ClipOp.Intersect, doAntiAlias?);
canvas.clipPath(path, CK.ClipOp.Intersect, doAntiAlias?);
canvas.clipRRect(rrect, CK.ClipOp.Intersect, doAntiAlias?);
```

当前只支持 `CK.ClipOp.Intersect`。

### 形状

```js
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
```

`drawColor*()` 当前只支持 `CK.BlendMode.SrcOver`。

### 图像

```js
var img = ctx.getImage('hero-asset');

canvas.drawImage(img, x, y, paint?);
canvas.drawImageRect(img, srcRect, destRect, paint?, fastSample?);
```

`paint` 对图像绘制主要使用 alpha 和 antiAlias。复杂 color filter / image filter 尚未通过 JS CanvasKit facade 暴露。

### 文字

```js
var font = new CK.Font(null, 32);
canvas.drawText('OpenCat', 40, 80, paint, font);
```

当前仅支持系统默认字体，`new CK.Font(typeface, ...)` 的 `typeface` 必须传 `null`。

### Subtree Picture

```js
var picture = canvas.getSubTree();
canvas.drawPicture(picture, 0, 0);
```

`drawPicture()` 接受 `getSubTree()` 返回的 handle。它用于把 canvas hidden children 作为 picture 绘制到 canvas 上。

---

## Paint

```js
var paint = new CK.Paint();

paint.setStyle(CK.PaintStyle.Fill);
paint.setStyle(CK.PaintStyle.Stroke);

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

paint.setShader(shader);
```

辅助：

```js
paint.copy();
paint.delete(); // no-op
paint.getColor();
paint.getStrokeWidth();
paint.getStrokeCap();
paint.getStrokeJoin();
```

当前 `paint.setShader(shader)` 的主要有效路径是 RuntimeEffect shader 配合 `canvas.drawRect(rect, paint)`。image shader / picture shader 通常作为 RuntimeEffect child 使用，不要把它们当作普通 fill shader 直接依赖。

---

## Path

```js
var path = new CK.Path();

path.moveTo(x, y);
path.lineTo(x, y);
path.quadTo(x1, y1, x2, y2);
path.cubicTo(x1, y1, x2, y2, x3, y3);
path.close();

path.addRect(CK.XYWHRect(10, 10, 80, 40));
path.addRRect(CK.RRectXY(CK.XYWHRect(10, 10, 80, 40), 8, 8));
path.addOval(CK.XYWHRect(10, 10, 80, 40));
path.addArc(CK.XYWHRect(10, 10, 80, 40), 0, 180);

path.reset();
path.rewind();
path.copy();
path.delete(); // no-op
```

---

## Font

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

辅助：

```js
font.copy();
font.delete(); // no-op
font.getSize();
```

限制：

- 自定义 typeface 未支持，`new CK.Font(null, size)` 是当前稳定写法。
- `measureText()` 返回当前默认字体模型下的测量宽度。

---

## Image 与 Shader

图像句柄：

```js
var img = ctx.getImage('hero-asset');

canvas.drawImage(img, 40, 40);
canvas.drawImageRect(
  img,
  CK.XYWHRect(0, 0, 320, 180),
  CK.XYWHRect(40, 40, 160, 90)
);
```

图像也可以作为 RuntimeEffect child shader：

```js
var imgShader = ctx.getImage('hero-asset').makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);
```

Subtree picture 也可以作为 RuntimeEffect child shader：

```js
var subtreeShader = canvas.getSubTree().makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);
```

当前 tile mode 参数会被 API 接收；native picture child 当前按 clamp 方式采样，文档中不要依赖 repeat / mirror / decal 对 subtree picture 生效。

---

## RuntimeEffect

`CK.RuntimeEffect.Make(sksl)` 创建 RuntimeEffect。它可以直接生成 shader，也可以带 image / picture child shader。

最常见用法是：把 canvas hidden subtree 作为 shader child，经过 SKSL 处理后画回 canvas。

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvasById('surface');
var subtreeShader = canvas.getSubTree().makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);

var sksl = [
  'uniform shader image;',
  'uniform float progress;',
  '',
  'half4 main(float2 xy) {',
  '  float wave = sin((xy.x + progress * 120.0) * 0.03) * 4.0;',
  '  return image.eval(float2(xy.x, xy.y + wave));',
  '}',
].join('\n');

var effect = CK.RuntimeEffect.Make(sksl);
if (effect) {
  var shader = effect.makeShaderWithChildren(
    [ctx.currentTime],
    [subtreeShader]
  );

  var paint = new CK.Paint();
  paint.setShader(shader);
  canvas.drawRect(CK.XYWHRect(0, 0, 360, 480), paint);
}
```

API：

```js
var effect = CK.RuntimeEffect.Make(sksl);
effect.makeShader(uniforms);
effect.makeShaderWithChildren(uniforms, children);
effect.delete(); // no-op
```

限制：

- `children` 当前支持 image shader、subtree picture shader 和 generated（帧级生成图像，如溶解 field 烘焙）。
- gradient shader child 当前没有 JS facade。
- RuntimeEffect shader 当前通过 `paint.setShader(shader)` + `canvas.drawRect()` 触发绘制。
- SKSL 必须能被 native / web CanvasKit 编译；`Make()` 对空字符串返回 `null`。
- image child 的采样空间 = effect dst 本地坐标（原点 = dst 左上角，与 `uv = xy - rect.xy` 一致）；subtree picture child 用画布坐标。

---

## Effect.fromLambda（效果 lambda DSL）

`CK.Effect.fromLambda(fn, spec)` 用一个 **JS lambda 表达任意逐像素效果** —— SKSL 类效果与"溶解"类逐像素效果统一形态。lambda **只被编译、从不被执行**：引擎读取 `fn.toString()` 的源码，解析并重建计算，因此：

- lambda 必须是源码内联的箭头函数（不能是原生函数或被压缩过的代码）；
- 它在 JS 里永远不会被调用，任何合法 JS 之外的语法都会在编译期被拒绝；
- 像素缓冲永不跨进 JS（架构原则 §16 的延续）。

```js
var CK = ctx.CanvasKit;
var effect = CK.Effect.fromLambda(
  (uv, u) => {
    // uv = rect 本地像素坐标（像素中心 i+0.5）
    const g = uv.x / u.w;
    return [g, g, g, 1]; // straight RGBA（0..1），自动 premul 输出
  },
  { uniforms: [['w', 'float']] }
);

var paint = new CK.Paint();
paint.setShader(effect.makeShader([360.0]));
canvas.drawRect(CK.XYWHRect(0, 0, 360, 480), paint);
```

### 参数约定

| 参数 | 含义 |
|---|---|
| 第一参 | `uv`：rect 本地像素坐标（`i + 0.5`），float2 |
| 名为 `rect` | dst 的 `[x, y, w, h]`（画布坐标），float4 |
| 名为 `u` | uniforms 访问器：`u.<name>`，名字与类型来自 spec |
| 其余参数 | child shader（依声明序）：`child.eval(float2)` → float4 |

### spec

`{ uniforms: [['name', 'type'], ...], backend: 'auto' }` —— uniforms 是**保序数组**（顺序 = 打包顺序，必须与传参一致）；type ∈ float/float2/float3/float4。uniforms 传参支持对象形式（按名字）或平铺数组（按 spec 顺序，向量按分量展开）。

### 后端自动派发（Rust 自主决定）

| lambda 用到的 op | 后端 | 执行方式 |
|---|---|---|
| 纯浮点（sin/mix/smoothstep/…） | SKSL | codegen SKSL → RuntimeEffect（与手写 SKSL 同一条 wire，web CanvasKit 零改动） |
| `h01` / `imul` / `u32` / `i32`（整数精确语义，溶解类） | 纯 CPU | f64 树解释器逐像素（rayon）→ 生成图像（与溶解同一条 wire） |

`spec.backend` 可强制 `'sksl'` 或 `'cpu'`（强制 sksl 但用了 CpuOnly op 会报错）。两后端共享同一语义锚：所有用户算术是 f64（JS Number），`uv` 语义一致，child 采样空间一致。

### 白名单（允许）

- 数值/布尔字面量、2-4 元数组字面量（构造向量）
- 算术 `+ - * / %`、比较、`&& || !`、三目、`if/else`、`let/const`、赋值（仅 let 变量）
- stdlib 内建：`abs sign floor ceil fract min max clamp pow sqrt exp exp2 log log2 sin cos tan asin acos atan atan2 mix smoothstep step length distance dot cross normalize`（SKSL 可表达）
- 精确整数内建：`byte(x)`（JS `x|0` 截断）、`u32` / `i32`（ToUint32/ToInt32）、`imul`（Math.imul）、`h01(a,b,c)`（确定性像素哈希，返回 [0,1)）
- `Math.*` 中的纯函数（`Math.random` 等破坏确定性的被拒绝）
- 向量 swizzle 读（`.x/.y/.z/.w/.rgba…`，分量不越界）

### 拒绝（编译期报错，带 lambda 源码行列与摘录）

嵌套函数、闭包捕获、循环（for/while）、try/catch、裸代码块、对象字面量、字符串/模板串、位运算符（提示 `imul`/`h01`/`byte`）、`**`（提示 `pow`）、`new`/`this`、`Math.random`、对 child/u/Math/builtin 的非白名单用法、return 后的不可达语句。

### 逐像素效果 + field child（溶解形态）

大区域的 mask/距离场等"构建一次、逐帧采样"的数据，由引擎烘焙成帧级生成图像、以 generated child 传入 lambda（像素不进 JS）：

```js
surface.buildDissolve(x, y, w, h);      // 构建一次（mask + chamfer 距离场）
var fieldShader = surface.bakeDissolve('my-field');  // 每帧烘焙（pending 表逐帧清空）
// field 打包：R=mask(0..255)，G=dist 低字节，B=dist 高字节（dist 单位 px*3）
// lambda 内还原：byte(c.r*255+0.5)、byte(c.g*255+0.5)+256*byte(c.b*255+0.5)

var paint = new CK.Paint();
paint.setShader(effect.makeShaderWithChildren({ t: ctx.currentTime }, [fieldShader]));
canvas.drawRect(CK.XYWHRect(x, y, w, h), paint);
```

参考实现：`examples/k3-promo.xml` 场景 H（scramble dissolve 全量迁移到 lambda）。

---

## 推荐模板

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvasById('stage');

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
  p.setStrokeCap(CK.StrokeCap.Round);
  p.setStrokeJoin(CK.StrokeJoin.Round);
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

## 常用模式

### 粒子系统

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvasById('particles');

function hash(x, y) {
  var n = x * 374761393 + y * 668265263;
  n = (n ^ (n >> 13)) * 1274126177;
  return ((n ^ (n >> 16)) & 0x7fffffff) / 0x7fffffff;
}

var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Fill);

canvas.clear();

for (var i = 0; i < 100; i++) {
  var x = hash(i, 0) * 1280;
  var y = hash(i, 1) * 720;
  var size = hash(i, 2) * 5 + 1;
  var pulse = 0.35 + 0.65 * Math.sin(ctx.currentTime * 3 + i);
  paint.setColorComponents(1, 1, 1, 0.2 + 0.5 * pulse);
  canvas.drawCircle(x, y, size, paint);
}
```

### 路径绘制动画

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvasById('path-canvas');

var progress = ctx.sceneDuration > 0 ? Math.min(ctx.currentTime / ctx.sceneDuration, 1) : 1;
var path = new CK.Path();
path.moveTo(50, 100);
path.lineTo(200, 50);
path.lineTo(350, 100);

var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Stroke);
paint.setColor(CK.parseColorString('#38bdf8'));
paint.setStrokeWidth(4);
paint.setStrokeCap(CK.StrokeCap.Round);

var totalLen = 280;
var drawLen = Math.max(1, totalLen * progress);
paint.setPathEffect(CK.PathEffect.MakeDash([drawLen, totalLen], 0));

canvas.clear(CK.Color(10, 10, 10, 1));
canvas.drawPath(path, paint);
```

### 动态数据可视化

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvasById('chart');

var data = [65, 40, 85, 50, 70, 90, 55];
var barWidth = 40;
var gap = 20;
var maxHeight = 200;
var progress = ctx.sceneDuration > 0 ? Math.min(ctx.currentTime / ctx.sceneDuration, 1) : 1;

var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Fill);
paint.setColor(CK.parseColorString('#3b82f6'));

canvas.clear();

data.forEach(function(value, i) {
  var height = (value / 100) * maxHeight * progress;
  var x = i * (barWidth + gap) + 50;
  var y = 300 - height;
  canvas.drawRect(CK.XYWHRect(x, y, barWidth, height), paint);
});
```

### HTML in Canvas + RuntimeEffect

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvasById('surface');
var child = canvas.getSubTree().makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);

var sksl = [
  'uniform shader image;',
  'uniform float t;',
  'half4 main(float2 xy) {',
  '  float2 uv = xy + float2(sin(xy.y * 0.04 + t * 4.0) * 6.0, 0);',
  '  return image.eval(uv);',
  '}',
].join('\n');

var effect = CK.RuntimeEffect.Make(sksl);
if (effect) {
  var shader = effect.makeShaderWithChildren([ctx.currentTime], [child]);
  var paint = new CK.Paint();
  paint.setShader(shader);
  canvas.drawRect(CK.XYWHRect(0, 0, 360, 480), paint);
}
```

---

## 当前限制

- `ctx.getCanvas()` 不存在；必须使用 `ctx.getCanvasById(id)`。
- `clipRect()` / `clipPath()` / `clipRRect()` 只支持 `CK.ClipOp.Intersect`。
- `drawColor()` / `drawColorInt()` / `drawColorComponents()` 只支持 `CK.BlendMode.SrcOver`。
- `PathEffect` 只支持 `MakeDash()`。
- `Font` 只支持系统默认字体，不支持自定义 typeface。
- JS facade 暂未暴露 gradient shader、color filter、mask filter、image filter、完整 BlendMode。
- `paint.setShader()` 当前稳定用于 RuntimeEffect + `drawRect()`；不要依赖普通 image shader / picture shader 直接填充任意形状。
- Subtree picture shader 的 tile mode 当前不要依赖 repeat / mirror / decal。
- API 每帧重跑，不能依赖上一帧 canvas 内容累积；需要持久状态时用时间函数重新计算。
