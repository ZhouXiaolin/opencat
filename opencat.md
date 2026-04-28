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
{"id": "search", "parentId": "scene1", "type": "icon", "className": "w-[24px] h-[24px] text-slate-400", "icon": "search"}
```

| Field | Required | Description |
|-------|----------|-------------|
| `icon` | yes | Lucide icon name in kebab-case |

Color icons with `text-{color}`, not `bg-{color}`.

### 3.5 `canvas`

Canvas drawing surface. Requires a child `script` for drawing commands.

```json
{"id": "chart", "parentId": "scene1", "type": "canvas", "className": "w-[300px] h-[200px]"}
{"type": "script", "parentId": "chart", "src": "var CK = ctx.CanvasKit;\nvar canvas = ctx.getCanvas();\ncanvas.clear('#ffffff');"}
```

See §6 Canvas API for the full drawing reference.

### 3.6 `audio`

Audio playback node. Equivalent to `<audio>`.

```json
{"id": "bgm", "parentId": "root", "type": "audio", "path": "/tmp/bgm.mp3"}
{"id": "sfx", "parentId": "root", "type": "audio", "url": "https://example.com/sfx.mp3"}
```

Specify exactly one source: `path` (local) or `url` (remote).

The `parentId` controls when the audio plays:
- Attached under a scene node → plays during that scene.
- `parentId: null` → plays for the entire composition (timeline-level).

### 3.7 `video`

Video playback node. Equivalent to `<video>`.

```json
{"id": "clip", "parentId": "scene1", "type": "video", "className": "w-full h-full object-cover", "path": "clip.mp4"}
```

| Field | Required | Description |
|-------|----------|-------------|
| `path` | yes | Local video file path |

### 3.8 `caption`

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

### 3.9 `tl`

Timeline container. See §2.3 for full specification.

| Field | Description |
|-------|-------------|
| `id` | Required |
| `parentId` | Parent node |
| `className` | Tailwind styling |

No `duration` field — the total is derived from child scenes and transitions.

### 3.10 `transition`

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
| `transition-*` `animate-*` `duration-*` `ease-*` `delay-*` | `ctx.animate()` / `ctx.stagger()` / `ctx.sequence()` |
| `transform` `translate-*` `rotate-*` `scale-*` `skew-*` | `ctx.getNode(...).translateX()` / `translateY()` / `scale()` / `rotate()` / `skew()` |

> Tailwind handles static styling. Scripts handle motion.

### 5.1 Easing Reference

Easing names are shared by `ctx.animate()`, `ctx.sequence()` steps, and `transition.timing`.

| Preset | Effect |
|--------|--------|
| `'linear'` | Constant speed |
| `'ease'` / `'ease-in'` / `'ease-out'` / `'ease-in-out'` | Standard CSS-like cubic curves |
| `'back-in'` / `'back-out'` / `'back-in-out'` | Slight overshoot (UI snap) |
| `'elastic-in'` / `'elastic-out'` / `'elastic-in-out'` | Damped oscillation |
| `'bounce-in'` / `'bounce-out'` / `'bounce-in-out'` | Ground-bounce style |
| `'steps(N)'` | Quantised into N discrete steps |
| `'spring-default'` | General spring |
| `'spring-gentle'` | Soft spring |
| `'spring-stiff'` | Stiffer spring |
| `'spring-slow'` | Slower spring |
| `'spring-wobbly'` | Wobbly spring |

Custom spring (JS):

```js
easing: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

Cubic bezier (JS):

```js
easing: [0.25, 0.1, 0.25, 1.0]
```

Transition `timing` field also accepts a string form: `"bezier:0.4,0,0.2,1"`.

---

## 6. Scripting

Scripts are attached to nodes via `script` records and run on every frame using QuickJS.

```json
{"type": "script", "parentId": "scene1", "src": "var node = ctx.getNode('title');\nvar anim = ctx.animate({from:{opacity:0},to:{opacity:1},duration:20,easing:'spring-gentle'});\nnode.opacity(anim.opacity);"}
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

### Design: Precise Mathematical Computation

OpenCat's animation system is **functionally pure**: every animated value is computed as `value = f(current_frame)` through exact mathematical formulas. There is no internal tick loop, no accumulated state, and no non-deterministic drift.

- **Interpolation**: linear `from + (to - from) * easing(progress)`
- **Spring**: solved from physical parameters (`stiffness`, `damping`, `mass`) with exact settle-time detection
- **Color**: HSLA space with shortest-path hue rotation (handles 360° wrap-around)
- **Path**: Skia `ContourMeasure` for sub-pixel-accurate arc-length sampling

Scripts are re-executed every frame. The pattern is: declare an animation → read its current value → write it to a node. Declarative `targets` support (see below) automates the write step without changing the underlying math.

---

### ctx.animate(opts)

Declare a `from → to` animation. The returned object exposes animated values through getters.

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

Additional getters on the returned object:

- `anim.progress`: `0` → `1`
- `anim.settled`: whether a spring has settled
- `anim.settleFrame`: frame where the spring settled

**Declarative `targets` (auto-apply):**

Pass `targets` to have the animation values applied to nodes automatically on every frame. This eliminates the manual `node.property(anim.property)` boilerplate.

```js
ctx.animate({
  targets: 'hero',
  from: { opacity: 0, translateY: 40 },
  to:   { opacity: 1, translateY: 0 },
  duration: 30,
  easing: 'spring-gentle',
});
```

`targets` accepts:
- A single node id: `targets: 'node1'`
- An array of ids: `targets: ['node1', 'node2', 'node3']`
- Split-text parts: `targets: ctx.splitTextNode('title', { granularity: 'graphemes' })`

**`from`-only semantics:**

If you only specify `from`, the animation infers `to` from identity defaults (`opacity: 1`, `translateX/Y: 0`, `scale: 1`, `rotation: 0`, etc.). This matches GSAP's `gsap.from()` behaviour.

```js
ctx.animate({
  targets: 'box',
  from: { opacity: 0, translateY: 24 },
  duration: 20,
});
// Implicit to: { opacity: 1, translateY: 0 }
```

**Stagger via `ctx.animate`:**

When `targets` is an array, pass `stagger` to offset each target's delay:

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

**Repeat options:**

| Field | Default | Description |
|-------|---------|-------------|
| `repeat` | `0` | Additional cycles. `0` = once, `N` = N+1 times, `-1` = infinite |
| `yoyo` | `false` | Reverse on alternate cycles |
| `repeatDelay` | `0` | Frames to hold before restarting each cycle |

#### Path Animation

Pass `path` (SVG path string) instead of `from`/`to` to animate along a curve. Returns `x`, `y`, `rotation` getters.

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

`path` and `from`/`to` can coexist on the same animation — use `from`/`to` for properties like `opacity` alongside path-driven position.

| Field | Default | Description |
|-------|---------|-------------|
| `path` | — | SVG path string (see supported commands below) |
| `orient` | `0` | Rotation offset in degrees from path tangent. Use `-90` for upward-oriented shapes |

### Keyframes (multiple stops in a single animation)

For a single animation that needs more than two stops, pass `keyframes` instead of `from`/`to`:

```js
// Shorthand: numeric values evenly spaced over [0, 1]
var a = ctx.animate({
  keyframes: { scale: [1, 1.4, 0.8, 1] },
  duration: 60,
});
ctx.getNode('card').scale(a.scale);

// Full form: explicit `at` (normalised time in [0, 1]) + optional per-segment easing
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

Notes:

- Only **numeric values** are supported in keyframes (color keyframes are not yet supported -- animate colour with `from`/`to`).
- `at` is normalised to `[0, 1]`; the **outer** `easing` (and `repeat`/`yoyo`) on `ctx.animate` still applies first, then the resulting progress is mapped through the per-segment easing.
- `keyframes` and `from`/`to` may co-exist on the same animation, but keys defined in both are taken from `keyframes`.

### ctx.stagger(count, opts)

Like `animate`, but creates multiple animations with staggered delay. Still returns an array of value objects, but now also supports `targets` for declarative application.

```js
var anims = ctx.stagger(4, {
  from: { opacity: 0, translateY: 30 },
  to:   { opacity: 1, translateY: 0 },
  gap: 4,
  duration: 20,
  easing: 'spring-gentle',
});
```

**With `targets`:**

When `targets` is provided, `count` is inferred from the target list length and each target receives its own staggered animation:

```js
ctx.stagger(0, {
  targets: ['a', 'b', 'c', 'd'],
  from: { opacity: 0, scale: 0.9 },
  to:   { opacity: 1, scale: 1 },
  gap: 3,
  duration: 18,
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

| Use case | API | Notes |
|----------|-----|-------|
| Single animation, manual apply | `ctx.animate({ from, to })` | Returns value object; you call `node.prop(anim.prop)` |
| Single animation, auto-apply | `ctx.animate({ targets, from, to })` | `targets` handles the `node.prop()` calls for you |
| N identical animations, uniform gap, manual apply | `ctx.stagger(count, { from, to, gap })` | Returns array of value objects |
| N identical animations, uniform gap, auto-apply | `ctx.stagger(0, { targets, from, to, gap })` | `count` inferred from `targets` length |
| Text units (graphemes / words), auto-apply | `ctx.splitTextNode(id, opts).animate({...})` | Built-in stagger; no manual loop |
| Heterogeneous steps, irregular gaps, overlaps, parallel branches | `ctx.sequence` | Per-step `duration`, `easing`, `at` |

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

Character counting currently uses `Array.from()`, so the effect is code-point based.
This works well for ASCII, CJK, and many single-emoji cases, but it is not a full grapheme-cluster
splitter for ZWJ emoji or combining-mark sequences.

`ctx.typewriter()` is a content-replacement helper: it produces the current string for the frame and
is typically applied via `ctx.getNode(id).text(tw.text)`.

---

### Text Animation (`ctx.splitTextNode`)

OpenCat splits text into **grapheme clusters** (not code points) through Rust-side `unicode-segmentation`. This means ZWJ emoji (👨‍👩‍👧‍👦) and combining marks (é) are treated as single units, matching visual perception.

`ctx.splitTextNode` reads the **resolved text source** — the actual text that will be rendered this frame, after any `text_content` mutations have been applied. This prevents double-source drift.

```js
var parts = ctx.splitTextNode('title', { granularity: 'graphemes' });
```

Each `part` exposes:

| Property | Description |
|----------|-------------|
| `index` | Unit index |
| `text`  | Unit string |
| `start` / `end` | Byte offsets in the source string |

And one method:

| Method | Description |
|--------|-------------|
| `part.set({ opacity, translateX, translateY, scale, rotation })` | Batch-write visual overrides for this unit |

#### Declarative animation: `parts.animate(opts)`

Instead of manual `for` loops, use the built-in `animate` method on the parts array:

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

This is equivalent to `ctx.stagger(parts.length, { targets: parts, ... })` — exact same math, zero boilerplate.

#### Reverting overrides: `parts.revert()`

Clear all overrides on the node and restore default appearance:

```js
var parts = ctx.splitTextNode('title', { granularity: 'graphemes' });
// ... animate ...
parts.revert();
```

#### `words` granularity

```js
ctx.splitTextNode('title', { granularity: 'words' }).animate({
  from: { opacity: 0, translateX: 18 },
  to:   { opacity: 1, translateX: 0 },
  duration: 20,
  stagger: 5,
});
```

Whitespace is preserved as part of the source text; word units include spaces so layout cadence remains natural.

#### Text animation + content effects

`text_content` mutations (typewriter, scramble) and `text_unit_overrides` (split text) operate on **two independent layers**:

1. **Content layer** (`text_content`): changes the string that gets laid out
2. **Unit style layer** (`text_unit_overrides`): changes visual properties of already-laid-out units

They can coexist. The resolved text source is determined first, then split text reads that resolved source, then overrides are applied.

```js
ctx.getNode('title').text('Hello');        // content layer
ctx.splitTextNode('title', { granularity: 'graphemes' }).animate({
  from: { opacity: 0 },
  to:   { opacity: 1 },
  duration: 12,
  stagger: 1,
});                                         // unit style layer
```

---

### ctx.alongPath(svgPath)

Low-level path sampler. For most cases, prefer the `path` option on `ctx.animate()` (see above) which handles caching and timing automatically.

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

Only the **first contour** is sampled. Multiple `M` commands (subpaths) — subsequent ones are ignored.

#### Keyframes

For more than two stops, use `keyframes` instead of `from`/`to`:

```js
// Shorthand: evenly spaced
var a = ctx.animate({
  keyframes: { scale: [1, 1.4, 0.8, 1] },
  duration: 60,
});

// Full form: explicit `at` + optional per-segment easing
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
```

Notes:

- Only **numeric values** in keyframes. Color keyframes are not supported — use `from`/`to` for color animation.
- `at` is normalised to `[0, 1]`. The outer `easing` applies first, then per-segment easing maps the result.
- `keyframes` and `from`/`to` may coexist; keys in both are taken from `keyframes`.

### 6.3 ctx.stagger(count, opts)

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

### 6.4 ctx.sequence(steps)

Heterogeneous chain of animations with per-step timing, overlaps, and parallel branches.

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

**Per-step fields:**

| Field | Default | Description |
|-------|---------|-------------|
| `from`, `to`, `duration`, `easing`, `clamp` | — | Same as `ctx.animate()` |
| `delay` | `0` | Extra offset before this step starts |
| `gap` | `0` | Cursor advance after this step. Negative to overlap. |
| `at` | — | Absolute start frame. Pins the step; does not advance the cursor. |

**Parallel branches with `at`:**

```js
var seq = ctx.sequence([
  { to: { opacity: 1 }, duration: 20 },       // runs 0..20, cursor → 20
  { to: { opacity: 1 }, duration: 30, at: 5 }, // pinned at frame 5, cursor untouched
  { to: { opacity: 1 }, duration: 10 },        // runs 20..30 (cursor is 20)
]);
```

**When to pick which:**

| Use case | API |
|----------|-----|
| Single animation | `ctx.animate` |
| N identical animations, uniform gap | `ctx.stagger` |
| Heterogeneous steps, irregular gaps, overlaps, parallel branches | `ctx.sequence` |

### 6.5 ctx.typewriter(fullText, opts)

Type out a string character by character.

```js
var tw = ctx.typewriter('Hello OpenCat', {
  duration: 30,
  delay: 6,
  easing: 'linear',
  caret: '▍',
});
ctx.getNode('title').text(tw.text);
```

| Field | Default | Description |
|-------|---------|-------------|
| `duration` | — | Required. Frames from empty to full string. |
| `delay` | `0` | Frames to wait before typing starts. |
| `easing` | `'linear'` | Any easing. Non-linear varies typing speed. |
| `clamp` | `true` | Prevents overshoot from producing out-of-range character counts. |
| `caret` | `''` | Appended while typing. Disappears when complete. |

Character counting uses `Array.from()` — grapheme-safe for CJK and emoji.

### 6.6 ctx.alongPath(svgPath)

Low-level path sampler. For most cases, prefer `ctx.animate({ path: ... })` which handles caching and timing automatically.

Returns `{ getLength(), at(t), dispose() }`. `at(t)` takes `t in [0, 1]` and returns `{ x, y, angle }` — `angle` is the path tangent in degrees.

```js
if (!ctx.__along) {
  ctx.__along = ctx.alongPath('M100 360 C400 80 880 640 1180 360');
}
var pos = ctx.__along.at(0.5);
```

Always cache instances on `ctx.__yourKey`. `dispose()` is optional but recommended for long-running compositions.

### 6.7 Animating Colors

`ctx.animate()` automatically interpolates color values. Colors are converted to HSLA, interpolated along the shortest arc (handling 360→0 wrap), and returned as `rgba(...)`.

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

Supported color literals:

- `#rgb` / `#rrggbb` / `#rrggbbaa`
- `rgb(r, g, b)` / `rgba(r, g, b, a)`
- `hsl(h, s%, l%)` / `hsla(h, s%, l%, a)`

Colors are always clamped — spring overshoot won't push values out of gamut.

> Tailwind tokens like `'blue-500'` are **not** interpolated. To animate colors, write them as hex/rgb/hsl in `from` / `to`.

### 6.8 ctx.utils

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

### 6.9 Node API

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

### 6.10 Common Patterns

**Staggered entrance:**

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

**Linked motion:**

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

**Looping pulse:**

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

### 6.11 Restrictions

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
| Coloring icons with `bg-{color}` | Use `text-{color}` for icons |
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
