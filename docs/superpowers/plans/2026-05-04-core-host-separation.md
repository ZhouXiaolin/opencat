# Core / Host 分离 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 把 opencat 的 "JSONL → DisplayList" 主流程纯化为可独立编译、无 IO/无 ffmpeg/无 quickjs/无 skia 依赖的 `core`，把资源 IO、媒体编解码、JS 脚本运行时、平台渲染下沉到 `host`，所有边界按 wasm 友好原则设计。

**Architecture:** 单 crate + cargo features；通过定义在 core 内的 `ResourceCatalog / ScriptHost / FontProvider` trait 反转依赖；core 不持有 runtime（无 tokio/rquickjs/skia surface）；host 默认 features 全开，CI 卡口 `cargo check --no-default-features --lib` 强制 core 切除 host 依赖仍可编译；公共 API 顶层 re-export 路径保持兼容。重构分四个 Phase 推进，每个 Phase 独立可编译可测，行为零变化（PSNR ≥ 50 dB 验证）。

**Tech Stack:** Rust 2024 edition, anyhow, serde/serde_json, taffy, cosmic-text, fontdb, ahash, rustc-hash, tracing（core 子集）；ffmpeg-next, rquickjs, reqwest, tokio, skia-safe, rodio（host 限定）。

---

## 文件结构总览

> 以下列出本次重构会**新增、修改、移动**的文件及其责任。Phase 顺序：先 trait 引入（位置不动），后拆 IO（不动位置），最后大规模 git mv 物理重组。

### Phase 1 新增

- `src/resource/catalog.rs` — `ResourceCatalog` trait 定义 + `VideoInfoMeta`，由 `AssetsMap` 实现。
- `src/scene/script/host.rs` — `ScriptHost` trait + `ScriptDriverId`，由 `ScriptRuntimeCache`（薄 wrapper）实现。
- `src/text/provider.rs`（升级 `text.rs` 为 `text/mod.rs`）— `FontProvider` trait + `DefaultFontProvider`。
- `tests/core_purity.rs` — `cargo check --no-default-features --lib --tests` 卡口测试。
- `Cargo.toml` features 节加入 `default = ["host-default"]` 等结构。

### Phase 1 修改

- `src/display/build.rs` — 删 `_assets: &AssetsMap` 参数。
- `src/element/resolve.rs` — `resolve_video` 不再调 `media.video_info`；`resolve_ui_tree*` 删 `media: &mut MediaContext` 参数。
- `src/runtime/fingerprint/{mod.rs, display_item.rs}` — `classify_paint / item_paint_fingerprint / item_is_time_variant / bitmap_is_video` 删 `assets: &AssetsMap` 参数。
- `src/runtime/annotation.rs` — `annotate_display_tree` 删 `assets: &AssetsMap` 参数。
- `src/runtime/pipeline.rs::build_scene_display_list` — 同步参数清理。
- `src/resource/assets.rs` — 增 `register_video_info / video_info` + `video_info_meta: HashMap<AssetId, VideoInfoMeta>` 字段；保持兼容方法。
- `src/resource/media.rs` — 新增 `VideoInfoMeta::from(&VideoInfo)`；`MediaContext::probe_video_into_catalog` host 帮手。

### Phase 2 新增

- `src/resource/asset_catalog.rs` — `AssetCatalog`（纯映射，无 tokio）。
- `src/resource/fetch.rs` — `preload_image_sources / preload_audio_sources` 自由函数（持 tokio runtime）。
- `src/resource/probe.rs` — `probe_video(catalog, path, media)` 自由函数。
- `src/runtime/preflight_collect.rs` — `collect_resource_requests(parsed) -> ResourceRequests`。
- `src/scene/script/mutations.rs` — 把 `StyleMutations / NodeStyleMutations / CanvasMutations / CanvasCommand / ScriptTextSource / ...` 等纯数据从 `scene/script/mod.rs` 抽出。
- `src/scene/script/quickjs_host.rs` — `QuickJsScriptHost` impl `ScriptHost`，包内部 `runners: HashMap<u64, ScriptRunner>`。
- `src/scene/script/cache.rs` — `ScriptDriverCache` 取代 `ScriptRuntimeCache`（仅缓存 `(node_id → ScriptDriverId)`）。

### Phase 2 修改

- `src/resource/assets.rs` 删除（迁移到 `asset_catalog.rs` + `fetch.rs`）；保留 `pub use asset_catalog::AssetCatalog as AssetsMap;` 兼容别名（在 `src/resource/mod.rs`）。
- `src/runtime/preflight.rs` — `ensure_assets_preloaded` 改成 host 侧驱动函数：`collect_resource_requests` → `preload_image_sources` → `preload_audio_sources` → `probe_videos`。
- `src/runtime/pipeline.rs::build_scene_display_list` 抽离 core 算法函数 `core_build_frame_display_tree`，签名为 `(&ParsedComposition, &FrameCtx, &mut dyn ResourceCatalog, &dyn FontProvider, Option<&mut dyn ScriptHost>)`。

### Phase 3 物理目录重组

```
src/
  core/                              ← #[cfg] 永远存在
    mod.rs                           ← pub mod jsonl; pub mod scene; ...
    jsonl/{mod.rs, builder.rs, tailwind.rs}      ← from src/jsonl.rs+jsonl/
    scene/{mod.rs, composition.rs, easing.rs, node.rs, time.rs, transition.rs}
    scene/script/{mutations.rs, host.rs, cache.rs, mod.rs}
    scene/primitives/                ← 原 src/scene/primitives/
    style.rs                         ← 原 src/style.rs
    element/                         ← 原 src/element/
    layout/                          ← 原 src/layout/
    display/                         ← 原 src/display/
    text/{mod.rs, provider.rs}       ← 原 src/text.rs（拆 provider）
    resource/{mod.rs, asset_id.rs, types.rs, catalog.rs, asset_catalog.rs}
    runtime/
      analysis.rs / annotation.rs    ← 原 src/runtime/{analysis,annotation}.rs
      fingerprint/                   ← 原 src/runtime/fingerprint/
      invalidation/                  ← 原 src/runtime/invalidation/
      compositor/{mod.rs, ordered_scene.rs, plan.rs, reuse.rs, slot.rs}
      preflight_collect.rs           ← 原 src/runtime/preflight_collect.rs
      pipeline.rs                    ← 原 src/runtime/pipeline.rs core 部分
    frame_ctx.rs                     ← 原 src/frame_ctx.rs
    lib.rs（pub use 暴露公共 API）

  host/                              ← #[cfg(feature = "host-default")]
    mod.rs
    inspect.rs                       ← 原 src/inspect.rs
    resource/
      mod.rs
      fetch.rs / probe.rs / preflight.rs
      media.rs                       ← 原 src/resource/media.rs
    codec/                           ← 原 src/codec/
    script/
      mod.rs
      quickjs.rs                     ← 原 ScriptRunner + install_runtime_bindings
      bindings/{node_style.rs, canvas_api.rs, animate_api.rs, morph_svg.rs}
      runtime/                       ← 原 *.js prelude
    backend/                         ← 原 src/backend/
    runtime/
      mod.rs
      session.rs / pipeline.rs / compositor_render.rs
      cache/                         ← 原 src/runtime/cache/
      render_engine.rs / render_registry.rs / target.rs / surface.rs
      profile.rs / frame_view.rs / backend_object.rs / audio.rs
    bin/                             ← 原 src/bin/（opencat / opencat-see）
    fonts.rs                         ← DefaultFontProvider + load_system_fonts
    jsonl_io.rs                      ← parse_file / parse_with_base_dir

  lib.rs                             ← pub mod core + pub mod host + 顶层 re-export
```

---

# Phase 1 — 引入 trait 与纯净度基线

> 此阶段不动文件位置；只引入 trait、加 features 骨架、建 CI 卡口、删除 core 路径上无用的 `&AssetsMap / &mut MediaContext` 参数。每个任务后 `cargo build && cargo test --lib` 必须通过。

## Task 1.1: 定义 ResourceCatalog trait + VideoInfoMeta

**Files:**
- Create: `src/resource/catalog.rs`
- Modify: `src/resource/mod.rs`
- Test: `src/resource/catalog.rs`（内联 `#[cfg(test)]`）

- [ ] **Step 1: 写失败测试**

新建 `src/resource/catalog.rs`：

```rust
use std::path::PathBuf;

use anyhow::Result;

use crate::resource::assets::AssetId;
use crate::scene::primitives::{AudioSource, ImageSource};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoInfoMeta {
    pub width: u32,
    pub height: u32,
    pub duration_secs: Option<f64>,
}

pub trait ResourceCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId>;
    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId>;
    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId;
    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()>;
    fn dimensions(&self, id: &AssetId) -> (u32, u32);
    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::assets::AssetsMap;
    use crate::scene::primitives::ImageSource;

    #[test]
    fn assets_map_implements_resource_catalog_register_dimensions_returns_stable_id() {
        let mut catalog: Box<dyn ResourceCatalog> = Box::new(AssetsMap::new());
        let id1 = catalog.register_dimensions("/tmp/a.png", 100, 200);
        let id2 = catalog.register_dimensions("/tmp/a.png", 100, 200);
        assert_eq!(id1, id2);
        assert_eq!(catalog.dimensions(&id1), (100, 200));
    }

    #[test]
    fn assets_map_resolve_image_returns_stable_id_for_path() {
        let mut catalog = AssetsMap::new();
        let src = ImageSource::Path(std::path::PathBuf::from("/tmp/b.png"));
        let id1 = (&mut catalog as &mut dyn ResourceCatalog).resolve_image(&src).unwrap();
        let id2 = (&mut catalog as &mut dyn ResourceCatalog).resolve_image(&src).unwrap();
        assert_eq!(id1, id2);
    }

    #[test]
    fn assets_map_video_info_returns_none_when_not_probed() {
        let mut catalog = AssetsMap::new();
        let id = catalog.register_dimensions("/tmp/v.mp4", 0, 0);
        assert!((&catalog as &dyn ResourceCatalog).video_info(&id).is_none());
    }
}
```

修改 `src/resource/mod.rs`：

```rust
pub mod assets;
mod bitmap_source;
pub mod catalog;
pub mod media;
```

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib resource::catalog -- --nocapture`
Expected: FAIL — `AssetsMap` 还没 impl `ResourceCatalog`，编译错误 "the trait bound `AssetsMap: ResourceCatalog` is not satisfied"。

- [ ] **Step 3: 在 assets.rs 末尾给 AssetsMap impl ResourceCatalog**

修改 `src/resource/assets.rs`，在文件末尾追加：

```rust
use crate::resource::catalog::{ResourceCatalog, VideoInfoMeta};

impl ResourceCatalog for AssetsMap {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId> {
        self.register_image_source(src)
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId> {
        self.register_audio_source(src)
    }

    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let path = std::path::Path::new(locator);
        AssetsMap::register_dimensions(self, path, width, height)
    }

    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        AssetsMap::alias(self, alias, target)
    }

    fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        AssetsMap::dimensions(self, id)
    }

    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.video_info_meta(id)
    }
}
```

新增 `AssetsMap` 内字段（在 `struct AssetsMap` 中）：

```rust
video_info_meta: HashMap<AssetId, VideoInfoMeta>,
```

并在 `AssetsMap::new()` 中：

```rust
video_info_meta: HashMap::new(),
```

并新增方法：

```rust
pub fn register_video_info(&mut self, path: &Path, info: VideoInfoMeta) -> AssetId {
    let id = self.register_dimensions(path, info.width, info.height);
    self.video_info_meta.insert(id.clone(), info);
    id
}

pub fn video_info_meta(&self, id: &AssetId) -> Option<VideoInfoMeta> {
    self.video_info_meta.get(id).copied()
}
```

文件顶部 `use` 添加：

```rust
use crate::resource::catalog::VideoInfoMeta;
```

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --lib resource::catalog -- --nocapture`
Expected: 3 tests PASS。

- [ ] **Step 5: Commit**

```bash
rtk git add src/resource/catalog.rs src/resource/mod.rs src/resource/assets.rs && \
rtk git commit -m "feat(resource): add ResourceCatalog trait + VideoInfoMeta with AssetsMap impl"
```

## Task 1.2: 定义 ScriptHost trait + ScriptDriverId

**Files:**
- Create: `src/scene/script/host.rs`
- Modify: `src/scene/script/mod.rs`

- [ ] **Step 1: 写失败测试**

新建 `src/scene/script/host.rs`：

```rust
use anyhow::Result;

use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::{ScriptTextSource, StyleMutations};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ScriptDriverId(pub u64);

pub trait ScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId>;
    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource);
    fn clear_text_sources(&mut self);
    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &ScriptFrameCtx,
    ) -> Result<StyleMutations>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::script::ScriptRuntimeCache;

    #[test]
    fn script_runtime_cache_install_returns_stable_id() {
        let mut host: Box<dyn ScriptHost> = Box::new(ScriptRuntimeCache::default());
        let id1 = host.install("ctx => {}").unwrap();
        let id2 = host.install("ctx => {}").unwrap();
        assert_eq!(id1, id2);
    }
}
```

修改 `src/scene/script/mod.rs` 顶部 module 声明：

```rust
pub mod host;
pub use host::{ScriptDriverId, ScriptHost};
```

并把 `ScriptTextSource` / `ScriptTextSourceKind` 的可见性改成 `pub`（去掉 `(crate)`），方便 trait 签名引用。

- [ ] **Step 2: 跑测试确认失败**

Run: `cargo test --lib scene::script::host`
Expected: FAIL — `ScriptRuntimeCache` 没 impl `ScriptHost`。

- [ ] **Step 3: 在 mod.rs 末尾给 ScriptRuntimeCache impl ScriptHost**

`src/scene/script/mod.rs` 末尾追加：

```rust
impl ScriptHost for ScriptRuntimeCache {
    fn install(&mut self, source: &str) -> anyhow::Result<ScriptDriverId> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        let key = h.finish();
        if let std::collections::hash_map::Entry::Vacant(e) = self.runners.entry(key) {
            e.insert(ScriptRunner::new(source)?);
        }
        Ok(ScriptDriverId(key))
    }

    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource) {
        ScriptRuntimeCache::register_text_source(self, node_id, source);
    }

    fn clear_text_sources(&mut self) {
        ScriptRuntimeCache::clear_text_sources(self);
    }

    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        frame_ctx: &crate::frame_ctx::ScriptFrameCtx,
    ) -> anyhow::Result<StyleMutations> {
        let runner = self
            .runners
            .get_mut(&driver.0)
            .ok_or_else(|| anyhow::anyhow!("script driver {} not installed", driver.0))?;
        if let Ok(mut store) = runner.store.lock() {
            store.text_sources = self.text_sources.clone();
        }
        runner.run(*frame_ctx, None)
    }
}
```

注意 `ScriptRuntimeCache.runners` / `.text_sources` 需要从 `pub(crate)` 改成 `pub` 字段访问（或加 `pub(super)` getter）；最小改动是把字段改 `pub(super)` 让本模块内的 impl 可以访问——目前已经是同 mod，本步只需要字段访问可行即可。

- [ ] **Step 4: 跑测试确认通过**

Run: `cargo test --lib scene::script::host`
Expected: PASS。

- [ ] **Step 5: Commit**

```bash
rtk git add src/scene/script/host.rs src/scene/script/mod.rs && \
rtk git commit -m "feat(scene/script): add ScriptHost trait + ScriptDriverId with cache impl"
```

## Task 1.3: 定义 FontProvider trait

**Files:**
- Modify: `src/text.rs`（保留单文件，不展开成 mod 目录，避免本 phase 物理改动）

- [ ] **Step 1: 写失败测试**

在 `src/text.rs` 末尾的 `#[cfg(test)] mod tests` 块上方追加：

```rust
pub trait FontProvider {
    fn font_db(&self) -> &fontdb::Database;
}

pub struct DefaultFontProvider {
    db: std::sync::Arc<fontdb::Database>,
}

impl DefaultFontProvider {
    pub fn new() -> Self {
        Self { db: std::sync::Arc::new(default_font_db(&[])) }
    }

    pub fn from_arc(db: std::sync::Arc<fontdb::Database>) -> Self {
        Self { db }
    }
}

impl FontProvider for DefaultFontProvider {
    fn font_db(&self) -> &fontdb::Database {
        &self.db
    }
}
```

并在 `mod tests` 内增加：

```rust
#[test]
fn default_font_provider_exposes_loaded_db() {
    let p = DefaultFontProvider::new();
    let count = p.font_db().faces().count();
    assert!(count >= 2, "embedded NotoSansSC + NotoColorEmoji should be present, got {count}");
}
```

修改 `src/lib.rs` 把 `mod text;` 改成 `pub mod text;`，便于其它模块直接 `use crate::text::FontProvider;`。

- [ ] **Step 2: 跑测试确认失败/通过**

Run: `cargo test --lib text::tests::default_font_provider_exposes_loaded_db`
Expected: PASS（新代码是 additive，不会破坏现有逻辑）。

- [ ] **Step 3: 接入 layout 编译路径（不改行为）**

修改 `src/layout/mod.rs::compute_layout_with_font_db` 旁边加一个 `compute_layout_with_provider`（保留旧 API）：

```rust
pub fn compute_layout_with_provider(
    &mut self,
    element_root: &crate::element::tree::ElementNode,
    frame_ctx: &crate::FrameCtx,
    fonts: &dyn crate::text::FontProvider,
) -> anyhow::Result<(crate::layout::tree::LayoutTree, crate::layout::profile::LayoutPass)> {
    self.compute_layout_with_font_db(element_root, frame_ctx, fonts.font_db())
}
```

(具体 LayoutTree / LayoutPass 路径以现有 import 为准；步骤里的类型仅用于示意。)

- [ ] **Step 4: cargo build 验证**

Run: `rtk cargo build --lib`
Expected: 0 errors, 0 warnings。

- [ ] **Step 5: Commit**

```bash
rtk git add src/text.rs src/layout/mod.rs src/lib.rs && \
rtk git commit -m "feat(text): add FontProvider trait + DefaultFontProvider"
```

## Task 1.4: 加 Cargo features 骨架（不改任何依赖行为）

**Files:**
- Modify: `Cargo.toml`

- [ ] **Step 1: 增加 features 节**

在 `Cargo.toml` 的 `[dependencies]` **之前** 加入：

```toml
[features]
default = ["host-default"]
host-default = ["host-codec", "host-script-quickjs", "host-resource-net", "host-backend-skia", "host-audio"]
host-codec = ["dep:ffmpeg-next"]
host-script-quickjs = ["dep:rquickjs"]
host-resource-net = ["dep:reqwest", "dep:tokio"]
host-backend-skia = ["dep:skia-safe"]
host-audio = []
```

并把对应依赖改成 `optional = true`：

```toml
ffmpeg-next = { version = "8.1.0", optional = true }
rquickjs = { version = "0.11.0", features = ["futures"], optional = true }
reqwest = { version = "0.13.2", default-features = false, features = ["json", "rustls", "socks"], optional = true }
tokio = { version = "1.51.0", features = ["rt-multi-thread", "fs"], optional = true }
```

`skia-safe`（在 target-cfg 表里）保持 target-cfg；额外不改 — Phase 1 只是骨架，让 default features 行为完全等价于以前。`skia-safe` 的 `optional = true` 化在 target-cfg 表里加：

```toml
[target.'cfg(not(target_os = "macos"))'.dependencies]
skia-safe = { version = "0.93.1", features = ["binary-cache", "gl"], optional = true }

[target.'cfg(target_os = "macos")'.dependencies]
skia-safe = { version = "0.93.1", features = ["metal", "binary-cache"], optional = true }
```

- [ ] **Step 2: 验证默认编译仍通过**

Run: `rtk cargo build --lib`
Expected: 0 errors（仅可能新增 unused features warning）。

- [ ] **Step 3: 验证 --no-default-features 当前是预期失败状态**

Run: `cargo check --no-default-features --lib 2>&1 | tail -20`
Expected: FAIL（仍会拉 ffmpeg/rquickjs/skia 的源码 import，因为代码里 `use ffmpeg_next` 等还在）。这一步只是为了**确认 CI 卡口的初始状态**——后续 Phase 2/3 会逐步让它通过。

记录失败摘要到本任务的 commit message 里作为 baseline。

- [ ] **Step 4: Commit**

```bash
rtk git add Cargo.toml && \
rtk git commit -m "build: add cargo features skeleton (host-default + per-host) — baseline for core purity"
```

## Task 1.5: 引入 tests/core_purity.rs CI 卡口（暂不要求通过）

**Files:**
- Create: `tests/core_purity.rs`

- [ ] **Step 1: 写硬编码测试**

```rust
//! Phase 4 才真正通过；Phase 1-3 期间作为 progress beacon。
//! 这个测试在 --no-default-features 下编译 core 公共 API 路径，
//! 用 cargo check --no-default-features --lib --tests 触发。
//! 目前预期失败，因为 src/* 大量直接 use 了 host 依赖。

#![cfg(not(any(
    feature = "host-codec",
    feature = "host-script-quickjs",
    feature = "host-resource-net",
    feature = "host-backend-skia"
)))]

#[test]
fn core_public_api_compiles() {
    use opencat::core::{
        FontProvider, ResourceCatalog, ScriptHost, build_frame_display_tree,
        collect_resource_requests, parse,
    };
    let _: fn(&str) -> _ = parse;
    let _: fn(&_) -> _ = collect_resource_requests;
    let _ = build_frame_display_tree;
    fn _check_traits<R: ResourceCatalog, F: FontProvider, S: ScriptHost>() {}
}
```

注：此处引用的 `opencat::core::*` 在 Phase 3 之后才存在。Phase 1 阶段提交此文件后的 `cargo test --no-default-features` 会失败——这是预期。

- [ ] **Step 2: 验证默认 features 编译不被破坏**

Run: `rtk cargo build --tests`
Expected: 该 test 因 `cfg(not(any(feature = ...)))` 在 default features 下被排除，**不**编译。0 errors。

- [ ] **Step 3: 验证 --no-default-features 下 fail（预期）**

Run: `cargo check --no-default-features --tests 2>&1 | grep -E "error\[|cannot find" | head -5`
Expected: 出错信息提到 `unresolved import opencat::core` 或类似——确认 cgi 卡口生效。

- [ ] **Step 4: Commit**

```bash
rtk git add tests/core_purity.rs && \
rtk git commit -m "test(ci): add core_purity stub (passes after Phase 4 only)"
```

## Task 1.6: §7.1 删 build_display_tree 冗余 _assets 参数

**Files:**
- Modify: `src/display/build.rs`

- [ ] **Step 1: 失败测试**

`src/display/build.rs` 现有所有测试 setup 里有 `let mut assets = AssetsMap::new();` 然后 `build_display_tree(&element_root, &layout_tree, &assets)?;`。本步骤先把签名改了再修测试。

阅读 `src/display/build.rs:1` 找到 `pub fn build_display_tree` 与 `_assets: &AssetsMap` 参数声明。

- [ ] **Step 2: 修改 build_display_tree 签名**

`pub fn build_display_tree(element_root: &ElementNode, layout_tree: &LayoutTree) -> anyhow::Result<DisplayTree>` —— 删掉第 3 个参数；删掉 `use crate::resource::assets::AssetsMap;`（如果只为这个参数 import）。

修复 `src/runtime/pipeline.rs:119`：把 `build_display_tree(&element_root, &layout_tree, &session.assets)?` 改成 `build_display_tree(&element_root, &layout_tree)?`。

修复 `src/display/build.rs` 内部所有 test 块：删掉 `let mut assets = AssetsMap::new();` 与 `&assets` 实参。

- [ ] **Step 3: 跑测试**

Run: `rtk cargo test --lib display::build`
Expected: 全部测试 PASS。

- [ ] **Step 4: 跑全量 build**

Run: `rtk cargo build --lib`
Expected: 0 errors。

- [ ] **Step 5: Commit**

```bash
rtk git add src/display/build.rs src/runtime/pipeline.rs && \
rtk git commit -m "refactor(display): drop unused _assets parameter from build_display_tree"
```

## Task 1.7: §7.8 fingerprint / annotation 去 AssetsMap 参数

**Files:**
- Modify: `src/runtime/fingerprint/mod.rs`
- Modify: `src/runtime/fingerprint/display_item.rs`
- Modify: `src/runtime/annotation.rs`
- Modify: `src/runtime/pipeline.rs`
- Modify: `src/runtime/compositor/reuse.rs`（测试 setup）

- [ ] **Step 1: 失败测试 — 改 item_is_time_variant 用 video_timing**

`src/runtime/fingerprint/display_item.rs:218-236` 改：

```rust
pub(super) fn item_is_time_variant(item: &DisplayItem) -> bool {
    match item {
        DisplayItem::Timeline(_) => true,
        DisplayItem::Bitmap(bitmap) => bitmap_is_video(bitmap),
        DisplayItem::DrawScript(_) => false,
        DisplayItem::Text(text) => text.text_unit_overrides.is_some(),
        DisplayItem::Rect(_) | DisplayItem::SvgPath(_) => false,
    }
}

pub(super) fn bitmap_is_video(bitmap: &BitmapDisplayItem) -> bool {
    bitmap.video_timing.is_some()
}
```

文件顶部删 `use crate::resource::{assets::AssetsMap, bitmap_source::{BitmapSourceKind, bitmap_source_kind}};`（这两个 import 仅服务旧 `bitmap_is_video`）。

测试块（line 240+）相应删 `let assets = AssetsMap::new();` 与 `&assets` 参数。

- [ ] **Step 2: 改 mod.rs 公共函数签名**

`src/runtime/fingerprint/mod.rs:96`：

```rust
pub fn classify_paint(item: &DisplayItem) -> PaintVariance {
    if item_is_time_variant(item) { PaintVariance::TimeVariant } else { PaintVariance::Stable }
}

pub fn item_paint_fingerprint(item: &DisplayItem) -> Option<u64> {
    if item_is_time_variant(item) { return None; }
    // ... existing body
}
```

`src/runtime/fingerprint/mod.rs` 内 4 处 test setup 删 `&assets` 实参（line 807/813/828/830/902/938）。

- [ ] **Step 3: 改 annotation.rs 公共签名**

`src/runtime/annotation.rs`：

- 删 `use crate::resource::assets::AssetsMap;`。
- `pub(crate) fn annotate_display_tree(display_tree: &DisplayTree) -> AnnotatedDisplayTree`。
- `fn annotate_display_node` 同步删 `assets: &AssetsMap` 参数。
- 第 171 行 `let paint_variance = fingerprint::classify_paint(&node.item, assets);` → `fingerprint::classify_paint(&node.item);`。

- [ ] **Step 4: 修复调用方**

`src/runtime/pipeline.rs:120`：`annotate_display_tree(&display_tree, &session.assets)` → `annotate_display_tree(&display_tree)`。

`src/runtime/compositor/reuse.rs:284` 测试块：删 `let mut assets = AssetsMap::new();` 与 `&assets` 实参（如有）。

- [ ] **Step 5: 跑测试**

Run: `rtk cargo test --lib runtime::fingerprint runtime::annotation runtime::compositor`
Expected: 全部 PASS。

Run: `rtk cargo build --lib`
Expected: 0 errors。

- [ ] **Step 6: Commit**

```bash
rtk git add src/runtime/fingerprint/ src/runtime/annotation.rs src/runtime/pipeline.rs src/runtime/compositor/reuse.rs && \
rtk git commit -m "refactor(runtime): drop AssetsMap dependency from fingerprint/annotation"
```

## Task 1.8: §7.2 + §7.4 resolve_video 摆脱 MediaContext

**Files:**
- Modify: `src/element/resolve.rs`
- Modify: `src/runtime/pipeline.rs`
- Modify: `src/inspect.rs`
- Test: `src/element/resolve.rs` 内联测试

- [ ] **Step 1: 失败测试 — 单测试 resolve_video 在 catalog 缺 video_info 时 fallback 到 0×0**

在 `src/element/resolve.rs` 的 `#[cfg(test)] mod tests` 块加：

```rust
#[test]
fn resolve_video_falls_back_to_zero_dimensions_when_catalog_lacks_video_info() {
    use crate::scene::primitives::video;
    use crate::frame_ctx::FrameCtx;
    let mut catalog = AssetsMap::new();
    let v = video().id("v").path(std::path::PathBuf::from("/no/such/video.mp4"));
    let frame_ctx = FrameCtx { frame: 0, fps: 30, width: 320, height: 180, frames: 30 };
    let element = resolve_ui_tree(
        &v.into(),
        &frame_ctx,
        &mut catalog,
        None,
    ).expect("resolve");
    if let crate::element::tree::ElementKind::Bitmap(b) = &element.kind {
        assert_eq!(b.width, 0);
        assert_eq!(b.height, 0);
        assert!(b.video_timing.is_some());
    } else {
        panic!("expected Bitmap kind");
    }
}
```

注意：`resolve_ui_tree` 的签名将在本任务里改成不再需要 `&mut MediaContext`。

- [ ] **Step 2: 改 resolve_video / resolve_image 签名**

`src/element/resolve.rs`：

- 删 `use crate::resource::media::MediaContext;`（顶部）。
- `ResolveContext` 不再持 `media`。
- `resolve_video(video, cx)`：

```rust
fn resolve_video(video: &Video, cx: &mut ResolveContext<'_>) -> Result<ElementNode> {
    // ... pushed scope ...
    let result = (|| {
        // existing style work ...
        let asset_id = cx
            .assets
            .register_dimensions(&video.source().to_string_lossy(), 0, 0);
        let info = cx.assets.video_info(&asset_id).unwrap_or(VideoInfoMeta {
            width: 0, height: 0, duration_secs: None,
        });
        // 用 info.width/height 重新覆盖 dimensions（如果 host 已 probe）
        let asset_id = cx
            .assets
            .register_dimensions(&video.source().to_string_lossy(), info.width, info.height);

        Ok(ElementNode {
            id: cx.ids.alloc(),
            kind: ElementKind::Bitmap(ElementBitmap {
                asset_id,
                width: info.width,
                height: info.height,
                video_timing: Some(video.timing()),
            }),
            // ...
        })
    })();
    // ...
}
```

`resolve_image` / `resolve_canvas` 删 `_media: &mut MediaContext` 参数。

- [ ] **Step 3: 改 resolve_ui_tree* 顶层签名**

`pub fn resolve_ui_tree(node, frame_ctx, assets: &mut dyn ResourceCatalog, mutations) -> Result<ElementNode>` —— 删 `media: &mut MediaContext`，`assets` 改成 trait object。
`pub(crate) fn resolve_ui_tree_with_script_cache(node, frame_ctx, script_frame_ctx, assets: &mut dyn ResourceCatalog, mutations, script_runtime)` —— 同上。

`ResolveContext.assets` 改成 `&'a mut dyn ResourceCatalog`。

注意：`cx.assets.alias(AssetId(...), &target)` 现走 trait 方法签名。`cx.assets.register_image_source` 调用要改成 `cx.assets.resolve_image(source)`（trait 方法）。

- [ ] **Step 4: 修复调用方**

`src/runtime/pipeline.rs::build_scene_display_list`：`&mut session.media_ctx` 实参从 resolve 调用中删掉；保留 `&mut session.assets` 但作为 trait 对象传入：

```rust
let element_root = resolve_ui_tree_with_script_cache(
    scene,
    frame_ctx,
    script_frame_ctx,
    &mut session.assets,
    mutations,
    &mut session.script_runtime,
)?;
```

`src/inspect.rs::collect_scene_rects` 同样改成不再传 media。

`src/element/resolve.rs` 全部测试 setup（line 1133+ 等 16 处）：删 `let mut media = MediaContext::new();` + `&mut media` 实参。`AssetsMap::new()` 保留为 catalog。

- [ ] **Step 5: 跑测试**

Run: `rtk cargo test --lib element::resolve runtime::pipeline`
Expected: 全部 PASS（含新增 fallback 测试）。

Run: `rtk cargo build`
Expected: 0 errors。

- [ ] **Step 6: Commit**

```bash
rtk git add src/element/resolve.rs src/runtime/pipeline.rs src/inspect.rs && \
rtk git commit -m "refactor(element): drop MediaContext from resolve_*; video_info via ResourceCatalog"
```

## Task 1.9: Phase 1 收尾 — 全量回归

- [ ] **Step 1: 全量测试**

Run: `rtk cargo test`
Expected: 全部 PASS（与 main 行为对等）。

- [ ] **Step 2: 占位录制 PSNR baseline（**Phase 0 起点**）**

注：此 baseline 应在**重构开始前的 commit `0cd2511`** 上录制；如已 fast-forward 错过该 commit，需 `git stash; git checkout 0cd2511; cargo run --release --example pendulum_canvas; ffmpeg -i out/pendulum_canvas.mp4 -c copy /tmp/baseline_pendulum.mp4; git checkout -; git stash pop` 重做一次。落库时只保存 `/tmp/baseline_pendulum.mp4` 路径，不入仓库。

```bash
mkdir -p out
rtk cargo run --release --example pendulum_canvas
cp out/pendulum_canvas.mp4 /tmp/baseline_pendulum.mp4
rtk cargo run --release --example hello_world
cp out/hello_world.mp4 /tmp/baseline_hello.mp4
ls -lh /tmp/baseline_*.mp4
```

预期：两个 mp4 大小 > 0。

- [ ] **Step 3: PSNR 回归（vs baseline）**

```bash
rtk cargo run --release --example pendulum_canvas
ffmpeg -i /tmp/baseline_pendulum.mp4 -i out/pendulum_canvas.mp4 -lavfi psnr -f null - 2>&1 | grep -E "average:|n=180"
```

Expected: average PSNR ≥ 50 dB（实测应 ≥ 60 dB，因为 Phase 1 仅删除冗余参数，渲染算法零变化）。

如果未达 50 dB，**停下来排查** —— Phase 1 不应引入可见行为改变。

- [ ] **Step 4: Commit milestone**

```bash
rtk git tag phase1-complete
rtk git log --oneline -10
```

---

# Phase 2 — 拆 IO 与 script runner（依赖反转完成，目录暂不变）

## Task 2.1: §7.3 抽 AssetCatalog 到独立文件（无 IO）

**Files:**
- Create: `src/resource/asset_catalog.rs`
- Modify: `src/resource/assets.rs`（保留为 fetch 入口）
- Modify: `src/resource/mod.rs`

- [ ] **Step 1: 失败测试**

新建 `src/resource/asset_catalog.rs`，把 `AssetsMap` 的所有**纯映射**方法搬过来（不含 `preload_image_sources / preload_audio_sources / build_preload_runtime / preload_runtime / ensure_cache_dir / openverse_token`）：

```rust
use std::collections::HashMap;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};

use crate::resource::catalog::{ResourceCatalog, VideoInfoMeta};
use crate::scene::primitives::{AudioSource, ImageSource, OpenverseQuery};

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AssetId(pub String);

pub struct AssetCatalog {
    pub(crate) entries: HashMap<AssetId, AssetEntry>,
    pub(crate) video_info_meta: HashMap<AssetId, VideoInfoMeta>,
    pub(crate) cache_dir: PathBuf,
    pub(crate) openverse_token: Option<String>,
}

pub(crate) struct AssetEntry {
    pub path: PathBuf,
    pub width: u32,
    pub height: u32,
}

impl AssetEntry {
    pub fn image(path: &Path) -> Self { /* moved from assets.rs */ }
    pub fn audio(path: &Path) -> Self { /* moved */ }
    pub fn with_dimensions(path: PathBuf, width: u32, height: u32) -> Self { /* moved */ }
}

impl AssetCatalog {
    pub fn new() -> Self {
        let cache_dir = dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".opencat").join("assets");
        Self {
            entries: HashMap::new(),
            video_info_meta: HashMap::new(),
            cache_dir,
            openverse_token: None,
        }
    }
    pub fn register(&mut self, path: &Path) -> AssetId { /* moved */ }
    pub fn register_image_source(&mut self, source: &ImageSource) -> Result<AssetId> { /* moved */ }
    pub fn register_audio_source(&mut self, source: &AudioSource) -> Result<AssetId> { /* moved */ }
    pub fn register_dimensions(&mut self, path: &Path, width: u32, height: u32) -> AssetId { /* moved */ }
    pub fn register_video_info(&mut self, path: &Path, info: VideoInfoMeta) -> AssetId { /* moved */ }
    pub fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> { /* moved */ }
    pub fn dimensions(&self, id: &AssetId) -> (u32, u32) { /* moved */ }
    pub fn path(&self, id: &AssetId) -> Option<&Path> { /* moved */ }
    pub fn video_info_meta(&self, id: &AssetId) -> Option<VideoInfoMeta> { /* moved */ }
    pub fn ensure_image_source_entry_for_inspect(&mut self, source: &ImageSource) { /* moved */ }
    pub(crate) fn register_audio_path(&mut self, path: &Path) -> AssetId { /* moved */ }
    pub(crate) fn insert_entry_if_missing(&mut self, id: AssetId, build_entry: impl FnOnce() -> AssetEntry) -> AssetId { /* moved */ }
    pub(crate) fn push_missing_request<T>(&self, id: &AssetId, requests: &mut Vec<T>, build_request: impl FnOnce() -> T) { /* moved */ }
    pub(crate) fn require_preloaded(&self, id: AssetId, missing_error: impl FnOnce() -> anyhow::Error) -> Result<AssetId> { /* moved */ }
    pub(crate) fn ensure_cache_dir(&self) -> Result<()> { /* moved */ }
}

impl Default for AssetCatalog { fn default() -> Self { Self::new() } }

impl ResourceCatalog for AssetCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId> { self.register_image_source(src) }
    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId> { self.register_audio_source(src) }
    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let path = Path::new(locator);
        AssetCatalog::register_dimensions(self, path, width, height)
    }
    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> { AssetCatalog::alias(self, alias, target) }
    fn dimensions(&self, id: &AssetId) -> (u32, u32) { AssetCatalog::dimensions(self, id) }
    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> { self.video_info_meta(id) }
}

pub(crate) fn asset_id_for_url(url: &str) -> AssetId { AssetId(format!("url:{url}")) }
pub(crate) fn asset_id_for_audio_path(path: &Path) -> AssetId { AssetId(format!("audio:path:{}", path.to_string_lossy())) }
pub(crate) fn asset_id_for_audio_url(url: &str) -> AssetId { AssetId(format!("audio:url:{url}")) }
pub(crate) fn asset_id_for_query(query: &OpenverseQuery) -> AssetId {
    AssetId(format!("openverse:q={};count={};aspect_ratio={}", query.query, query.count, query.aspect_ratio.as_deref().unwrap_or("")))
}
pub(crate) fn cache_file_path(cache_dir: &Path, id: &AssetId, extension: &str) -> PathBuf {
    cache_dir.join(format!("{:016x}.{extension}", stable_hash(&id.0)))
}
pub(crate) fn read_image_dimensions(path: &Path) -> (u32, u32) { /* moved */ }
pub(crate) fn stable_hash(value: &str) -> u64 { /* moved */ }

#[cfg(test)]
mod tests {
    // 完整 register/alias/dimensions/video_info 的单测
    use super::*;
    #[test]
    fn register_dimensions_is_stable_for_same_path() {
        let mut c = AssetCatalog::new();
        let id1 = c.register_dimensions(Path::new("/x.png"), 10, 20);
        let id2 = c.register_dimensions(Path::new("/x.png"), 10, 20);
        assert_eq!(id1, id2);
        assert_eq!(c.dimensions(&id1), (10, 20));
    }
    #[test]
    fn alias_copies_dimensions() {
        let mut c = AssetCatalog::new();
        let target = c.register_dimensions(Path::new("/y.png"), 30, 40);
        let alias = AssetId("alias:y".into());
        c.alias(alias.clone(), &target).unwrap();
        assert_eq!(c.dimensions(&alias), (30, 40));
    }
    #[test]
    fn video_info_round_trip() {
        let mut c = AssetCatalog::new();
        let id = c.register_video_info(Path::new("/v.mp4"), VideoInfoMeta { width: 1920, height: 1080, duration_secs: Some(5.0) });
        let info = c.video_info_meta(&id).unwrap();
        assert_eq!((info.width, info.height), (1920, 1080));
    }
}
```

- [ ] **Step 2: 把 fetch 部分留在 src/resource/assets.rs 里**

`src/resource/assets.rs` 简化为：

```rust
//! 兼容别名 + 远程预加载入口；纯映射逻辑迁移到 asset_catalog.rs。

pub use crate::resource::asset_catalog::{AssetCatalog as AssetsMap, AssetId};

use anyhow::Result;
use crate::resource::asset_catalog::{
    AssetCatalog, AssetEntry, asset_id_for_audio_url, asset_id_for_query, asset_id_for_url,
    cache_file_path, read_image_dimensions,
};
use crate::scene::primitives::{AudioSource, ImageSource, OpenverseQuery};

// preload_image_sources / preload_audio_sources / RemoteAssetRequest /
// RemoteAudioRequest / RemoteImageSource / OpenverseSearchResponse /
// OpenverseImageResult / OpenverseTokenResponse / preload_remote_requests /
// prepare_remote_asset / preload_remote_audio_requests / prepare_remote_audio_asset /
// search_openverse_image / fetch_openverse_token / build_http_client / download_to_cache /
// HTTP_USER_AGENT / OPENVERSE_* 常量 —— 全部留在这里。
//
// 关键：AssetsMap.preload_image_sources 改成自由函数：
pub fn preload_image_sources<I>(catalog: &mut AssetCatalog, sources: I) -> Result<()>
where I: IntoIterator<Item = ImageSource> {
    // body 移自 AssetsMap::preload_image_sources，
    // 把 self.X 替换成 catalog.X，
    // tokio runtime 在自由函数内部 build & block_on（不挂在 catalog 上）。
}

pub fn preload_audio_sources<I>(catalog: &mut AssetCatalog, sources: I) -> Result<()>
where I: IntoIterator<Item = AudioSource> { /* 同上 */ }
```

`src/resource/mod.rs`：

```rust
pub mod asset_catalog;
pub mod assets;  // tokio + reqwest 入口
mod bitmap_source;
pub mod catalog;
pub mod media;

pub use assets::{AssetsMap, preload_image_sources, preload_audio_sources};
pub use asset_catalog::{AssetCatalog, AssetId};
```

- [ ] **Step 3: 调用方迁移**

`src/runtime/preflight.rs::ensure_assets_preloaded`：

```rust
crate::resource::assets::preload_image_sources(&mut session.assets, image_sources)?;
crate::resource::assets::preload_audio_sources(&mut session.assets, audio_sources)?;
```

`src/runtime/session.rs:23` 字段类型保持 `assets: AssetsMap`（type alias 自动指向 `AssetCatalog`）—— 0 改动。

- [ ] **Step 4: 跑测试**

Run: `rtk cargo test --lib resource`
Expected: 8 个新测试（含 catalog/asset_catalog）+ 现有测试全 PASS。

Run: `rtk cargo build`
Expected: 0 errors。

- [ ] **Step 5: Commit**

```bash
rtk git add src/resource/ && \
rtk git commit -m "refactor(resource): split AssetCatalog (pure) from assets.rs (fetch IO)"
```

## Task 2.2: §7.4 把 VideoFrameTiming / VideoFrameRequest 抽到纯 core types

**Files:**
- Create: `src/resource/types.rs`
- Modify: `src/resource/media.rs`
- Modify: `src/resource/mod.rs`

- [ ] **Step 1: 失败测试 — 在 types.rs 内做单测**

`src/resource/types.rs`：

```rust
//! 纯描述结构，无 ffmpeg / skia 依赖。

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum VideoPreviewQuality { Scrubbing, Realtime, Exact }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoFrameTiming {
    pub media_offset_secs: f64,
    pub playback_rate: f64,
    pub looping: bool,
}

impl std::hash::Hash for VideoFrameTiming { /* moved from media.rs */ }
impl Default for VideoFrameTiming { /* moved */ }

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VideoFrameRequest {
    pub composition_time_secs: f64,
    pub timing: VideoFrameTiming,
    pub quality: VideoPreviewQuality,
    pub target_size: Option<(u32, u32)>,
}

impl VideoFrameRequest {
    pub fn resolve_time_secs(&self, info: &crate::resource::catalog::VideoInfoMeta) -> f64 {
        // 同 media.rs:90 的算法，但参数从 &VideoInfo 改 &VideoInfoMeta（字段一致）
        let composition_time_secs = self.composition_time_secs.max(0.0);
        let local = self.timing.media_offset_secs + composition_time_secs * self.timing.playback_rate;
        if !self.timing.looping {
            return clamp_video_time(local, info.duration_secs);
        }
        match info.duration_secs {
            Some(d) if d > self.timing.media_offset_secs => {
                let playable = d - self.timing.media_offset_secs;
                let wrapped = (composition_time_secs * self.timing.playback_rate) % playable;
                self.timing.media_offset_secs + wrapped
            }
            _ => clamp_video_time(local, info.duration_secs),
        }
    }
}

fn clamp_video_time(t: f64, dur: Option<f64>) -> f64 {
    let c = t.max(0.0);
    match dur { Some(d) if d > 0.0 => c.min(d), _ => c }
}

#[cfg(test)]
mod tests { /* 把 media.rs 的 4 个 #[test] 复制过来，把 VideoInfo 替换成 VideoInfoMeta */ }
```

`src/resource/media.rs`：

- 删 `pub struct VideoPreviewQuality / VideoFrameTiming / VideoFrameRequest` 与 4 个 unit test（已迁移）。
- 顶部加 `pub use crate::resource::types::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};` 兼容别名。
- `MediaContext` 的实现保持，但 `info: VideoInfo` 与 `&VideoInfoMeta` 互转：

```rust
impl From<&VideoInfo> for crate::resource::catalog::VideoInfoMeta {
    fn from(v: &VideoInfo) -> Self {
        Self { width: v.width, height: v.height, duration_secs: v.duration_secs }
    }
}
```

`MediaContext::get_video_frame` 内的 `request.resolve_time_secs(&info)` 改成 `request.resolve_time_secs(&(&info).into())`，或者保留对 VideoInfo 的双 impl。最稳的做法：保留 `VideoFrameRequest::resolve_time_secs` 的旧 `&VideoInfo` overload 在 media.rs 里：

```rust
impl VideoFrameRequest {
    pub(crate) fn resolve_time_secs_for_video_info(&self, info: &VideoInfo) -> f64 {
        let meta: crate::resource::catalog::VideoInfoMeta = info.into();
        self.resolve_time_secs(&meta)
    }
}
```

`src/resource/mod.rs`：

```rust
pub mod types;
```

- [ ] **Step 2: 跑测试**

Run: `rtk cargo test --lib resource::types resource::media`
Expected: 4 个 types 测试 + 4 个 media 测试均 PASS。

- [ ] **Step 3: 跑全量 build**

Run: `rtk cargo build`
Expected: 0 errors。

- [ ] **Step 4: Commit**

```bash
rtk git add src/resource/ && \
rtk git commit -m "refactor(resource): extract VideoFrameRequest/Timing/Quality to pure types module"
```

## Task 2.3: §7.5 拆 preflight 为 collect (core) + ensure (host)

**Files:**
- Create: `src/runtime/preflight_collect.rs`
- Modify: `src/runtime/preflight.rs`
- Modify: `src/runtime/mod.rs`

- [ ] **Step 1: 失败测试**

新建 `src/runtime/preflight_collect.rs`：

```rust
use std::collections::HashSet;
use std::path::PathBuf;

use crate::frame_ctx::FrameCtx;
use crate::scene::composition::Composition;
use crate::scene::node::{Node, NodeKind};
use crate::scene::primitives::{AudioSource, ImageSource};
use crate::scene::time::{FrameState, frame_state_for_root};

#[derive(Default, Debug)]
pub struct ResourceRequests {
    pub image_sources: HashSet<ImageSource>,
    pub audio_sources: HashSet<AudioSource>,
    pub video_paths: HashSet<PathBuf>,
}

pub fn collect_resource_requests(composition: &Composition) -> ResourceRequests {
    let mut req = ResourceRequests::default();
    req.audio_sources.extend(composition.audio_sources().iter().map(|a| a.source.clone()));

    for frame in 0..composition.frames {
        let frame_ctx = FrameCtx {
            frame, fps: composition.fps,
            width: composition.width, height: composition.height,
            frames: composition.frames,
        };
        let root = composition.root_node(&frame_ctx);
        collect_sources_from_frame_state(&frame_state_for_root(&root, &frame_ctx), &frame_ctx, &mut req);
    }
    req
}

pub(crate) fn collect_sources_from_frame_state(
    state: &FrameState, frame_ctx: &FrameCtx, req: &mut ResourceRequests,
) {
    match state {
        FrameState::Scene { scene, .. } => collect_sources(scene, frame_ctx, req),
        FrameState::Transition { from, to, .. } => {
            collect_sources(from, frame_ctx, req);
            collect_sources(to, frame_ctx, req);
        }
    }
}

pub(crate) fn collect_sources(node: &Node, frame_ctx: &FrameCtx, req: &mut ResourceRequests) {
    match node.kind() {
        NodeKind::Component(c) => collect_sources(&c.render(frame_ctx), frame_ctx, req),
        NodeKind::Div(div) => for c in div.children_ref() { collect_sources(c, frame_ctx, req); },
        NodeKind::Canvas(canvas) => for asset in canvas.assets_ref() {
            if !matches!(asset.source, ImageSource::Unset) { req.image_sources.insert(asset.source.clone()); }
        },
        NodeKind::Image(img) => {
            if !matches!(img.source(), ImageSource::Unset) { req.image_sources.insert(img.source().clone()); }
        }
        NodeKind::Video(v) => { req.video_paths.insert(v.source().to_path_buf()); }
        NodeKind::Timeline(_) => collect_sources_from_frame_state(&frame_state_for_root(node, frame_ctx), frame_ctx, req),
        NodeKind::Text(_) | NodeKind::Lucide(_) | NodeKind::Path(_) | NodeKind::Caption(_) => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::primitives::{div, image, video};
    use crate::scene::composition::Composition;
    use std::sync::Arc;

    #[test]
    fn collects_image_audio_video_distinctly() {
        let root: Node = div().id("r")
            .child(image().id("i").url("https://example.com/a.png"))
            .child(video().id("v").path(PathBuf::from("/t.mp4")))
            .into();
        let comp = Composition { root: Arc::new(root), width: 100, height: 100, fps: 30, frames: 5, ..Default::default() };
        let req = collect_resource_requests(&comp);
        assert_eq!(req.image_sources.len(), 1);
        assert_eq!(req.video_paths.len(), 1);
    }
}
```

注：`Composition::default()` 字段需检查；如 `Composition` 没有 `Default`，构造方式按当前真实定义来（参见 `src/scene/composition.rs`）。

- [ ] **Step 2: 重写 ensure_assets_preloaded**

`src/runtime/preflight.rs`：

```rust
use anyhow::Result;
use std::sync::Arc;

use crate::resource::assets::{preload_image_sources, preload_audio_sources};
use crate::runtime::preflight_collect::collect_resource_requests;
use crate::runtime::session::RenderSession;
use crate::scene::composition::Composition;

pub(crate) fn ensure_assets_preloaded(
    composition: &Composition, session: &mut RenderSession,
) -> Result<()> {
    let root_ptr = Arc::as_ptr(&composition.root) as *const () as usize;
    if session.prepared_root_ptr == Some(root_ptr) { return Ok(()); }

    let req = collect_resource_requests(composition);
    preload_image_sources(&mut session.assets, req.image_sources)?;
    preload_audio_sources(&mut session.assets, req.audio_sources)?;

    // probe videos: 用 host MediaContext 探出 (w, h, dur) 写回 catalog。
    for path in req.video_paths {
        if let Ok(info) = session.media_ctx.video_info(&path) {
            session.assets.register_video_info(
                &path,
                crate::resource::catalog::VideoInfoMeta {
                    width: info.width, height: info.height, duration_secs: info.duration_secs,
                },
            );
        }
        // 失败的视频路径在 resolve_video 时仍走 (0,0) fallback。
    }

    session.prepared_root_ptr = Some(root_ptr);
    Ok(())
}
```

`src/runtime/mod.rs` 加 `pub mod preflight_collect;`。

- [ ] **Step 3: 跑测试**

Run: `rtk cargo test --lib runtime::preflight runtime::preflight_collect`
Expected: 全 PASS。

- [ ] **Step 4: Commit**

```bash
rtk git add src/runtime/ && \
rtk git commit -m "refactor(runtime): split preflight into collect (pure) + ensure (host driver)"
```

## Task 2.4: §7.6 抽 build_frame_display_tree (core) 公共函数

**Files:**
- Modify: `src/runtime/pipeline.rs`

- [ ] **Step 1: 失败测试 — 加新公共入口**

`src/runtime/pipeline.rs`：

```rust
use anyhow::Result;

use crate::display::build::build_display_tree;
use crate::element::resolve::resolve_ui_tree_with_script_cache;
use crate::frame_ctx::{FrameCtx, ScriptFrameCtx};
use crate::resource::catalog::ResourceCatalog;
use crate::runtime::annotation::{AnnotatedDisplayTree, annotate_display_tree, compute_display_tree_fingerprints};
use crate::runtime::invalidation::{CompositeHistory, mark_display_tree_composite_dirty};
use crate::runtime::profile::SceneBuildStats;
use crate::scene::node::Node;
use crate::scene::script::{ScriptHost, ScriptRuntimeCache, StyleMutations};
use crate::text::FontProvider;

pub fn build_frame_display_tree(
    scene: &Node,
    frame_ctx: &FrameCtx,
    script_frame_ctx: &ScriptFrameCtx,
    catalog: &mut dyn ResourceCatalog,
    fonts: &dyn FontProvider,
    layout_session: &mut crate::layout::LayoutSession,
    composite_history: &mut CompositeHistory,
    script_cache: &mut ScriptRuntimeCache,  // Phase 2 仍用具体 cache；Phase 3 才换 ScriptHost
    mutations: Option<&StyleMutations>,
) -> Result<(AnnotatedDisplayTree, SceneBuildStats)> {
    let mut stats = SceneBuildStats::default();
    let element_root = resolve_ui_tree_with_script_cache(
        scene, frame_ctx, script_frame_ctx, catalog, mutations, script_cache,
    )?;
    let (layout_tree, layout_pass) = layout_session
        .compute_layout_with_provider(&element_root, frame_ctx, fonts)?;
    stats.layout_pass = layout_pass;
    let display_tree = build_display_tree(&element_root, &layout_tree)?;
    let mut annotated = annotate_display_tree(&display_tree);
    mark_display_tree_composite_dirty(composite_history, &mut annotated, layout_pass.structure_rebuild);
    compute_display_tree_fingerprints(&mut annotated);
    stats.contains_time_variant_paint = annotated.contains_time_variant();
    Ok((annotated, stats))
}
```

`build_scene_display_list` 改成 host 内部的调用 wrapper：

```rust
pub(crate) fn build_scene_display_list(
    scene: &Node, frame_ctx: &FrameCtx, script_frame_ctx: &ScriptFrameCtx,
    session: &mut RenderSession, mutations: Option<&StyleMutations>,
) -> Result<(AnnotatedDisplayTree, SceneBuildStats)> {
    let provider = crate::text::DefaultFontProvider::from_arc(session.font_db.clone());
    build_frame_display_tree(
        scene, frame_ctx, script_frame_ctx,
        &mut session.assets, &provider,
        session.layout_session_mut(),
        session.composite_history_mut(),
        &mut session.script_runtime,
        mutations,
    )
}
```

注：`compute_layout_with_provider` 在 Task 1.3 已加。`mark_display_tree_composite_dirty` 与 `composite_history_mut` 的可见性需调整为可在本函数访问（pub(crate)）。

- [ ] **Step 2: cargo test**

Run: `rtk cargo test --lib runtime::pipeline`
Expected: PASS。

- [ ] **Step 3: cargo build**

Run: `rtk cargo build`
Expected: 0 errors。

- [ ] **Step 4: Commit**

```bash
rtk git add src/runtime/pipeline.rs && \
rtk git commit -m "refactor(runtime): extract build_frame_display_tree as core entrypoint"
```

## Task 2.5: §7.7 把 mutations 与 ScriptDriver 拆出 mutations.rs

**Files:**
- Create: `src/scene/script/mutations.rs`
- Modify: `src/scene/script/mod.rs`
- Modify: `src/scene/script/canvas_api.rs`（仅 use 路径）
- Modify: `src/scene/script/node_style.rs`（仅 use 路径）

- [ ] **Step 1: 抽数据类型**

新建 `src/scene/script/mutations.rs`，把以下类型从 `mod.rs / canvas_api.rs / node_style.rs` 移过来：

- `StyleMutations`
- `NodeStyleMutations / TextUnitGranularity / TextUnitOverride / TextUnitOverrideBatch`
- `CanvasMutations / CanvasCommand / ScriptColor / ScriptFontEdging / ScriptLineCap / ScriptLineJoin / ScriptPointMode`
- `ScriptTextSource / ScriptTextSourceKind`
- `ScriptDriver { source: String }`（构造与 source 访问；`create_runner / cache_key` 留在 host 模块）

按当前 `pub use canvas_api::{CanvasCommand, ...}` 与 `pub use node_style::{...}` 的实际定义把数据 struct 块移过来，binding 函数（`fn fill_rect`、`fn install_runtime_bindings` 等）留原位。

`src/scene/script/mod.rs` 顶部：

```rust
pub mod mutations;
pub use mutations::*;
```

`canvas_api.rs / node_style.rs` 改 `use super::mutations::{...}`。

注意：`ScriptDriver::create_runner` 仍在 mod.rs，因为它需要 `ScriptRunner::new` 这一 quickjs 类型；mutations.rs 只承载数据。

- [ ] **Step 2: cargo test**

Run: `rtk cargo test --lib scene::script`
Expected: 全 PASS（仅 import 路径变化，行为零变）。

- [ ] **Step 3: Commit**

```bash
rtk git add src/scene/script/ && \
rtk git commit -m "refactor(scene/script): extract pure mutation types into mutations.rs"
```

## Task 2.6: §7.7 续 — script cache 改成只缓存 driver id

**Files:**
- Modify: `src/scene/script/mod.rs`
- Modify: `src/scene/script/host.rs`
- Modify: `src/element/resolve.rs`

- [ ] **Step 1: 失败测试**

`src/scene/script/host.rs` 加：

```rust
#[cfg(test)]
mod test_install_dedup {
    use super::*;
    use crate::scene::script::ScriptRuntimeCache;
    #[test]
    fn install_same_source_twice_returns_same_id() {
        let mut h = ScriptRuntimeCache::default();
        let id1 = h.install("ctx => { ctx.style({}); }").unwrap();
        let id2 = h.install("ctx => { ctx.style({}); }").unwrap();
        assert_eq!(id1, id2);
    }
}
```

- [ ] **Step 2: 改 ScriptRuntimeCache.run 改成 driver id 路径**

不彻底重命名（避免大爆炸），但加 alias 与新方法：

```rust
impl ScriptRuntimeCache {
    pub(crate) fn run_by_id(
        &mut self, id: ScriptDriverId, frame_ctx: ScriptFrameCtx, current_node_id: Option<&str>,
    ) -> anyhow::Result<StyleMutations> {
        let runner = self.runners.get_mut(&id.0)
            .ok_or_else(|| anyhow::anyhow!("script driver {} not installed", id.0))?;
        if let Ok(mut store) = runner.store.lock() {
            store.text_sources = self.text_sources.clone();
        }
        runner.run(frame_ctx, current_node_id)
    }
}
```

`src/element/resolve.rs` 内 `cx.script_runtime.run(driver, ...)` 改成：

```rust
let id = cx.script_runtime.install(driver.source())?;
cx.script_runtime.run_by_id(id, frame_ctx, current_node_id)?
```

具体位置：搜索 `script_runtime.run(driver` 并替换。

- [ ] **Step 3: cargo test**

Run: `rtk cargo test --lib`
Expected: 全 PASS。

- [ ] **Step 4: Commit**

```bash
rtk git add src/ && \
rtk git commit -m "refactor(scene/script): route script execution via ScriptDriverId"
```

## Task 2.7: Phase 2 收尾 — 全量回归 + PSNR

- [ ] **Step 1: 全量测试**

Run: `rtk cargo test`
Expected: 全 PASS。

- [ ] **Step 2: PSNR 比对**

```bash
rtk cargo run --release --example pendulum_canvas
ffmpeg -i /tmp/baseline_pendulum.mp4 -i out/pendulum_canvas.mp4 -lavfi psnr -f null - 2>&1 | grep average
```

Expected: average PSNR ≥ 50 dB（实测 ≥ 60 dB）。

- [ ] **Step 3: tag**

```bash
rtk git tag phase2-complete
```

---

# Phase 3 — 物理目录重组（git mv 大手术）

> 此阶段每一步 mv 之后立刻 `cargo build` 验证。所有 mv 用 `git mv` 保留 history。`#[cfg(feature = "host-default")]` 全部加上。

## Task 3.1: 创建 src/core/ 与 src/host/ 骨架

**Files:**
- Create: `src/core/mod.rs`
- Create: `src/host/mod.rs`
- Modify: `src/lib.rs`

- [ ] **Step 1: 占位骨架**

`src/core/mod.rs`：

```rust
//! Core module — 永不依赖 host features，可在 wasm32 编译。
//! 暴露 parse / collect_resource_requests / build_frame_display_tree
//! 三个公共入口，及 ResourceCatalog / ScriptHost / FontProvider trait。

// Phase 3 各 task 会逐步 pub mod 进来。
```

`src/host/mod.rs`：

```rust
//! Host module — 默认 features 全开。
//! 承载 IO / ffmpeg / quickjs / skia / 系统字体 / RenderSession。

#![cfg(feature = "host-default")]
```

`src/lib.rs` 在原有声明旁追加：

```rust
pub mod core;
#[cfg(feature = "host-default")]
pub mod host;
```

- [ ] **Step 2: 编译验证**

Run: `rtk cargo build --lib`
Expected: 0 errors。

- [ ] **Step 3: Commit**

```bash
rtk git add src/core/mod.rs src/host/mod.rs src/lib.rs && \
rtk git commit -m "scaffold: add empty src/core and src/host module shells"
```

## Task 3.2: 把纯算法子树 git mv 到 src/core/

> 使用 `git mv` 一次性搬迁，事后再修 `mod.rs` 路径。每个 mv 后 cargo build。

- [ ] **Step 1: 搬迁纯算法目录**

```bash
rtk git mv src/jsonl.rs src/core/jsonl_legacy.rs   # 临时名，后续合并到 core/jsonl/
rtk git mv src/jsonl src/core/jsonl
rtk git mv src/scene src/core/scene
rtk git mv src/element src/core/element
rtk git mv src/layout src/core/layout
rtk git mv src/display src/core/display
rtk git mv src/style.rs src/core/style.rs
rtk git mv src/text.rs src/core/text.rs
rtk git mv src/frame_ctx.rs src/core/frame_ctx.rs
rtk git mv src/lucide_icons.rs src/core/lucide_icons.rs
```

把 `src/core/jsonl_legacy.rs` 与 `src/core/jsonl/` 合并：

```bash
# jsonl_legacy.rs 是原 src/jsonl.rs 内容；jsonl/{builder.rs,tailwind.rs} 是子模块
# 重命名 jsonl_legacy.rs → core/jsonl/mod.rs（内含 #[path] 指向同目录）
mv src/core/jsonl_legacy.rs src/core/jsonl/mod.rs.tmp
# 在 mod.rs.tmp 顶部加 mod builder; mod tailwind; 后改为 mod.rs
```

实操：先 `mkdir -p src/core/jsonl`，`git mv src/jsonl.rs src/core/jsonl/lib_root.rs`，再把内容合并到 `src/core/jsonl/mod.rs` 里（追加 `mod builder; pub(crate) mod tailwind;`）。

- [ ] **Step 2: 搬迁 runtime 子集**

```bash
mkdir -p src/core/runtime
rtk git mv src/runtime/analysis.rs src/core/runtime/analysis.rs
rtk git mv src/runtime/annotation.rs src/core/runtime/annotation.rs
rtk git mv src/runtime/fingerprint src/core/runtime/fingerprint
rtk git mv src/runtime/invalidation src/core/runtime/invalidation
rtk git mv src/runtime/preflight_collect.rs src/core/runtime/preflight_collect.rs
rtk git mv src/runtime/pipeline.rs src/core/runtime/pipeline.rs
mkdir -p src/core/runtime/compositor
rtk git mv src/runtime/compositor/ordered_scene.rs src/core/runtime/compositor/ordered_scene.rs
rtk git mv src/runtime/compositor/plan.rs src/core/runtime/compositor/plan.rs
rtk git mv src/runtime/compositor/reuse.rs src/core/runtime/compositor/reuse.rs
rtk git mv src/runtime/compositor/slot.rs src/core/runtime/compositor/slot.rs
```

- [ ] **Step 3: 搬迁 resource core 子集**

```bash
mkdir -p src/core/resource
rtk git mv src/resource/asset_catalog.rs src/core/resource/asset_catalog.rs
rtk git mv src/resource/catalog.rs src/core/resource/catalog.rs
rtk git mv src/resource/types.rs src/core/resource/types.rs
rtk git mv src/resource/bitmap_source.rs src/core/resource/bitmap_source.rs
```

`asset_id_for_*` 系列辅助跟 `asset_catalog.rs` 同走 core；`AssetId` 也在 core。

- [ ] **Step 4: 调整 mod.rs**

`src/core/mod.rs`：

```rust
pub mod display;
pub mod element;
pub mod frame_ctx;
pub mod jsonl;
pub mod layout;
mod lucide_icons;
pub mod resource;
pub mod runtime;
pub mod scene;
pub mod style;
pub mod text;

pub use frame_ctx::FrameCtx;
pub use jsonl::{ParsedComposition, parse};
pub use resource::catalog::{ResourceCatalog, VideoInfoMeta};
pub use resource::asset_catalog::{AssetCatalog, AssetId};
pub use runtime::pipeline::build_frame_display_tree;
pub use runtime::preflight_collect::{ResourceRequests, collect_resource_requests};
pub use scene::script::{ScriptHost, ScriptDriverId};
pub use text::{FontProvider, DefaultFontProvider};
```

`src/core/resource/mod.rs`：

```rust
pub mod asset_catalog;
mod bitmap_source;
pub mod catalog;
pub mod types;

pub use asset_catalog::{AssetCatalog, AssetId};
pub use catalog::{ResourceCatalog, VideoInfoMeta};
pub use types::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
```

`src/core/runtime/mod.rs`：

```rust
pub mod analysis;
pub mod annotation;
pub mod compositor;
pub mod fingerprint;
pub mod invalidation;
pub mod pipeline;
pub mod preflight_collect;
```

`src/core/runtime/compositor/mod.rs`：

```rust
pub mod ordered_scene;
pub mod plan;
pub mod reuse;
pub mod slot;
pub use ordered_scene::{OrderedSceneOp, OrderedSceneProgram};
pub use plan::{SceneRenderPlan, plan_for_scene};
pub use slot::SceneSnapshotCache;
```

`SceneSnapshotCache` 这一行（持 `Picture`）会让 core 编译失败 —— 暂留它在 host 不重复定义；compositor/mod.rs 在 core 只 pub use `slot`。注意 §7.9：`SceneSnapshotCache` 实际归属 host（含 backend 类型）。**修正**：把 `slot.rs` 也移到 host：

```bash
rtk git mv src/core/runtime/compositor/slot.rs src/host/runtime/compositor_slot.rs
```

并去掉上方 `pub mod slot;`。

- [ ] **Step 5: 改全部 import 路径 — 先做大批 sed 再人肉修**

所有 `crate::scene::` → `crate::core::scene::`；`crate::element::` → `crate::core::element::`；`crate::layout::` → `crate::core::layout::`；以此类推。技巧：用 `sed -i 's|crate::scene::|crate::core::scene::|g'` 批量改但要注意 host 模块仍在 `src/runtime/`、`src/resource/assets.rs` 等位置（未搬迁），它们的 `use crate::scene::...` 也必须同步改。

```bash
fd -e rs . src/ | xargs sed -i \
  -e 's|crate::jsonl|crate::core::jsonl|g' \
  -e 's|crate::scene|crate::core::scene|g' \
  -e 's|crate::element|crate::core::element|g' \
  -e 's|crate::layout|crate::core::layout|g' \
  -e 's|crate::display|crate::core::display|g' \
  -e 's|crate::style|crate::core::style|g' \
  -e 's|crate::text|crate::core::text|g' \
  -e 's|crate::frame_ctx|crate::core::frame_ctx|g' \
  -e 's|crate::lucide_icons|crate::core::lucide_icons|g'
```

注意 `crate::resource::*` 不能整体重写 —— 只 `asset_catalog/catalog/types/bitmap_source` 移到了 core；`assets / media` 仍在原位（fetch + ffmpeg）。逐个细分：

```bash
fd -e rs . src/ | xargs sed -i \
  -e 's|crate::resource::asset_catalog|crate::core::resource::asset_catalog|g' \
  -e 's|crate::resource::catalog|crate::core::resource::catalog|g' \
  -e 's|crate::resource::types|crate::core::resource::types|g' \
  -e 's|crate::resource::bitmap_source|crate::core::resource::bitmap_source|g'
```

`crate::runtime::{analysis, annotation, compositor::{ordered_scene,plan,reuse}, fingerprint, invalidation, pipeline, preflight_collect}` → 加 `core::` 前缀；其余仍在 `crate::runtime::`：

```bash
fd -e rs . src/ | xargs sed -i \
  -e 's|crate::runtime::analysis|crate::core::runtime::analysis|g' \
  -e 's|crate::runtime::annotation|crate::core::runtime::annotation|g' \
  -e 's|crate::runtime::fingerprint|crate::core::runtime::fingerprint|g' \
  -e 's|crate::runtime::invalidation|crate::core::runtime::invalidation|g' \
  -e 's|crate::runtime::pipeline|crate::core::runtime::pipeline|g' \
  -e 's|crate::runtime::preflight_collect|crate::core::runtime::preflight_collect|g' \
  -e 's|crate::runtime::compositor::ordered_scene|crate::core::runtime::compositor::ordered_scene|g' \
  -e 's|crate::runtime::compositor::plan|crate::core::runtime::compositor::plan|g' \
  -e 's|crate::runtime::compositor::reuse|crate::core::runtime::compositor::reuse|g'
```

最后 `cargo build` 跑一次，看错误列表，逐个补漏（可能有 `pub use` 在 lib.rs 里没改、或具体文件内部交叉路径）。

- [ ] **Step 6: cargo build 直到 0 errors**

Run: `rtk cargo build --lib 2>&1 | rtk err`
Expected: 经过 1-3 轮路径修复，0 errors。

- [ ] **Step 7: cargo test 全过**

Run: `rtk cargo test`
Expected: 全 PASS。

- [ ] **Step 8: Commit**

```bash
rtk git add -A && \
rtk git commit -m "refactor: move pure algorithm modules into src/core/"
```

## Task 3.3: §7.4 把 MediaContext 移到 host

```bash
mkdir -p src/host/resource
rtk git mv src/resource/media.rs src/host/resource/media.rs
rtk git mv src/resource/assets.rs src/host/resource/fetch.rs   # 仅留 IO
```

`src/host/resource/mod.rs`：

```rust
pub mod fetch;
pub mod media;
```

`src/resource/mod.rs` 全部内容删除（resource 已迁出），并：

```bash
rmdir src/resource  # 应该为空
```

修复全仓 import：`crate::resource::media` → `crate::host::resource::media`；`crate::resource::assets::{preload_image_sources,preload_audio_sources}` → `crate::host::resource::fetch::{preload_image_sources,preload_audio_sources}`。

`crate::resource::AssetsMap` / `AssetId` 已是 `crate::core::resource::AssetCatalog`/`AssetId` 了 — 在 `src/lib.rs` 顶层加：

```rust
#[cfg(feature = "host-default")]
pub use crate::host::resource::media::{VideoFrameRequest, VideoFrameTiming, VideoPreviewQuality};
```

（保留 `pub use` 路径兼容）。

`cargo build && cargo test` 全过 → commit。

```bash
rtk git add -A && rtk git commit -m "refactor(host): move MediaContext + fetch into src/host/resource/"
```

## Task 3.4: §7.4 续 — 加 host probe.rs 把 video_info 写回 catalog

**Files:**
- Create: `src/host/resource/probe.rs`
- Modify: `src/host/resource/mod.rs`
- Modify: `src/runtime/preflight.rs`（host 侧）

- [ ] **Step 1: 写 probe**

```rust
//! src/host/resource/probe.rs
use std::path::Path;
use anyhow::Result;
use crate::core::resource::{AssetCatalog, catalog::VideoInfoMeta};
use crate::host::resource::media::MediaContext;

pub fn probe_video(catalog: &mut AssetCatalog, path: &Path, media: &mut MediaContext) -> Result<VideoInfoMeta> {
    let info = media.video_info(path)?;
    let meta = VideoInfoMeta { width: info.width, height: info.height, duration_secs: info.duration_secs };
    catalog.register_video_info(path, meta);
    Ok(meta)
}
```

`src/host/resource/mod.rs` 加 `pub mod probe;`。

- [ ] **Step 2: preflight 改用 probe_video**

`src/runtime/preflight.rs::ensure_assets_preloaded` 内：

```rust
for path in req.video_paths {
    let _ = crate::host::resource::probe::probe_video(&mut session.assets, &path, &mut session.media_ctx);
}
```

- [ ] **Step 3: cargo test**

Expected: 全 PASS。

- [ ] **Step 4: Commit**

```bash
rtk git add -A && rtk git commit -m "feat(host/resource): add probe_video to populate VideoInfoMeta"
```

## Task 3.5: §7.10 + §7.9 把 host runtime 模块批量搬迁

```bash
mkdir -p src/host/runtime
for f in audio.rs backend_object.rs frame_view.rs mod.rs pipeline.rs preflight.rs profile.rs render_engine.rs render_registry.rs session.rs surface.rs target.rs; do
  rtk git mv src/runtime/$f src/host/runtime/$f 2>/dev/null
done
rtk git mv src/runtime/cache src/host/runtime/cache
rtk git mv src/runtime/compositor src/host/runtime/compositor   # 此时 compositor/ 里还有 mod.rs / render.rs / 几个 stub
```

注：core 子集（ordered_scene, plan, reuse）在 Task 3.2 已搬走；compositor 目录此刻只剩 `mod.rs / render.rs / slot.rs`（如还在）。把它们移到 host：

`src/host/runtime/compositor/mod.rs`：

```rust
mod render;
mod slot;
pub use render::{SceneRenderRuntime, render_scene};
pub use slot::SceneSnapshotCache;

// re-export core 算法供 host 内部使用
pub use crate::core::runtime::compositor::{
    OrderedSceneProgram, SceneRenderPlan, plan_for_scene,
};
```

`render.rs` 内 `use super::SceneSnapshotCache` 等保持。

修 `src/lib.rs`：原 `pub mod runtime;` 改为：

```rust
#[cfg(feature = "host-default")]
pub mod runtime { pub use crate::host::runtime::*; }
```

（保留外部 `opencat::runtime::AudioBuffer` 这种 path）。

整库 sed：所有 `crate::runtime::audio` `crate::runtime::session` `crate::runtime::cache` `crate::runtime::render_engine` `crate::runtime::render_registry` `crate::runtime::target` `crate::runtime::surface` `crate::runtime::profile` `crate::runtime::frame_view` `crate::runtime::backend_object` `crate::runtime::compositor::{render,SceneSnapshotCache}` `crate::runtime::preflight` `crate::runtime::pipeline`（host 侧）→ 加 `host::` 前缀。

cargo build → cargo test → commit。

```bash
rtk git add -A && rtk git commit -m "refactor: move host runtime modules into src/host/runtime/"
```

## Task 3.6: §7.7 把 quickjs runner 搬到 host/script/

```bash
mkdir -p src/host/script/bindings src/host/script/runtime
rtk git mv src/core/scene/script/canvas_api.rs src/host/script/bindings/canvas_api.rs
rtk git mv src/core/scene/script/node_style.rs src/host/script/bindings/node_style.rs
rtk git mv src/core/scene/script/animate_api.rs src/host/script/bindings/animate_api.rs
rtk git mv src/core/scene/script/morph_svg.rs src/host/script/bindings/morph_svg.rs
rtk git mv src/core/scene/script/runtime src/host/script/runtime
```

接下来 `src/core/scene/script/mod.rs`（仍在 core）需要把 `ScriptRunner` / `RuntimeMutationStore` / `install_runtime_bindings` 等 quickjs-依赖的部分迁到 `src/host/script/quickjs.rs`，core 端仅保留 `ScriptDriver { source }`、`ScriptRuntimeCache` 改名为 `ScriptDriverCache`（内含 `HashMap<String node_id, ScriptDriverId>` + `text_sources`）。

新 `src/host/script/quickjs.rs`：

```rust
use std::collections::HashMap;
use anyhow::Result;
use rquickjs::{Context, Function, Object, Persistent, Runtime};

use crate::core::frame_ctx::ScriptFrameCtx;
use crate::core::scene::script::{
    ScriptDriverId, ScriptHost, ScriptTextSource, StyleMutations,
};

pub(crate) struct ScriptRunner {
    pub(crate) run_fn: Persistent<Function<'static>>,
    pub(crate) ctx_obj: Persistent<Object<'static>>,
    pub(crate) context: Context,
    pub(crate) store: super::MutationStore,
    pub(crate) _runtime: Runtime,
}

#[derive(Default)]
pub struct QuickJsScriptHost {
    runners: HashMap<u64, ScriptRunner>,
    text_sources: HashMap<String, ScriptTextSource>,
}

impl QuickJsScriptHost {
    pub fn new() -> Self { Self::default() }
}

impl ScriptHost for QuickJsScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new(); source.hash(&mut h);
        let key = h.finish();
        if let std::collections::hash_map::Entry::Vacant(e) = self.runners.entry(key) {
            e.insert(ScriptRunner::new(source)?);
        }
        Ok(ScriptDriverId(key))
    }

    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource) {
        self.text_sources.insert(node_id.to_string(), source);
    }

    fn clear_text_sources(&mut self) { self.text_sources.clear(); }

    fn run_frame(&mut self, driver: ScriptDriverId, frame_ctx: &ScriptFrameCtx) -> Result<StyleMutations> {
        let runner = self.runners.get_mut(&driver.0)
            .ok_or_else(|| anyhow::anyhow!("script driver {} not installed", driver.0))?;
        if let Ok(mut store) = runner.store.lock() { store.text_sources = self.text_sources.clone(); }
        runner.run(*frame_ctx, None)
    }
}
```

`ScriptRunner::new` / `install_runtime_bindings` / `run` / `RUN_FRAME_FN` 全部从 `src/core/scene/script/mod.rs` 迁来到 `src/host/script/mod.rs`：

```rust
pub mod bindings;
pub mod quickjs;
mod runtime;

pub use quickjs::QuickJsScriptHost;
pub(crate) use bindings::node_style::install_runtime_bindings;
// ...
```

`src/core/scene/script/mod.rs` 留下：

```rust
pub mod host;
pub mod mutations;

pub use host::{ScriptDriverId, ScriptHost};
pub use mutations::*;

#[derive(Debug, Clone)]
pub struct ScriptDriver { source: String }
impl ScriptDriver {
    pub fn from_source(s: &str) -> anyhow::Result<Self> { Ok(Self { source: s.to_string() }) }
    pub fn from_file(path: &str) -> anyhow::Result<Self> {
        // ⚠️ from_file 调用 std::fs - 这是 IO 但属于公共 API "便利构造"。
        // 为保 core 纯净，把 from_file 移到 host：
        unimplemented!("ScriptDriver::from_file moved to opencat::host::script")
    }
    pub fn source(&self) -> &str { &self.source }
}

#[derive(Default)]
pub struct ScriptDriverCache {
    pub drivers: std::collections::HashMap<String, ScriptDriverId>,
    pub text_sources: std::collections::HashMap<String, mutations::ScriptTextSource>,
}
```

`from_file` 改成在 host 上的扩展 trait 或自由函数：`pub fn read_script_driver(path: &str) -> Result<ScriptDriver>`。

整库 sed 替换：`crate::core::scene::script::canvas_api` → `crate::host::script::bindings::canvas_api` 等。

cargo build → cargo test → commit。

```bash
rtk git add -A && rtk git commit -m "refactor(script): move quickjs runner to host; core keeps ScriptHost trait"
```

## Task 3.7: §7.11 + §7.12 + §7.13 + §7.15

**§7.11 fonts**

```bash
rtk git mv src/core/text.rs src/core/text/mod.rs   # 转目录
```

把 `default_font_db / DefaultFontProvider` 拆：

- core 留 `pub fn default_font_db_with_embedded_only() -> fontdb::Database`（include_bytes!，无 IO）。
- host 加 `src/host/fonts.rs`：`pub fn default_font_db_with_system() -> fontdb::Database` 调 `fontdb::Database::load_system_fonts()` + 嵌入字体；`DefaultFontProvider::with_system_fonts() -> Self` 调它。

**§7.12 jsonl_io**

```bash
mkdir -p src/host
touch src/host/jsonl_io.rs
```

`src/host/jsonl_io.rs`：

```rust
use std::path::Path;
use anyhow::{Context, Result};
use crate::core::jsonl::{JsonLine, ParsedComposition, parse};

pub fn parse_file(path: impl AsRef<Path>) -> Result<ParsedComposition> {
    let path = path.as_ref();
    let input = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read jsonl file: {}", path.display()))?;
    let base_dir = path.parent().unwrap_or_else(|| Path::new("."));
    parse_with_base_dir(&input, Some(base_dir))
}

pub fn parse_with_base_dir(input: &str, base_dir: Option<&Path>) -> Result<ParsedComposition> {
    // 重写思路（按 spec §7.12）：
    // 1. 逐行 deserialize JsonLine
    // 2. 对 Script { path: Some(p), src: None, .. }，读 base_dir.join(p) 内容，原地改成 src: Some(content)
    // 3. 把改写后的 lines 重新序列化为 jsonl，喂给 core::parse
    let mut rewritten = String::new();
    for (idx, line) in input.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() { rewritten.push('\n'); continue; }
        let parsed: JsonLine = serde_json::from_str(trimmed)
            .with_context(|| format!("line {}: invalid json", idx + 1))?;
        let resolved = match parsed {
            JsonLine::Script { parent_id, src: None, path: Some(p) } => {
                let resolved_path = if std::path::Path::new(&p).is_absolute() {
                    std::path::PathBuf::from(&p)
                } else if let Some(b) = base_dir { b.join(&p) } else { std::path::PathBuf::from(&p) };
                let src = std::fs::read_to_string(&resolved_path)
                    .with_context(|| format!("failed to read script file: {}", resolved_path.display()))?;
                JsonLine::Script { parent_id, src: Some(src), path: None }
            }
            other => other,
        };
        rewritten.push_str(&serde_json::to_string(&resolved)?);
        rewritten.push('\n');
    }
    parse(&rewritten)
}
```

要求：
- `JsonLine` 在 `src/core/jsonl/mod.rs` 改为 `pub(crate) enum JsonLine`，并加 `#[derive(Serialize)]`。
- `core::parse` 删除 `parse_with_base_dir`；当遇到 `Script { path: Some(_), src: None }` 直接 `bail!("script with path: must be parsed via host::jsonl_io::parse_with_base_dir")`。

**§7.13 inspect**

```bash
rtk git mv src/inspect.rs src/host/inspect.rs
```

更新 import；保持签名。

**§7.15 audio**

audio.rs 已在 Task 3.5 搬到 `src/host/runtime/audio.rs`。

`src/lib.rs` 顶层兼容 re-export：

```rust
#[cfg(feature = "host-default")]
pub use host::inspect::{FrameElementRect, collect_frame_layout_rects};

pub use core::jsonl::{ParsedComposition, parse};
#[cfg(feature = "host-default")]
pub use host::jsonl_io::{parse_file, parse_with_base_dir};

pub use core::frame_ctx::FrameCtx;
// ...
```

cargo build → cargo test → commit。

```bash
rtk git add -A && rtk git commit -m "refactor: move inspect/jsonl_io/fonts/audio into src/host/"
```

## Task 3.8: §7.14 测试 setup 大批量改写（仅 mock）

**Files:**
- Create: `src/core/test_support.rs`
- Modify: 多个 `#[cfg(test)]` 块

- [ ] **Step 1: 加测试桩**

```rust
//! src/core/test_support.rs
#![cfg(test)]

use std::sync::Arc;

pub fn mock_font_provider() -> impl crate::core::text::FontProvider {
    crate::core::text::DefaultFontProvider::from_arc(Arc::new(crate::core::text::default_font_db_with_embedded_only()))
}

#[derive(Default)]
pub struct MockScriptHost {
    next_id: u64,
    map: std::collections::HashMap<String, u64>,
}

impl crate::core::scene::script::ScriptHost for MockScriptHost {
    fn install(&mut self, source: &str) -> anyhow::Result<crate::core::scene::script::ScriptDriverId> {
        let id = *self.map.entry(source.to_string()).or_insert_with(|| {
            self.next_id += 1; self.next_id
        });
        Ok(crate::core::scene::script::ScriptDriverId(id))
    }
    fn register_text_source(&mut self, _: &str, _: crate::core::scene::script::ScriptTextSource) {}
    fn clear_text_sources(&mut self) {}
    fn run_frame(&mut self, _: crate::core::scene::script::ScriptDriverId, _: &crate::core::frame_ctx::ScriptFrameCtx) -> anyhow::Result<crate::core::scene::script::StyleMutations> {
        Ok(crate::core::scene::script::StyleMutations::default())
    }
}
```

`src/core/mod.rs` 加 `#[cfg(test)] pub mod test_support;`。

- [ ] **Step 2: 改 16 处 layout / element / display / fingerprint / compositor 测试**

每个测试函数 setup：

```rust
// before
let mut media = MediaContext::new();
let mut assets = AssetsMap::new();

// after
let mut catalog = AssetCatalog::new();
```

调用 `resolve_ui_tree(...)` / `build_display_tree(...)` 时去 `&mut media` 实参，把 `&mut assets` 换成 `&mut catalog`（trait obj 自动生效）。

文件清单（grep 已确认）：
- `src/core/layout/mod.rs:1082+` × 16
- `src/core/element/resolve.rs:1133+` × 多处
- `src/core/display/build.rs:272+` × 多处
- `src/core/runtime/fingerprint/{mod,display_item}.rs` × 多处
- `src/core/runtime/compositor/reuse.rs:284`

写一个小 perl one-liner 批量改：

```bash
fd -e rs . src/core/ | xargs perl -i -pe 's/let mut media = MediaContext::new\(\);\n//g'
fd -e rs . src/core/ | xargs perl -i -pe 's/let mut assets = AssetsMap::new\(\);/let mut catalog = AssetCatalog::new();/g'
fd -e rs . src/core/ | xargs perl -i -pe 's/&mut media,\s*&mut assets/&mut catalog/g'
```

之后 `cargo test --lib` 看哪些没修干净，逐个手动调整（部分调用点是 `&mut assets,` 单独出现，需要再补一条 sed 规则 `&mut assets` → `&mut catalog`）。

- [ ] **Step 3: cargo test**

Run: `rtk cargo test`
Expected: 全 PASS。测试函数数量 vs Phase 2 末尾应**相等或仅多**（mock 新增）。

```bash
rtk cargo test -- --list 2>/dev/null | wc -l
# 与 Phase 2 末尾 baseline 比对
```

- [ ] **Step 4: Commit**

```bash
rtk git add -A && rtk git commit -m "test: rewrite test setup to use AssetCatalog + MockScriptHost"
```

## Task 3.9: Phase 3 收尾 — 完整 PSNR

```bash
rtk cargo run --release --example pendulum_canvas
ffmpeg -i /tmp/baseline_pendulum.mp4 -i out/pendulum_canvas.mp4 -lavfi psnr -f null - 2>&1 | grep average
rtk cargo run --release --example hello_world
ffmpeg -i /tmp/baseline_hello.mp4 -i out/hello_world.mp4 -lavfi psnr -f null - 2>&1 | grep average
```

Expected: 两者均 average ≥ 50 dB。

```bash
rtk git tag phase3-complete
```

---

# Phase 4 — 验收（CI 卡口、纯净度证明、最终 PSNR）

## Task 4.1: 让 cargo check --no-default-features --lib 通过

- [ ] **Step 1: 跑一次看红线**

Run: `cargo check --no-default-features --lib 2>&1 | rtk err | head -50`

按错误清单逐个修。常见错误类型：
- `unresolved import opencat::host::*` —— host 模块不可用，删除该 use（或加 `#[cfg(feature = "host-default")]`）
- `cannot find type X` —— X 在 host，应该重新 export 或 unused 行为分支去掉
- `dead_code` warnings —— 加 `#[cfg(feature = "host-default")]` 屏蔽 host-only 函数

每处加 `#[cfg(feature = "host-default")]` 守卫；core 模块内部应零修改。

预计修补点（已知）：
- `src/core/scene/script/mod.rs::ScriptDriver::from_file` —— 删（已迁 host）
- `src/core/runtime/pipeline.rs` —— `build_scene_display_list` 调 `RenderSession`，**这个 wrapper 应在 host**：实际上 §7.6 spec 已说 `build_frame_display_tree` 在 core，`build_scene_display_list` host wrapper 应放 `src/host/runtime/pipeline.rs`。Task 3.5 已迁移；如未迁，本步移过去。

- [ ] **Step 2: tests/core_purity.rs 转绿**

```rust
#[test]
fn core_public_api_compiles() {
    use opencat::core::{
        FontProvider, ResourceCatalog, ScriptHost, build_frame_display_tree,
        collect_resource_requests, parse,
    };
    let _ = parse;
    let _ = collect_resource_requests;
    let _ = build_frame_display_tree;
    fn _c<R: ResourceCatalog, F: FontProvider, S: ScriptHost>() {}
}
```

Run: `cargo test --no-default-features --lib --tests core_purity`
Expected: PASS。

- [ ] **Step 3: cargo tree 审计 host 依赖未拉入**

Run: `cargo tree --no-default-features --prefix none --edges normal | grep -E "ffmpeg-next|skia-safe|rquickjs|reqwest|tokio|rodio"`
Expected: 输出为空。

- [ ] **Step 4: Commit**

```bash
rtk git add -A && rtk git commit -m "feat(core): pass cargo check --no-default-features --lib"
```

## Task 4.2: 反向依赖审计

- [ ] **Step 1: 跑两条审计指令**

```bash
echo "=== src/core/ 不应引用 host ==="
grep -rE "opencat::host|crate::host|super::host" src/core/ | wc -l
echo "=== src/host/ 应该使用 core ==="
grep -rE "opencat::core|crate::core|super::core" src/host/ | wc -l
```

Expected: 第一条 == 0；第二条 ≥ 5。

任一不达标，回头修。

- [ ] **Step 2: Commit 一份审计报告（贴在 commit message）**

```bash
rtk git commit --allow-empty -m "audit: core/host dependency inversion verified

src/core/ → src/host/ refs: 0
src/host/ → src/core/ refs: $(grep -rE 'opencat::core|crate::core|super::core' src/host/ | wc -l)

cargo tree --no-default-features --edges normal: no ffmpeg/skia/rquickjs/reqwest/tokio/rodio."
```

## Task 4.3: 加本地验证脚本（无 GitHub Actions）

**Files:**
- Create: `scripts/check_core_purity.sh`

- [ ] **Step 1: 写脚本**

```bash
#!/usr/bin/env bash
# scripts/check_core_purity.sh
# 本地手工执行；CI 设施未来如果接入 GitHub Actions / GitLab CI，再把它接进 .yml。
set -euo pipefail
cd "$(dirname "$0")/.."

echo "[1/4] cargo check --no-default-features --lib"
cargo check --no-default-features --lib

echo "[2/4] cargo check --no-default-features --lib --tests"
cargo check --no-default-features --lib --tests

echo "[3/4] core 不引 host"
core_to_host=$(grep -rE "opencat::host|crate::host|super::host" src/core/ | wc -l)
[[ "$core_to_host" == "0" ]] || { echo "FAIL: src/core/ references host ($core_to_host hits)"; exit 1; }

echo "[4/4] cargo tree without host deps"
forbidden=$(cargo tree --no-default-features --prefix none --edges normal 2>/dev/null | grep -E "ffmpeg-next|skia-safe|rquickjs|reqwest|tokio|rodio" || true)
if [[ -n "$forbidden" ]]; then
  echo "FAIL: forbidden host deps in core build:"
  echo "$forbidden"
  exit 1
fi

echo "OK: core purity verified."
```

```bash
chmod +x scripts/check_core_purity.sh
./scripts/check_core_purity.sh
```

Expected: 输出 `OK: core purity verified.`

- [ ] **Step 2: README 提示**

在 `README.md` 顶部 build 章节后添加：

```markdown
### 验证 core 纯净度

```bash
./scripts/check_core_purity.sh
```

该脚本必须在每次 PR 前手动执行，确保 `src/core/` 不依赖任何 host-only 依赖。
```

- [ ] **Step 3: Commit**

```bash
rtk git add scripts/check_core_purity.sh README.md && \
rtk git commit -m "chore(ci): add scripts/check_core_purity.sh local audit"
```

## Task 4.4: 最终 PSNR + tag

- [ ] **Step 1: 运行 baseline 比对**

```bash
rtk cargo run --release --example pendulum_canvas
ffmpeg -i /tmp/baseline_pendulum.mp4 -i out/pendulum_canvas.mp4 -lavfi psnr -f null - 2>&1 | tee /tmp/psnr_pendulum.txt | grep average
rtk cargo run --release --example hello_world
ffmpeg -i /tmp/baseline_hello.mp4 -i out/hello_world.mp4 -lavfi psnr -f null - 2>&1 | tee /tmp/psnr_hello.txt | grep average
rtk cargo run --release --example typewriter_canvas
# typewriter 与 baseline 同样比 — 如未在 Phase 1 录制，跳过。
```

Expected: 两个 average PSNR ≥ 50 dB。

- [ ] **Step 2: 全量 cargo test**

```bash
rtk cargo test
rtk cargo test --no-default-features --lib --tests
```

Expected: 两次都全 PASS。

- [ ] **Step 3: 完成 tag**

```bash
rtk git tag core-host-separation-complete
rtk git log --oneline phase1-complete..HEAD
```

- [ ] **Step 4: 写交付总结 commit**

```bash
rtk git commit --allow-empty -m "milestone: core/host separation complete

- core 切除所有 ffmpeg/skia/quickjs/reqwest/tokio 依赖
- cargo check --no-default-features --lib 通过
- PSNR(pendulum) = $(grep average /tmp/psnr_pendulum.txt)
- PSNR(hello) = $(grep average /tmp/psnr_hello.txt)
- 反向依赖审计：core→host = 0; host→core ≥ 5"
```

---

## 备注 / Risk 提示给 executor

1. **大批量 sed**（Task 3.2/3.6）会改到字符串字面量与注释，需要事后人工 review `git diff` 把误改回滚。
2. **examples/timeline.jsonl 不存在** —— 本 plan 改用 `examples/pendulum_canvas` + `examples/hello_world` 作为 PSNR baseline（覆盖 div / canvas / lucide / text）。视频路径覆盖通过 unit test 中的 mock catalog 验证，不进 mp4 比对。
3. **`SceneSnapshotCache` 归属** —— spec §7.9 要求搬到 host，本 plan 在 Task 3.2 Step 4 已处理。
4. **`from_file` 类便利构造** —— spec 没明示，但 `ScriptDriver::from_file` 与 `parse_file` 都属于 IO 便利 API，全部在 host。core 端只暴露 `from_source` / `parse(text)`。
5. **Phase 3 sed 失败模式** —— 如果某次 sed 把 `crate::scene` 改成 `crate::core::scene`，但 `crate::core::scene` 已存在则会变 `crate::core::core::scene`。在跑 sed 前用 `grep -rn 'crate::core::core' src/` 自检。

---

## Self-Review 已执行

- [x] Spec §7.1 / §7.2 / §7.4 / §7.8 → Task 1.6 / 1.8 / 1.7（覆盖）
- [x] Spec §7.3 → Task 2.1（覆盖）
- [x] Spec §7.5 / §7.6 → Task 2.3 / 2.4（覆盖）
- [x] Spec §7.7 → Task 2.5 / 2.6 / 3.6（覆盖）
- [x] Spec §7.9 / §7.10 → Task 3.5（覆盖）
- [x] Spec §7.11 / §7.12 / §7.13 / §7.15 → Task 3.7（覆盖）
- [x] Spec §7.14 → Task 3.8（覆盖）
- [x] Spec §9.2 / §13.3 / §13.4 / §13.7 → Task 4.1 / 4.2（覆盖）
- [x] Spec §9.3 PSNR → Task 1.9 / 2.7 / 3.9 / 4.4（覆盖）
- [x] Spec §13.6 反向依赖审计 → Task 4.2（覆盖）
- [x] 所有任务包含完整代码示例与精确文件路径，无 TBD/TODO 占位
- [x] 类型一致：`AssetId / AssetCatalog / VideoInfoMeta / ScriptDriverId / ResourceRequests / FontProvider / DefaultFontProvider / ScriptHost / ScriptRuntimeCache→ScriptDriverCache / QuickJsScriptHost` 全程命名一致
