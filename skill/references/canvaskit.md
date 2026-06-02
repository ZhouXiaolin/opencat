# CanvasKit 子集

`canvas` 节点提供 CanvasKit 风格的绘制表面。绘制脚本必须是 canvas 节点的子 `script`，每帧重新执行。

---

## 入口点

| 对象 | 用途 |
|------|------|
| `ctx.CanvasKit` | 辅助函数、构造函数、枚举 |
| `ctx.getCanvas()` | 绘制接口 |
| `ctx.getImage(assetId)` | 图像句柄 |

---

## CanvasKit 辅助函数

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

// 枚举/常量
CK.BLACK / CK.WHITE
CK.PaintStyle.Fill / CK.PaintStyle.Stroke
CK.StrokeCap.Butt / Round / Square
CK.StrokeJoin.Miter / Round / Bevel
CK.FontEdging.Alias / AntiAlias / SubpixelAntiAlias
CK.BlendMode.SrcOver
CK.ClipOp.Intersect / Difference
CK.PointMode.Points / Lines / Polygon
```

---

## Canvas 方法

```js
var canvas = ctx.getCanvas();

// 状态和变换
canvas.clear(color?);
canvas.save();
canvas.restore();
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
canvas.drawRect(rect, paint);
canvas.drawRRect(rrect, paint);
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

// 文字
canvas.drawText(text, x, y, paint, font);
```

---

## Paint

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

---

## Path

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

---

## 文字

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

---

## 图像资源

```js
var img = ctx.getImage('hero-asset');
canvas.drawImage(img, 40, 40);
canvas.drawImageRect(
  img,
  CK.XYWHRect(0, 0, 320, 180),
  CK.XYWHRect(40, 40, 160, 90)
);
```

---

## 推荐模板

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

## 常用模式

### 粒子系统

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();

function hash(x, y) {
  var n = x * 374761393 + y * 668265263;
  n = (n ^ (n >> 13)) * 1274126177;
  return ((n ^ (n >> 16)) & 0x7fffffff) / 0x7fffffff;
}

var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Fill);

for (var i = 0; i < 100; i++) {
  var x = hash(i, 0) * 1920;
  var y = hash(i, 1) * 1080;
  var size = hash(i, 2) * 5 + 1;
  // 循环动画使用 ctx.frame
  paint.setColorComponents(1, 1, 1, hash(i, ctx.frame * 0.1));
  canvas.drawCircle(x, y, size, paint);
}
```

### 路径绘制动画

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();

var progress = Math.min(ctx.currentFrame / ctx.sceneFrames, 1);
var path = new CK.Path();
path.moveTo(50, 100);
path.lineTo(200, 50);
path.lineTo(350, 100);

var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Stroke);
paint.setColor(CK.parseColorString('#c84f1c'));
paint.setStrokeWidth(4);
paint.setStrokeCap(CK.StrokeCap.Round);

var totalLen = 280;
var drawLen = totalLen * progress;
paint.setPathEffect(CK.PathEffect.MakeDash([drawLen, totalLen], 0));

canvas.clear(CK.Color(10, 10, 10, 1));
canvas.drawPath(path, paint);
```

### 动态数据可视化

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();

var data = [65, 40, 85, 50, 70, 90, 55];
var barWidth = 40;
var gap = 20;
var maxHeight = 200;

data.forEach(function(value, i) {
  var height = (value / 100) * maxHeight * Math.min(ctx.currentFrame / ctx.sceneFrames, 1);
  var x = i * (barWidth + gap) + 50;
  var y = 300 - height;

  var paint = new CK.Paint();
  paint.setStyle(CK.PaintStyle.Fill);
  paint.setColor(CK.parseColorString('#3b82f6'));
  canvas.drawRect(CK.XYWHRect(x, y, barWidth, height), paint);
});
```

---

## 限制

- `clipRect()`/`clipPath()`/`clipRRect()` — 仅 `CK.ClipOp.Intersect`
- `drawColor()`/`drawColorInt()`/`drawColorComponents()` — 仅 `CK.BlendMode.SrcOver`
- `PathEffect` — 仅 `MakeDash()`
- 文字 — 仅系统默认字体
- `ctx.getImage()` — 仅资源 id 句柄
