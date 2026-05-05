# Web 渲染管线设计

日期: 2026-05-05

## 目标

跑通完整 web 渲染流程：JSONL 输入 → Rust/WASM 管线计算 → CanvasKit 渲染 → ffmpeg.wasm 导出。

## 整体架构

```
┌─ Web (JS/TS) ──────────────────────────────────────────────────────────────────┐
│                                                                                 │
│  1. 加载 JSONL ──→ WASM parse_jsonl() ──→ ParsedComposition                    │
│  2. 资源收集 ────→ WASM collect_resources_json() ──→ 下载 (fetch+bitmap)       │
│  3. 脚本执行 ────→ 浏览器原生 JS ctx API ──→ StyleMutations JSON               │
│  4. 管线调用 ────→ WASM build_frame(parsed, frame, resource_meta, mutations)   │
│                    ──→ DisplayTree JSON (match types.ts 接口)                   │
│  5. 渲染 ────────→ drawDisplayTree() @ CanvasKit → surface.flush()             │
│  6. 导出 ────────→ captureFramePixels() → ffmpeg.wasm encode → MP4/PNG         │
│                                                                                 │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## 前置依赖：DisplayTree 序列化

所有 display 类型需添加 `#[derive(Serialize)]` + `#[serde(rename_all = "camelCase")]`，
匹配 web/src/types.ts 的 DisplayNodeJson/DisplayItemJson。serde 已是 opencat-core 的现有依赖。

需要序列化的类型链：
```
DisplayTree → DisplayNode → DisplayItem (enum)
  ├── RectDisplayItem (RectPaintStyle)
  ├── TimelineDisplayItem (TransitionKind)
  ├── TextDisplayItem (ComputedTextStyle, TextUnitOverrideBatch)
  ├── BitmapDisplayItem (BitmapPaintStyle)
  ├── DrawScriptDisplayItem (CanvasCommand, DropShadow)
  └── SvgPathDisplayItem (SvgPathPaintStyle)
+ DisplayRect, DisplayClip, DisplayTransform, Transform
+ BackgroundFill, BorderRadius, ColorToken, BorderStyle
+ BoxShadow, DropShadow, InsetShadow
```

CanvasCommand 序列化用 `#[serde(tag = "type")]`，字段保持 camelCase。

## 组件设计

### 1. WASM 管线层 (`opencat-web` crate)

新增导出函数：

```rust
#[wasm_bindgen]
pub fn build_frame(
    jsonl_input: &str,
    frame: u32,
    resource_meta: &str,   // ResourceMeta JSON, 见下方格式
    mutations_json: &str,  // StyleMutations JSON (每帧由JS执行脚本后生成)
) -> String  // DisplayTree JSON，或 {"error": "..."}
```

内部流程：
1. 解析 JSONL → `ParsedComposition{ width, height, fps, frames, root, scripts, audio_sources }`
2. 构建 `FrameCtx { frame, total_frames: frames, fps, width, height }`
3. 构建 `HashMapResourceCatalog`（从 resource_meta 注入）
4. 构建 `PrecomputedScriptHost`（从 mutations_json 注入）
5. 组装 fonts: `fontdb::Database` 用内嵌字体（`default_font_db_with_embedded_only()` 已在 core 中存在）
6. 调用 core 管线:
   - `resolve_ui_tree(&root, &frame_ctx, &mut catalog, parent_composition: None, &mut script_host)`
   - `LayoutSession::default().compute_layout_with_font_db(&element_root, &frame_ctx, &font_db)`
   - `build_display_tree(&element_root, &layout_tree)`
7. 序列化 DisplayTree 为 JSON 返回

每帧新建 `LayoutSession`，不跨帧缓存（牺牲少量性能换简洁，后续可优化）。

涉及改动：
- 所有 display 类型加 `#[derive(Serialize)]`
- `opencat-web/src/wasm_entry.rs`: 新增 `build_frame`
- `opencat-core/src/resource/catalog.rs`: 新增 `HashMapResourceCatalog`
- `opencat-core/src/scene/script/`: 新增 `PrecomputedScriptHost`

### 2. TypeScript 脚本运行时 (`web/src/script-runtime.ts`)

在浏览器原生 JS 中实现 QuickJS 暴露给动画脚本的 `ctx` API。

**ctx 对象 API:**
- `ctx.frame` / `ctx.totalFrames` / `ctx.currentFrame` / `ctx.sceneFrames` — 帧上下文
- `ctx.fromTo(nodeIds, from, to, opts)` → 返回 `AnimateResult[]`，每项有 `.opacity/.y/.scale/.rotate` 等
  - opts: `{ duration, delay, stagger, ease, clamp, repeat, yoyo, repeatDelay }`
  - 内部用 `animator.ts` 的 `animateValue()`/`computeProgress()`
- `ctx.getNode(nodeId)` → `NodeStyler` (链式设置最终值)
  - `.opacity(v)` `.translateX(v)` `.translateY(v)` `.scale(v)` `.rotate(v)`
  - `.width(v)` `.height(v)` `.bgColor(c)` `.textColor(c)` `.fontSize(px)` 等
- `ctx.getCanvas(nodeId)` → `CanvasStyler`
  - `.fillRect()` `.strokeRect()` `.fillCircle()` `.drawText()` `.drawImage()` 等
- `ctx.setTextSource(text, kind)` — 提供文字源

**与脚本源码的接口：**
脚本源码使用 IIFE + ctx 闭包:
```js
(function() {
  var hero = ctx.fromTo('title', {opacity: 0, y: 40}, {opacity: 1, y: 0, ease: 'spring.gentle'});
  ctx.getNode('title').opacity(hero.opacity).translateY(hero.y);
})();
```

**执行流程 (per frame):**
1. JS 收到当前帧号 N
2. 创建 ctx 沙箱对象（含 frame 上下文 + animator 能力）
3. 对所有注册的脚本，`new Function('ctx', source)(ctx)` 执行
4. ctx 上的链式调用收集 mutation 到内部 map
5. 最终序列化为 `StyleMutations` JSON

**Mutation JSON 格式（传给 WASM 的 mutations_json）:**
```json
{
  "mutations": {
    "node-id": {
      "opacity": 0.5,
      "translateY": 40,
      "scale": 0.95
    }
  },
  "canvas_mutations": {}
}
```

### 3. 资源预加载 (`web/src/resource.ts` 扩展)

流程：
1. WASM `collect_resources_json()` 返回 `{ images: [...], videos: [...], audios: [...], icons: [...] }`
2. JS 遍历列表下载：
   - 图片: `fetch` → `createImageBitmap` → 记录宽高
   - 视频: `fetch` (head request) 或用 ffmpeg.wasm probe 获取 duration
   - Lucide 图标: 从本地 `/lucide/` 目录加载 SVG
3. 构建 resource_meta JSON:

```json
{
  "images/puppy.png": { "width": 800, "height": 600, "kind": "image" },
  "images/logo.svg":  { "width": 200, "height": 200, "kind": "image" },
  "videos/clip.mp4":  { "width": 1920, "height": 1080, "kind": "video", "duration_secs": 12.5 }
}
```

每个条目的定位符（key）与 JSONL 中的 `src` 一致。
4. 注入 WASM 管线计算
5. 同时将 CanvasKit Image 存入 `loadedImages` map，供渲染时查找

### 4. PrecomputedScriptHost (`opencat-core`)

新增非 test-only 的 ScriptHost 实现，用于 web 侧预计算 mutations 场景：

```rust
pub struct PrecomputedScriptHost {
    pub mutations: HashMap<ScriptDriverId, StyleMutations>,
}

impl ScriptHost for PrecomputedScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId> {
        // 用 core 已使用的 DefaultHasher 做稳定 hash，不编译脚本
        use std::hash::{DefaultHasher, Hash, Hasher};
        let mut h = DefaultHasher::new();
        source.hash(&mut h);
        Ok(ScriptDriverId(h.finish()))
    }

    fn register_text_source(&mut self, node_id: &str, source: ScriptTextSource) {
        // no-op: 文字源由 JS 侧通过 mutations 的 text_content 字段注入
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
            .ok_or_else(|| anyhow!("no precomputed mutations for script"))
    }
}
```

注意: `run_frame` 签名使用 `&mut self`，符合 `ScriptHost` trait 要求（`host.rs:10`）。

### 5. HashMapResourceCatalog (`opencat-core`)

```rust
pub struct HashMapResourceCatalog {
    entries: HashMap<String, ResourceMeta>,
    // asset path → AssetId 映射
    asset_cache: HashMap<String, AssetId>,
    next_id: u64,
}

pub struct ResourceMeta {
    pub width: u32,
    pub height: u32,
    pub kind: ResourceKind,
    pub duration_secs: Option<f64>,
}

pub enum ResourceKind { Image, Video, Audio }
```

构造器 `HashMapResourceCatalog::from_json(json: &str)` 反序列化 resource_meta JSON，
遍历每个 entry 调用 `register_dimensions()` 注册到 `asset_cache`。
AssetId 由路径 hash 生成（`fxhash::hash64` 等同 `cache_key()` 使用的算法）。

实现 `ResourceCatalog` trait 的全部方法：
- `resolve_image(&mut self, src: &ImageSource)` — 将 ImageSource 路径映射为 AssetId
- `resolve_audio(&mut self, src: &AudioSource)` — 将 AudioSource 路径映射为 AssetId
- `register_dimensions(&mut self, locator, w, h)` — 注册并返回 AssetId
- `alias(&mut self, alias, target)` — 建立别名
- `dimensions(&self, id)` — 返回 (width, height)，从 entries 查询
- `video_info(&self, id)` — 返回 `Option<VideoInfoMeta>`，含 duration_secs

## 数据流

```
Frame N 渲染完整流程:

1. JS: 构建 ctx 对象 (frame=N, totalFrames, ...)
2. JS: 对每个 script 执行 new Function('ctx', source)(ctx)
       ctx 上的链式调用自动收集 mutation
3. JS: mutations_json = JSON.stringify(collectedMutations)
4. JS: display_json = wasm.build_frame(jsonl, N, resource_meta, mutations_json)
5. WASM: parse JSONL → Composition.root_node(N)
6. WASM: resolve_ui_tree(root, frame_ctx, catalog, PrecomputedScriptHost)
         ├─ 遍历 Node tree，应用 mutation stack
         ├─ 处理 Timeline: compute FrameState (Scene vs Transition)
         ├─ Resolve 图片/视频 通过 ResourceCatalog
         └─ 输出 ElementTree
7. WASM: LayoutSession.compute_layout_with_font_db(element_root, frame_ctx, &font_db)
         └─ taffy flexbox → 每个 node 得到 position/size
8. WASM: build_display_tree(element_root, layout_tree)
         └─ ElementNode + LayoutNode → DisplayNode → DisplayItem
9. WASM: serde_json::to_string(&display_tree) → 返回 DisplayTree JSON
10. JS: drawDisplayTree(displayTree) @ CanvasKit
11. JS: surface.flush() → canvas 显示
```

## 导出流程

导出复用预览管线（WASM `build_frame` → CanvasKit 渲染），在 JS 侧串联 ffmpeg.wasm。

### MP4 导出

```
1. 初始化 ffmpeg.wasm (已有 @ffmpeg/ffmpeg 依赖)
2. 设置进度 UI (export progress bar)
3. for frame N in 0..totalFrames:
   a. 运行脚本 → mutations_json
   b. wasm.build_frame(jsonl, N, resource_meta, mutations_json)
   c. drawDisplayTree(displayTree) → surface.flush()
   d. captureFramePixels(surface) → RGBA Uint8Array
   e. ffmpeg.writeFrame(frameData, frameIndex)
4. ffmpeg.finalize() → MP4 blob
5. downloadMp4(blob)
```

ffmpeg.wasm 配置:
- 输入: rawvideo, pixel_format=rgba, s=WxH, framerate=fps
- 输出: libx264, crf=23, preset=medium, pixel_format=yuv420p
- 不加载 ffmpeg.wasm 的音频能力（此阶段仅视频导出）

### PNG 导帧

单帧导出:
1. 运行脚本 + build_frame + drawDisplayTree（与预览相同）
2. surface.makeImageSnapshot() → CanvasKit Image
3. image.encodeToBytes(CanvasKit.ImageFormat.PNG) → Uint8Array
4. 触发 blob download

## 字体处理

WASM 环境中使用 `default_font_db_with_embedded_only()`（已在 core 中存在），
它从编译时嵌入的字体数据构建 `fontdb::Database`。`LayoutSession` 的
`compute_layout_with_font_db()` 直接使用此 db。

后续可扩展为通过 JS bridge 加载 web fonts。

## 错误处理

- **脚本语法错误**: `new Function()` 抛出 SyntaxError，script-runtime.ts 捕获并在 UI 提示
- **WASM 管线错误**: `build_frame` 返回含 `"error"` 字段的 JSON，JS 侧解析展示
- **资源下载失败**: 预加载阶段失败用占位色块 + console.warn，不阻塞管线

## 不与桌面端共享的组件

| 组件       | 桌面                       | Web                              |
|-----------|----------------------------|----------------------------------|
| 脚本引擎   | rquickjs (QuickJS)         | 原生 `new Function()` + script-runtime.ts |
| 资源加载   | 磁盘/网络 (reqwest)        | fetch + createImageBitmap        |
| 渲染后端   | Skia (skia-safe)           | CanvasKit (canvaskit-wasm)       |
| 字体       | 系统字体 + fontdb 扫描      | 内嵌字体 (default_font_db_with_embedded_only) |
| 视频编解码 | ffmpeg-next                | ffmpeg.wasm                      |

## 实现步骤

1. 在 core display 类型上加 `#[derive(Serialize)]`
2. `HashMapResourceCatalog` in `opencat-core/src/resource/catalog.rs`
3. `PrecomputedScriptHost` in `opencat-core/src/scene/script/`
4. `build_frame` WASM 导出 in `opencat-web/src/wasm_entry.rs`
5. `script-runtime.ts` — ctx API 实现
6. `web/src/main.ts` 串联完整流程（文件选择 → 预加载 → 逐帧渲染）
7. 端到端测试: hello_world.jsonl + hello_world_anim.js 在浏览器中渲染
