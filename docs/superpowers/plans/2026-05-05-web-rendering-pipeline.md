# Web 渲染管线 Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 跑通 web 渲染管线：JSONL 输入 → Rust WASM 构建 DisplayTree → CanvasKit 渲染 → ffmpeg.wasm 导出。

**Architecture:** Rust 侧在 core crate 新增两种纯数据结构（HashMapResourceCatalog、PrecomputedScriptHost），在 opencat-web 新增 `build_frame` WASM 函数串起 core 管线，JS 侧新增 script-runtime.ts 在浏览器原生执行动画脚本并生成 StyleMutations，main.ts 串联完整流程。

**Tech Stack:** Rust/wasm-bindgen, serde, taffy, fontdb, CanvasKit, @ffmpeg/ffmpeg, TypeScript/Vite

---

## Chunk 1: Display Tree 序列化 + 核心基础设施

### Task 1.1: 给 ColorToken 生成代码加 Serialize/Deserialize

**Files:**
- Modify: `crates/opencat-core/build.rs:346-347`

ColorToken 由 build.rs 生成，当前只 derive `Debug, Clone, Copy, PartialEq, Eq, Hash`，需要加 `Serialize, Deserialize`。

- [ ] **Step 1: 修改 build.rs 的 derive 宏**

```rust
// build.rs:346-347 改为:
output.push_str("#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]\n");
output.push_str("#[serde(rename_all = \"camelCase\")]\n");
output.push_str("pub enum ColorToken {\n");
```

- [ ] **Step 2: 验证构建通过**

```bash
rtk cargo build -p opencat-core
```

- [ ] **Step 3: Commit**

```bash
rtk git add crates/opencat-core/build.rs && rtk git commit -m "feat(core): add Serialize/Deserialize to generated ColorToken"
```

### Task 1.2: 给 Style 辅助类型加 Serialize

**Files:**
- Modify: `crates/opencat-core/src/style.rs`

需要给以下类型加 `#[derive(Serialize, Deserialize)]`（serde 已是 core 的依赖）:
- `BorderStyle` (line 194)
- `BackgroundFill` + `GradientDirection` (line 430)
- `Transform` (line 441)
- `BorderRadius` (line 166)
- `BoxShadow` (line 237)
- `InsetShadow` (line 246)
- `DropShadow` (line 255)
- `ComputedTextStyle` (line 630)
- `ObjectFit`
- `FontWeight`

注意：`Transform` 已有手动 `Hash` impl，加 serde derive 不冲突。`TextAlign` 等需要 camelCase rename。

- [ ] **Step 1: 逐一加 derive 宏**

为每个类型加 `#[derive(serde::Serialize, serde::Deserialize)]` 和必要的 `#[serde(rename_all = "camelCase")]`。

`ComputedTextStyle` 需要特殊处理——它是 `Clone, Debug`，需要改为 derive Serialize。serde 已经在 style.rs 通过 build.rs include 可用（因为 build.rs 生成的代码引用了 serde）。

关键改动示例：

```rust
// BorderStyle
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum BorderStyle { ... }

// BackgroundFill
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BackgroundFill { ... }

// Transform - serde enum tag
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Transform { ... }

// ComputedTextStyle
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputedTextStyle { ... }
```

- [ ] **Step 2: 构建验证**

```bash
rtk cargo build -p opencat-core
```

- [ ] **Step 3: Commit**

```bash
rtk git add crates/opencat-core/src/style.rs && rtk git commit -m "feat(core): add Serialize/Deserialize to style types"
```

### Task 1.3: 给 Display 类型加 Serialize

**Files:**
- Modify: `crates/opencat-core/src/display/list.rs`
- Modify: `crates/opencat-core/src/display/tree.rs`
- Modify: `crates/opencat-core/src/scene/script/mutations.rs` (CanvasCommand, ScriptColor, etc.)
- Modify: `crates/opencat-core/src/scene/transition.rs` (TransitionKind)

这是最大的序列化工作。需要添加 `Serialize, Deserialize` 到所有 display 类型。注意与 web/src/types.ts 的接口匹配（camelCase 命名）。

- [ ] **Step 1: display/list.rs 所有类型加 Serialize**

```rust
// DisplayRect
#[derive(Clone, Copy, Debug, serde::Serialize, serde::Deserialize)]
pub struct DisplayRect { ... }

// DisplayClip
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayClip { ... }

// DisplayTransform
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayTransform { ... }

// DisplayItem - tagged enum
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DisplayItem {
    Rect(RectDisplayItem),
    Timeline(TimelineDisplayItem),
    Text(TextDisplayItem),
    Bitmap(BitmapDisplayItem),
    DrawScript(DrawScriptDisplayItem),
    SvgPath(SvgPathDisplayItem),
}
// serde tag variants: "rect", "timeline", "text", "bitmap", "drawScript", "svgPath"

// 各子类型同样加 Serialize + camelCase
```

- [ ] **Step 2: display/tree.rs 加 Serialize**

```rust
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayTree { pub root: DisplayNode }

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DisplayNode { ... }
```

- [ ] **Step 3: mutations.rs 的 CanvasCommand 加 Serialize**

```rust
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CanvasCommand { ... }

// ScriptColor, ScriptLineCap, ScriptLineJoin 等辅助类型也要加
```

CanvasCommand 已有手动 `Hash` impl，加 serde derive 不冲突。

- [ ] **Step 4: transition.rs 的 TransitionKind**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum TransitionKind { ... }
```

- [ ] **Step 5: TextUnitOverrideBatch 等也要加**

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextUnitOverrideBatch { ... }
```

- [ ] **Step 6: 构建验证**

```bash
rtk cargo build -p opencat-core
```

需要确保 `opencat-engine` 和 `opencat-web` 也能编译（因为它们依赖 core）。

```bash
rtk cargo build -p opencat-engine
rtk cargo build -p opencat-web --target wasm32-unknown-unknown
```

- [ ] **Step 7: Commit**

```bash
rtk git add crates/opencat-core/src/display/ crates/opencat-core/src/scene/ && rtk git commit -m "feat(core): add Serialize/Deserialize to display and script types"
```

---

## Chunk 2: HashMapResourceCatalog + PrecomputedScriptHost

### Task 2.1: HashMapResourceCatalog

**Files:**
- Create: `crates/opencat-core/src/resource/hash_map_catalog.rs`
- Modify: `crates/opencat-core/src/resource/mod.rs`
- Modify: `crates/opencat-core/src/lib.rs`

- [ ] **Step 1: 编写 HashMapResourceCatalog**

```rust
// crates/opencat-core/src/resource/hash_map_catalog.rs
use std::collections::HashMap;
use anyhow::Result;
use crate::resource::asset_id::AssetId;
use crate::resource::catalog::{ResourceCatalog, VideoInfoMeta};
use crate::scene::primitives::{AudioSource, ImageSource};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResourceMeta {
    pub width: u32,
    pub height: u32,
    pub kind: ResourceKind,
    pub duration_secs: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ResourceKind {
    Image,
    Video,
    Audio,
}

/// Catalog built from JS-preloaded resource metadata.
/// AssetId is derived from the resource locator string hash.
pub struct HashMapResourceCatalog {
    entries: HashMap<AssetId, ResourceMeta>,
    asset_cache: HashMap<String, AssetId>,
    next_id: u64,
}

impl HashMapResourceCatalog {
    /// Build catalog from JSON string:
    /// `{ "path/to/image.png": { "width": 800, "height": 600, "kind": "image" }, ... }`
    pub fn from_json(json: &str) -> Result<Self> {
        let map: HashMap<String, ResourceMeta> = serde_json::from_str(json)?;
        let mut catalog = Self {
            entries: HashMap::new(),
            asset_cache: HashMap::new(),
            next_id: 0,
        };
        for (locator, meta) in &map {
            catalog.register_dimensions(locator, meta.width, meta.height);
        }
        // Second pass: set metadata for all registered assets
        for (locator, meta) in map {
            if let Some(id) = catalog.asset_cache.get(&locator) {
                catalog.entries.insert(*id, meta);
            }
        }
        Ok(catalog)
    }

    fn resolve_key(&mut self, key: &str) -> AssetId {
        if let Some(id) = self.asset_cache.get(key) {
            return *id;
        }
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        key.hash(&mut h);
        let id = AssetId(h.finish());
        self.asset_cache.insert(key.to_string(), id);
        id
    }
}

impl ResourceCatalog for HashMapResourceCatalog {
    fn resolve_image(&mut self, src: &ImageSource) -> Result<AssetId> {
        let key = match src {
            ImageSource::Path(p) => p.to_string_lossy().to_string(),
            ImageSource::Url(u) => u.clone(),
            ImageSource::DataUri(_) => "data:".to_string(),
        };
        Ok(self.resolve_key(&key))
    }

    fn resolve_audio(&mut self, src: &AudioSource) -> Result<AssetId> {
        let key = match src {
            AudioSource::Path(p) => p.to_string_lossy().to_string(),
            AudioSource::Url(u) => u.clone(),
        };
        Ok(self.resolve_key(&key))
    }

    fn register_dimensions(&mut self, locator: &str, width: u32, height: u32) -> AssetId {
        let id = self.resolve_key(locator);
        self.entries.entry(id).or_insert(ResourceMeta {
            width,
            height,
            kind: ResourceKind::Image,
            duration_secs: None,
        });
        id
    }

    fn alias(&mut self, alias: AssetId, target: &AssetId) -> Result<()> {
        let meta = self.entries.get(target).cloned();
        if let Some(m) = meta {
            self.entries.insert(alias, m);
        }
        Ok(())
    }

    fn dimensions(&self, id: &AssetId) -> (u32, u32) {
        self.entries
            .get(id)
            .map(|m| (m.width, m.height))
            .unwrap_or((0, 0))
    }

    fn video_info(&self, id: &AssetId) -> Option<VideoInfoMeta> {
        self.entries.get(id).and_then(|m| {
            if m.kind == ResourceKind::Video {
                Some(VideoInfoMeta {
                    width: m.width,
                    height: m.height,
                    duration_secs: m.duration_secs,
                })
            } else {
                None
            }
        })
    }
}
```

- [ ] **Step 2: 注册模块**

```rust
// resource/mod.rs 加一行:
pub mod hash_map_catalog;

// lib.rs 加 re-export:
pub use self::resource::hash_map_catalog::{HashMapResourceCatalog, ResourceMeta, ResourceKind};
```

- [ ] **Step 3: 写测试**

在 `hash_map_catalog.rs` 末尾加 `#[cfg(test)] mod tests`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::resource::catalog::ResourceCatalog;
    use crate::scene::primitives::ImageSource;
    use std::path::PathBuf;

    #[test]
    fn from_json_parses_and_resolves() {
        let json = r#"{"/img/a.png":{"width":100,"height":200,"kind":"image"}}"#;
        let catalog = HashMapResourceCatalog::from_json(json).unwrap();
        let src = ImageSource::Path(PathBuf::from("/img/a.png"));
        let id = catalog.resolve_image(&src).unwrap();
        assert_eq!(catalog.dimensions(&id), (100, 200));
    }

    #[test]
    fn video_info_returns_duration() {
        let json = r#"{"/v/b.mp4":{"width":1920,"height":1080,"kind":"video","durationSecs":5.5}}"#;
        let catalog = HashMapResourceCatalog::from_json(json).unwrap();
        let src = ImageSource::Path(PathBuf::from("/v/b.mp4"));
        let id = catalog.resolve_image(&src).unwrap();
        let info = catalog.video_info(&id).unwrap();
        assert_eq!(info.width, 1920);
        assert_eq!(info.duration_secs, Some(5.5));
    }

    #[test]
    fn unknown_resource_returns_zero_dimensions() {
        let catalog = HashMapResourceCatalog::from_json("{}").unwrap();
        assert_eq!(catalog.dimensions(&AssetId(999)), (0, 0));
    }
}
```

- [ ] **Step 4: 运行测试**

```bash
rtk cargo test -p opencat-core -- hash_map_catalog
```

- [ ] **Step 5: 构建验证**

```bash
rtk cargo build -p opencat-core
rtk cargo build -p opencat-engine
```

- [ ] **Step 6: Commit**

```bash
rtk git add crates/opencat-core/src/resource/ crates/opencat-core/src/lib.rs && rtk git commit -m "feat(core): add HashMapResourceCatalog for web resource injection"
```

### Task 2.2: PrecomputedScriptHost

**Files:**
- Create: `crates/opencat-core/src/scene/script/precomputed_host.rs`
- Modify: `crates/opencat-core/src/scene/script/mod.rs`
- Modify: `crates/opencat-core/src/lib.rs`

- [ ] **Step 1: 编写 PrecomputedScriptHost**

```rust
// crates/opencat-core/src/scene/script/precomputed_host.rs
use std::collections::HashMap;
use anyhow::{Result, anyhow};
use crate::frame_ctx::ScriptFrameCtx;
use crate::scene::script::{ScriptDriverId, ScriptHost, ScriptTextSource, StyleMutations};

/// ScriptHost that reads from precomputed mutations.
/// Web side runs scripts natively in JS and passes mutations as JSON.
pub struct PrecomputedScriptHost {
    mutations: HashMap<ScriptDriverId, StyleMutations>,
}

impl PrecomputedScriptHost {
    /// Build host from JSON string. Format matches StyleMutations serialization.
    /// `{ "mutations": { "node-id": { "opacity": 0.5, ... } }, "canvas_mutations": {} }`
    pub fn from_json(json: &str) -> Result<Self> {
        let mutations: StyleMutations = serde_json::from_str(json)?;
        Ok(Self::from_single(mutations))
    }

    /// Build with pre-constructed StyleMutations.
    /// driver_id hint is inserted — by convention uses hash 0 for the single script case.
    pub fn from_single(mutations: StyleMutations) -> Self {
        let mut map = HashMap::new();
        map.insert(ScriptDriverId(0), mutations);
        Self { mutations: map }
    }

    /// Insert mutations for a specific script driver.
    pub fn insert(&mut self, id: ScriptDriverId, mutations: StyleMutations) {
        self.mutations.insert(id, mutations);
    }
}

impl ScriptHost for PrecomputedScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        Ok(ScriptDriverId(h.finish()))
    }

    fn register_text_source(&mut self, _node_id: &str, _source: ScriptTextSource) {
        // no-op: text content handled via mutations text_content field
    }

    fn clear_text_sources(&mut self) {}

    fn run_frame(
        &mut self,
        driver: ScriptDriverId,
        _frame_ctx: &ScriptFrameCtx,
        _current_node_id: Option<&str>,
    ) -> Result<StyleMutations> {
        self.mutations
            .remove(&driver)
            .ok_or_else(|| anyhow!("no precomputed mutations for script driver {:?}", driver))
    }
}
```

- [ ] **Step 2: 注册模块**

```rust
// scene/script/mod.rs 加:
pub mod precomputed_host;
pub use precomputed_host::PrecomputedScriptHost;

// lib.rs 加:
pub use self::scene::script::PrecomputedScriptHost;
```

- [ ] **Step 3: 构建验证**

```bash
rtk cargo build -p opencat-core
```

- [ ] **Step 4: 写单元测试**

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::script::{NodeStyleMutations, StyleMutations};
    use std::collections::HashMap;

    #[test]
    fn from_json_parses_mutations() {
        let json = r#"{"mutations":{"node1":{"opacity":0.5}},"canvas_mutations":{}}"#;
        let host = PrecomputedScriptHost::from_json(json).unwrap();
        let id = ScriptDriverId(0);
        // Note: run_frame consumes with remove(), so test insertion separately
    }

    #[test]
    fn install_returns_stable_hash() {
        let mut host = PrecomputedScriptHost::from_single(StyleMutations::default());
        let id1 = host.install("var x = 1;").unwrap();
        let id2 = host.install("var x = 1;").unwrap();
        assert_eq!(id1, id2);
        let id3 = host.install("var y = 2;").unwrap();
        assert_ne!(id1, id3);
    }
}
```

- [ ] **Step 5: 运行测试**

```bash
rtk cargo test -p opencat-core -- precomputed
```

- [ ] **Step 6: Commit**

```bash
rtk git add crates/opencat-core/src/scene/script/ crates/opencat-core/src/lib.rs && rtk git commit -m "feat(core): add PrecomputedScriptHost for web mutation injection"
```

---

## Chunk 3: build_frame WASM 导出

### Task 3.1: 实现 build_frame

**Files:**
- Modify: `crates/opencat-web/src/wasm_entry.rs`

- [ ] **Step 1: 添加 build_frame 函数**

在 `wasm_entry.rs` 末尾添加：

```rust
use opencat_core::display::build::build_display_tree;
use opencat_core::display::tree::DisplayTree;
use opencat_core::element::resolve::resolve_ui_tree;
use opencat_core::frame_ctx::FrameCtx;
use opencat_core::layout::LayoutSession;
use opencat_core::resource::hash_map_catalog::HashMapResourceCatalog;
use opencat_core::scene::script::PrecomputedScriptHost;
use opencat_core::text;
use std::sync::Arc;

/// Build display tree for a single frame.
///
/// Parameters:
/// - `jsonl_input`: raw JSONL content
/// - `frame`: frame number (0-indexed)
/// - `resource_meta`: JSON `{ "path": { "width", "height", "kind", "durationSecs" } }`
/// - `mutations_json`: StyleMutations JSON from JS script execution
///
/// Returns: DisplayTree JSON or `{"error": "message"}` on failure
#[wasm_bindgen]
pub fn build_frame(
    jsonl_input: &str,
    frame: u32,
    resource_meta: &str,
    mutations_json: &str,
) -> String {
    match build_frame_impl(jsonl_input, frame, resource_meta, mutations_json) {
        Ok(tree) => serde_json::to_string(&tree).unwrap_or_else(|e| {
            format!(r#"{{"error":"serialization failed: {}"}}"#, e)
        }),
        Err(e) => format!(r#"{{"error":"{}"}}"#, e),
    }
}

fn build_frame_impl(
    jsonl_input: &str,
    frame: u32,
    resource_meta: &str,
    mutations_json: &str,
) -> anyhow::Result<DisplayTree> {
    // 1. Parse JSONL
    let parsed = opencat_core::jsonl::parse(jsonl_input)?;

    let frame_ctx = FrameCtx {
        frame,
        total_frames: parsed.frames,
        fps: parsed.fps as u32,
        width: parsed.width as u32,
        height: parsed.height as u32,
    };

    // 2. Build resource catalog from JS-provided metadata
    let mut catalog = HashMapResourceCatalog::from_json(resource_meta)?;

    // 3. Build script host from JS-provided mutations
    let mut script_host = PrecomputedScriptHost::from_json(mutations_json)?;

    // 4. Get the scene node for this frame
    let scene_node = parsed.composition.root_node(&frame_ctx);

    // 5. Resolve UI tree (applies mutations, styles, etc.)
    let element_root = resolve_ui_tree(
        &scene_node,
        &frame_ctx,
        &mut catalog,
        None, // parent_composition
        &mut script_host,
    )?;

    // 6. Compute layout
    let font_db = text::default_font_db_with_embedded_only();
    let mut layout_session = LayoutSession::default();
    layout_session.compute_layout_with_font_db(&element_root, &frame_ctx, &font_db)?;

    // 7. Build display tree
    let display_tree = build_display_tree(&element_root, &layout_session.tree())?;

    Ok(display_tree)
}
```

- [ ] **Step 2: 检查 LayoutSession API**

`LayoutSession::tree()` 是否暴露？需要确认。如果 `tree` 是 pub 的，直接用。如果 layout_tree 需要通过其他方式获取，需要检查。

运行 `cargo doc -p opencat-core --no-deps` 或直接阅读 layout/mod.rs 来确认。

有一种可能是 `compute_layout_with_font_db` 内部构建了 tree 并通过另一个方法返回。如果不存在 tree() 方法，可能需要用 `compute_layout_with_font_db_fn` 函数版。

```bash
rtk grep "pub fn tree\|pub.*layout_tree\|pub.*fn layout" crates/opencat-core/src/layout/mod.rs
```

- [ ] **Step 3: 添加 `compute_layout_with_font_db_fn` 备选方案**

如果 `LayoutSession.tree()` 不是 pub，则在 core 添加一个简单的自由函数来获取 layout result：

```rust
// 临时在 wasm_entry.rs 中用 compute_layout_with_font_db_fn 替代
let layout_tree = opencat_core::layout::compute_layout_with_font_db_fn(
    &element_root, &frame_ctx, &font_db
)?;
let display_tree = build_display_tree(&element_root, &layout_tree)?;
```

- [ ] **Step 4: 处理 ScriptHost install 调用**

JSONL 解析后可能有 scripts，resolve_ui_tree 内部会调用 `script_host.install(source)`。如果脚本源码对应的 hash 不在 PrecomputedScriptHost 的 mutations map 中，`from_json` 只创建一个 ScriptDriverId(0) entry。需要让 JS 侧负责将所有脚本源码收集后，为每个脚本计算 hash 并批量注入 mutations。

**简化方案**: 初始版本假设每个 frame JSONL 只有一个 script（最常见场景），`from_json` 默认 key 为 ScriptDriverId(0)。后续多脚本场景再扩展。

- [ ] **Step 5: 构建 WASM**

```bash
rtk cargo build -p opencat-web --target wasm32-unknown-unknown
```

- [ ] **Step 6: 更新 JS wasm 类型定义**

```typescript
// web/src/wasm.ts 的 WasmModule 接口加:
build_frame(jsonl_input: string, frame: number, resource_meta: string, mutations_json: string): string;
```

同时导出 TypeScript 包装函数:

```typescript
export function buildFrame(
  jsonlInput: string,
  frame: number,
  resourceMeta: string,
  mutationsJson: string,
): any {
  if (!wasmModule) throw new Error('WASM not initialized');
  const json = wasmModule.build_frame(jsonlInput, frame, resourceMeta, mutationsJson);
  const result = JSON.parse(json);
  if (result.error) throw new Error(result.error);
  return result;
}
```

- [ ] **Step 7: Commit**

```bash
rtk git add crates/opencat-web/src/wasm_entry.rs web/src/wasm.ts && rtk git commit -m "feat(wasm): add build_frame WASM export for web rendering pipeline"
```

---

## Chunk 4: script-runtime.ts

### Task 4.1: TypeScript 脚本运行时

**Files:**
- Create: `web/src/script-runtime.ts`

这个文件实现在浏览器原生 JS 中执行动画脚本的 `ctx` API，镜像 Rust 侧 QuickJS 暴露给脚本的接口。

- [ ] **Step 1: 创建 ctx 工厂函数和 NodeStyler**

```typescript
// web/src/script-runtime.ts
import {
  computeProgress,
  animateValue,
  animateColor,
  parseEasing,
  getEasing,
  type SpringConfig,
} from './animator';

// ── Mutation types (match Rust StyleMutations) ──

interface NodeMutations {
  opacity?: number;
  translateX?: number;
  translateY?: number;
  scale?: number;
  rotate?: number;
  width?: number;
  height?: number;
  bgColor?: string;
  textColor?: string;
  fontSize?: number;
  // ... more fields as needed
}

interface CollectedMutations {
  mutations: Record<string, NodeMutations>;
}

// ── AnimateResult (like Rust's AnimateEntry) ──

interface AnimInput {
  duration?: number;
  delay?: number;
  clamp?: boolean;
  ease?: string;
  stagger?: number;
  repeat?: number;
  yoyo?: boolean;
  repeatDelay?: number;
}

interface AnimOutput {
  opacity: number;
  y: number;
  scale: number;
  rotate: number;
  x: number;
}

// ── Ctx API ──

export function createContext(
  frame: number,
  totalFrames: number,
  sceneFrames: number,
): ScriptCtx {
  return new ScriptCtx(frame, totalFrames, sceneFrames);
}
```

- [ ] **Step 2: 实现 ScriptCtx 类**

```typescript
export class ScriptCtx {
  frame: number;
  totalFrames: number;
  currentFrame: number;
  sceneFrames: number;
  private mutations: Record<string, NodeMutations> = {};

  constructor(frame: number, totalFrames: number, sceneFrames: number) {
    this.frame = frame;
    this.totalFrames = totalFrames;
    this.currentFrame = frame;
    this.sceneFrames = sceneFrames;
  }

  fromTo(
    nodeIds: string | string[],
    from: Record<string, number>,
    to: Record<string, number>,
    opts: AnimInput = {},
  ): AnimOutput[] {
    const ids = Array.isArray(nodeIds) ? nodeIds : [nodeIds];
    const {
      duration = this.sceneFrames || this.totalFrames,
      delay = 0,
      clamp = false,
      ease,
      stagger = 0,
      repeat = 0,
      yoyo = false,
      repeatDelay = 0,
    } = opts;

    const { easing, spring } = parseEasing(ease || 'ease');

    return ids.map((_id, i) => {
      const staggeredDelay = delay + i * stagger;
      const progress = computeProgress(
        this.currentFrame, duration, staggeredDelay,
        easing, spring, clamp, repeat, yoyo, repeatDelay,
      );

      const result: AnimOutput = { opacity: 0, y: 0, scale: 1, rotate: 0, x: 0 };
      for (const [key, toVal] of Object.entries(to)) {
        const fromVal = from[key] ?? 0;
        const val = fromVal + (toVal - fromVal) * progress;
        (result as any)[key] = val;
      }
      return result;
    });
  }

  getNode(nodeId: string): NodeStyler {
    if (!this.mutations[nodeId]) {
      this.mutations[nodeId] = {};
    }
    return new NodeStyler(this.mutations[nodeId]);
  }

  collectMutations(): CollectedMutations {
    // Remove empty mutation entries
    const cleaned: Record<string, NodeMutations> = {};
    for (const [id, muts] of Object.entries(this.mutations)) {
      if (Object.keys(muts).length > 0) {
        cleaned[id] = muts;
      }
    }
    return { mutations: cleaned };
  }
}
```

- [ ] **Step 3: 实现 NodeStyler**

```typescript
class NodeStyler {
  constructor(private node: NodeMutations) {}

  opacity(v: number): NodeStyler { this.node.opacity = v; return this; }
  translateX(v: number): NodeStyler { this.node.translateX = v; return this; }
  translateY(v: number): NodeStyler { this.node.translateY = v; return this; }
  x(v: number): NodeStyler { this.node.translateX = v; return this; }
  y(v: number): NodeStyler { this.node.translateY = v; return this; }
  scale(v: number): NodeStyler { this.node.scale = v; return this; }
  rotate(v: number): NodeStyler { this.node.rotate = v; return this; }
  width(v: number): NodeStyler { this.node.width = v; return this; }
  height(v: number): NodeStyler { this.node.height = v; return this; }
  bgColor(c: string): NodeStyler { this.node.bgColor = c; return this; }
  textColor(c: string): NodeStyler { this.node.textColor = c; return this; }
  fontSize(px: number): NodeStyler { this.node.fontSize = px; return this; }
}
```

- [ ] **Step 4: 执行脚本的入口函数**

```typescript
export function runScript(
  ctx: ScriptCtx,
  scriptSource: string,
): void {
  try {
    const fn = new Function('ctx', scriptSource);
    fn(ctx);
  } catch (err) {
    console.error('Script execution error:', err);
    throw err;
  }
}
```

- [ ] **Step 5: 编译检查**

```bash
cd web && npx tsc --noEmit
```

- [ ] **Step 6: Commit**

```bash
rtk git add web/src/script-runtime.ts && rtk git commit -m "feat(web): add script-runtime.ts - browser-native ctx API for animation scripts"
```

---

## Chunk 5: main.ts 串联

### Task 5.1: 整合主线流程

**Files:**
- Modify: `web/src/main.ts`
- Modify: `web/src/wasm.ts` (已有 buildFrame 导出)

重构 main.ts 把当前的 `drawFallbackFrame` 替换为完整的管线渲染。

- [ ] **Step 1: 添加资源预加载逻辑**

在 `web/src/main.ts` 中添加:

```typescript
import { loadImages, setCanvasKit } from './resource';
import { createContext, runScript, type ScriptCtx } from './script-runtime';
import { buildFrame } from './wasm';

// Resource meta storage
let resourceMeta: Record<string, { width: number; height: number; kind: string; durationSecs?: number }> = {};

async function preloadResources(jsonlContent: string): Promise<void> {
  const requests = collectResources(jsonlContent);
  resourceMeta = {};

  // Load images
  const baseUrl = currentFile ? currentFile.path.replace(/\/[^/]+$/, '/') : '/json/';
  for (const imgPath of requests.images) {
    try {
      const loaded = await loadImage(imgPath, imgPath.startsWith('http') ? imgPath : new URL(imgPath, baseUrl).href);
      resourceMeta[imgPath] = { width: loaded.width, height: loaded.height, kind: 'image' };
    } catch (e) {
      console.warn(`Failed to load image: ${imgPath}`, e);
      // Use placeholder dimensions
      resourceMeta[imgPath] = { width: 100, height: 100, kind: 'image' };
    }
  }

  // Videos: probe via ffmpeg.wasm for dimensions + duration
  for (const vidPath of requests.videos) {
    // TODO: Implement video probing with ffmpeg.wasm
    resourceMeta[vidPath] = { width: 1920, height: 1080, kind: 'video', durationSecs: 0 };
  }
}
```

- [ ] **Step 2: 实现逐帧渲染函数**

替换 `drawFallbackFrame`:

```typescript
async function renderFrame(frame: number): Promise<void> {
  if (!currentJsonlContent || !currentComposition || !ckCanvas || !surface) return;

  const comp = currentComposition;
  const sceneFrames = comp.frames;

  // 1. Execute scripts
  const ctx = createContext(frame + 1, comp.frames, sceneFrames);
  // Note: scripts are embedded in JSONL as "script" lines
  // For now, parse scripts from parsed result
  const parsed = parseJsonl(currentJsonlContent);
  const scriptElements = parsed.elements.filter(e => e.type === 'script');
  
  for (const script of scriptElements) {
    if (script.scriptSource) {
      runScript(ctx, script.scriptSource);
    }
  }
  
  const mutations = ctx.collectMutations();
  const mutationsJson = JSON.stringify(mutations);
  const resourceMetaJson = JSON.stringify(resourceMeta);

  // 2. WASM build display tree
  const result = buildFrame(currentJsonlContent, frame, resourceMetaJson, mutationsJson);

  // 3. Render via CanvasKit
  ckCanvas.clear(CanvasKit.Color4f(0.06, 0.06, 0.09, 1.0));
  drawDisplayTree(result, currentComposition, frame);
  surface.flush();
}
```

- [ ] **Step 3: 更新文件选择流程**

当用户点击文件时：
1. 调用 `preloadResources(jsonlContent)` 预下载资源
2. 调用 `renderFrame(0)` 渲染首帧
3. 启用播放控制

- [ ] **Step 4: 构建前端**

```bash
cd web && npx vite build
```

- [ ] **Step 5: Commit**

```bash
rtk git add web/src/main.ts && rtk git commit -m "feat(web): integrate full pipeline - preload → script → build_frame → drawDisplayTree"
```

---

## Chunk 6: 端到端验证

### Task 6.1: 创建测试 JSONL 并验证

**Files:**
- Create: `json/test_web_render.jsonl` (simple test scene)
- 无需额外 Rust 测试（管线已由 core 现有测试覆盖）

- [ ] **Step 1: 创建测试 JSONL**

```jsonl
{"type": "composition", "width": 800, "height": 600, "fps": 24, "frames": 24}
{"type": "div", "id": "root", "className": "w-full h-full bg-slate-900 flex items-center justify-center"}
{"type": "text", "id": "title", "parentId": "root", "className": "text-white text-4xl font-bold", "text": "Hello OpenCat Web!"}
{"type": "script", "parentId": "root", "scriptSource": "(function() { ctx.getNode('title').opacity(ctx.frame / 24); })();"}
```

- [ ] **Step 2: 构建 WASM + 前端**

```bash
rtk cargo build -p opencat-web --target wasm32-unknown-unknown --release
cp target/wasm32-unknown-unknown/release/opencat_web.wasm web/wasm/
cd web && npx vite build
```

- [ ] **Step 3: 启动 dev server 并手动验证**

```bash
cd web && npx vite --host
```

在浏览器打开，选择 `test_web_render.jsonl`，检查：
- 资源列表正确显示
- 首帧渲染：黑色背景 + "Hello OpenCat Web!" 文字
- 播放：文字 opacity 随帧号变化（淡入效果）
- 导出按钮可点击（MP4 导出至少不崩溃）

- [ ] **Step 4: 验证 hello_world_anim.js 兼容性**

创建使用 `ctx.fromTo()` 的测试 JSONL 确认 script-runtime 能执行现有的 `hello_world_anim.js` 风格脚本。

```jsonl
{"type": "composition", "width": 800, "height": 600, "fps": 30, "frames": 60}
{"type": "div", "id": "root", "className": "w-full h-full bg-slate-900 flex items-center justify-center"}
{"type": "text", "id": "title", "parentId": "root", "className": "text-white text-5xl font-bold", "text": "Animated"}
{"type": "script", "parentId": "root", "scriptSource": "(function() { var hero = ctx.fromTo('title', {opacity: 0, y: 40}, {opacity: 1, y: 0, ease: 'spring.gentle'}); ctx.getNode('title').opacity(hero[0].opacity).y(hero[0].y); })();"}
```

- [ ] **Step 5: Commit**

```bash
rtk git add json/ && rtk git commit -m "test: add web rendering test scenes"
```

---

## Summary

| Chunk | 内容 | 行数估计 |
|-------|------|---------|
| 1 | Serialize 所有 display/style/script 类型 | ~200 行改 |
| 2 | HashMapResourceCatalog + PrecomputedScriptHost | ~150 行新增 |
| 3 | build_frame WASM 导出 | ~80 行新增 |
| 4 | script-runtime.ts | ~150 行新增 |
| 5 | main.ts 串联 | ~80 行改 |
| 6 | e2e 验证 | ~30 行 JSONL |

总计约 690 行，分为 6 个独立可验证的 chunk。
