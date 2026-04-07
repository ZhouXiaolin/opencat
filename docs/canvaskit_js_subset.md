# CanvasKit JS 子集约定

这个文件定义 OpenCat 脚本层采用的 CanvasKit JS 命名和对象模型。

## OpenCat 当前支持的标准子集

OpenCat 在脚本运行时暴露以下对象：

- `globalThis.CanvasKit`
- `ctx.CanvasKit`
- `ctx.getCanvas()`
- `ctx.getImage(assetId)`

### `CanvasKit`

当前支持：

- `CanvasKit.Color(r, g, b, a?)`
- `CanvasKit.Color4f(r, g, b, a?)`
- `CanvasKit.ColorAsInt(r, g, b, a?)`
- `CanvasKit.parseColorString(color)`
- `CanvasKit.multiplyByAlpha(color, alpha)`
- `CanvasKit.LTRBRect(left, top, right, bottom)`
- `CanvasKit.XYWHRect(x, y, width, height)`
- `CanvasKit.RRectXY(rect, rx, ry)`
- `new CanvasKit.Paint()`
- `new CanvasKit.Path()`
- `CanvasKit.PaintStyle.Fill`
- `CanvasKit.PaintStyle.Stroke`
- `CanvasKit.StrokeCap.Butt`
- `CanvasKit.StrokeCap.Round`
- `CanvasKit.StrokeCap.Square`
- `CanvasKit.StrokeJoin.Miter`
- `CanvasKit.StrokeJoin.Round`
- `CanvasKit.StrokeJoin.Bevel`
- `CanvasKit.ClipOp.Intersect`
- `CanvasKit.BLACK`
- `CanvasKit.WHITE`

### `Canvas`

`ctx.getCanvas()` 返回 CanvasKit 风格的 `Canvas` 子集，当前支持：

- `save()`
- `restore()`
- `translate(dx, dy)`
- `scale(sx, sy)`
- `rotate(degrees)`
- `rotate(degrees, rx, ry)`
- `clear(color?)`
- `clipRect(rect, CanvasKit.ClipOp.Intersect)`
- `drawRect(rect, paint)`
- `drawRRect(rrect, paint)`
- `drawCircle(cx, cy, radius, paint)`
- `drawLine(x0, y0, x1, y1, paint)`
- `drawPath(path, paint)`
- `drawImageRect(image, src, dest)`

### `Paint`

当前支持：

- `copy()`
- `delete()`
- `getColor()`
- `getStrokeCap()`
- `getStrokeJoin()`
- `getStrokeWidth()`
- `setAlphaf(alpha)`
- `setAntiAlias(aa)`
- `setColor(color)`
- `setColorComponents(r, g, b, a?)`
- `setColorInt(colorInt)`
- `setStrokeCap(cap)`
- `setStrokeJoin(join)`
- `setStrokeWidth(width)`
- `setStyle(style)`

### `Path`

当前支持：

- `copy()`
- `delete()`
- `moveTo(x, y)`
- `lineTo(x, y)`
- `quadTo(x1, y1, x2, y2)`
- `cubicTo(x1, y1, x2, y2, x3, y3)`
- `close()`

### 图片对象

图片不是通过完整 CanvasKit `Image` 加载链路进入脚本，而是通过 OpenCat 的资产系统进入：

- `const image = ctx.getImage("hero")`
- `canvas.drawImageRect(image, srcRect, destRect)`

注意：

- `ctx.getImage(assetId)` 是 OpenCat 扩展，不是原生 CanvasKit API。
- 目前 `drawImageRect()` 的 `src` 参数仅用于保持 CanvasKit 风格签名；OpenCat 当前实现按整张资产图像缩放到 `dest`，不做源矩形裁切。

## 推荐脚本模板

```js
const CK = ctx.CanvasKit;
const canvas = ctx.getCanvas();

function fill(color) {
  const paint = new CK.Paint();
  paint.setStyle(CK.PaintStyle.Fill);
  paint.setColor(CK.parseColorString(color));
  return paint;
}

function stroke(color, width = 1) {
  const paint = new CK.Paint();
  paint.setStyle(CK.PaintStyle.Stroke);
  paint.setColor(CK.parseColorString(color));
  paint.setStrokeWidth(width);
  return paint;
}

const path = new CK.Path();
path.moveTo(10, 10);
path.lineTo(60, 10);
path.lineTo(60, 40);
path.close();

canvas.clear(CK.WHITE);
canvas.drawRect(CK.XYWHRect(0, 0, 120, 80), fill("#0f172a"));
canvas.drawPath(path, stroke("#38bdf8", 2));
canvas.drawCircle(80, 40, 12, fill("#f8fafc"));
```

## 给 LLM 的生成规则

如果要让 LLM 为 OpenCat 生成脚本，应明确约束它：

- 只使用本文件列出的 API。
- 优先使用 `const CK = ctx.CanvasKit;`。
- 所有几何图元都用 `drawRect/drawRRect/drawCircle/drawLine/drawPath`。
- 所有样式都通过 `CanvasKit.Paint` 表达。
- 所有路径都通过 `CanvasKit.Path` 表达。
- 图片统一走 `ctx.getImage(assetId)` + `drawImageRect(...)`。
