# OpenCat JSONL

> **Format rules**
> - **One JSON object per line.** Do not split a single JSON object across multiple lines.
> - **No comments inside script content.** Script code must stay clean.

OpenCat JSONL is a JSON Lines format for describing motion graphics compositions. Each line is a node declaration, script attachment, or metadata record. The runtime parses the file, builds a scene tree, and renders frames using Skia + Taffy + QuickJS.

---

## 1. Composition Header

The first line must be a `composition` record.

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 180}
```

| Field | Type | Description |
|-------|------|-------------|
| `width` | `i32` | Canvas width in pixels |
| `height` | `i32` | Canvas height in pixels |
| `fps` | `i32` | Frames per second |
| `frames` | `i32` | Total frame count. `frames / fps` = duration in seconds. |

---

## 2. Node Tree

### 2.1 Parent-Child Relationships

Every node (except `composition` and `script`/`transition`) has an `id` and a `parentId`. The tree is built from these links.

- Exactly one root node must have `parentId: null`.
- `parentId` must reference a previously declared `id`.
- `script` and `transition` records have no `id`; they attach to their `parentId`.

### 2.2 Plain Tree (Single Scene)

For single scenes, static overlays, or any composition without scene-to-scene transitions.

```json
{"type": "composition", "width": 390, "height": 844, "fps": 30, "frames": 60}
{"id": "scene1", "parentId": null, "type": "div", "className": "flex flex-col w-[390px] h-[844px] bg-white", "duration": 60}
{"id": "title", "parentId": "scene1", "type": "text", "className": "text-[24px] font-bold", "text": "Hello"}
```

### 2.3 Timeline (Multi-Scene with Transitions)

For two or more scenes with transitions between them.

```json
{"type":"composition","width":390,"height":844,"fps":30,"frames":162}
{"id":"root","parentId":null,"type":"div","className":"relative w-[390px] h-[844px]"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene1","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-white","duration":60}
{"id":"scene2","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-slate-900","duration":90}
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"fade","duration":12}
```

Rules:

- `tl` must be an explicit node in the tree. Root-level multi-scene inference is not supported.
- `tl` follows `NodeStyle` — layout and scripts on the `tl` node itself are preserved.
- A `tl` must have at least two direct child scenes, and every adjacent pair must have a matching `transition`.
- `tl` has no `duration` field. Its total is derived: `sum(scene.duration) + sum(transition.duration)`.
- `transition.parentId` is required and must reference the owning `tl` node.
- Place `tl` and persistent overlays (e.g. `caption`) as siblings under a shared parent `div` for z-order compositing.
- Keep `composition.frames` aligned with the derived total.

---

## 3. Node Types

Every element is one JSON line. `className` uses Tailwind-style classes (see §5 Styling).

### 3.1 `div`

Container with flex layout. Equivalent to `<div>`.

```json
{"id": "box", "parentId": "root", "type": "div", "className": "flex flex-col items-center gap-4 p-6"}
```

No special fields beyond `id`, `parentId`, `className`, `duration`.

### 3.2 `text`

Text content node. Equivalent to `<span>` / `<p>`.

```json
{"id": "title", "parentId": "box", "type": "text", "className": "text-[24px] font-bold text-slate-900", "text": "Hello"}
```

| Field | Required | Description |
|-------|----------|-------------|
| `text` | yes | Text content string |

### 3.3 `image`

Image node. Equivalent to `<img>`.

```json
{"id": "hero", "parentId": "scene1", "type": "image", "className": "w-[300px] h-[200px] object-cover rounded-lg", "query": "mountain landscape"}
```

Specify exactly one image source:

| Field | Description |
|-------|-------------|
| `path` | Local file path |
| `url` | Remote URL |
| `query` | Openverse search query (1-4 nouns) |

When using `query`, optional fields:

| Field | Default | Description |
|-------|---------|-------------|
| `queryCount` | `1` | Number of images to fetch |
| `aspectRatio` | — | Aspect ratio filter (e.g. `"square"`) |

### 3.4 `icon`

Lucide icon node. Uses kebab-case icon names.

```json
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] stroke-slate-400", "icon": "search"}
```

| Field | Required | Description |
|-------|----------|-------------|
| `icon` | yes | Lucide icon name in kebab-case |

Use standard SVG Tailwind utilities:
- `stroke-{color}` / `stroke-[#hex]` — icon stroke color (default Black)
- `stroke-0` / `stroke-1` / `stroke-2` — icon stroke width (default 2)
- `stroke-[n]` — arbitrary stroke width
- `fill-{color}` / `fill-[#hex]` — icon fill (default none)

### 3.5 `path`

SVG path node. Renders one or more SVG path data strings using dedicated fill/stroke styling.

```json
{"id": "triangle", "parentId": "scene1", "type": "path", "className": "w-[100px] h-[100px] fill-red-500 stroke-blue stroke-2", "d": "M0 0 L100 0 L50 100 Z"}
```

| Field | Required | Description |
|-------|----------|-------------|
| `d` | yes | SVG path data string |

Styled with the same SVG Tailwind utilities as `icon`:
- `fill-{color}` / `fill-[#hex]` — fill color (default none)
- `stroke-{color}` / `stroke-[#hex]` — stroke color (default none)
- `stroke-0` / `stroke-1` / `stroke-2` / `stroke-[n]` — stroke width

Unlike `icon`, `path` has no default intrinsic size — set `w`/`h` via `className` or use layout.

### 3.6 `canvas`

Canvas drawing surface. Requires a child `script` for drawing commands.

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

See §6 Canvas API for the full drawing reference.

### 3.7 `audio`

Audio playback node. Equivalent to `<audio>`.

```json
{"id": "bgm", "parentId": "root", "type": "audio", "path": "/tmp/bgm.mp3"}
{"id": "sfx", "parentId": "root", "type": "audio", "url": "https://example.com/sfx.mp3"}
```

Specify exactly one source: `path` (local) or `url` (remote).

The `parentId` controls when the audio plays:
- Attached under a scene node → plays during that scene.
- `parentId: null` → plays for the entire composition (timeline-level).

### 3.8 `video`

Video playback node. Equivalent to `<video>`.

```json
{"id": "clip", "parentId": "scene1", "type": "video", "className": "w-full h-full object-cover", "path": "clip.mp4"}
```

| Field | Required | Description |
|-------|----------|-------------|
| `path` | yes | Local video file path |

### 3.9 `caption`

SRT-driven text node. Displayed content is selected from subtitle entries using the nearest inherited time context.

```json
{"id": "subs", "parentId": "root", "type": "caption", "className": "absolute inset-x-[48px] bottom-[32px] text-center text-white", "path": "subtitles.utf8.srt"}
```

| Field | Required | Description |
|-------|----------|-------------|
| `path` | yes | Local SRT file path |
| `duration` | no | Usually omitted; visibility is driven by SRT timestamps |

Implementation notes:

- `path` is resolved relative to the JSONL file location when loaded with `parse_file(...)`.
- SRT timestamps are converted to frames using `composition.fps`.
- Inside a timeline scene, `caption` uses that scene's local frame context.
- The loader reads subtitle files as UTF-8. UTF-16 / GBK files will not parse correctly.
- Read/parse failure degrades to an empty subtitle track. If captions do not appear, check path and encoding first.
- Caption content can be overridden per-frame by scripts: `ctx.getNode('subs').text(...)`.

### 3.10 `tl`

Timeline container. See §2.3 for full specification.

| Field | Description |
|-------|-------------|
| `id` | Required |
| `parentId` | Parent node |
| `className` | Tailwind styling |

No `duration` field — the total is derived from child scenes and transitions.

### 3.11 `transition`

Transition between two adjacent scenes inside a `tl`. See §4 for full specification.

---

## 4. Transitions

Transitions describe the handoff between two adjacent scenes inside a `tl` node. They consume additional frames.

```json
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 12}
```

| Field | Required | Description |
|-------|----------|-------------|
| `parentId` | yes | Must reference the owning `tl` node |
| `from` | yes | Source scene id (must be a direct child of the `tl`) |
| `to` | yes | Target scene id (must be adjacent to `from`) |
| `effect` | yes | Effect name (see below) |
| `duration` | yes | Transition duration in frames |
| `direction` | no | Direction for `slide` / `wipe` effects |
| `timing` | no | Easing name (default `"linear"`). See §5.1. |
| `damping` | no | Custom spring damping |
| `stiffness` | no | Custom spring stiffness |
| `mass` | no | Custom spring mass |

### Effect Types

| effect | Description | direction (optional) |
|--------|-------------|----------------------|
| `fade` | Cross fade | — |
| `slide` | Sliding transition | `from_left` (default) / `from_right` / `from_top` / `from_bottom` |
| `wipe` | Wipe transition | `from_left` (default) / `from_right` / `from_top` / `from_bottom` / `from_top_left` / `from_top_right` / `from_bottom_left` / `from_bottom_right` |
| `clock_wipe` | Clock wipe | — |
| `iris` | Iris open/close | — |
| `light_leak` | Light leak | — |

`light_leak` extra fields: `seed` (`f32`), `hueShift` (`f32`), `maskScale` (`f32`, range `0.03125`–`1.0`).

### Examples

```json
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "fade", "duration": 20, "timing": "ease-out"}
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "slide", "direction": "from_right", "duration": 15, "timing": "bezier:0.4,0,0.2,1"}
{"type": "transition", "parentId": "main-tl", "from": "scene1", "to": "scene2", "effect": "wipe", "direction": "from_right", "duration": 12, "damping": 10, "stiffness": 100, "mass": 1}
```

---

## 5. Styling (Tailwind)

`className` uses Tailwind-style classes for layout, color, spacing, border radius, and related visual properties.

**Restrictions:**

- Do not use CSS animation classes (`transition-*`, `animate-*`, `duration-*`, `ease-*`, `delay-*`).
- Do not use transform classes in `className` (`transform`, `translate-*`, `rotate-*`, `scale-*`, `skew-*`). Use the script Node API instead.

| Avoid | Use instead |
|------|-------------|
| `transition-*` `animate-*` `duration-*` `ease-*` `delay-*` | `ctx.to()` / `ctx.from()` / `ctx.fromTo()` / `ctx.timeline()` |
| `transform` `translate-*` `rotate-*` `scale-*` `skew-*` | `ctx.getNode(...).translateX()` / `translateY()` / `scale()` / `rotate()` / `skew()` |

> Tailwind handles static styling. Scripts handle motion.

### 5.1 Easing Reference

Easing names are shared by `ctx.to()` / `ctx.from()` / `ctx.fromTo()` / `ctx.timeline()` and `transition.timing`.

| Preset | Effect |
|--------|--------|
| `'linear'` | Constant speed |
| `'ease'` / `'ease-in'` / `'ease-out'` / `'ease-in-out'` | Standard CSS-like cubic curves |
| `'back-in'` / `'back-out'` / `'back-in-out'` | Slight overshoot (UI snap) |
| `'elastic-in'` / `'elastic-out'` / `'elastic-in-out'` | Damped oscillation |
| `'bounce-in'` / `'bounce-out'` / `'bounce-in-out'` | Ground-bounce style |
| `'steps(N)'` | Quantised into N discrete steps |
| `'spring.default'` / `'spring-default'` | General spring |
| `'spring.gentle'` / `'spring-gentle'` | Soft spring |
| `'spring.stiff'` / `'spring-stiff'` | Stiffer spring |
| `'spring.slow'` / `'spring-slow'` | Slower spring |
| `'spring.wobbly'` / `'spring-wobbly'` | Wobbly spring |

Custom spring (JS):

```js
ease: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

Cubic bezier (JS):

```js
ease: [0.25, 0.1, 0.25, 1.0]
```

Transition `timing` field also accepts a string form: `"bezier:0.4,0,0.2,1"`.

---

## 6. Scripting

Scripts are attached to nodes via `script` records and run on every frame using QuickJS.

```json
{"type": "script", "parentId": "scene1", "src": "ctx.fromTo('title',{opacity:0},{opacity:1,duration:20,ease:'spring.gentle'});"}
{"type": "script", "parentId": "scene1", "path": "scene1.js"}
```

| Field | Description |
|-------|-------------|
| `parentId` | Optional. Attach to a node scope, or omit for global scope. |
| `src` | Inline JavaScript code |
| `path` | External `.js` file path (resolved relative to the JSONL file) |

`src` and `path` are mutually exclusive. Exactly one is required.

### 6.1 Execution Context

| Field | Description |
|------|-------------|
| `ctx.frame` | Global frame index |
| `ctx.totalFrames` | Total frame count |
| `ctx.currentFrame` | Frame index within the current scene (`0 → sceneFrames - 1`) |
| `ctx.sceneFrames` | Frame count of the current scene |

For scene-local animation, prefer `ctx.currentFrame` and `ctx.sceneFrames`.

### 6.2 Design: Precise Mathematical Computation

OpenCat's animation system is **functionally pure**: every animated value is computed as `value = f(current_frame)` through exact mathematical formulas. There is no internal tick loop, no accumulated state, and no non-deterministic drift.

- **Interpolation**: linear `from + (to - from) * easing(progress)`
- **Spring**: solved from physical parameters (`stiffness`, `damping`, `mass`) with exact settle-time detection
- **Color**: HSLA space with shortest-path hue rotation (handles 360° wrap-around)
- **Path**: Skia `ContourMeasure` for sub-pixel-accurate arc-length sampling

Scripts are re-executed every frame. The GSAP-like API declares tweens and timelines, but the runtime still samples them as pure functions of the current frame.

---

### 6.3 GSAP-Style Tween API

The public animation API is intentionally small:

```js
ctx.set(targets, vars);
ctx.to(targets, vars);
ctx.from(targets, vars);
ctx.fromTo(targets, fromVars, toVars);
```

`targets` accepts a node id, an array of node ids, or `ctx.splitText(...)` parts. The API shape follows GSAP, but each call is sampled from the current frame and remains deterministic.

```js
ctx.fromTo('hero',
  { opacity: 0, y: 40, scale: 0.95 },
  { opacity: 1, y: 0, scale: 1, duration: 30, ease: 'spring.gentle' }
);
```

Common property aliases:

| Property | Applies to |
|----------|------------|
| `opacity` | Visual opacity |
| `x`, `y` | `translateX`, `translateY` |
| `scale`, `scaleX`, `scaleY` | Transform scale |
| `rotate`, `rotation` | Rotation in degrees |
| `skewX`, `skewY` | Skew in degrees |
| `left`, `top`, `right`, `bottom`, `width`, `height` | Layout dimensions |
| `backgroundColor`, `bg` | Background color |
| `color`, `textColor` | Text color |
| `borderColor`, `borderRadius`, `borderWidth` | Border style |
| `fillColor`, `strokeColor`, `strokeWidth` | SVG/icon/path paint |
| `text` | Text content layer, revealed with grapheme-safe typewriter semantics |

Timing fields live in the destination vars:

| Field | Default | Description |
|-------|---------|-------------|
| `duration` | required for non-spring | Duration in frames |
| `delay` | `0` | Start offset in frames |
| `ease` / `easing` | `'linear'` | Easing name, bezier array, or spring object |
| `repeat` | `0` | Additional cycles. `-1` = infinite |
| `yoyo` | `false` | Reverse alternate cycles |
| `repeatDelay` | `0` | Hold between repeated cycles |
| `stagger` | `0` | Per-target delay offset for arrays or split-text parts |

Returned tween objects expose `progress`, `settled`, `settleFrame`, `values`, and each sampled property directly:

```js
var hero = ctx.fromTo('title', { opacity: 0, y: 40 }, { opacity: 1, y: 0, duration: 20 });
ctx.getNode('subtitle').opacity(hero.opacity * 0.8).translateY(hero.y * 0.5);
```

### 6.4 Colors, Paths, And Keyframes

Color properties are interpolated in HSLA space with shortest-path hue rotation:

```js
ctx.fromTo('card',
  { backgroundColor: '#ef4444' },
  { backgroundColor: 'hsl(220, 90%, 55%)', duration: 60, repeat: -1, yoyo: true }
);
```

Supported color literals: `#rgb` / `#rrggbb` / `#rrggbbaa`, `rgb(...)` / `rgba(...)`, `hsl(...)` / `hsla(...)`. Tailwind color tokens are not interpolated; use explicit color literals in tweens.

Path animation:

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

Keyframes:

```js
ctx.to('card', {
  keyframes: { scale: [1, 1.4, 0.8, 1] },
  duration: 60,
});

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

Only numeric keyframes are supported. For color keyframes, chain separate color tweens or use `fromTo`.

### 6.5 ctx.timeline(opts)

`ctx.timeline()` provides GSAP-style choreography while still compiling down to exact per-frame tween sampling.

```js
ctx.timeline({ defaults: { duration: 18, ease: 'spring.gentle' } })
  .from('title', { opacity: 0, y: 30 })
  .from('subtitle', { opacity: 0, y: 18 }, '-=8')
  .fromTo('cta', { scale: 0.8 }, { scale: 1, duration: 24 }, '+=6');
```

Position arguments:

| Position | Meaning |
|----------|---------|
| omitted | Start at the current timeline cursor |
| number | Absolute frame in the timeline |
| `'+=N'` | N frames after the cursor |
| `'-=N'` | N frames before the cursor |
| label | Label registered by `addLabel(name, position)` |

Explicit positions do not advance the cursor. This is useful for parallel branches.

### 6.6 Text Content Animation

Text content is animated through the normal tween API:

```js
ctx.to('title', {
  text: 'Hello OpenCat',
  duration: 30,
  delay: 6,
  ease: 'linear',
});
```

The `text` channel reveals content by grapheme cluster, not JavaScript code point. ZWJ emoji and combining marks are not split mid-cluster.

### 6.7 Text Unit Animation (`ctx.splitText`)

`ctx.splitText(id, { type })` reads the resolved text source for the frame, then returns visual units that can be animated with the same tween API.

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

Supported `type` values:

| Type | Meaning |
|------|---------|
| `'chars'` | Grapheme clusters |
| `'words'` | Unicode word-boundary units for whitespace-separated text; CJK text falls back to `chars` |
| `'lines'` | Reserved for layout-derived line ranges; not implemented yet |

Each part exposes `index`, `text`, `start`, `end`, and `part.set({ opacity, x, y, scale, rotate })`.

Text animation has two independent layers:

1. **Content layer**: `ctx.to('title', { text: ... })` changes the string that gets laid out.
2. **Unit style layer**: `ctx.splitText(...); ctx.from(parts, ...)` changes visual properties of laid-out units.

They can coexist in the same frame:

```js
ctx.set('title', { text: 'Hello' });
ctx.from(ctx.splitText('title', { type: 'chars' }), {
  opacity: 0,
  y: 12,
  duration: 12,
  stagger: 1,
});
```

---

### 6.8 ctx.alongPath(svgPath)

Low-level path sampler. For most cases, prefer the `path` option on `ctx.to()` (see above) which handles caching and timing automatically.

Returns a small object with `getLength()`, `at(t)`, and `dispose()`. `at(t)` takes `t in [0, 1]` and returns `{ x, y, angle }` -- `angle` is the path tangent in **degrees**.

The SVG string is parsed once on creation; sampling is computed in Rust via Skia's `ContourMeasure`.

```js
// Manual usage (advanced): cache the measurer yourself
if (!ctx.__along) {
  ctx.__along = ctx.alongPath('M100 360 C400 80 880 640 1180 360');
}
var pos = ctx.__along.at(0.5);
// pos = { x: ..., y: ..., angle: ... }
```

**Supported SVG path commands** (uppercase = absolute, lowercase = relative):

| Command | Meaning |
|---|---|
| `M x y` / `m dx dy` | Move to |
| `L x y` / `l dx dy` | Line to |
| `H x` / `h dx` | Horizontal line to |
| `V y` / `v dy` | Vertical line to |
| `C x1 y1 x2 y2 x y` | Cubic Bezier |
| `S x2 y2 x y` | Smooth cubic Bezier |
| `Q x1 y1 x y` | Quadratic Bezier |
| `T x y` | Smooth quadratic Bezier |
| `A rx ry x-axis-rot large sweep x y` | Elliptic arc |
| `Z` / `z` | Close path |

When the path contains multiple `M` commands (subpaths), they are **concatenated end-to-end** into a single unified path. A `t` of `0.0` samples the start of the first subpath and `1.0` samples the end of the last subpath.

### 6.9 ctx.utils

Numeric helpers and **deterministic** random.

```js
ctx.utils.clamp(value, min, max);
ctx.utils.snap(value, step);
ctx.utils.wrap(value, min, max);           // (value - min) wrapped into [min, max)
ctx.utils.mapRange(value, inMin, inMax, outMin, outMax);

ctx.utils.random(min, max, seed?);         // [min, max)
ctx.utils.randomInt(min, max, seed?);      // integer in [min, max]
```

> When `seed` is omitted, `ctx.utils.random` falls back to `Math.random()` and produces **different output per render**. **For video rendering, always pass a seed.**

### 6.10 Node API

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

### 6.11 Common Patterns

**Staggered entrance (prefer `targets`):**

```js
ctx.fromTo(
  ['card-1', 'card-2', 'card-3'],
  { opacity: 0, y: 30, scale: 0.9 },
  {
    opacity: 1,
    y: 0,
    scale: 1,
    stagger: 4,
    ease: { spring: { stiffness: 80, damping: 14, mass: 1 } },
  }
);
```

**Per-node manual control** (for custom easing per item):

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

**Linked motion:**

```js
var hero = ctx.fromTo('title',
  { opacity: 0, y: 40 },
  { opacity: 1, y: 0, duration: 20, ease: 'spring.gentle' }
);
ctx.getNode('subtitle')
  .opacity(Math.min(0.85, hero.opacity * 0.85))
  .translateY(hero.y * 0.6);
```

**Looping pulse (manual control required):**

```js
var icons = ['icon-a', 'icon-b', 'icon-c'];
var frame = ctx.frame;
var cycleLen = 30;
var activeIndex = Math.floor((frame % (icons.length * cycleLen)) / cycleLen);
var cycleStart = frame - (frame % cycleLen);

var entrance = ctx.fromTo(icons,
  { scale: 0.85, y: 18 },
  { scale: 1, y: 0, stagger: 4, ease: 'spring.default' }
);

ctx.fromTo(icons[activeIndex],
  { scale: 1 },
  { scale: 1.08, duration: cycleLen, delay: cycleStart, ease: 'spring.wobbly' }
);
```

### 6.12 Restrictions

- Do not use `document`, `window`, `requestAnimationFrame`, or `element.style`.
- Access nodes only through `ctx.getNode()`.
- `duration` is required for non-spring easing.

---

## 7. Canvas API

A `canvas` node provides a CanvasKit-style drawing surface. The drawing script must be a child `script` of the canvas node and is re-executed on every frame.

### Entry Points

| Object | Purpose |
|--------|---------|
| `ctx.CanvasKit` / `globalThis.CanvasKit` | Helpers, constructors, enums |
| `ctx.getCanvas()` | Drawing interface for the current canvas node |
| `ctx.getImage(assetId)` | Image handle for a host-provided asset id |

### CanvasKit Helpers

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

### Canvas Methods

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

Only dash path effects are currently supported.

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

Current constraints:

- `typeface` must be `null` (system default font).
- Custom font objects, `Typeface`, `FontMgr`, and font assets are not supported.
- `TextBlob` and `Paragraph` are not supported.

### Image Resources

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

### Limits

- This is a CanvasKit subset, not full CanvasKit.
- `clipRect()`, `clipPath()`, `clipRRect()` — only `CK.ClipOp.Intersect`.
- `drawColor()`, `drawColorInt()`, `drawColorComponents()` — only `CK.BlendMode.SrcOver`.
- `PathEffect` — only `MakeDash()`.
- Text drawing — only system default font.
- `ctx.getImage()` — only asset id handles.

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

## Appendix: Common Errors

| Wrong | Correct |
|------|---------|
| `type: "div"` with a `text` field | Only `type: "text"` accepts `text` |
| Coloring icons/paths with `bg-{color}` | Use `fill-{color}` for SVG fill, `stroke-{color}` for SVG stroke |
| `id` contains "icon" but `type: "div"` | Use `type: "icon"` with a Lucide icon name |
| Image `query` contains adjectives | Use only 1-4 nouns |
| Relying on `absolute` layout by default | Prefer flex layout; use `absolute` only for overlap or pinned edges |
| Putting transform Tailwind classes in `className` | Use node transform APIs such as `translateX()`, `translateY()`, `scale()`, `rotate()`, and `skew()` |
| `parentId` points to an invalid id | `parentId` must reference an existing node |
| Expecting a `layer` record type | The `layer` type has been removed; use `div` with `parentId: null` and arrange children under a `tl` node instead |
| Multiple root scenes plus root-level `transition` | Declare a `tl` node explicitly and move scenes under it |
| Using `tl` for a single scene | Use a plain `div` tree; reserve `tl` for two or more scenes with transitions |
| `tl` has scenes but no transition between adjacent pairs | Add the missing `transition`, or remove `tl` and use a plain tree |
| Root `caption` without a parent `div`, but expecting it to persist across transitions | Put the main visuals and the root `caption` under a shared parent `div` |
| `caption.path` points to a UTF-16 subtitle file | Convert the SRT to UTF-8 first; the current loader reads UTF-8 text |
| Frame count mismatch in timeline mode | Runtime total is derived from `sum(scene.duration) + sum(transition.duration)` |
| `"effect": "slide-left"` | Use separate fields: `"effect": "slide", "direction": "from_left"` |
