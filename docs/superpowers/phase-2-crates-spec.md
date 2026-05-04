# Crate 拆分规格

## 目标

将当前单一 crate 拆分为 workspace 下的三个 crate，同时重命名 `host` → `engine`：

```
opencat/
├── Cargo.toml              # [workspace] members = ["crates/*"]
└── crates/
    ├── opencat-core/       # 纯数据 + trait + 算法，零 io/平台依赖
    ├── opencat-engine/     # 渲染引擎（原 host），实现所有 core trait
    ├── opencat-web/        # 未来 WASM/Web target（占位）
    └── opencat/            # CLI 二进制入口 + backend/codec + render.rs
```

---

## Phase 1 完成状态

```
cargo test --test core_purity --no-default-features  # ✅ 1 passed
cargo test --lib                                       # ✅ 284/285 passed（1 个 pre-existing chromedriver failure）
cargo check                                            # ✅ 0 errors
```

### 当前 core 中残留 feature gate（6 处）

| 位置 | 原因 | Phase |
|------|------|-------|
| `core/scene/script/mod.rs:144` | 测试依赖 `ScriptDriver.run()`（QuickJS impl） | 3 |
| `core/element/resolve.rs:646` | `compute_path_view_box` skia 版本 | 2 |
| `core/element/resolve.rs:674` | `compute_path_view_box` 无 skia fallback | 2 |
| `core/element/resolve.rs:1120` | 测试导入 `ScriptRuntimeCache` | 3 |
| `core/element/resolve.rs:1149-1544` | 5 个测试依赖 QuickJS 脚本执行 | 3 |

### 尚未从 host 提取到 core 的 trait

```rust
// ❌ 当前定义在 host/runtime/render_engine.rs，需提到 core
pub(crate) trait RenderEngine: Send + Sync {
    fn target_frame_view_kind(&self) -> RenderFrameViewKind;
    fn render_frame_to_target(&self, composition: &Composition, frame_index: u32,
        session: &mut RenderSession, target: &mut RenderTargetHandle) -> Result<()>;
    fn render_frame_rgba(&self, composition: &Composition,
        frame_index: u32, session: &mut RenderSession) -> Result<Vec<u8>>;
    fn draw_scene_snapshot(&self, snapshot: &SceneSnapshot,
        frame_view: RenderFrameView) -> Result<()>;
    fn record_display_tree_snapshot(&self, runtime: &mut SceneRenderContext<'_>,
        display_tree: &AnnotatedDisplayTree) -> Result<SceneSnapshot>;
    fn draw_ordered_scene(&self, runtime: &mut SceneRenderContext<'_>,
        display_tree: &AnnotatedDisplayTree, ordered_scene: &OrderedSceneProgram,
        frame_view: RenderFrameView) -> Result<()>;
}
```

---

## Phase 2: Workspace + opencat-core

### 2.1 创建目录结构

```bash
mkdir -p crates/opencat-core/src
mkdir -p crates/opencat-engine/src
mkdir -p crates/opencat-web/src
mkdir -p crates/opencat/src
```

### 2.2 顶层 Cargo.toml

```toml
[workspace]
resolver = "3"
members = ["crates/*"]
```

### 2.3 opencat-core Cargo.toml

```toml
[package]
name = "opencat-core"
version = "0.1.0"
edition = "2024"

[dependencies]
anyhow = "1.0"
taffy = "0.10.0"
image = "0.25.10"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
cosmic-text = "0.12"
fontdb = "0.16"
ahash = "0.8"
rustc-hash = "2.1"
unicode-segmentation = "1.11"
tracing = "0.1"
num_cpus = "1.17.0"
```

不声明 `[features]`，零 optional dep。

### 2.4 移动文件

将 `src/core/` 下所有内容移入 `crates/opencat-core/src/`：

```
src/core/display/      → crates/opencat-core/src/display/
src/core/element/      → crates/opencat-core/src/element/
src/core/frame_ctx.rs  → crates/opencat-core/src/frame_ctx.rs
src/core/jsonl/        → crates/opencat-core/src/jsonl/
src/core/layout/       → crates/opencat-core/src/layout/
src/core/lucide_icons/ → crates/opencat-core/src/lucide_icons/
src/core/resource/     → crates/opencat-core/src/resource/
src/core/runtime/      → crates/opencat-core/src/runtime/  # analysis, annotation, compositor, fingerprint, invalidation, preflight_collect
src/core/scene/        → crates/opencat-core/src/scene/
src/core/style.rs      → crates/opencat-core/src/style.rs
src/core/text/         → crates/opencat-core/src/text/
src/core/mod.rs        → crates/opencat-core/src/lib.rs
src/core/test_support.rs → crates/opencat-core/src/test_support.rs
```

### 2.5 路径修复

在 `opencat-core` 内部：

```
crate::core::display::  → crate::display::
crate::core::element::  → crate::element::
crate::core::frame_ctx  → crate::frame_ctx
crate::core::jsonl::    → crate::jsonl::
crate::core::layout::   → crate::layout::
crate::core::resource:: → crate::resource::
crate::core::runtime::  → crate::runtime::
crate::core::scene::    → crate::scene::
crate::core::style::    → crate::style::
crate::core::text::     → crate::text::
crate::core::test_support → crate::test_support
```

在 `opencat`（原 crate）中：

```
crate::core::  → opencat_core::
```

### 2.6 opencat-core 重导出

`crates/opencat-core/src/lib.rs`:

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

pub use self::frame_ctx::FrameCtx;
pub use self::jsonl::{ParsedComposition, parse};
pub use self::resource::catalog::{ResourceCatalog, VideoInfoMeta};
pub use self::resource::asset_catalog::{AssetCatalog, AssetId};
pub use self::runtime::preflight_collect::{ResourceRequests, collect_resource_requests};
pub use self::scene::script::{ScriptHost, ScriptDriverId};
pub use self::text::{FontProvider, DefaultFontProvider};

#[cfg(test)]
pub mod test_support;
```

### 2.7 提取 PathBoundsComputer trait

当前 `compute_path_view_box` 使用 `skia_safe::Path::from_svg`，依赖 `host-backend-skia` feature。
Core 中定义 trait，engine 提供 skia 实现：

```rust
// opencat-core/src/scene/path_bounds.rs
use anyhow::Result;

pub trait PathBoundsComputer {
    fn compute_view_box(&self, path_data: &[String]) -> Result<[f32; 4]>;
}

/// 无 skia 时的默认实现（总是返回默认 view_box）
pub struct DefaultPathBounds;
impl PathBoundsComputer for DefaultPathBounds {
    fn compute_view_box(&self, _path_data: &[String]) -> Result<[f32; 4]> {
        Ok([0.0, 0.0, 100.0, 100.0])
    }
}
```

resolve 函数签名更新：

```rust
// core/element/resolve.rs
use crate::scene::path_bounds::PathBoundsComputer;

struct ResolveContext<'a> {
    // ... existing fields ...
    path_bounds: &'a dyn PathBoundsComputer,
}

// compute_path_view_box 从 resolve.rs 删除，调用改为 cx.path_bounds.compute_view_box(...)
```

engine 中提供 skia 实现：

```rust
// opencat-engine/src/path_bounds.rs
use opencat_core::scene::path_bounds::PathBoundsComputer;
use anyhow::{Result, anyhow};

pub struct SkiaPathBounds;
impl PathBoundsComputer for SkiaPathBounds {
    fn compute_view_box(&self, path_data: &[String]) -> Result<[f32; 4]> {
        // skia_safe::Path::from_svg 逻辑
        // ...
    }
}
```

### 2.8 提取 RenderEngine trait

trait 定义从 `host/runtime/render_engine.rs` 移到 `opencat-core/src/runtime/render_engine.rs`。

类型需要同时提取或参数化：

| 原类型（host） | 处理方案 |
|---------------|---------|
| `Composition` | 已在 core |
| `RenderSession` | 在 engine，trait 方法接受它作为泛型或关联类型 |
| `RenderTargetHandle` | 提取到 core 作为 trait |
| `SceneSnapshot` | 定义为 `type SceneSnapshot = BackendObject`，提取 trait |
| `SceneRenderContext` | 提取到 core |
| `AnnotatedDisplayTree` | 已在 core |
| `OrderedSceneProgram` | 已在 core |
| `RenderFrameView` / `RenderFrameViewKind` | 提取到 core |

RenderEngine trait 简化版（core 中）：

```rust
// opencat-core/src/runtime/render_engine.rs
use anyhow::Result;
use crate::frame_ctx::FrameCtx;
use crate::runtime::annotation::AnnotatedDisplayTree;
use crate::runtime::compositor::OrderedSceneProgram;

#[derive(Clone, Copy, Debug)]
pub enum RenderFrameViewKind { Software, Accelerated }

#[derive(Clone, Copy, Debug)]
pub struct RenderFrameView { /* ... */ }

/// 平台渲染后端的不透明句柄
pub trait RenderSnapshot: Send + Sync {
    fn as_any(&self) -> &dyn std::any::Any;
}

/// 渲染目标 trait
pub trait RenderTarget: Send {
    fn begin_frame(&mut self, width: i32, height: i32) -> Result<Box<dyn RenderTarget>>;
    fn resolve_frame_view(&self, surface: &dyn RenderTarget) -> Result<RenderFrameView>;
}

pub trait RenderEngine: Send + Sync {
    fn target_frame_view_kind(&self) -> RenderFrameViewKind;
    fn draw_scene_snapshot(&self, snapshot: &dyn RenderSnapshot,
        frame_view: RenderFrameView) -> Result<()>;
    fn record_display_tree_snapshot(&self,
        assets: &crate::resource::asset_catalog::AssetCatalog,
        frame_ctx: &FrameCtx,
        display_tree: &AnnotatedDisplayTree,
    ) -> Result<Box<dyn RenderSnapshot>>;
    fn draw_ordered_scene(&self,
        snapshot_runtime: &mut dyn RenderSceneRuntime,
        display_tree: &AnnotatedDisplayTree,
        ordered_scene: &OrderedSceneProgram,
        frame_view: RenderFrameView,
    ) -> Result<()>;
}
```

### 2.9 验证

```bash
cargo test -p opencat-core                      # core 测试
cargo test -p opencat-core --no-default-features # 纯化验证
cargo check -p opencat-core                      # 编译验证
```

---

## Phase 3: 创建 opencat-engine + host → engine 重命名

### 3.1 opencat-engine Cargo.toml

```toml
[package]
name = "opencat-engine"
version = "0.1.0"
edition = "2024"

[features]
default = []
full = ["codec-ffmpeg", "script-quickjs", "resource-net", "backend-skia", "audio"]
codec-ffmpeg = ["dep:ffmpeg-next"]
script-quickjs = ["dep:rquickjs"]
resource-net = ["dep:reqwest", "dep:tokio"]
backend-skia = ["dep:skia-safe"]
audio = []

[dependencies]
opencat-core = { path = "../opencat-core" }
anyhow = "1.0"
tracing = "0.1"
# ... 其他始终需要的公共依赖

ffmpeg-next = { version = "8.1.0", optional = true }
rquickjs = { version = "0.11.0", optional = true }
reqwest = { version = "0.13.2", optional = true }
tokio = { version = "1.51.0", optional = true }
skia-safe = { version = "0.93.1", optional = true }
```

### 3.2 旧 host 目录 → 新 opencat-engine/src

| 原路径 | 新路径 |
|--------|--------|
| `src/host/mod.rs` | `crates/opencat-engine/src/lib.rs` |
| `src/host/script/` | `crates/opencat-engine/src/script/` |
| `src/host/runtime/` | `crates/opencat-engine/src/runtime/` |
| `src/host/resource/` | `crates/opencat-engine/src/resource/` |
| `src/host/inspect/` | `crates/opencat-engine/src/inspect/` |
| `src/host/fonts.rs` | `crates/opencat-engine/src/fonts.rs` |
| `src/host/jsonl_io.rs` | `crates/opencat-engine/src/jsonl_io.rs` |

### 3.3 全局替换

需要在 **整个 workspace** 中进行以下替换：

| 原文本 | 新文本 |
|--------|--------|
| `crate::host::` | `opencat_engine::` |
| `use crate::host` | `use opencat_engine` |
| `pub mod host;` | (删除，改为 Cargo.toml 依赖) |
| `host::runtime::` | `opencat_engine::runtime::` |
| `host::script::` | `opencat_engine::script::` |
| `host::resource::` | `opencat_engine::resource::` |
| `host::inspect::` | `opencat_engine::inspect::` |

Feature flag 名称保持不变（`host-default`, `host-codec` 等），避免破坏现有 CI 配置。

### 3.4 opencat-engine/src/lib.rs

```rust
#![cfg(feature = "host-default")]
pub mod fonts;
pub mod inspect;
pub mod jsonl_io;
pub mod resource;
pub mod runtime;
pub mod script;

// 重导出 core 模块（保持兼容）
pub use opencat_core::runtime::analysis;
pub use opencat_core::runtime::annotation;
pub use opencat_core::runtime::fingerprint;
pub use opencat_core::runtime::invalidation;
pub use opencat_core::runtime::preflight_collect;

// engine 自身的 pipeline
pub use self::runtime::pipeline::{
    build_frame_display_tree,
    build_scene_display_list,
    render_frame_on_surface,
};

// 重命名别名（兼容过渡期）
pub mod host {
    pub use crate::*;
}
```

### 3.5 移动 host-gated 测试

从 `core/element/resolve.rs` 移出 5 个 `#[cfg(feature = "host-default")]` 测试：

| 测试名 | 新位置 |
|--------|--------|
| `node_script_only_affects_its_own_subtree` | `opencat-engine/tests/resolve_integration.rs` |
| `transition_scenes_keep_node_scripts_isolated` | 同上 |
| `timeline_scripts_receive_scene_local_frames` | 同上 |
| `parent_script_can_split_descendant_text_before_child_resolution` | 同上 |
| `resolve_caption_uses_scene_local_time_inside_timeline` | 同上 |

从 `core/scene/script/mod.rs` 移出测试模块：

| 测试 | 新位置 |
|------|--------|
| `script_driver_records_text_alignment_and_line_height` | `opencat-engine/tests/script_driver.rs` |
| `script_driver_exposes_global_and_scene_frame_fields` | 同上 |
| `script_driver_preserves_transform_call_order` | 同上 |
| `script_driver_records_lucide_fill_and_stroke` | 同上 |
| `script_driver_records_standard_canvaskit_rect_and_image_commands` | 同上 |

### 3.6 验证

```bash
cargo test -p opencat-engine                # engine 测试
cargo test --workspace                       # 全部测试
cargo check --workspace --no-default-features # 无 host 编译
```

---

## Phase 4: opencat-web 占位

### 4.1 Cargo.toml

```toml
# crates/opencat-web/Cargo.toml
[package]
name = "opencat-web"
version = "0.1.0"
edition = "2024"

[dependencies]
opencat-core = { path = "../opencat-core" }

[target.'cfg(target_arch = "wasm32")'.dependencies]
wasm-bindgen = "0.2"
web-sys = { version = "0.3", features = ["CanvasRenderingContext2d", "HtmlCanvasElement"] }
```

### 4.2 src/lib.rs

```rust
//! opencat-web — WASM/Web rendering target for opencat-core.
//!
//! This crate provides a web-based render engine that implements
//! `opencat_core::runtime::render_engine::RenderEngine` using
//! HTML Canvas and Web APIs.

pub struct WebRenderEngine {
    // TODO: canvas, context, etc.
}

// TODO: impl opencat_core::runtime::render_engine::RenderEngine for WebRenderEngine
```

### 4.3 验证

```bash
cargo check -p opencat-web
```

---

## 最终完整目录

```
opencat/
├── Cargo.toml                          # [workspace] members = ["crates/*"]
├── crates/
│   ├── opencat-core/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── display/
│   │       ├── element/
│   │       ├── frame_ctx.rs
│   │       ├── jsonl/
│   │       ├── layout/
│   │       ├── lucide_icons/
│   │       ├── resource/
│   │       ├── runtime/
│   │       │   ├── analysis/
│   │       │   ├── annotation/
│   │       │   ├── compositor/
│   │       │   ├── fingerprint/
│   │       │   ├── invalidation/
│   │       │   ├── preflight_collect/
│   │       │   └── render_engine.rs    # 🆕 RenderEngine trait
│   │       ├── scene/
│   │       │   ├── path_bounds.rs      # 🆕 PathBoundsComputer trait
│   │       │   └── script/
│   │       ├── style.rs
│   │       ├── text/
│   │       └── test_support.rs
│   ├── opencat-engine/
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── fonts.rs
│   │       ├── jsonl_io.rs
│   │       ├── path_bounds.rs          # 🆕 SkiaPathBounds impl
│   │       ├── script/
│   │       ├── runtime/
│   │       │   ├── pipeline.rs         # moved from core
│   │       │   ├── session.rs
│   │       │   ├── render_engine.rs    # SkiaRenderEngine impl
│   │       │   ├── compositor/
│   │       │   ├── profile/
│   │       │   ├── cache/
│   │       │   ├── frame_view.rs
│   │       │   ├── preflight.rs
│   │       │   ├── surface.rs
│   │       │   ├── target.rs
│   │       │   └── ...
│   │       ├── resource/
│   │       └── inspect/
│   ├── opencat-web/
│   │   ├── Cargo.toml
│   │   └── src/lib.rs
│   └── opencat/
│       ├── Cargo.toml                  # depends on opencat-core, opencat-engine
│       └── src/
│           ├── lib.rs                  # re-exports
│           ├── render.rs
│           ├── backend/
│           │   └── skia/
│           ├── codec/
│           └── bin/
│               ├── opencat.rs
│               └── opencat-see.rs
├── assets/                             # 字体等
├── tests/
│   └── core_purity.rs                  # 可删除（workspace 天然保证隔离）
├── examples/
└── scripts/
```

---

## 执行顺序总结

| # | 步骤 | 影响面 | 验证 |
|---|------|--------|------|
| 2.1-2.3 | 创建 workspace + Cargo.toml | 顶层 + opencat-core | `cargo metadata` |
| 2.4 | 移动文件到 opencat-core | 整个 core 目录 | 文件树 |
| 2.5 | 路径修复 `crate::core::` → `crate::` | opencat-core 内全部文件 | `cargo check -p opencat-core` |
| 2.6 | opencat-core 重导出 `lib.rs` | 1 个文件 | 编译 |
| 2.7 | 提取 `PathBoundsComputer` trait | resolve.rs + 新建 path_bounds.rs | 编译 + 测试 |
| 2.8 | 提取 `RenderEngine` trait 到 core | render_engine.rs（host → core） | 编译 |
| 2.9 | opencat 依赖 opencat-core，修复引用 | opencat 中所有 `crate::core::` | `cargo check --workspace` |
| 3.1-3.2 | 创建 opencat-engine，移动文件 | host/ → opencat-engine/ | 文件树 |
| 3.3 | 全局替换 `crate::host::` → `opencat_engine::` | 全部文件 | `cargo check --workspace` |
| 3.4 | opencat-engine lib.rs | 1 个文件 | 编译 |
| 3.5 | 移动 host-gated 测试 | resolve.rs + script/mod.rs | `cargo test --workspace` |
| 4.1-4.3 | 创建 opencat-web 占位 | 2 个文件 | `cargo check -p opencat-web` |
| 清理 | 删除 `tests/core_purity.rs`（不再需要） | 1 个文件 | `cargo test --workspace` |

---

## 完成状态

**Phase 4 已完成** — `phase-4-complete` tag。

```
cargo test --workspace         # 285 passed, 1 expected failure (chromedriver)
cargo check --workspace        # 0 errors
cargo tree -p opencat-web      # 无 host 依赖
check_core_purity.sh           # OK
```

Workspace 成员：`opencat-core`, `opencat-engine`, `opencat-web`, `opencat`。
