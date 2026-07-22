# Architecture: Rendering Pipeline

## Overview

Explicit lifecycle (issue #12 / #24). Core is a pure derivation kernel; hosts own
fetch, cache, decode, and platform APIs. DrawOp is a **Skia-compatible IR** shared
by the native Skia engine and CanvasKit web backends.

Host migration (old open paths → prepare/`open_pipeline`, HostInputs, AudioPlan,
RenderFrame, OCIR v4): see [`docs/MIGRATION.md`](docs/MIGRATION.md).

```
Input (XML / JSONL)
  │
  ▼
CompositionDraft::parse / from_parsed
  │
  ├── HostRequirements  (canonical AssetId + kind + logical locator; no I/O)
  │
  ▼
Host: fetch bytes / probe metadata / load fonts / script text / SRT
  │
  ▼
HostInputs  (ResourceMetadata + subtitle/script/font content; no ordinary media bytes into core prepare)
  │
  ▼
CompositionDraft::prepare(HostInputs)  →  PreparedComposition
  │   (font merge, caption hydration, script inject, metadata validate)
  ▼
PreparedComposition::open_pipeline(scripts)  →  DefaultPipeline
  │
  render_frame(frame)  →  RenderFrame { draw: DrawOpFrame, media: FrameMediaPlan }
  │                         (generated images: full RGBA in media plan)
  │
  ┌───────────┴───────────┐
  Engine (Skia)           Web (CanvasKit / OCIR v4)
  MP4 / PNG               Canvas / MP4
```

**Contracted (deleted production seams):** dual `ResourceCatalog` /
`HashMapResourceCatalog`, public host entry `open_with_prepared_catalog`,
`LiveScriptHost` / `ScriptRunner` / `ScriptRuntimeCache`, `RenderSession`,
`EnginePlatform`, FrameConsumer-based open paths, engine/CLI re-exports of core
internals, and long-lived deprecation wrappers on the `opencat` facade.

---

## 1. Input Formats

Two interchangeable input formats, both producing `ParsedComposition`:

**Markup (XML)** — `crates/opencat-core/src/parse/markup/`:
```rust
parse::markup::parse(input)  // crate/parse/markup/mod.rs
```
Supports `<template>`, `<slot>`, `<transition>`, `<script>`, `<tl>`, etc. Templates expand at parse time. The XML path goes through `parse::document::builder::build_parsed_document()` which produces a `ParsedDocumentParts` → assembled into `ParsedComposition`.

**JSONL** — `crates/opencat-core/src/parse/jsonl/mod.rs`:
```rust
parse::jsonl::parse(input)  // crates/opencat-core/src/parse/jsonl/mod.rs:162
```
One JSON object per line. Directly maps each line type to a `ParsedElement` (Div, Text, Image, Video, Audio, Canvas, Icon, Path, Caption, Timeline, Transition). Then calls `build_tree()` or `build_tree_with_tl()` to assemble the node tree.

Both produce the same `ParsedComposition` (`crates/opencat-core/src/parse/document.rs:132`):
```rust
pub struct ParsedComposition {
    pub width: i32,
    pub height: i32,
    pub fps: i32,
    pub duration: f64,
    pub root: Node,              // parse::node::Node (Arc<NodeKind>)
    pub script: Option<String>,
    pub audio_sources: Vec<CompositionAudioSource>,
    pub font_manifest: FontManifest,
}
```

`Node` (`crates/opencat-core/src/parse/node.rs:14`) wraps `Arc<NodeKind>` — a typed AST node holding a `NodeStyle` and optional children. The tree is immutable after parse; per-frame time-dependent values are resolved later.

---

## 2. Explicit Lifecycle (Host-Owned Preparation)

Core never performs I/O. Ordinary image/video/audio/Lottie **bytes stay on the host**;
core prepare consumes metadata and content-level inputs (fonts, SRT text, script text).

### 2a. Draft + requirements

```rust
let draft = CompositionDraft::parse(input)?;
// or CompositionDraft::from_parsed(parsed)
let reqs = draft.requirements(); // HostRequirements
// crates/opencat-core/src/lifecycle/
```

Each `ResourceRequest` carries canonical `AssetId`, `ResourceKind`, and a logical
`ResourceLocator` (no real FS path). Hosts resolve locators against their base dir / VFS / URL.

### 2b. Host supplies `HostInputs`

Hosts fetch/probe independently, then fill:

- image / video / Lottie **metadata** (`insert_image` / `insert_video` / `insert_lottie`)
- audio presence (`insert_audio`)
- subtitle SRT text (`insert_subtitle_text`) — core parses SRT
- document font bytes (`insert_document_font`) — core merges into fontdb
- external script text (`insert_script_text`)

Optional pure helper for hosts that still want core-side header probes:

```rust
let prepared = build_catalog(&requests, &bytes); // → PreparedResourceCatalog
// then HostInputs::fill_from_prepared_catalog(...)
```

### 2c. Prepare + open

```rust
let prepared = draft.prepare(inputs)?;
let pipeline = prepared.open_pipeline(scripts)?;
// crates/opencat-core/src/lifecycle/mod.rs
```

Prepare validates inputs (fail-fast on missing layout/time-critical metadata),
merges fonts, hydrates captions, injects script texts. `open_pipeline` builds
pipeline state (composition, layout/display sessions, one `ScriptRealm` per
pipeline, render cache). Hosts must not call internal open helpers.

Production open paths:

- Engine: `opencat_engine::pipeline::open` → lifecycle
- Web: `WebRenderer::open_design` → lifecycle
- CLI: `render_*` → engine open

---

## 3. Core Pipeline State

### `Composition` (`crates/opencat-core/src/parse/composition.rs`)

```rust
pub struct Composition {
    pub id: String,
    pub width: i32, pub height: i32,
    pub fps: u32, pub duration: f64, pub frames: u32,
    pub root: Arc<dyn Fn(&FrameCtx) -> Node>,
    pub audio_sources: Arc<Vec<CompositionAudioSource>>,
}
```

The root is a closure, not a static tree: it is re-called each frame so time-dependent logic (e.g., scene switches, timeline position) produces different nodes at different frames.

### Pipeline (`crates/opencat-core/src/pipeline/default.rs`)

```rust
pub struct DefaultPipeline<S: JsContext> {
    composition: Composition,
    info: CompositionInfo,
    catalog: PreparedResourceCatalog, // metadata only; ResourceResolver impl
    scripts: ScriptRealm<S>,          // one realm per pipeline
    layout_session: LayoutSession,
    display_build_session: DisplayBuildSession,
    composite_history: CompositeHistory,
    analyze_fingerprint_history: AnalyzeFingerprintHistory,
    font_db: Arc<fontdb::Database>,
    cache: RenderCache,
    last_ordered_scene: OrderedSceneProgram,
    generated_images: GeneratedImageTable, // internal; hosts use FrameMediaPlan
}
```

---

## 4. Per-Frame Rendering

`pipeline.render_frame(frame_index)` → `crates/opencat-core/src/pipeline/frame.rs:32`

### Step-by-step:

#### 4a. Frame Context

```rust
let frame_ctx = FrameCtx { frame, fps, width, height, frames };
// crates/opencat-core/src/frame_ctx.rs
```
Carries the time-independent parameters for this frame.

#### 4b. Resolve UI Tree

```rust
composition.root_node(&frame_ctx)     // → Node (time-dependent AST)
resolve_ui_tree(&root, &frame_ctx, ...) // → ElementNode
// crates/opencat-core/src/resolve/tree.rs
```
The `Node` is evaluated into an `ElementNode` — a flat, resolved tree with:
- All styles finalized (percentages → pixels, auto → computed)
- Transforms, opacity, clips resolved
- Script-driven mutations applied (via `ScriptHost`)
- `VideoFrameTiming` computed from composition time

#### 4c. Layout

```rust
layout_session.compute_layout(&element_root, &frame_ctx, &font_provider)
// crates/opencat-core/src/layout/mod.rs
```
Taffy layout engine. The `LayoutSession` uses **Merkle-tree caching**:
- Each node hashes its style + children → `structure_subtree_hash`
- On cache hit (`input_merkle_full_hit`), skip rebuild
- On cache miss, rebuild Taffy tree from scratch
- Layout output (rects) are also hashed → `layout_subtree_hash`
- Cache hit on layout → skip paint-node fingerprint recompute

Output: `LayoutTree { root: LayoutNode { id, rect, output_fingerprint, children } }`

#### 4d. Display Tree

```rust
display_build_session.build(&element_root, &layout_tree, &frame_ctx)
// crates/opencat-core/src/display/build.rs
```
Merges `ElementNode` (visual semantics) with `LayoutNode` (positions) into a `DisplayTree`. Each `DisplayNode` contains:
- A `DisplayItem` (bitmap, text block, shape path, canvas, video frame, icon, generated image, or group)
- `CompositeSemantics` (opacity, clip, backdrop blur, CSS filters, layer bounds)
- `PaintClipInfo` (border-radius clip tracking)

Also Merkle-cached via `DisplayBuildSession`.

#### 4e. Annotation & Analysis

```rust
let mut annotated = annotate_display_tree(&display_tree);
mark_display_tree_apply_changed(composite_history, ...);
compute_display_tree_fingerprints_with_history(analyze_fingerprint_history, ...);
// crates/opencat-core/src/render/analyze.rs
```
Walks the display tree to produce an `AnnotatedDisplayTree`:
- `AnnotatedNodeHandle` → typed index into flat arena
- Each node gets a `subtree_fingerprint` (hash of its own content + children's fingerprints)
- `AnalyzeReuseState`: Fresh / ReusedFromHistory / CompositeBlocked
- Scene snapshots: if the frame has the same `root_fingerprint` as the last frame, the entire `DrawOpFrame` is reused (cache hit), skipping all rendering

#### 4f. Scene Program

```rust
let ordered_scene = OrderedSceneProgram::build(&annotated);
// crates/opencat-core/src/render/scene.rs
```
Produces a DAG of `OrderedSceneOp` — either `LiveSubtree` (needs full rendering) or `ReusedSubtree` (can replay cached draw ops). This is the traversal order for render dispatch.

#### 4g. Render Dispatch → DrawOpFrame

```rust
render_display_tree(&mut ctx, &annotated, &mut cache)
// crates/opencat-core/src/render/dispatch.rs:480
```
Walks the `OrderedSceneProgram` and emits `DrawOp`s into a `DrawOpBuilder`. Key caching mechanisms:
- **Scene snapshot**: if root fingerprint matches last frame, return the entire cached `DrawOpFrame`
- **Node-own segment cache**: a node's own rendering (minus children) is cached; when only children change, the parent's own segment is imported from cache
- **Apply segment cache**: transform/opacity/layer setup is cached per composite plan

Produces `DrawOpFrame { ops, subtrees, paints, paths, strings, bytes, f32_pool, ... }`.

#### 4h. Media Plan

```rust
let media_plan = build_media_plan(&frame);
// crates/opencat-core/src/render/media_plan.rs
```
Walks the finished `DrawOpFrame` and collects:
- `images`: static image refs (for decode)
- `video_frames`: video frame refs with `time_micros` (for seek + decode)
- `lottie_bundles`: bundle ids for Lottie animation
- `runtime_effects`: SkSL shader refs (for compile)
- `generated_images`: color-emoji bitmap ids

---

## 5. DrawOp IR

`crates/opencat-core/src/ir/draw_op.rs:162`

The canonical cross-platform draw instruction set. Divided into categories:

| Category | Ops |
|----------|-----|
| Stack | `Save`, `SaveLayer`, `Restore`, `RestoreToCount` |
| Transform | `Translate`, `Scale`, `Rotate`, `Skew`, `Concat` |
| Paint state | `SetFillStyle`, `SetStrokeStyle`, `SetLineWidth`, `SetLineCap`, `SetLineJoin`, `SetLineDash`, `ClearLineDash`, `SetGlobalAlpha`, `SetAntiAlias` |
| Path | `BeginPath`, `Path(PathOp)`, `FillPath`, `StrokePath`, `ClipPath` |
| Drawing | `Clear`, `Paint`, `Rect`, `RRect`, `DRRect`, `Oval`, `Circle`, `Arc`, `Line`, `Points`, `DrawPath`, `Image`, `ImageRect` |
| Media | `LottieRect` (skottie animation) |
| Shader | `RuntimeEffect` (SkSL with uniforms + child inputs) |
| Cache | `ReplayRange`, `DrawSubtreePicture`, `ReplaySubtreePicture` |
| Script | `ScriptRuntimeEffect` (intermediate form, resolved before encoding) |

All variant payloads reference **side tables** via IDs (`PaintId`, `PathId`, `EffectId`, `StringId`, `ImageRef`, `F32Range`, etc.) for deduplication and compact binary encoding.

---

## 6. Binary Encoding (Web Transfer)

`crates/opencat-core/src/ir/draw_encoding.rs`

`encode_draw_frame()` serializes a `DrawOpFrame` into an `EncodedDrawFrame` — a set of flat `Vec<u8>`/`Vec<f32>`/`Vec<TableRange>` that is passed to JS via wasm-bindgen as typed arrays.

The binary envelope:
- **Section 1 — Ops**: Little-endian op stream (opcode u16 + flags u16 + payload_len u32 + payload)
- **Section 2 — f32_pool**: Flat f32 array (shared by Points, SetLineDash, etc.)
- **Section 3 — Strings**: UTF-8 concatenation + range table
- **Section 4 — Subtrees**: Length-prefixed op streams for hidden picture subtrees
- **Section 12 — Generated image delta**: RGBA for new color-emoji glyphs

On the JS side (`crates/opencat-web/web/src/draw-ir.ts`):
- `decodeFrame()` parses the envelope into a `DecodedFrame`
- `renderEncodedDrawFrame()` replays ops onto a CanvasKit `Canvas`
- Each opcode maps to a CanvasKit API call via `OPCODES` dispatch table

---

## 7. Engine (Skia GPU) Implementation

`crates/opencat-engine/`

### 7a. Pipeline Open

```rust
pub fn open(input: &str, loader: EngineLoader, scripts: RqJsContext) -> Result<EnginePipeline>
// crates/opencat-engine/src/pipeline.rs:98
```
The engine's `open` function implements the full host-owned chain:
1. Parse input (markup or JSONL)
2. `loader.load_font_manifest()` → fetch declared font bytes
3. `engine_font_db_with_document_fonts()` → build `Arc<fontdb::Database>`
4. `build_parsed_document()` (markup only, expands templates)
5. `open_parsed_host_owned()` (chain below)

```rust
pub fn open_parsed_host_owned(
    parsed: ParsedComposition,
    mut loader: EngineLoader,
    scripts: RqJsContext,
    font_db: Arc<fontdb::Database>,
) -> Result<EnginePipeline>
// crates/opencat-engine/src/pipeline.rs:141
```
1. `collect_resource_requests_from_parsed()`
2. **Engine fetches resources** via `EngineLoader` (file system reads with caching)
3. `build_catalog()` — pure probe
4. `hydrate_captions()` — pure SRT parse
5. `PreparedComposition::open_pipeline()`

### 7b. Frame Rendering to RGBA

```rust
fn render_pipeline_frame_to_rgba(...) -> Result<Vec<u8>>
// crates/opencat-engine/src/render.rs:61
```
1. `pipeline.render_frame(frame_index)` → `RenderFrame`
2. Create Skia raster surface (`RasterN32Premul`)
3. `EngineLoaderFrameConsumer::consume_frame()`:
   - `prepare_frame()` (`crates/opencat-engine/src/consumer.rs:82`):
     - Decode static images from file paths → Skia `Image`
     - Seek video frames via `MediaContext::frame_rgba_at_time_by_path()` → Skia `Image`
     - Build `RuntimeEffect` from SkSL
     - Look up generated images (color-emoji) from `GeneratedImageTable`
     - Load Lottie animations into cache
   - `executor.execute()` (`crates/opencat-engine/src/executor/mod.rs:80`):
     - `begin_frame()` resets paint/path state
     - `replay_frame()` (`crates/opencat-engine/src/executor/replay.rs`): walks `DrawOpFrame.ops` and maps each `DrawOp` to `skia_safe::Canvas` API
4. `surface.image_snapshot().read_pixels()` → RGBA `Vec<u8>`

### 7c. MP4 Encoding

Engine uses `ffmpeg-next` to encode rendered frames into MP4. Audio mixing via `audio_plan` module. Lottie uses `skottie` from `skia-safe`.

---

## 8. Web (CanvasKit WASM) Implementation

`crates/opencat-web/`

### 8a. WASM Pipeline Open

```rust
WebRenderer::open_design(&mut self, source: String) -> Result<String>
// crates/opencat-web/src/wasm_bridge.rs:145
// → open_design_pipeline() at line 414
```
1. `preload_assets(source)` (`crates/opencat-web/src/resource/wasm_api.rs:65`):
   - Downloads all resources (fonts, images, videos, Lottie, subtitles)
   - Stores bytes in thread-local `BlobStore` (keyed by `AssetId`)
   - Probes metadata via `build_catalog()`
   - Scans Lottie dependencies recursively
2. Build font database (default NotoSansSC + NotoColorEmoji + document fonts)
3. Parse source with font database
4. `collect_resource_requests_from_parsed()`
5. `build_catalog()` from `BlobStore` bytes
6. `hydrate_captions()` from fetched SRT
7. `PreparedComposition::open_pipeline()`

### 8b. Frame Rendering → Binary IR

```rust
WebRenderer::build_frame_ir(&mut self, frame: u32) -> Result<Vec<u8>>
// crates/opencat-web/src/wasm_bridge.rs:177
```
1. `pipeline.render_frame(frame)` → `RenderFrame`
2. `WebFrameConsumer::consume_frame()` (web consumer):
   - `encode_draw_frame()` → binary `EncodedDrawFrame`
   - Appends generated-image delta (new color-emoji glyphs since last frame)
3. Returns binary envelope to JS

### 8c. JS CanvasKit Execution

```
build_frame_ir(f)     → Uint8Array (Rust wasm-bindgen)
decodeFrame(bytes)    → DecodedFrame  (draw-ir.ts)
renderEncodedDrawFrame(decoded, canvas) → void
```

`renderEncodedDrawFrame` (`crates/opencat-web/web/src/draw-ir.ts`):
- Walks the decoded op stream
- Each opcode dispatches to the corresponding CanvasKit API:
  - `Save`/`Restore` → `canvas.save()`/`canvas.restore()`
  - `Translate`/`Scale`/`Rotate`/`Concat` → `canvas.translate()` etc.
  - `Rect`/`RRect`/`Circle`/`Oval` → `canvas.drawRect()` with interned Paint
  - `Image`/`ImageRect` → `canvas.drawImage*()` with image from `loadResourceBytes()`
  - `LottieRect` → `skottie.animation.seekFrame()` + `canvas.drawSkottie()`
  - `RuntimeEffect` → `canvas.drawRect()` with `RuntimeEffect.makeShader()`
  - `ReplaySubtreePicture` → replay a subtree op stream onto a sub-canvas, then draw it as a picture shader
- Paint interned references are resolved via `buildPaintById()` → `new CK.Paint()`
- Image refs: static → `loadResourceBytes()`, video → decoded frame, generated → cached RGBA from delta

### 8d. Media Preparation (JS side)

The `FrameMediaPlan` (returned as JSON by `prepare_frame()`) tells JS which media to prepare. The JS exporter uses `@webav/av-cliper` for video decoding and `WebCodecs` for encoding.

---

## 9. Key Design Principles

### Separation of Concerns

```
Core:  ParsedComposition → ResourceRequests → Catalog → Layout → DisplayTree → DrawOpFrame
Host:  Fetch bytes → Build catalog → Hydrate captions → Font DB → [Pipeline] → Decode → Execute
```

Core never does I/O. It declares what resources are needed (`ResourceRequests`), probes metadata from bytes (`build_catalog`), and produces deterministic draw instructions (`DrawOpFrame`). Hosts (engine Skia, web CanvasKit) implement resource fetching, decoding, and pixel pushing independently.

### Incremental / Cached Rendering

Three layers of caching from coarsest to finest:

1. **Scene snapshot** (`RenderCache.last_scene_snapshot`): if the whole frame's `root_fingerprint` matches the last frame, reuse the entire `DrawOpFrame`. This is a no-op for frames where nothing changes.

2. **Subtree reuse** (`AnalyzeFingerprintHistory`): each display subtree has a `subtree_fingerprint`. When it matches history, the node is marked `ReusedSubtree` and its cached `DrawOp` segment is imported — only the composite layer (transform + opacity + clip) is re-emitted.

3. **Node-own segment cache** (`node_own_segments`): a node's own rendering (its item + clip, without children) is cached independently. When only children change, the parent's draw ops are imported from cache without re-recording.

### Deterministic by Construction

`RenderFrame { draw: DrawOpFrame, media: FrameMediaPlan }` is a pure function of `(pipeline, frame_index)`. The same frame on the same pipeline always yields byte-identical draw ops. This enables SSIM-based regression testing between engine and web.

### Binary IR for Cross-Language Transfer

The `EncodedDrawFrame` format bridges Rust (WASM) and JS/CanvasKit. Instead of JSON serialization of draw commands (slow, verbose), ops are packed into a compact binary envelope (opcode + payload_len + payload). Side tables (paints, paths, strings, f32_pool) are deduplicated and interned during build, then encoded flat for zero-copy transfer via wasm-bindgen typed arrays.

---

## 10. File Map

| Path | Role |
|------|------|
| `crates/opencat-core/src/parse/` | XML/JSONL parsing → `ParsedComposition` |
| `crates/opencat-core/src/parse/composition.rs` | `Composition` struct (time-dependent root) |
| `crates/opencat-core/src/parse/node.rs` | `Node` (parse AST, `Arc<NodeKind>`) |
| `crates/opencat-core/src/parse/markup/` | XML parser with templates |
| `crates/opencat-core/src/parse/jsonl/` | JSONL parser |
| `crates/opencat-core/src/parse/jsonl/tailwind.rs` | Tailwind class → `NodeStyle` |
| `crates/opencat-core/src/probe/prepare.rs` | `build_catalog()`, `hydrate_captions()` |
| `crates/opencat-core/src/probe/requests.rs` | `collect_resource_requests_from_parsed()` |
| `crates/opencat-core/src/resolve/tree.rs` | `resolve_ui_tree()` → `ElementNode` |
| `crates/opencat-core/src/layout/mod.rs` | `LayoutSession` (Taffy + Merkle cache) |
| `crates/opencat-core/src/display/build.rs` | `DisplayBuildSession` → `DisplayTree` |
| `crates/opencat-core/src/render/analyze.rs` | Fingerprinting, reuse decision |
| `crates/opencat-core/src/render/scene.rs` | `OrderedSceneProgram` |
| `crates/opencat-core/src/render/dispatch.rs` | `render_display_tree()` → `DrawOp` emission |
| `crates/opencat-core/src/render/builder.rs` | `DrawOpBuilder` (side-table interning) |
| `crates/opencat-core/src/render/media_plan.rs` | `build_media_plan()` |
| `crates/opencat-core/src/render/cache/` | `RenderCache` (scene/segment/node-own caches) |
| `crates/opencat-core/src/ir/draw_op.rs` | `DrawOp` enum (canonical draw IR) |
| `crates/opencat-core/src/ir/draw_types.rs` | Side-table ID types (`PaintId`, `PathId`, `EffectId`, etc.) |
| `crates/opencat-core/src/ir/draw_frame.rs` | `DrawOpFrame`, `RenderFrame` |
| `crates/opencat-core/src/ir/draw_encoding.rs` | Binary envelope encoding → `EncodedDrawFrame` |
| `crates/opencat-core/src/ir/media_plan.rs` | `FrameMediaPlan` |
| `crates/opencat-core/src/ir/generated_image.rs` | `GeneratedImageTable` (color-emoji) |
| `crates/opencat-core/src/lifecycle/` | `CompositionDraft` → `prepare` → `PreparedComposition::open_pipeline()` |
| `crates/opencat-core/src/pipeline/default.rs` | `DefaultPipeline` (opened via lifecycle only) |
| `crates/opencat-core/src/pipeline/frame.rs` | `render_frame_with_state()` (per-frame orchestration) |
| `crates/opencat-core/src/pipeline/mod.rs` | `Pipeline` trait |
| `crates/opencat-core/src/frame_ctx.rs` | `FrameCtx` |
| `crates/opencat-core/src/canvas/` | Paint/Shader/Canvas API specs |
| `crates/opencat-core/src/script/` | Script runtime (animation engine) |
| `crates/opencat-core/src/style/` | `NodeStyle` (Tailwind → style) |
| `crates/opencat-core/src/text/` | Text shaping, font database, emoji |
| `crates/opencat-engine/src/pipeline.rs` | `EnginePipeline`, `open()`, `open_parsed_host_owned()` |
| `crates/opencat-engine/src/render.rs` | `render_pipeline_frame_to_rgba()`, full MP4 render |
| `crates/opencat-engine/src/executor/` | `EngineDrawExecutor` (DrawOp → Skia Canvas) |
| `crates/opencat-engine/src/consumer.rs` | `EngineLoaderFrameConsumer` (decode + execute) |
| `crates/opencat-engine/src/resource/` | `EngineLoader` (file system assets) |
| `crates/opencat-engine/src/audio_plan.rs` | Engine audio mixing |
| `crates/opencat-engine/src/inspect/browser.rs` | ChromeDriver harness, `compute_ssim_rgba()` |
| `crates/opencat-web/src/wasm_bridge.rs` | `WebRenderer` (open_design, build_frame_ir) |
| `crates/opencat-web/src/resource/` | Web resource fetching (fetch API, BlobStore) |
| `crates/opencat-web/src/consumer.rs` | `WebFrameConsumer` (encode DrawOpFrame → binary) |
| `crates/opencat-web/web/src/draw-ir.ts` | CanvasKit draw-op executor |
| `crates/opencat-web/web/src/wasm.ts` | `initWasm()`, `openDesign()`, WASM/JS glue |
| `crates/opencat/src/bin/opencat.rs` | CLI entry point |
