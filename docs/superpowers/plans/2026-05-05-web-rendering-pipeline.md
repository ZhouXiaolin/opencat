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

需要给以下类型加 `#[derive(Serialize, Deserialize)]`（serde 已是 core 的依赖）。**保持所有已有的 derive（Debug, Clone, Copy, PartialEq, Eq, Hash 等），只追加 serde derive**：

| 类型 | 文件位置 | 需要注意 |
|------|---------|---------|
| `BorderStyle` | style.rs:194 | `#[serde(rename_all = "camelCase")]` |
| `TextAlign` | style.rs:131 | camelCase |
| `TextTransform` | style.rs:158 | camelCase |
| `ObjectFit` | style.rs | camelCase |
| `FontWeight` | style.rs:148 | newtype `FontWeight(pub u16)` — **必须加 `#[serde(transparent)]`** 否则序列化为数组 `[700]` 而非数字 `700` |
| `GradientDirection` | style.rs | camelCase |
| `BackgroundFill` | style.rs:430 | tagged enum — 见下方注解 |
| `Transform` | style.rs:441 | tagged enum — 见下方注解 |
| `BorderRadius` | style.rs:166 | camelCase |
| `BoxShadow` | style.rs:237 | camelCase |
| `InsetShadow` | style.rs:246 | camelCase |
| `DropShadow` | style.rs:255 | camelCase |
| `ComputedTextStyle` | style.rs:630 | camelCase；**保持 `Copy` derive** |

- [ ] **Step 1: 逐一加 derive 宏**

**BackgroundFill** — `types.ts:90-97` 期望 `type: "solid"` 时字段叫 `color`（不是默认的 `value`）：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum BackgroundFill {
    #[serde(rename = "solid")]
    Solid(#[serde(rename = "color")] ColorToken),
    LinearGradient {
        direction: GradientDirection,
        from: ColorToken,
        via: Option<ColorToken>,
        to: ColorToken,
    },
}
```

**Transform** — `types.ts:167-172` 期望 `{ type: "translate", x: ..., y: ... }`。tuple variant 字段需重命名避免默认的 `v0`/`v1`：

```rust
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Transform {
    #[serde(rename = "translateX")]
    TranslateX(#[serde(rename = "value")] f32),
    #[serde(rename = "translateY")]
    TranslateY(#[serde(rename = "value")] f32),
    #[serde(rename = "translate")]
    Translate(#[serde(rename = "x")] f32, #[serde(rename = "y")] f32),
    #[serde(rename = "scale")]
    Scale(#[serde(rename = "value")] f32),
    #[serde(rename = "scaleX")]
    ScaleX(#[serde(rename = "value")] f32),
    #[serde(rename = "scaleY")]
    ScaleY(#[serde(rename = "value")] f32),
    #[serde(rename = "rotate")]
    RotateDeg(#[serde(rename = "value")] f32),
    #[serde(rename = "skewX")]
    SkewXDeg(#[serde(rename = "value")] f32),
    #[serde(rename = "skewY")]
    SkewYDeg(#[serde(rename = "value")] f32),
    #[serde(rename = "skew")]
    SkewDeg(#[serde(rename = "x")] f32, #[serde(rename = "y")] f32),
}
```

**FontWeight** — 必须 transparent：

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct FontWeight(pub u16);
```

**ComputedTextStyle** — 保持 `Copy`：

```rust
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
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
- Modify: `crates/opencat-core/src/display/list.rs` — DisplayRect, DisplayClip, DisplayTransform, DisplayItem + 所有子结构
- Modify: `crates/opencat-core/src/display/tree.rs` — DisplayTree, DisplayNode
- Modify: `crates/opencat-core/src/element/tree.rs` — ElementId (DisplayNode.element_id 字段的类型)
- Modify: `crates/opencat-core/src/resource/asset_id.rs` — AssetId (BitmapDisplayItem.asset_id)
- Modify: `crates/opencat-core/src/resource/types.rs` — VideoFrameTiming, VideoPreviewQuality
- Modify: `crates/opencat-core/src/scene/script/mutations.rs` — CanvasCommand, ScriptColor, ScriptLineCap, ScriptLineJoin, ScriptPointMode, ScriptFontEdging, TextUnitGranularity, TextUnitOverride, TextUnitOverrideBatch
- Modify: `crates/opencat-core/src/scene/transition.rs` — TransitionKind, SlideDirection, WipeDirection

需要添加 `Serialize, Deserialize` 到所有 display 类型。注意与 web/src/types.ts 的接口匹配（camelCase 命名）。

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

- [ ] **Step 5: 所有 transitive 类型加 Serialize**

逐一为以下文件加 Serialize/Deserialize：

`crates/opencat-core/src/element/tree.rs` — ElementId:
```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct ElementId(pub u64);
```

`crates/opencat-core/src/resource/asset_id.rs` — AssetId:
```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(transparent)]
pub struct AssetId(pub String);
```

`crates/opencat-core/src/resource/types.rs` — VideoFrameTiming, VideoPreviewQuality:
```rust
#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoFrameTiming { ... }

#[derive(Clone, Copy, Debug, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum VideoPreviewQuality { Scrubbing, Realtime, Exact }
```

`crates/opencat-core/src/scene/script/mutations.rs` — 除了 CanvasCommand 外的子类型:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptColor { pub r: u8, pub g: u8, pub b: u8, pub a: u8 }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptLineCap { Butt, Round, Square }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptLineJoin { Miter, Round, Bevel }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptPointMode { Points, Lines, Polygon }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ScriptFontEdging { Alias, AntiAlias, SubpixelAntiAlias }

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextUnitGranularity { Grapheme, Word }

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextUnitOverride { ... }

#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TextUnitOverrideBatch { ... }
```

`crates/opencat-core/src/scene/transition.rs` — SlideDirection, WipeDirection:
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SlideDirection { Left, Right, Top, Bottom }

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WipeDirection { ... }
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
rtk git add crates/opencat-core/src/display/ crates/opencat-core/src/scene/ crates/opencat-core/src/element/tree.rs crates/opencat-core/src/resource/asset_id.rs crates/opencat-core/src/resource/types.rs && rtk git commit -m "feat(core): add Serialize/Deserialize to display and script types"
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
/// AssetId uses string keys matching resource locators.
pub struct HashMapResourceCatalog {
    entries: HashMap<AssetId, ResourceMeta>,
    asset_cache: HashMap<String, AssetId>,
}

impl HashMapResourceCatalog {
    /// Build catalog from JSON string:
    /// `{ "path/to/image.png": { "width": 800, "height": 600, "kind": "image" }, ... }`
    pub fn from_json(json: &str) -> Result<Self> {
        let map: HashMap<String, ResourceMeta> = serde_json::from_str(json)?;
        let mut catalog = Self {
            entries: HashMap::new(),
            asset_cache: HashMap::new(),
        };
        for (locator, meta) in map {
            let id = AssetId(locator.clone());
            catalog.asset_cache.insert(locator.clone(), id.clone());
            catalog.entries.insert(id, meta);
        }
        Ok(catalog)
    }

    fn resolve_key(&mut self, key: &str) -> AssetId {
        if let Some(id) = self.asset_cache.get(key) {
            return id.clone();
        }
        let id = AssetId(key.to_string());
        self.asset_cache.insert(key.to_string(), id.clone());
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
        assert_eq!(catalog.dimensions(&AssetId("unknown".to_string())), (0, 0));
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
        _driver: ScriptDriverId,  // Ignored: JS side pre-computes all mutations
        _frame_ctx: &ScriptFrameCtx,
        _current_node_id: Option<&str>,
    ) -> Result<StyleMutations> {
        // Always return the stored mutations, regardless of which driver called.
        // In single-script mode, takes the first (and only) entry.
        // Use mutable drain to consume (resolve_ui_tree calls run_frame once per frame).
        self.mutations
            .drain()
            .next()
            .map(|(_, m)| m)
            .ok_or_else(|| anyhow!("no precomputed mutations available"))
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
    use crate::scene::script::{NodeStyleMutations, StyleMutations, ScriptHost};
    use std::collections::HashMap;

    #[test]
    fn from_json_parses_and_returns_mutations() {
        let json = r#"{"mutations":{"node1":{"opacity":0.5}},"canvas_mutations":{}}"#;
        let mut host = PrecomputedScriptHost::from_json(json).unwrap();
        // install is a no-op for ID tracking
        let id = host.install("test script").unwrap();
        // run_frame returns the stored mutations regardless of driver ID
        let result = host.run_frame(id, &Default::default(), None).unwrap();
        let node_muts = result.mutations.get("node1").unwrap();
        assert_eq!(node_muts.opacity, Some(0.5));
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

    #[test]
    fn run_frame_with_no_mutations_returns_error() {
        let mut host = PrecomputedScriptHost::from_single(StyleMutations::default());
        let id = host.install("script").unwrap();
        // First run_frame succeeds (takes the default mutations)
        host.run_frame(id, &Default::default(), None).unwrap();
        // Second run_frame fails (no more stored mutations)
        assert!(host.run_frame(id, &Default::default(), None).is_err());
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

- [ ] **Step 2: 确认 ParsedComposition 和 LayoutSession API**

```bash
# 检查 ParsedComposition 字段 — 确认 parsed.frames/fps/width/height 是顶层字段
rtk grep "pub struct ParsedComposition" crates/opencat-core/src/jsonl/mod.rs

# 检查 LayoutSession 是否有 pub fn tree() 方法
rtk grep "pub fn tree\|pub.*layout_tree" crates/opencat-core/src/layout/mod.rs
```

根据结果选择实现方式：

- 如果 `ParsedComposition` 字段为 `composition: Composition` 嵌套结构，调整为 `parsed.composition.root_node()` 等。
- 如果 `LayoutSession` 没有 `tree()` 方法，使用 fallback 函数 `compute_layout_with_font_db_fn`：

```rust
// Fallback: use free function instead of LayoutSession
let font_db = text::default_font_db_with_embedded_only();
let layout_tree = opencat_core::layout::compute_layout_with_font_db_fn(
    &element_root, &frame_ctx, &font_db,
)?;
let display_tree = build_display_tree(&element_root, &layout_tree)?;
```

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
  parseEasing,
  type SpringConfig,
} from './animator';

// ── Mutation types (match Rust NodeStyleMutations after camelCase serde) ──

interface NodeMutations {
  opacity?: number;
  width?: number;
  height?: number;
  bgColor?: string;
  textColor?: string;
  textPx?: number;
  // Temporary accumulators (flushed to transforms before serialization)
  y?: number;
  scale?: number;
  rotate?: number;
  transforms?: TransformEntry[];
}

interface TransformEntry {
  type: string;   // "translate", "scale", "rotate", etc.
  x?: number;
  y?: number;
  value?: number;
}

interface CollectedMutations {
  mutations: Record<string, NodeMutations>;
  canvas_mutations: Record<string, never>;  // Required by Rust StyleMutations
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
    // Design note: only properties in `to` are interpolated.
    // Properties only in `from` are ignored (they don't define an animation target).
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
    // Flush transform accumulations
    for (const nodeId of Object.keys(this.mutations)) {
      const styler = new NodeStyler(this.mutations[nodeId]);
      styler.flushTransforms();
    }
    // Remove empty mutation entries
    const cleaned: Record<string, NodeMutations> = {};
    for (const [id, muts] of Object.entries(this.mutations)) {
      if (Object.keys(muts).length > 0) {
        cleaned[id] = muts;
      }
    }
    return { mutations: cleaned, canvas_mutations: {} };
  }
}
```

- [ ] **Step 3: 实现 NodeStyler**

```typescript
class NodeStyler {
  constructor(private node: NodeMutations) {}

  opacity(v: number): NodeStyler { this.node.opacity = v; return this; }
  width(v: number): NodeStyler { this.node.width = v; return this; }
  height(v: number): NodeStyler { this.node.height = v; return this; }
  bgColor(c: string): NodeStyler { this.node.bgColor = c; return this; }
  textColor(c: string): NodeStyler { this.node.textColor = c; return this; }
  textPx(px: number): NodeStyler { this.node.textPx = px; return this; }

  // Transform-based properties accumulate into `transforms` array (matches Rust Transform enum)
  translateY(v: number): NodeStyler { this.node.y = v; return this; }
  y(v: number): NodeStyler { this.node.y = v; return this; }
  scale(v: number): NodeStyler { this.node.scale = v; return this; }
  rotate(v: number): NodeStyler { this.node.rotate = v; return this; }

  // Flatten transforms before serialization
  private addTransform(t: TransformEntry): void {
    if (!this.node.transforms) this.node.transforms = [];
    this.node.transforms.push(t);
  }

  flushTransforms(): void {
    if (this.node.scale !== undefined) { this.addTransform({ type: 'scale', value: this.node.scale }); delete this.node.scale; }
    if (this.node.y !== undefined) { this.addTransform({ type: 'translateY', value: this.node.y }); delete this.node.y; }
    if (this.node.rotate !== undefined) { this.addTransform({ type: 'rotate', value: this.node.rotate }); delete this.node.rotate; }
  }
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
import { drawDisplayTree } from './renderer';

async function renderFrame(frame: number): Promise<void> {
  if (!currentJsonlContent || !currentComposition || !ckCanvas || !surface) return;

  try {
    const comp = currentComposition;
    const sceneFrames = comp.frames;

    // 1. Execute scripts
    const ctx = createContext(frame + 1, comp.frames, sceneFrames);
    // Extract script sources from JSONL lines.
    // parseJsonl wraps `parse_jsonl` which returns `{ composition, elements, elementCount }`.
    // "script" elements have `scriptSource` field from the JSONL.
    const parsed = parseJsonl(currentJsonlContent);
    const scriptElements = (parsed.elements || []).filter(
      (e: any) => e.type === 'script'
    );

    for (const script of scriptElements) {
      // Script source may be in `source`, `scriptSource`, or `script` field
      const source = script.scriptSource || script.source || script.script || '';
      if (source) {
        runScript(ctx, source);
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

    // Update frame info
    frameLabel.textContent = `${frame + 1} / ${comp.frames}`;
    frameSlider.value = String(frame);
  } catch (err) {
    console.error('Render error:', err);
    // Show error on canvas
    ckCanvas.clear(CanvasKit.Color4f(0.1, 0.05, 0.05, 1.0));
    const errorPaint = new CanvasKit.Paint();
    errorPaint.setColor(CanvasKit.Color4f(1, 0.3, 0.3, 1.0));
    ckCanvas.drawText(
      `Render error: ${err}`,
      12, 24, errorPaint,
      new CanvasKit.Font(null, 14),
    );
    surface.flush();
  }
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

创建使用 `ctx.fromTo()` 的测试 JSONL 确认 script-runtime 能执行现有的动画脚本风格。

```jsonl
{"type": "composition", "width": 800, "height": 600, "fps": 30, "frames": 60}
{"type": "div", "id": "root", "className": "w-full h-full bg-slate-900 flex items-center justify-center"}
{"type": "text", "id": "title", "parentId": "root", "className": "text-white text-5xl font-bold", "text": "Animated"}
{"type": "script", "parentId": "root", "scriptSource": "(function() { var hero = ctx.fromTo('title', {opacity: 0, y: 40}, {opacity: 1, y: 0}, {ease: 'spring.gentle'}); ctx.getNode('title').opacity(hero[0].opacity); })();"}
```

注意: `ease` 在第 4 参数 `opts` 中，不是第 3 参数的 `to` 对象中。

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
