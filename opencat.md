# OpenCat JSONL

> **⚠️ Important format rules**
> - **One JSON object per line**. Do not split a single JSON object across multiple lines.
> - **Do not put comments inside script content**. Script code must stay clean.

OpenCat JSONL is a JSON Lines format for describing compositions, scene nodes, scripts, and transitions.

---

## 1. File Structure

### 1.1 Composition (first line, required)

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 180}
```

`frames / fps` defines the total duration in seconds.

### 1.2 Two Modes: Single Scene vs Multi Scene

#### Single Scene

There is one root node with `parentId: null`, and no transition. `composition.frames` must match that scene's `duration`.

```text
Timeline: [   scene1: 60 frames   ]
Constraint: composition.frames = scene1.duration
```

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 60}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold", "text": "Hello"}
```

#### Multi Scene + Transition

There can be multiple root nodes with `parentId: null`. Scenes are connected by `transition` records. Each scene has its own independent node tree. Transitions consume extra frames and overlap the two scenes during the handoff.

```text
Timeline: [ scene1: 60 frames ] [ fade: 12 frames ] [ scene2: 90 frames ]
Constraint: composition.frames = 60 + 12 + 90 = 162
```

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 162}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Part 1"}
{"id": "scene2", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-slate-900", "duration": 90}
{"id": "subtitle", "parentId": "scene2", "type": "text", "className": "text-[20px] font-semibold text-white", "text": "Part 2"}
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

**Key rules**:

- Each scene owns its own node tree. Nodes are not shared across scenes.
- `composition.frames = sum(all scene.duration) + sum(all transition.duration)`
- Transitions are chained in order: `scene1 -> transition(scene1->scene2) -> scene2 -> ...`

### 1.3 Element Nodes

Each element is one JSON line. Parent-child relationships are defined through `parentId`.

`className` uses Tailwind-style classes for layout and visual properties, similar to how you would style an HTML node with Tailwind.

```json
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Hello"}
{"id": "hero", "parentId": "scene1", "type": "image", "className": "w-[300px] h-[200px] object-cover rounded-lg", "query": "mountain landscape"}
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] text-slate-400", "icon": "search"}
```

**Type mapping**:

| type | HTML equivalent | Special fields |
|------|-----------------|----------------|
| `div` | `<div>` | — |
| `text` | `<span>` / `<p>` | `text`: text content |
| `image` | `<img>` | `query`: image search query (1-4 nouns) |
| `icon` | Lucide icon | `icon`: icon name in kebab-case |
| `canvas` | `<canvas>` | requires a matching script |
| `audio` | `<audio>` | `path` or `url` |
| `video` | `<video>` | — |

### 1.4 Script

> **⚠️ `script.src` must not contain comments**

Scripts are attached to nodes and run on every frame.

```json
{"type": "script", "parentId": "scene1", "src": "var node = ctx.getNode('title');\nvar anim = ctx.animate({from:{opacity:0},to:{opacity:1},duration:20,easing:'spring-gentle'});\nnode.opacity(anim.opacity);"}
{"type": "script", "parentId": "scene1", "path": "scene1.js"}
```

### 1.5 Transition

Transitions are only used in multi-scene mode. A transition describes the handoff between two scenes and consumes additional frames.

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

**Effect types** (`effect` and `direction` are separate fields):

| effect | Description | direction (optional) |
|--------|-------------|----------------------|
| `fade` | Cross fade | — |
| `slide` | Sliding transition | `from_left` (default) / `from_right` / `from_top` / `from_bottom` |
| `wipe` | Wipe transition | `from_left` (default) / `from_right` / `from_top` / `from_bottom` / `from_top_left` / `from_top_right` / `from_bottom_left` / `from_bottom_right` |
| `clock_wipe` | Clock wipe | — |
| `iris` | Iris open/close | — |
| `light_leak` | Light leak | — (`seed`, `hueShift`, `maskScale` are supported) |

**Timing control** (available for all effects):

`timing` uses the same easing names as `ctx.animate()`. The default is `"linear"`.

| timing | Description |
|--------|-------------|
| `"linear"` (default) | Constant speed |
| `"ease"` | CSS ease |
| `"ease-in"` | Ease in |
| `"ease-out"` | Ease out |
| `"ease-in-out"` | Ease in and out |
| `"spring-default"` / `"spring-gentle"` / … | Spring presets |
| `"bezier:x1,y1,x2,y2"` | Cubic bezier |

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 20, "timing": "ease-out"}
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "slide", "direction": "from_right", "duration": 15, "timing": "bezier:0.4,0,0.2,1"}
```

Custom spring parameters can also be used directly through `damping`, `stiffness`, and `mass`:

```json
{"type": "transition", "from": "scene1", "to": "scene2", "effect": "wipe", "direction": "from_right", "duration": 12, "damping": 10, "stiffness": 100, "mass": 1}
```

---

## 2. Styling (Tailwind)

Most Tailwind classes work directly for layout, color, spacing, border radius, and related visual styling.

**Main restrictions**:

- Do not use CSS animation classes
- Do not generate transform-related Tailwind classes in `className`
  - This includes classes such as `transform`, `translate-*`, `rotate-*`, `scale-*`, and `skew-*`
  - Use the script node API instead for transforms

| Avoid | Use instead |
|------|-------------|
| `transition-*` `animate-*` `duration-*` `ease-*` `delay-*` | `ctx.animate()` / `ctx.stagger()` / `ctx.sequence()` |
| `transform` `translate-*` `rotate-*` `scale-*` `skew-*` | `ctx.getNode(...).translateX()` / `translateY()` / `scale()` / `rotate()` / `skew()` |

> Tailwind handles static styling. Scripts handle motion.

---

## 3. Animation System

Animations are declared in JavaScript. Scripts run on every frame and read interpolated animation values to drive node properties.

### Context

| Field | Description |
|------|-------------|
| `ctx.frame` | Global frame index |
| `ctx.totalFrames` | Total frame count |
| `ctx.currentFrame` | Frame index within the current scene (`0 -> sceneFrames - 1`) |
| `ctx.sceneFrames` | Frame count of the current scene |

For scene-local animation, prefer `ctx.currentFrame` and `ctx.sceneFrames`.

### ctx.animate(opts)

Declare a `from -> to` animation. The returned object exposes animated values through getters.

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

Additional fields:

- `anim.progress`: progress from `0` to `1`
- `anim.settled`: whether a spring animation has settled
- `anim.settleFrame`: the frame where the spring settled

### ctx.stagger(count, opts)

Like `animate`, but creates multiple animations with staggered delay.

```js
var anims = ctx.stagger(4, {
  from: { opacity: 0, translateY: 30 },
  to:   { opacity: 1, translateY: 0 },
  gap: 4,
  duration: 20,
  easing: 'spring-gentle',
});
```

### ctx.sequence(steps)

Declare a heterogeneous chain of animations. Each step advances an internal cursor so per-step `duration`, `easing`, `from`, and `to` can differ. This is the right tool when `ctx.stagger` (same animation, uniform gap) is not expressive enough — irregular timing, overlaps, or parallel branches.

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

**Per-step fields**:

| Field | Default | Description |
|-------|---------|-------------|
| `from`, `to`, `duration`, `easing`, `clamp` | — | Same as `ctx.animate()` |
| `delay` | `0` | Extra offset added to the cursor before this step starts |
| `gap` | `0` | Advance the cursor by this many frames after this step ends. Negative to overlap with the next step. |
| `at` | — | Absolute start frame. When set, this step ignores the cursor and **does not advance it**. Useful for parallel branches or pinned anchors. |

Each returned item exposes the same getters as `ctx.animate()` (`progress`, `settled`, `settleFrame`, plus every animated key).

**Parallel branches with `at`**:

```js
var seq = ctx.sequence([
  { to: { opacity: 1 }, duration: 20 },
  { to: { opacity: 1 }, duration: 30, at: 5 },
  { to: { opacity: 1 }, duration: 10 },
]);
```

Step 0 runs `0..20` and advances cursor to `20`. Step 1 is pinned at frame `5` (runs `5..35`) and the cursor is untouched. Step 2 starts from the cursor at `20` and runs `20..30`.

**When to pick which**:

| Use case | API |
|----------|-----|
| Single animation | `ctx.animate` |
| N identical animations, uniform gap | `ctx.stagger` |
| Heterogeneous steps, irregular gaps, overlaps, parallel branches | `ctx.sequence` |

### ctx.typewriter(fullText, opts)

Type out a string character by character, driven by an animation curve. Returns an object whose `text` getter produces the current substring for the given frame.

```js
var tw = ctx.typewriter('Hello OpenCat', {
  duration: 30,
  delay: 6,
  easing: 'linear',
  caret: '▍',
});

ctx.getNode('title').text(tw.text);
```

**Options**:

| Field | Default | Description |
|-------|---------|-------------|
| `duration` | — | Required. Frames from empty to full string. |
| `delay` | `0` | Frames to wait before typing starts. |
| `easing` | `'linear'` | Any easing supported by `ctx.animate()`. Non-linear varies typing speed. |
| `clamp` | `true` | Prevents spring/bezier overshoot from producing out-of-range character counts. |
| `caret` | `''` | String appended while typing is in progress. Disappears once the full text is revealed. |

Also exposes `progress`, `settled`, and `settleFrame` like `ctx.animate()`.

Character counting uses `Array.from()`, so the effect is **grapheme-safe for CJK and emoji** — no broken surrogates.

### Easing

| Preset | Effect |
|--------|--------|
| `'linear'` | Constant speed |
| `'spring-default'` | General spring |
| `'spring-gentle'` | Soft spring |
| `'spring-stiff'` | Stiffer spring |
| `'spring-slow'` | Slower spring |
| `'spring-wobbly'` | Wobbly spring |

Custom spring:

```js
easing: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

Cubic bezier:

```js
easing: [0.25, 0.1, 0.25, 1.0]
```

### Animating Colors

`ctx.animate()` automatically interpolates color values when `from` or `to` is a string. Colors are converted to HSLA, the hue is interpolated along the shortest arc (handling 360->0 wrap-around), and the result is fed back as an `rgba(...)` string compatible with `node.bg()`, `node.textColor()`, `node.borderColor()`, etc.

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

Supported color literals (in `from` / `to`):

- `#rgb` / `#rrggbb` / `#rrggbbaa`
- `rgb(r, g, b)` / `rgba(r, g, b, a)`
- `hsl(h, s%, l%)` / `hsla(h, s%, l%, a)`

Colors are always clamped (`progress` outside `[0, 1]` from spring overshoot is normalised back into range), so spring easing won't push values out of the visible gamut.

> Tailwind tokens like `'blue-500'` remain as discrete `node.bg(token)` calls — they are **not** interpolated. To animate, write the color as hex/rgb/hsl in `from` / `to`.

### Node API

`ctx.getNode('id')` returns a chainable proxy object.

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

// Content (text nodes only — overrides the JSONL `text` field for the current frame)
node.text('Hello world');
```

### Common Patterns

**Staggered entrance**:

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

**Linked motion**:

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

**Looping pulse**:

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

### Restrictions

- Do not use `document`, `window`, `requestAnimationFrame`, or `element.style`
- Access nodes only through `ctx.getNode()`
- `duration` is required for non-spring easing

---

## 4. Canvas (CanvasKit-style subset)

A `type: "canvas"` node behaves like a canvas surface, but only supports the CanvasKit subset currently exposed by OpenCat. The drawing script must be attached as a child script of that canvas node and is re-executed on every frame.

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

### Entry Points

| Object | Purpose |
|--------|---------|
| `ctx.CanvasKit` / `globalThis.CanvasKit` | CanvasKit-style helpers, constructors, and enums |
| `ctx.getCanvas()` | Returns the drawing interface for the current canvas node |
| `ctx.getImage(assetId)` | Returns an image handle for a host-provided asset id |

### Supported CanvasKit Helpers

```js
var CK = ctx.CanvasKit;

// Color
CK.Color(r, g, b, a?)
CK.Color4f(r, g, b, a?)
CK.ColorAsInt(r, g, b, a?)
CK.parseColorString('#ff0000')
CK.multiplyByAlpha(color, 0.5)

// Geometry
CK.LTRBRect(l, t, r, b)
CK.XYWHRect(x, y, w, h)
CK.RRectXY(rect, rx, ry)

// Constructors
new CK.Paint()
new CK.Path()
new CK.Font(null, size?, scaleX?, skewX?)
CK.PathEffect.MakeDash(intervals, phase?)

// Enums / constants
CK.BLACK / CK.WHITE
CK.PaintStyle.Fill / CK.PaintStyle.Stroke
CK.StrokeCap.Butt / Round / Square
CK.StrokeJoin.Miter / Round / Bevel
CK.FontEdging.Alias / AntiAlias / SubpixelAntiAlias
CK.BlendMode.SrcOver
CK.ClipOp.Intersect / Difference
CK.PointMode.Points / Lines / Polygon
```

### Supported `ctx.getCanvas()` Methods

Methods are chainable unless noted otherwise.

```js
var canvas = ctx.getCanvas();

// State and transforms
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

// Clipping
canvas.clipRect(rect, CK.ClipOp.Intersect, doAntiAlias?);
canvas.clipPath(path, CK.ClipOp.Intersect, doAntiAlias?);
canvas.clipRRect(rrect, CK.ClipOp.Intersect, doAntiAlias?);

// Shapes
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

// Images
canvas.drawImage(image, x, y, paint?);
canvas.drawImageRect(image, srcRect, destRect, paint?, fastSample?);

// Text
canvas.drawText(text, x, y, paint, font);
```

### `Paint` Support

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

Only dash path effects are currently supported.

### `Path` Support

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

### Text API Support

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

Current constraints:

- `typeface` must currently be `null`, which means the system default font
- Custom font objects, `Typeface`, `FontMgr`, and font assets are not supported
- `TextBlob` and `Paragraph` are not supported

### Image Resource Rules

Canvas scripts must acquire images through `ctx.getImage(assetId)`. URLs, file paths, and arbitrary native image objects are not accepted.

```js
var img = ctx.getImage('hero-asset');
canvas.drawImage(img, 40, 40);
canvas.drawImageRect(
  img,
  CK.XYWHRect(0, 0, 320, 180),
  CK.XYWHRect(40, 40, 160, 90)
);
```

### Current Explicit Limits

- This is a CanvasKit subset, not full CanvasKit
- `clipRect()`, `clipPath()`, and `clipRRect()` currently only support `CK.ClipOp.Intersect`
- `drawColor()`, `drawColorInt()`, and `drawColorComponents()` currently only support `CK.BlendMode.SrcOver`
- `PathEffect` currently only supports `MakeDash()`
- Text drawing only supports the system default font
- `ctx.getImage()` only accepts asset id handles

### Recommended Template

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

## 5. Common Errors

| Wrong | Correct |
|------|---------|
| `type: "div"` with a `text` field | Only `type: "text"` accepts `text` |
| Coloring icons with `bg-{color}` | Use `text-{color}` for icons |
| `id` contains "icon" but `type: "div"` | Use `type: "icon"` with a Lucide icon name |
| Image `query` contains adjectives | Use only 1-4 nouns |
| Relying on `absolute` layout by default | Prefer flex layout; use `absolute` only for overlap or pinned edges |
| Putting transform Tailwind classes in `className` | Use node transform APIs such as `translateX()`, `translateY()`, `scale()`, `rotate()`, and `skew()` |
| `parentId` points to an invalid id | `parentId` must reference an existing node |
| Frame count mismatch | `composition.frames == sum(scene.duration) + sum(transition.duration)` |
| `"effect": "slide-left"` | Use separate fields: `"effect": "slide", "direction": "from_left"` |
