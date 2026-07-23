# 架构文档：渲染管线

## 概览

显式生命周期（issue #12 / #24）。core 是纯推导内核；host 拥有 fetch/cache/decode
与平台 API。DrawOp 是 **Skia-compatible IR**，由原生 Skia 与 CanvasKit 共用。

从旧 open 路径迁移到 prepare / `open_pipeline`、HostInputs、AudioPlan、RenderFrame、
OCIR v4：见 [`docs/MIGRATION.md`](docs/MIGRATION.md)。

```
输入 (XML / JSONL)
  │
  ▼
CompositionDraft::parse / from_parsed
  │
  ├── HostRequirements  (canonical AssetId + kind + 逻辑 locator；无 I/O)
  │
  ▼
Host：拉取字节 / 探测 metadata / 加载字体 / 脚本文本 / SRT
  │
  ▼
HostInputs  (ResourceMetadata + 字幕/脚本/字体内容；普通媒体 bytes 不进 core prepare)
  │
  ▼
CompositionDraft::prepare(HostInputs)  →  PreparedComposition
  │   （字体合并、字幕 hydration、脚本注入、metadata 校验）
  ▼
PreparedComposition::open_pipeline(scripts)  →  DefaultPipeline
  │
  render_frame(frame)  →  RenderFrame { draw: DrawOpFrame, media: FrameMediaPlan }
  │                         （generated image 完整 RGBA 在 media plan）
  │
  ┌───────────┴───────────┐
  Engine (Skia)           Web (CanvasKit / OCIR v4)
  MP4 / PNG               Canvas / MP4
```

**已收缩（生产路径删除）：** 双轨 `ResourceCatalog` / `HashMapResourceCatalog`、
公开 host 入口 `open_with_prepared_catalog`、`LiveScriptHost` / `ScriptRunner` /
`ScriptRuntimeCache`、`RenderSession`、`EnginePlatform`、FrameConsumer 打开路径、
engine/CLI 对 core 内部的测试便利 re-export，以及 `opencat` facade 上的长期
deprecation wrapper。

---

## 1. 输入格式

两种可互换的输入格式，最终都产生 `ParsedComposition`：

**Markup (XML)** — `crates/opencat-core/src/parse/markup/`：
```rust
parse::markup::parse(input)  // crates/opencat-core/src/parse/markup/mod.rs
```
支持 `<template>`、`<slot>`、`<transition>`、`<script>`、`<tl>` 等。模板在解析时展开。XML 路径经过 `parse::document::builder::build_parsed_document()` 产生 `ParsedDocumentParts` → 装配成 `ParsedComposition`。

**JSONL** — `crates/opencat-core/src/parse/jsonl/mod.rs`：
```rust
parse::jsonl::parse(input)  // crates/opencat-core/src/parse/jsonl/mod.rs:162
```
每行一个 JSON 对象。每行类型直接映射到 `ParsedElement`（Div、Text、Image、Video、Audio、Canvas、Icon、Path、Caption、Timeline、Transition）。然后调用 `build_tree()` 或 `build_tree_with_tl()` 组装节点树。

两者产生相同的 `ParsedComposition`（`crates/opencat-core/src/parse/document.rs:132`）：
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

`Node`（`crates/opencat-core/src/parse/node.rs:14`）包装 `Arc<NodeKind>` — 一个带类型标注的 AST 节点，持有 `NodeStyle` 和可选子节点。解析后树不可变；每帧的时间相关值在后续阶段解析。

---

## 2. 显式生命周期（Host 前置准备）

Core 从不做 I/O。普通 image/video/audio/Lottie **bytes 留在 host**；
core prepare 只消费 metadata 与内容级输入（字体、SRT 文本、脚本文本）。

### 2a. Draft + requirements

```rust
let draft = CompositionDraft::parse(input)?;
// 或 CompositionDraft::from_parsed(parsed)
let reqs = draft.requirements(); // HostRequirements
// crates/opencat-core/src/lifecycle/
```

每个 `ResourceRequest` 携带 canonical `AssetId`、`ResourceKind` 与逻辑
`ResourceLocator`（不含真实 FS 路径）。Host 按 base dir / VFS / URL 解释 locator。

### 2b. Host 填充 `HostInputs`

Host 自行 fetch/probe 后写入：

- image / video / Lottie **metadata**（`insert_image` / `insert_video` / `insert_lottie`）
- audio 存在性（`insert_audio`）
- 字幕 SRT 文本（`insert_subtitle_text`）— core 解析 SRT
- 文档字体 bytes（`insert_document_font`）— core 合并 fontdb
- 外部脚本文本（`insert_script_text`）

### 2c. Prepare + open

```rust
let prepared = draft.prepare(inputs)?;
let pipeline = prepared.open_pipeline(scripts)?;
// crates/opencat-core/src/lifecycle/mod.rs
```

prepare 校验输入（布局/时间关键 metadata 缺失 fail-fast）、合并字体、hydrate
字幕、注入脚本。`open_pipeline` 构建 pipeline 状态。生产路径不得调用内部 open helper。

生产 open：

- Engine：`opencat_engine::pipeline::open` → lifecycle
- Web：`WebRenderer::open_design` → lifecycle
- CLI：`render_*` → engine open

---

## 3. Core Pipeline 状态

### `Composition`（`crates/opencat-core/src/parse/composition.rs`）

```rust
pub struct Composition {
    pub id: String,
    pub width: i32, pub height: i32,
    pub fps: u32, pub duration: f64, pub frames: u32,
    pub root: Arc<dyn Fn(&FrameCtx) -> Node>,
    pub audio_sources: Arc<Vec<CompositionAudioSource>>,
}
```

root 是一个闭包而非静态树：每帧重新调用，使时间相关逻辑（场景切换、时间线位置）在不同帧产生不同的节点。

### Pipeline（`crates/opencat-core/src/pipeline/default.rs`）

```rust
pub struct DefaultPipeline<S: JsContext> {
    composition: Composition,
    info: CompositionInfo,
    catalog: PreparedResourceCatalog, // 仅 metadata；实现 ResourceResolver
    scripts: ScriptRealm<S>,          // 每个 pipeline 一个 realm
    layout_session: LayoutSession,
    display_build_session: DisplayBuildSession,
    composite_history: CompositeHistory,
    analyze_fingerprint_history: AnalyzeFingerprintHistory,
    font_db: Arc<fontdb::Database>,
    cache: RenderCache,
    last_ordered_scene: OrderedSceneProgram,
    generated_images: GeneratedImageTable, // 内部表；host 用 FrameMediaPlan
}
```

---

## 4. 逐帧渲染

`pipeline.render_frame(frame_index)` → `crates/opencat-core/src/pipeline/frame.rs:32`

### 逐步流程：

#### 4a. 帧上下文

```rust
let frame_ctx = FrameCtx { frame, fps, width, height, frames };
// crates/opencat-core/src/frame_ctx.rs
```
携带该帧的时间无关参数。

#### 4b. 解析 UI 树

```rust
composition.root_node(&frame_ctx)     // → Node（时间相关 AST）
resolve_ui_tree(&root, &frame_ctx, ...) // → ElementNode
// crates/opencat-core/src/resolve/tree.rs
```
`Node` 被求值成 `ElementNode` — 一个平坦的、已解析的树，包含：
- 最终化样式（百分比 → 像素，auto → 计算值）
- 已解析的变换、不透明度、裁剪
- 脚本驱动的变化（通过 `ScriptHost`）
- 从合成时间计算出的 `VideoFrameTiming`

#### 4c. 布局

```rust
layout_session.compute_layout(&element_root, &frame_ctx, &font_provider)
// crates/opencat-core/src/layout/mod.rs
```
Taffy 布局引擎。`LayoutSession` 使用 **Merkle 树缓存**：
- 每个节点对其样式 + 子节点做哈希 → `structure_subtree_hash`
- 缓存命中（`input_merkle_full_hit`）时跳过重建
- 缓存未命中时从头重建 Taffy 树
- 布局输出（rect）也做哈希 → `layout_subtree_hash`
- 布局缓存命中 → 跳过绘制节点指纹重算

输出：`LayoutTree { root: LayoutNode { id, rect, output_fingerprint, children } }`

#### 4d. 显示树

```rust
display_build_session.build(&element_root, &layout_tree, &frame_ctx)
// crates/opencat-core/src/display/build.rs
```
将 `ElementNode`（视觉语义）与 `LayoutNode`（位置）合并为 `DisplayTree`。每个 `DisplayNode` 包含：
- `DisplayItem`（位图、文本块、形状路径、画布、视频帧、图标、生成图或组）
- `CompositeSemantics`（不透明度、裁剪、背景模糊、CSS 滤镜、图层边界）
- `PaintClipInfo`（圆角裁剪追踪）

也通过 `DisplayBuildSession` 使用 Merkle 缓存。

#### 4e. 标注与分析

```rust
let mut annotated = annotate_display_tree(&display_tree);
mark_display_tree_apply_changed(composite_history, ...);
compute_display_tree_fingerprints_with_history(analyze_fingerprint_history, ...);
// crates/opencat-core/src/render/analyze.rs
```
遍历显示树产生 `AnnotatedDisplayTree`：
- `AnnotatedNodeHandle` → 平坦 arena 的类型化索引
- 每个节点有 `subtree_fingerprint`（自身内容 + 子节点指纹的哈希）
- `AnalyzeReuseState`：Fresh / ReusedFromHistory / CompositeBlocked
- 场景快照：如果帧的 `root_fingerprint` 与上一帧相同，重绘整个 `DrawOpFrame`（缓存命中），跳过所有渲染

#### 4f. 场景程序

```rust
let ordered_scene = OrderedSceneProgram::build(&annotated);
// crates/opencat-core/src/render/scene.rs
```
生成 `OrderedSceneOp` 的 DAG — 要么是 `LiveSubtree`（需要完整渲染），要么是 `ReusedSubtree`（可以重放缓存的绘制指令）。这是渲染调度的遍历顺序。

#### 4g. 渲染调度 → DrawOpFrame

```rust
render_display_tree(&mut ctx, &annotated, &mut cache)
// crates/opencat-core/src/render/dispatch.rs:480
```
遍历 `OrderedSceneProgram`，向 `DrawOpBuilder` 发出 `DrawOp`。关键缓存机制：
- **场景快照**：如果根指纹与上一帧匹配，直接返回缓存的 `DrawOpFrame`
- **节点自有段缓存**：节点自身渲染（不含子节点）被缓存；只有子节点变化时，父节点的自有段从缓存导入
- **Apply 段缓存**：变换/不透明度/图层设置按 composite plan 缓存

产生 `DrawOpFrame { ops, subtrees, paints, paths, strings, bytes, f32_pool, ... }`。

#### 4h. 媒体计划

```rust
let media_plan = build_media_plan(&frame);
// crates/opencat-core/src/render/media_plan.rs
```
遍历完成的 `DrawOpFrame`，收集：
- `images`：静态图片引用（用于解码）
- `video_frames`：视频帧引用 + `time_micros`（用于 seek + 解码）
- `lottie_bundles`：Lottie 动画的 bundle ID
- `runtime_effects`：SkSL 着色器引用（用于编译）
- `generated_images`：颜色-emoji 位图 ID

---

## 5. DrawOp IR

`crates/opencat-core/src/ir/draw_op.rs:162`

跨平台的规范绘制指令集。分为以下类别：

| 类别 | 指令 |
|------|------|
| 栈管理 | `Save`、`SaveLayer`、`Restore`、`RestoreToCount` |
| 变换 | `Translate`、`Scale`、`Rotate`、`Skew`、`Concat` |
| 画笔状态 | `SetFillStyle`、`SetStrokeStyle`、`SetLineWidth`、`SetLineCap`、`SetLineJoin`、`SetLineDash`、`ClearLineDash`、`SetGlobalAlpha`、`SetAntiAlias` |
| 路径 | `BeginPath`、`Path(PathOp)`、`FillPath`、`StrokePath`、`ClipPath` |
| 绘制 | `Clear`、`Paint`、`Rect`、`RRect`、`DRRect`、`Oval`、`Circle`、`Arc`、`Line`、`Points`、`DrawPath`、`Image`、`ImageRect` |
| 媒体 | `LottieRect`（skottie 动画） |
| 着色器 | `RuntimeEffect`（带 uniform 和子输入的 SkSL） |
| 缓存 | `ReplayRange`、`DrawSubtreePicture`、`ReplaySubtreePicture` |
| 脚本 | `ScriptRuntimeEffect`（中间形式，编码前解析） |

所有变体的负载通过 **ID 引用侧表**（`PaintId`、`PathId`、`EffectId`、`StringId`、`ImageRef`、`F32Range` 等），实现去重和紧凑的二进制编码。

---

## 6. 二进制编码（Web 传输）

`crates/opencat-core/src/ir/draw_encoding.rs`

`encode_draw_frame()` 将 `DrawOpFrame` 序列化为 `EncodedDrawFrame` — 一组平坦的 `Vec<u8>`/`Vec<f32>`/`Vec<TableRange>`，通过 wasm-bindgen 作为类型化数组传递给 JS。

二进制信封格式：
- **Section 1 — 指令流**：小端序指令流（opcode u16 + flags u16 + payload_len u32 + payload）
- **Section 2 — f32 池**：平坦 f32 数组（Points、SetLineDash 等共享）
- **Section 3 — 字符串**：UTF-8 拼接 + 范围表
- **Section 4 — 子树**：长度前缀的指令流（隐藏图片子树）
- **Section 12 — 生成图增量**：新颜色-emoji 字形的 RGBA

JS 侧（`crates/opencat-web/web/src/draw-ir.ts`）：
- `decodeFrame()` 解析信封为 `DecodedFrame`
- `renderEncodedDrawFrame()` 在 CanvasKit `Canvas` 上重放指令
- 每个 opcode 通过 `OPCODES` 调度表映射到 CanvasKit API 调用

---

## 7. Engine（Skia GPU）实现

`crates/opencat-engine/`

### 7a. 打开 Pipeline

```rust
pub fn open(input: &str, loader: EngineLoader, scripts: RqJsContext) -> Result<EnginePipeline>
// crates/opencat-engine/src/pipeline.rs:98
```
Engine 的 `open` 函数实现了完整的 Host 链：
1. 解析输入（markup 或 JSONL）
2. `loader.load_font_manifest()` → 获取声明的字体字节
3. `engine_font_db_with_document_fonts()` → 构建 `Arc<fontdb::Database>`
4. `build_parsed_document()`（仅 markup，展开模板）
5. `open_parsed_host_owned()`（下面的链）

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
2. **Engine 获取资源**（通过 `EngineLoader`，文件系统读取 + 缓存）
3. Host 从字节探测元数据并通过 `insert_*` 方法构建 `HostInputs`
4. `draft.prepare(inputs)` — 校验输入、合并字体、水合字幕
5. `PreparedComposition::open_pipeline()`

### 7b. 帧渲染 → RGBA

```rust
fn render_pipeline_frame_to_rgba(...) -> Result<Vec<u8>>
// crates/opencat-engine/src/render.rs:61
```
1. `pipeline.render_frame(frame_index)` → `RenderFrame`
2. 创建 Skia 光栅表面（`RasterN32Premul`）
3. `EngineLoaderFrameConsumer::consume_frame()`：
   - `prepare_frame()`（`crates/opencat-engine/src/consumer.rs:82`）：
     - 从文件路径解码静态图片 → Skia `Image`
     - 通过 `MediaContext::frame_rgba_at_time_by_path()` 寻址视频帧 → Skia `Image`
     - 从 SkSL 构建 `RuntimeEffect`
     - 从 `GeneratedImageTable` 查找生成图（颜色-emoji）
     - 加载 Lottie 动画到缓存
   - `executor.execute()`（`crates/opencat-engine/src/executor/mod.rs:80`）：
     - `begin_frame()` 重置画笔/路径状态
     - `replay_frame()`（`crates/opencat-engine/src/executor/replay.rs`）：遍历 `DrawOpFrame.ops`，将每个 `DrawOp` 映射到 `skia_safe::Canvas` API
4. `surface.image_snapshot().read_pixels()` → RGBA `Vec<u8>`

### 7c. MP4 编码

Engine 使用 `ffmpeg-next` 将渲染帧编码为 MP4。音频混音通过 `audio_plan` 模块。Lottie 使用 `skia-safe` 中的 `skottie`。

---

## 8. Web（CanvasKit WASM）实现

`crates/opencat-web/`

### 8a. WASM Pipeline 打开

```rust
WebRenderer::open_design(&mut self, source: String) -> Result<String>
// crates/opencat-web/src/wasm_bridge.rs:145
// → open_design_pipeline() 第 414 行
```
1. `preload_assets(source)`（`crates/opencat-web/src/resource/wasm_api.rs:65`）：
   - 下载所有资源（字体、图片、视频、Lottie、字幕）
   - 将字节存入线程本地 `BlobStore`（以 `AssetId` 为键）
   - 通过 host 侧 probe 函数探测元数据
   - 递归扫描 Lottie 依赖
2. 构建字体数据库（默认 NotoSansSC + NotoColorEmoji + 文档字体）
3. 使用字体数据库解析源码
4. `collect_resource_requests_from_parsed()`
5. 通过 `insert_*` 方法从 host 探测的元数据构建 `HostInputs`
6. `draft.prepare(inputs)` — 校验输入、合并字体、水合字幕
7. `PreparedComposition::open_pipeline()`

### 8b. 帧渲染 → 二进制 IR

```rust
WebRenderer::build_frame_ir(&mut self, frame: u32) -> Result<Vec<u8>>
// crates/opencat-web/src/wasm_bridge.rs:177
```
1. `pipeline.render_frame(frame)` → `RenderFrame`
2. `WebFrameConsumer::consume_frame()`（web consumer）：
   - `encode_draw_frame()` → 二进制 `EncodedDrawFrame`
   - 追加生成图增量（自上帧以来的新颜色-emoji 字形）
3. 返回二进制信封给 JS

### 8c. JS CanvasKit 执行

```
build_frame_ir(f)     → Uint8Array（Rust wasm-bindgen）
decodeFrame(bytes)    → DecodedFrame  (draw-ir.ts)
renderEncodedDrawFrame(decoded, canvas) → void
```

`renderEncodedDrawFrame`（`crates/opencat-web/web/src/draw-ir.ts`）：
- 遍历解码后的指令流
- 每个 opcode 分派到对应的 CanvasKit API：
  - `Save`/`Restore` → `canvas.save()`/`canvas.restore()`
  - `Translate`/`Scale`/`Rotate`/`Concat` → `canvas.translate()` 等
  - `Rect`/`RRect`/`Circle`/`Oval` → `canvas.drawRect()`（使用内联 Paint）
  - `Image`/`ImageRect` → `canvas.drawImage*()`（通过 `loadResourceBytes()` 获取图片）
  - `LottieRect` → `skottie.animation.seekFrame()` + `canvas.drawSkottie()`
  - `RuntimeEffect` → `canvas.drawRect()` + `RuntimeEffect.makeShader()`
  - `ReplaySubtreePicture` → 在子画布上重放子树指令流，然后作为 picture shader 绘制
- Paint 内联引用通过 `buildPaintById()` 解析 → `new CK.Paint()`
- 图片引用：静态 → `loadResourceBytes()`，视频 → 解码帧，生成图 → 缓存 RGBA（来自增量）

### 8d. 媒体准备（JS 侧）

`FrameMediaPlan`（通过 `prepare_frame()` 返回 JSON）告诉 JS 需要准备哪些媒体。JS 导出器使用 `@webav/av-cliper` 进行视频解码，`WebCodecs` 进行编码。

---

## 9. 关键设计原则

### 关注点分离

```
Core： ParsedComposition → ResourceRequests → Layout → DisplayTree → DrawOpFrame
Host： 获取字节 → 探测元数据 → 构建 HostInputs → 字体数据库 → [Pipeline] → 解码 → 执行
```

Core 从不做 I/O。它声明需要的资源（`ResourceRequests`），通过显式生命周期（`CompositionDraft` → `HostInputs` → `prepare`）校验 host 提供的元数据，产生确定性的绘制指令（`DrawOpFrame`）。Host（engine Skia、web CanvasKit）各自独立实现资源获取、探测和像素输出。

### 增量/缓存渲染

从粗到细的三层缓存：

1. **场景快照**（`RenderCache.last_scene_snapshot`）：如果整帧的 `root_fingerprint` 与上一帧匹配，重用整个 `DrawOpFrame`。对无变化的帧是零操作。

2. **子树重用**（`AnalyzeFingerprintHistory`）：每个显示子树有 `subtree_fingerprint`。当与历史匹配时，节点标记为 `ReusedSubtree`，导入缓存的 `DrawOp` 段——只重新发射 composite 层（变换 + 不透明度 + 裁剪）。

3. **节点自有段缓存**（`node_own_segments`）：节点自身的渲染（它的 item + 裁剪，不含子节点）独立缓存。当只有子节点变化时，父节点的绘制指令从缓存导入，无需重新录制。

### 确定性构造

`RenderFrame { draw: DrawOpFrame, media: FrameMediaPlan }` 是 `(pipeline, frame_index)` 的纯函数。同一 pipeline 的同一帧总是产生字节完全相同的绘制指令。这使得 engine 和 web 之间的 SSIM 回归测试成为可能。

### 跨语言传输的二进制 IR

`EncodedDrawFrame` 格式桥接 Rust（WASM）和 JS/CanvasKit。绘制命令不通过 JSON 序列化（慢、冗长），而是打包成紧凑的二进制信封（opcode + payload_len + payload）。侧表（paints、paths、strings、f32_pool）在构建时去重和内联，然后平坦编码，通过 wasm-bindgen 类型化数组实现零拷贝传输。

---

## 10. 文件映射

| 路径 | 作用 |
|------|------|
| `crates/opencat-core/src/parse/` | XML/JSONL 解析 → `ParsedComposition` |
| `crates/opencat-core/src/parse/composition.rs` | `Composition` 结构体（时间相关根节点） |
| `crates/opencat-core/src/parse/node.rs` | `Node`（解析 AST，`Arc<NodeKind>`） |
| `crates/opencat-core/src/parse/markup/` | XML 解析器（含模板） |
| `crates/opencat-core/src/parse/jsonl/` | JSONL 解析器 |
| `crates/opencat-core/src/parse/jsonl/tailwind.rs` | Tailwind class → `NodeStyle` |
| `crates/opencat-core/src/probe/prepare.rs` | `hydrate_captions()`、`parse_srt()` |
| `crates/opencat-core/src/parse/preflight.rs` | `collect_resource_requests_from_parsed()` |
| `crates/opencat-core/src/resolve/tree.rs` | `resolve_ui_tree()` → `ElementNode` |
| `crates/opencat-core/src/layout/mod.rs` | `LayoutSession`（Taffy + Merkle 缓存） |
| `crates/opencat-core/src/display/build.rs` | `DisplayBuildSession` → `DisplayTree` |
| `crates/opencat-core/src/render/analyze.rs` | 指纹分析、重用决策 |
| `crates/opencat-core/src/render/scene.rs` | `OrderedSceneProgram` |
| `crates/opencat-core/src/render/dispatch.rs` | `render_display_tree()` → `DrawOp` 发射 |
| `crates/opencat-core/src/render/builder.rs` | `DrawOpBuilder`（侧表内联） |
| `crates/opencat-core/src/render/media_plan.rs` | `build_media_plan()` |
| `crates/opencat-core/src/render/cache/` | `RenderCache`（场景/段/节点自有缓存） |
| `crates/opencat-core/src/ir/draw_op.rs` | `DrawOp` 枚举（规范绘制 IR） |
| `crates/opencat-core/src/ir/draw_types.rs` | 侧表 ID 类型（`PaintId`、`PathId`、`EffectId` 等） |
| `crates/opencat-core/src/ir/draw_frame.rs` | `DrawOpFrame`、`RenderFrame` |
| `crates/opencat-core/src/ir/draw_encoding.rs` | 二进制信封编码 → `EncodedDrawFrame` |
| `crates/opencat-core/src/ir/media_plan.rs` | `FrameMediaPlan` |
| `crates/opencat-core/src/ir/generated_image.rs` | `GeneratedImageTable`（颜色-emoji） |
| `crates/opencat-core/src/lifecycle/` | `CompositionDraft` → `prepare` → `PreparedComposition::open_pipeline()` |
| `crates/opencat-core/src/pipeline/default.rs` | `DefaultPipeline`（仅经 lifecycle 打开） |
| `crates/opencat-core/src/pipeline/frame.rs` | `render_frame_with_state()`（逐帧编排） |
| `crates/opencat-core/src/pipeline/mod.rs` | `Pipeline` trait |
| `crates/opencat-core/src/frame_ctx.rs` | `FrameCtx` |
| `crates/opencat-core/src/canvas/` | Paint/Shader/Canvas API 规范 |
| `crates/opencat-core/src/script/` | 脚本运行时（动画引擎） |
| `crates/opencat-core/src/style/` | `NodeStyle`（Tailwind → 样式） |
| `crates/opencat-core/src/text/` | 文字排版、字体数据库、emoji |
| `crates/opencat-engine/src/pipeline.rs` | `EnginePipeline`、`open()`、`open_parsed_host_owned()` |
| `crates/opencat-engine/src/render.rs` | `render_pipeline_frame_to_rgba()`、完整 MP4 渲染 |
| `crates/opencat-engine/src/executor/` | `EngineDrawExecutor`（DrawOp → Skia Canvas） |
| `crates/opencat-engine/src/consumer.rs` | `EngineLoaderFrameConsumer`（解码 + 执行） |
| `crates/opencat-engine/src/resource/` | `EngineLoader`（文件系统资源） |
| `crates/opencat-engine/src/audio_plan.rs` | Engine 音频混音 |
| `crates/opencat-engine/src/inspect/browser.rs` | ChromeDriver harness、`compute_ssim_rgba()` |
| `crates/opencat-web/src/wasm_bridge.rs` | `WebRenderer`（open_design、build_frame_ir） |
| `crates/opencat-web/src/resource/` | Web 资源获取（fetch API、BlobStore） |
| `crates/opencat-web/src/consumer.rs` | `WebFrameConsumer`（编码 DrawOpFrame → 二进制） |
| `crates/opencat-web/web/src/draw-ir.ts` | CanvasKit 绘制指令执行器 |
| `crates/opencat-web/web/src/wasm.ts` | `initWasm()`、`openDesign()`、WASM/JS 粘合 |
| `crates/opencat/src/bin/opencat.rs` | CLI 入口 |
