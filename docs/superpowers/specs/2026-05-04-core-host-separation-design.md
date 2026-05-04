# Core / Host 分离设计

> 本 spec 来自 brainstorming 阶段的收敛结果，目标是将 opencat 的 "JSONL → DisplayList" 主流程纯化为可独立编译、无 IO/无 ffmpeg/无 quickjs/无 skia 依赖的 `core`，把资源 IO、媒体编解码、JS 脚本运行时、平台渲染下沉到 `host`。本次重构**不打算立即输出 wasm 包**，但所有边界都按 wasm 友好原则设计。

## 1. 目标与非目标

### 1.1 目标

1. **Core 纯净度**：`opencat::core::*` 的所有公开 API 在不开任何 host feature 时也能 `cargo check` 通过。Core 在源码层面不出现 `ffmpeg_next`、`reqwest`、`tokio`、`rquickjs`、`skia_safe`、`rodio` 的任何符号。
2. **依赖翻转**：所有外部副作用通过定义在 core 内的 trait 注入（`ResourceCatalog`、`ScriptHost`、`FontProvider`）。Core 不主动持有可执行 runtime（无 `tokio::Runtime`，无 `rquickjs::Runtime`，无 `skia::Surface`）。
3. **数据轻量**：Core 与 host 的边界数据结构只含描述性元信息（`AssetId`、宽高、时长、`VideoFrameTiming`），不传字节流、不传 path 给 core 解读、不传 backend 句柄给 core 解读。
4. **行为等价**：重构后 native binary `opencat` / `opencat-see` 输出的视频/图像逐帧像素与重构前一致。所有现有测试通过。

### 1.2 非目标

- **不输出 wasm 产物**：本 spec 只到 "core 在 cargo features 切除 host 依赖时仍能编译通过" 为止。wasm-bindgen 胶水、CanvasKit backend、WebCodecs 资源 host 留待下一个 spec。
- **不拆 cargo workspace**：单 crate + features，避免一次重构里既动模块又动构建系统。workspace 拆分留待 wasm spec。
- **不改业务行为**：不调整 jsonl 协议、不改样式语义、不增减用户可见特性。
- **不优化性能**：保持现有缓存策略、指纹算法、复用判断不变；trait 调用引入的间接成本由编译器内联吸收。

## 2. 当前结构与破口（基线快照）

| 模块 | 当前位置 | core/host 应归属 | 当前破口 |
|---|---|---|---|
| jsonl 解析 | `src/jsonl.rs`、`src/jsonl/` | core | `parse_file` 直接读文件 |
| scene 树 | `src/scene/` | core（除 script 实现） | `script/{node_style,canvas_api,animate_api,morph_svg}.rs` 持 `rquickjs` |
| element resolve | `src/element/resolve.rs` | core | `resolve_video` 调 `media.video_info(path)` 触发 ffmpeg |
| layout | `src/layout/` | core | 内部依赖 `fontdb::Database`，加载需 IO |
| display list/build/tree | `src/display/` | core | `build_display_tree` 接 `_assets: &AssetsMap`（参数未使用） |
| AssetsMap | `src/resource/assets.rs` | core 仅留映射；下载/preload 移 host | 类内含 `tokio::Runtime`、`reqwest` 调用 |
| MediaContext | `src/resource/media.rs` | host | 调 ffmpeg、用 `skia_safe::Image` 解码图片 |
| codec | `src/codec/` | host | `ffmpeg_next` 直接依赖 |
| script driver | `src/scene/script/mod.rs` | mutations 数据 → core；runner → host | `ScriptRunner` 持 `rquickjs::{Runtime,Context,Persistent}` |
| backend skia | `src/backend/skia/` | host | 整模块 `skia_safe` |
| runtime/cache | `src/runtime/cache/` | host | 持 `skia::Image / Picture` |
| runtime/fingerprint | `src/runtime/fingerprint/` | core | `item_is_time_variant` 通过 `assets.path()` 推 video，可改用 `video_timing.is_some()` |
| runtime/annotation | `src/runtime/annotation.rs` | core | 透传 `&AssetsMap` 给 fingerprint |
| runtime/invalidation | `src/runtime/invalidation/` | core | 纯算法 |
| runtime/analysis | `src/runtime/analysis.rs` | core | 纯结构 |
| runtime/compositor/{ordered_scene,plan,reuse,slot} | `src/runtime/compositor/` | core | 纯算法 |
| runtime/compositor/render | `src/runtime/compositor/render.rs` | host | 持 `MediaContext + AssetsMap + SkiaRenderEngine` |
| runtime/{render_engine,render_registry,session,target,surface,profile,frame_view,backend_object,audio} | `src/runtime/` | host | backend 集成与会话 |
| runtime/preflight | `src/runtime/preflight.rs` | 拆：collect→core，ensure→host | `ensure_assets_preloaded` 触发下载 |
| runtime/pipeline | `src/runtime/pipeline.rs` | 拆：build→core，render→host | |

## 3. 目标模块结构

### 3.1 顶层目录

```
src/
  core/                ← 编译时永不依赖 host features
    jsonl/
    scene/
      primitives/
      composition.rs
      node.rs
      time.rs
      transition.rs
      easing.rs
      script/
        mutations.rs       ← StyleMutations / NodeStyleMutations / CanvasMutations / CanvasCommand
        host.rs            ← ScriptHost trait + ScriptDriver{source} 数据
    element/
    layout/
    display/
    style/
    text/                  ← cosmic-text 是纯 Rust，留 core
    resource/
      asset_id.rs          ← AssetId 与稳定哈希（asset_id_for_url/query/audio_*）
      types.rs             ← ImageSource / AudioSource / OpenverseQuery / VideoFrameTiming / VideoInfoMeta
      catalog.rs           ← ResourceCatalog trait
    runtime/               ← 仅纯算法子集
      analysis.rs
      annotation.rs
      fingerprint/
      invalidation/
      compositor/
        ordered_scene.rs
        plan.rs
        reuse.rs
        slot.rs
      preflight_collect.rs ← collect_resource_requests 的实现（树遍历）
      pipeline.rs          ← build_frame_display_tree 的实现
    frame_ctx.rs
    inspect.rs
    lib.rs                 ← 暴露 core 公开 API

  host/                  ← 默认 feature 全开
    resource/
      asset_catalog.rs     ← struct AssetCatalog impl ResourceCatalog（含状态：HashMap）
      fetch.rs             ← preload_image_sources / preload_audio_sources（reqwest + tokio）
      probe.rs             ← 用 codec 探测 (width,height,duration) 后写回 catalog
      media.rs             ← MediaContext（ffmpeg 帧采样）
      preflight.rs         ← ensure_assets_preloaded（驱动 fetch + probe）
    codec/                 ← decode/encode（ffmpeg）
    script/
      quickjs.rs           ← struct QuickJsScriptHost impl ScriptHost
      bindings/            ← 现 node_style/canvas_api/animate_api/morph_svg 的 binding 部分
      runtime/             ← 现 *.js prelude 文件
    backend/
      skia/                ← 不变
    runtime/
      session.rs           ← RenderSession
      pipeline.rs          ← render_frame_on_surface
      compositor_render.rs ← 原 compositor/render.rs
      cache/
      render_engine.rs / render_registry.rs / target.rs / surface.rs / profile.rs / frame_view.rs / backend_object.rs / audio.rs
    bin/                   ← opencat / opencat-see
    fonts.rs               ← FontProvider 默认实现 + 系统字体加载

  lib.rs                 ← pub mod core; pub mod host; 顶层 re-export
```

### 3.2 cargo features（`Cargo.toml`）

```toml
[features]
default = ["host-default"]
host-default = ["host-codec", "host-script-quickjs", "host-resource-net", "host-backend-skia", "host-audio"]

# 单独 feature，便于未来 wasm host 替换
host-codec        = ["dep:ffmpeg-next"]
host-script-quickjs = ["dep:rquickjs"]
host-resource-net = ["dep:reqwest", "dep:tokio"]
host-backend-skia = ["dep:skia-safe"]
host-audio        = []   # 仅触发 cfg(any(target_os="macos",target_os="windows")) 下的 rodio
```

`src/host/` 整个模块在 `#[cfg(feature = "host-default")]` 下编译；子模块用更细 feature 控制。

**`rodio` 的 target-cfg 处理**：现 `Cargo.toml` 已经把 `rodio` 写在 `[target.'cfg(any(target_os = "macos", target_os = "windows"))'.dependencies]`。重构保持这一约束不变，仅在 `host-audio` feature 控制 audio host 模块是否编译；linux 上 `host-default` 包含 `host-audio` 但 `rodio` 不被拉入（target-cfg 优先），audio 模块需在 linux 下走静默 stub 实现（与现状一致）。

**核心纯净度的硬性证明**：CI 增加一条 `cargo check --no-default-features --lib`，必须通过。

### 3.3 顶层 re-export 兼容性

为避免现有 binary 与 examples 大面积 broken，`src/lib.rs` 在 `host-default` 下保留现有 `pub use ...` 的全部符号路径（`opencat::parse_file`、`opencat::RenderSession` 等），仅内部实现挪到 `core::` / `host::` 子模块。新代码**鼓励**用 `opencat::core::*` 与 `opencat::host::*`。

## 4. 关键 trait 定义

### 4.1 `ResourceCatalog`（`core::resource::catalog`）

```rust
pub trait ResourceCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId>;
    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId>;
    /// 给一段不透明的资源定位字符串注册尺寸。Native host 传 path.to_string_lossy()，
    /// wasm host 传 blob URL 或自定义 scheme（"webasset://0001"）。
    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId;
    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()>;
    fn dimensions(&self, id: &AssetId) -> (u32, u32);
    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}
```

**契约**：
- `resolve_image / resolve_audio` 必须返回稳定 `AssetId`（同一 source 多次调用结果相等）。对 `Url` / `Query`：必须由 host 在 preflight 阶段已 preloaded，否则返回 `Err`。
- `register_dimensions` 接受**不透明 `&str` locator**，不是 `&Path`。Native host 传 `path.to_string_lossy().as_ref()`；wasm host 传 blob URL 或自定义 scheme。该参数仅用于在 catalog 内构造稳定 `AssetId`，core 不解读它的语义。
- `register_dimensions` 返回的 `AssetId` 与 native 现 `AssetsMap::register_dimensions(path, ...)` 在传 `path.to_string_lossy()` 时**逐字节相等**（保证迁移行为不变）。
- `video_info` 在未 probe 的视频上返回 `None`；core 路径的代码必须能容忍 `None`（fallback 到 0×0，与现 `unwrap_or_else` 行为一致）。
- 错误类型统一使用 `anyhow::Error`（已是 workspace 通用）。

### 4.2 `ScriptHost`（`core::scene::script::host`）

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScriptDriverId(pub u64);

pub trait ScriptHost {
    /// host 内部对 source 字符串去重，返回稳定 driver id。
    /// 同一 source 多次调用必须返回相等的 ScriptDriverId。
    fn install(&mut self, source: &str) -> Result<ScriptDriverId>;
    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
    ) -> Result<StyleMutations>;
}
```

**契约**：
- `install` 同一 source 字符串多次调用必须返回相等的 `ScriptDriverId`。**ID 生成算法 host 自决**（不要求与现 quickjs 实现的 hash 算法一致），core 不感知该算法。
- core 内部不缓存 `(source_string → ScriptDriverId)` 的映射；core 仅缓存 `(scene_node_id → ScriptDriverId)`，每次遇到一段 script source 都通过 `host.install(source)` 拿 id（host 的去重保证 id 稳定）。
- `run_frame` 是 core 路径上**唯一**触发用户 JS 执行的入口。Core 不感知 quickjs / browser eval。
- `StyleMutations`、`NodeStyleMutations`、`CanvasMutations`、`CanvasCommand` 等数据类型全部留在 core。
- text source 注册由 element resolve 阶段触发（与现 `ScriptRuntimeCache::register_text_source` 时机一致）。
- `run_frame` 返回 `Err` 时，core 必须把错误冒泡到 `build_frame_display_tree` 调用方；不静默吞错。

### 4.3 `FontProvider`（`core::text::provider`）

```rust
pub trait FontProvider {
    fn font_db(&self) -> &fontdb::Database;
}
```

简单到 host 给 `struct DefaultFontProvider(Arc<fontdb::Database>)` 就行。Core 内 layout 通过 `&dyn FontProvider` 拿 `font_db`。

## 5. Core 公共入口

`src/core/lib.rs` 暴露三个无副作用函数 + 必要数据类型：

```rust
// 1) 解析 jsonl 文本
pub fn parse(text: &str) -> Result<ParsedComposition>;

// 2) 扫描 ParsedComposition，输出资源需求清单
pub fn collect_resource_requests(parsed: &ParsedComposition) -> ResourceRequests;

// 3) 构建一帧 display tree
pub fn build_frame_display_tree(
    parsed: &ParsedComposition,
    frame_ctx: &FrameCtx,
    catalog: &mut dyn ResourceCatalog,
    fonts: &dyn FontProvider,
    script: Option<&mut dyn ScriptHost>,
) -> Result<AnnotatedDisplayTree>;
```

`ResourceRequests` 结构：

```rust
pub struct ResourceRequests {
    /// Image source 需要 host fetch 后才能 resolve_image
    pub image_sources: Vec<ImageSource>,
    /// Audio source 需要 host fetch 后才能 resolve_audio
    pub audio_sources: Vec<AudioSource>,
    /// 视频路径需要 host probe 出 (width,height,duration)
    pub video_paths: Vec<PathBuf>,
}
```

**保证**：`collect_resource_requests` 与 `build_frame_display_tree` 都是同步、无 IO、单线程。

## 6. 数据流

```
[jsonl text]
    │ core::parse
    ▼
ParsedComposition (Arc<Node>, audio_sources, script source, w/h/fps/frames)
    │ core::collect_resource_requests
    ▼
ResourceRequests
    │ host::resource::preflight::fulfill(requests, &mut catalog)
    │   - fetch URLs / Openverse → 写入 catalog
    │   - probe videos → register_dimensions + video_info_meta 写入 catalog
    ▼ (catalog 已就绪)
for each frame:
    │ core::build_frame_display_tree(parsed, frame_ctx, &mut catalog, &fonts, Some(&mut script_host))
    │   - element::resolve_ui_tree (查 catalog, 不触发 IO)
    │   - layout::compute_layout (用 fonts)
    │   - display::build_display_tree
    │   - runtime::annotate (纯算法)
    │   - runtime::fingerprint (纯算法)
    ▼
AnnotatedDisplayTree
    │ host::runtime::pipeline::render_frame_on_surface(annotated, frame_ctx, session, target)
    │   - host::compositor_render（消费 catalog 的 path/dimensions + MediaContext 采视频帧）
    │   - backend (skia) 出 RGBA / 编码
    ▼
帧像素
```

## 7. 现有代码改造（按文件）

### 7.1 `display::build::build_display_tree` 删冗余参数
- 删 `_assets: &AssetsMap` 参数及调用点。

### 7.2 `element::resolve` 摆脱 MediaContext
- `resolve_video`：去掉 `&mut MediaContext` 参数，从 `cx.assets.video_info(&asset_id)` 拿 `VideoInfoMeta`，未命中走 `(0, 0, None)` fallback。
- `resolve_image`：维持现状（已经只走 `assets.register_image_source` + `assets.dimensions`）。
- `resolve_canvas`：维持现状（`assets.register_image_source` + `alias`）。
- `resolve_ui_tree*` 顶层入口：去掉 `media: &mut MediaContext` 参数。
- 调用方（`runtime::pipeline::build_scene_display_list`）相应去掉 `&mut session.media_ctx` 实参。

### 7.3 `resource::assets` 拆 IO
- 新文件 `core::resource::asset_catalog`：保留 `register / register_image_source / register_audio_source / register_dimensions / register_video_info / alias / dimensions / path / video_info / require_preloaded` 等纯映射方法；移除 `preload_runtime` 字段；不再 `use reqwest / tokio`。
- 重命名为 `AssetCatalog`，`AssetsMap` 在 host re-export 处保留为 type alias 以最小化外部改动。
- 给 `AssetCatalog` 实现 `ResourceCatalog` trait。
- 新增字段 `video_info_meta: HashMap<AssetId, VideoInfoMeta>`，`video_info(id)` 查这张表；`register_video_info(path, info)` 同时调 `register_dimensions` 与写表。
- 新文件 `host::resource::fetch`：搬迁 `preload_image_sources / preload_audio_sources` 与所有内部辅助（`prepare_remote_image_requests`、Openverse token 流程、`HTTP_USER_AGENT` 常量等）；签名变成自由函数 `pub fn preload_image_sources(catalog: &mut AssetCatalog, sources: HashSet<ImageSource>) -> Result<()>`，内部自管 tokio runtime。

### 7.4 `resource::media` 整体 host 化
- `MediaContext` 整体搬到 `host::resource::media`。
- 新增 host 函数 `pub fn probe_video(catalog: &mut AssetCatalog, path: &Path, media: &mut MediaContext) -> Result<()>`：内部调 `media.video_info(path)`，写回 `catalog.register_video_info(path, info)`。
- `VideoFrameTiming / VideoFrameRequest / VideoPreviewQuality` 搬到 `core::resource::types`（纯描述）。
- `VideoInfo`（含 `duration_secs: Option<f64>`）：原结构留在 host 的 `media.rs`；新增 `core::resource::types::VideoInfoMeta` 是它的纯 core 镜像，二者字段一致。

### 7.5 `runtime::preflight` 拆两半
- core 侧 `core::runtime::preflight_collect`：保留 `collect_sources / collect_sources_from_frame_state`，对外暴露 `collect_resource_requests(parsed) -> ResourceRequests`。
- host 侧 `host::resource::preflight`：保留 `ensure_assets_preloaded(composition, session) -> Result<()>`，内部依次调用 `core::collect_resource_requests`、`fetch::preload_image_sources`、`fetch::preload_audio_sources`、`probe::probe_videos_in_requests`。

### 7.6 `runtime::pipeline` 拆两半
- core 侧 `core::runtime::pipeline::build_frame_display_tree`：原 `build_scene_display_list` 的全部逻辑，签名换成 trait 注入。
- host 侧 `host::runtime::pipeline::render_frame_on_surface`：保留外部入口，内部先调 core 拿 `AnnotatedDisplayTree`，再驱动 backend。

### 7.7 `scene::script` 拆 mutations / runner
- core 留：`mutations.rs`（`StyleMutations / NodeStyleMutations / CanvasMutations / CanvasCommand / TextUnitOverrideBatch / ScriptColor / ScriptFontEdging / ScriptLineCap / ScriptLineJoin / ScriptPointMode`）+ `host.rs`（`ScriptHost` trait + `ScriptDriverId` + `ScriptDriver{source}` + `ScriptTextSource{...}`）。
- host 侧 `host::script::quickjs`：搬迁 `ScriptRunner / install_runtime_bindings / map_js_result / RUN_FRAME_FN`。
- host 侧 `host::script::bindings::{node_style, canvas_api, animate_api, morph_svg}`：搬迁现有 `*_api.rs`，binding 函数把 mutations 写入 host 内部的 `Arc<Mutex<RuntimeMutationStore>>`，最后 collect 出 `StyleMutations`（core 类型）返回给 core。
- host 侧 `host::script::runtime::*.js`：搬迁现 `runtime/*.js` prelude（`include_str!` 路径相应调整）。
- 新结构 `QuickJsScriptHost { runners: HashMap<u64, ScriptRunner>, text_sources: HashMap<String, ScriptTextSource> }` 实现 `ScriptHost` trait。`install(source)` 用 source 的 FxHash 作为 `ScriptDriverId`（与现 `ScriptRunner` 的 cache 键策略一致）。
- core 路径上 `ScriptRuntimeCache` 改名为 `ScriptDriverCache { drivers: HashMap<u64, ScriptDriverId>, text_sources: HashMap<String, ScriptTextSource> }`，仅作为 driver id 缓存。

### 7.8 `runtime::fingerprint / annotation` 去 AssetsMap
- `item_is_time_variant(item)` / `bitmap_is_video(bitmap)`：改用 `bitmap.video_timing.is_some()` 判断是否为视频。删掉 `assets: &AssetsMap` 参数。
- `classify_paint(item)` / `item_paint_fingerprint(item)`：删 `assets: &AssetsMap` 参数。
- `annotate_display_tree(tree)` / `compute_display_tree_fingerprints(tree)`：删 `assets: &AssetsMap` 参数。
- 调用方相应清理（`runtime::pipeline::build_scene_display_list` 内）。
- 单元测试：原先 `let mut assets = AssetsMap::new(); ...` 的 setup 删除。

### 7.9 `runtime::compositor` 拆纯算法 vs 含 backend
- core：`ordered_scene.rs / plan.rs / reuse.rs / slot.rs` 整体搬入 `core::runtime::compositor`。
- host：`render.rs / mod.rs 内含 backend 的部分` 搬入 `host::runtime::compositor_render`。
- `SceneSnapshotCache`（在 `compositor/mod.rs` 内）：因含 `Picture`，搬到 host。

### 7.10 `runtime::cache / render_engine / render_registry / session / target / surface / profile / frame_view / backend_object / audio`
- 全部搬到 `host::runtime::*`。无逻辑改动，只挪位置。

### 7.11 `text.rs` 与字体加载
- `core::text::*`：保留所有 cosmic-text 包装。
- `default_font_db(&[])`（含 IO 加载系统字体）搬到 `host::fonts`，作为 `DefaultFontProvider::new()` 的初始化。
- core 内 layout 改为接 `&dyn FontProvider`。

### 7.12 `jsonl::parse_file` IO 与 script 内联化
- `parse_file(path)` 整体搬到 host：`host::jsonl_io::parse_file(path)`。逻辑：
  1. host 端 `fs::read_to_string` 读 jsonl。
  2. host 端用 `serde_json` 逐行解析 `JsonLine`（`JsonLine` 仍在 core 内 pub 出来，让 host 复用 enum 定义；这不破坏 core 纯净度因为 `JsonLine` 只是 `serde_json` 反序列化结构，serde_json 在 core 已有依赖）。
  3. 对每个 `JsonLine::Script { path: Some(p), src: None, .. }`，host `fs::read_to_string(base_dir.join(p))` 读出脚本内容，原地改写为 `JsonLine::Script { path: None, src: Some(content), .. }`。
  4. host 把改写后的行**重新 serialize 回 jsonl 字符串**（per-line `serde_json::to_string`），交给 `core::parse(text)`。
- core 侧 `parse / parse_with_base_dir`：删除 `parse_with_base_dir` 入口（它的语义已被 host 取代）；core 只暴露 `parse(text: &str)`，且 core 永不读文件。
- **影响**：`JsonLine` 从 `enum JsonLine`（私有）改为 `pub(crate) enum JsonLine`，host 模块通过 `pub use crate::core::jsonl::JsonLine` 访问；为最小化改动，给 `JsonLine` 加 `#[derive(Serialize)]`（现仅 `Deserialize`）。
- 顶层 re-export `pub use jsonl::{ParsedComposition, parse, parse_file, parse_with_base_dir}`：`parse_file` 改为指向 `host::jsonl_io::parse_file`；`parse_with_base_dir` 删除并加 deprecation 注释，建议下游切换到 `parse_file` 或 `parse`（实际现有代码只有 jsonl.rs 内部测试用 parse_with_base_dir，外部消费已用 parse_file）。

### 7.13 `inspect.rs` 去 `&mut AssetsMap`
- `collect_frame_layout_rects`（`src/inspect.rs:123`）签名从 `assets: &mut AssetsMap` 改为 `catalog: &mut dyn ResourceCatalog`。
- 顶层 re-export `opencat::collect_frame_layout_rects` 路径不变。
- inspect 内部所有 `assets.ensure_image_source_entry_for_inspect(...)` 等调用改为 trait 方法（spec §4.1 trait 需要相应增加 `ensure_for_inspect` 方法，或者把 inspect 用到的 catalog 维护逻辑降级为通用方法）。**决定**：在 `ResourceCatalog` trait 加一个 `register_for_inspect(src: &ImageSource)` 默认实现（通过 `resolve_image` 的"宽容版本"——失败时不报错而是 0×0 占位），统一 inspect 路径。

### 7.14 测试 setup 大批量改写
- 受影响文件：`src/layout/mod.rs:1082+`（约 16 处）、`src/runtime/compositor/reuse.rs:281`、`src/runtime/fingerprint/{mod,display_item}.rs` 内的测试块、`src/element/resolve.rs` 内的测试块。
- 共同改造：每个测试函数顶部 `let mut assets = AssetsMap::new(); let mut media = MediaContext::new();` 替换为 `let mut catalog = AssetCatalog::new();`，调用点 `&mut media, &mut assets` 缩为 `&mut catalog`（resolve 已删 media 参数）；fonts 与 script 测试入参用 `MockFontProvider`（裸 `fontdb::Database`）和 `MockScriptHost`（无脚本测试传 `None`）。
- 新增 `src/core/test_support.rs`（仅 `#[cfg(test)]` 编译）：暴露 `mock_font_provider() -> impl FontProvider` 与 `MockScriptHost::default()`，统一测试样板。
- `MockScriptHost::run_frame` 返回空 `StyleMutations`，测试 `install` 返回单调递增 id 即可。

### 7.15 audio 路径
- `runtime::audio.rs::*`（含 `build_audio_track / render_audio_chunk`）整体搬到 `host::runtime::audio`。函数签名中 `assets: &mut AssetsMap` 改为 `catalog: &mut AssetCatalog`（host 持具体类型，不必走 trait——因为 audio decode 本身在 host 跑，ffmpeg 调用就在邻近模块）。
- 顶层 re-export `pub use runtime::audio::AudioBuffer; pub use render::{build_audio_track, render_audio_chunk};` 路径不变。

## 8. 错误处理

- 所有 trait 方法返回 `anyhow::Result<T>`，与现有代码一致。
- `ResourceCatalog::resolve_image` 在 `Url`/`Query` 未 preload 时返回 `Err` 并附 source 描述，等价于现 `require_preloaded` 行为。
- `ResourceCatalog::video_info` 返回 `Option`，core 路径必须 graceful 处理 `None`，与现 `media.video_info(...).unwrap_or_else(...)` fallback 行为一致。
- `ScriptHost::run_frame` 返回 `Err` 时，core 必须把错误冒泡到 `build_frame_display_tree` 的调用方；不静默吞错。
- core 编译路径下不出现 `panic!` 用于资源缺失（保持现状，缺失走 `Err`）。

## 9. 测试策略

### 9.1 不增加新测试，但保证现有测试全过
- `cargo test`（默认 features）：所有现有单元测试 / integration 测试通过。
- `cargo test --no-default-features --lib`：core 模块下的所有单元测试通过（fingerprint / annotation / display / element / layout / scene 等）。

### 9.2 新增 1 个硬性纯净度测试（CI 卡口）
- 文件：`tests/core_purity.rs`。
- 内容：在 `--no-default-features` 下 `use opencat::core::{parse, collect_resource_requests, build_frame_display_tree, ResourceCatalog, FontProvider, ScriptHost};` 通过编译。
- CI 指令：`cargo check --no-default-features --lib --tests`。

### 9.3 行为等价性
- **基线**：在重构开始前的 commit（`git rev-parse HEAD` 锁定）上运行 `cargo run --release --bin opencat -- examples/timeline.jsonl --output /tmp/baseline.mp4 --frames 60`，留存 `/tmp/baseline.mp4`。
- **回归比对**：重构每个里程碑（Step 2 / Step 4 / Step 6 / Step 7）完成后，用同一指令重新输出 `/tmp/after.mp4`，运行 `ffmpeg -i /tmp/baseline.mp4 -i /tmp/after.mp4 -lavfi psnr -f null -` 解析平均 PSNR。
- **阈值**：每个 milestone 平均 PSNR ≥ 50 dB（阈值用于编码器浮点 jitter 兜底；预期得到 ≥ 60 dB 因为渲染算法未变）。
- **Example 选取**：使用 `examples/timeline.jsonl`（含 div / image / video / text / canvas / script，覆盖所有节点 kind）；如 plan 阶段确认该 example 不存在或已废弃，由 plan 重新选定一个等价覆盖度的 example 并在 plan 文档锁定。

## 10. 兼容性与外部影响

- **`opencat` / `opencat-see` binary**：路径不变，行为不变。
- **examples/**：不需要改动，因为顶层 `opencat::*` re-export 保留。
- **`opencat-creator/`**：不在本 spec 范围；如果它直接 `use opencat::resource::assets::AssetsMap;`，因为该路径仍存在（type alias）所以不破。
- **Public API 兼容**：所有 `pub use` 在 `src/lib.rs` 头部的符号路径在默认 feature 下保持不变；唯一例外是 `parse_with_base_dir` 被废弃（标 `#[deprecated]`，仍 re-export 一个委托到 `parse(text, base_dir.unwrap())` 的兼容 wrapper，下游有迁移期）。
- **`collect_frame_layout_rects` 签名变化**：`assets: &mut AssetsMap` → `catalog: &mut dyn ResourceCatalog`。`AssetsMap` 实现 `ResourceCatalog`，调用方传入相同变量编译可过，无源码改动；纯类型名差异（如下游用 `fn x(rects, assets: &mut AssetsMap)` 转发）需要本地修一行。

## 11. 风险与 Mitigation

| 风险 | 概率 | 影响 | Mitigation |
|---|---|---|---|
| trait `&mut dyn` 引入虚调用导致热路径性能退化 | 低 | 中 | core 路径 trait 方法每帧调用次数有界（resolve 阶段 N 次），分支预测器友好；如果回归测得退化 >5%，单态化关键 trait 调用点 |
| script binding 经 host trait 后 mutation 收集多一次拷贝 | 低 | 低 | `StyleMutations` 已经是 owned `HashMap`，trait 返回值就是 move 出去，与现状无差 |
| AssetCatalog 与 fetch 之间状态分裂导致 bug | 中 | 中 | fetch 函数全部签名为 `(&mut AssetCatalog, ...)`，状态变更点保持单一；新增 `core::resource::asset_catalog` 单元测试覆盖 register/alias/dimensions/video_info |
| feature 切换破 macos 平台特定代码 | 中 | 中 | macos 的 `metal/cocoa/foreign-types/skia metal feature` 仍走 `host-backend-skia`；CI matrix 加 `--no-default-features` 一项即可暴露 |
| `cargo check --no-default-features` 触发未预期的 transitive dep 拉入 | 低 | 低 | 第一次跑通后用 `cargo tree --no-default-features` 锁定核心依赖白名单（anyhow / serde / serde_json / taffy / cosmic-text / fontdb / ahash / rustc-hash / unicode-segmentation / tracing），写进 `tests/core_purity.rs` 注释 |
| cfg(test) setup 大批量改写导致测试覆盖被悄悄缩小 | 中 | 中 | 每个被改的测试函数在迁移 commit 中**先**保持原有断言不变，仅改 setup 行；CI 增加一项 `cargo test -- --list \| wc -l` 前后比对，测试函数个数不能减少（除明确 spec 决定删除的） |
| `JsonLine` 改为 `pub(crate)` + `Serialize` 引入跨边界耦合 | 低 | 低 | `JsonLine` 已经是 `serde` 派生结构，只是把 `Deserialize` 加上 `Serialize` 派生；core 与 host 都依赖 `serde_json` 无新增依赖 |

## 12. Out of Scope（明确不做）

- wasm-bindgen 胶水 / wasm32 target 验证。
- CanvasKit backend / WebCodecs 资源 host。
- workspace 多 crate 拆分。
- 脚本沙箱（不在主进程 globalThis 注入）。
- 性能优化（trait inlining、单态化关键路径、struct of arrays 等）。
- 新功能（任何用户可见行为变更）。

## 13. 验收标准

1. `cargo build` 默认 features 通过。
2. `cargo test` 默认 features 全部通过。
3. `cargo check --no-default-features --lib` 通过——core 完全切除 host 依赖仍可编译。
4. `tests/core_purity.rs` 在 `--no-default-features` 下编译通过。
5. `examples/timeline.jsonl`（或 plan 锁定的等价 example）用重构后 `opencat` binary 渲染输出，与 §9.3 留存的 baseline 平均 PSNR ≥ 50 dB。
6. 反向依赖审查：在仓库根执行
   ```
   grep -rE "opencat::host|crate::host|super::host" src/core/ | wc -l   # 必须 == 0
   grep -rE "opencat::core|crate::core|super::core" src/host/ | wc -l   # 必须 ≥ 5
   ```
   前者证明 core 不依赖 host；后者证明 host 实际使用了 core 公共 API。
7. 反向依赖再审：`cargo check --no-default-features --lib` 通过同时，
   ```
   cargo tree --no-default-features --prefix none --edges normal | grep -E "ffmpeg-next|skia-safe|rquickjs|reqwest|tokio|rodio"
   ```
   输出为空（这些 host 专属依赖在 core-only 编译时不被拉入）。
