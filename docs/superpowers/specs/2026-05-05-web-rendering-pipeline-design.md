# Web 渲染管线设计

日期: 2026-05-05

## 目标

跑通完整 web 渲染流程：JSONL 输入 → Rust/WASM 管线计算 → CanvasKit 渲染 → ffmpeg.wasm 导出。

## 整体架构

```
┌─ Web (JS/TS) ─────────────────────────────────────────────────┐
│                                                                │
│  1. 加载 JSONL ──→ WASM parse_jsonl() ──→ ParsedComposition   │
│  2. 资源收集 ────→ WASM collect_resources_json() ──→ 下载     │
│  3. 脚本执行 ────→ 浏览器原生 JS ctx API ──→ StyleMutations   │
│  4. 管线调用 ────→ WASM build_frame() ──→ DisplayTree JSON    │
│  5. 渲染 ────────→ drawDisplayTree() @ CanvasKit              │
│  6. 导出 ────────→ ffmpeg.wasm → MP4/PNG                      │
│                                                                │
└────────────────────────────────────────────────────────────────┘
```

## 组件设计

### 1. WASM 管线层 (`opencat-web` crate)

新增导出函数：

```rust
#[wasm_bindgen]
pub fn build_frame(
    jsonl_input: &str,
    frame: u32,
    resource_meta: &str,   // { id: {width, height, format} }
    mutations_json: &str,  // StyleMutations JSON
) -> String  // DisplayTree JSON
```

内部流程：
- 解析 JSONL → ParsedComposition
- 构建 `HashMapResourceCatalog`（从 resource_meta 注入）
- 构建 `PrecomputedScriptHost`（从 mutations_json 注入）
- 调用 core 管线: `resolve_ui_tree` → `LayoutSession` → `build_display_tree`
- 序列化 DisplayTree 为 JSON 返回

涉及改动：
- `opencat-web/Cargo.toml`: 添加 `opencat-core` 的 `test-support` feature（或单独 feature）
- `opencat-web/src/wasm_entry.rs`: 新增 `build_frame`
- `opencat-core/src/resource/`: 新增 `HashMapResourceCatalog` 实现
- `opencat-core/src/test_support.rs`: 提取 `PrecomputedScriptHost` 为非 test-only

### 2. TypeScript 脚本运行时 (`web/src/script-runtime.ts`)

在浏览器原生 JS 中实现 QuickJS 暴露给动画脚本的 `ctx` API：

**ctx 对象 API:**
- `ctx.frame` / `ctx.totalFrames` / `ctx.currentFrame` / `ctx.sceneFrames`
- `ctx.fromTo(nodeIds, from, to, opts)` → `AnimateResult[]`
- `ctx.getNode(nodeId)` → `NodeStyler` (链式: `.opacity()` `.translateY()` `.scale()` 等)
- `ctx.getCanvas(nodeId)` → `CanvasStyler`

**与现有 animator.ts 的关系：**
- `animator.ts` 提供底层 easing/spring/animate 函数
- `script-runtime.ts` 在其上构建 QuickJS 兼容的 ctx DSL
- 脚本源码用 `new Function('ctx', scriptSource)(ctx)` 方式执行

**Mutation 收集:**
- 每次链式调用（如 `.opacity(0.5)`）记录到 `Map<nodeId, NodeStyleMutations>`
- Canvas 操作记录到 `Map<nodeId, CanvasCommand[]>`
- 执行完成后序列化为 `StyleMutations` JSON 传给 WASM

### 3. 资源预加载 (`web/src/resource.ts` 扩展)

流程：
1. WASM `collect_resources_json()` 返回资源列表
2. JS 遍历列表，按类型下载：
   - 图片: `fetch` → `createImageBitmap` → 记录宽高
   - Lucide 图标: 从本地 `/lucide/` 目录加载 SVG
3. 构建 resource_meta JSON: `{ "image/xxx.png": { width: 800, height: 600, kind: "image" } }`
4. 注入 WASM 管线计算

### 4. PrecomputedScriptHost (`opencat-core`)

新增一个非 test-only 的 ScriptHost 实现：

```rust
pub struct PrecomputedScriptHost {
    pub mutations: HashMap<ScriptDriverId, StyleMutations>,
}
impl ScriptHost for PrecomputedScriptHost {
    fn install(&mut self, source: &str) -> Result<ScriptDriverId>;
    fn run_frame(&self, driver: ScriptDriverId, _frame_ctx: &ScriptFrameCtx, _current_node_id: Option<&str>) -> Result<StyleMutations>;
}
```

`install` 只记录 hash，不编译。`run_frame` 直接从 precomputed map 取结果返回。

### 5. HashMapResourceCatalog (`opencat-core`)

```rust
pub struct HashMapResourceCatalog {
    entries: HashMap<String, ResourceEntry>,
}
impl ResourceCatalog for HashMapResourceCatalog { ... }
```

从 JS 注入的资源元数据构建，提供图片/视频尺寸查询。

## 数据流

```
Frame N 渲染:

1. JS: 执行所有脚本 → StyleMutations { "title": {opacity: 0.5}, ... }
2. JS: 序列化 mutations 为 JSON
3. JS: 调用 WASM build_frame(jsonl, N, resource_meta, mutations_json)
4. WASM: Composition.root_node → resolve_ui_tree(PrecomputedScriptHost) → LayoutSession → build_display_tree
5. WASM: 返回 DisplayTree JSON
6. JS: drawDisplayTree(displayTree) @ CanvasKit
7. JS: surface.flush() → canvas 显示
```

## 错误处理

- 脚本语法错误: `script-runtime.ts` 在 `new Function()` 时捕获，显示在预览 UI
- 资源下载失败: 预加载阶段失败用占位图，带错误标记
- WASM 管线错误: `build_frame` 返回 Result-like JSON，JS 侧解析并展示

## 不与桌面端共享的组件

| 组件 | 桌面 | Web |
|------|------|-----|
| 脚本引擎 | rquickjs (QuickJS) | 原生 `new Function()` + script-runtime.ts |
| 资源加载 | 磁盘/网络 (reqwest) | fetch + createImageBitmap |
| 渲染后端 | Skia (skia-safe) | CanvasKit (canvaskit-wasm) |
| 字体 | 系统字体 + fontdb | Web fonts / 内嵌字体 |
| 视频编解码 | ffmpeg-next | ffmpeg.wasm |
| 代码 | encode MP4 via ffmpeg-next | ffmpeg.wasm |

## 实现步骤

1. `HashMapResourceCatalog` in core
2. `PrecomputedScriptHost` in core
3. `build_frame` WASM 导出
4. `script-runtime.ts` — ctx API 实现
5. `create_ts_object` 在 web/src/main.ts — 串联流程
7. 端到端测试: hello_world.jsonl 渲染
