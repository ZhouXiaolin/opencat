# 迁移说明：core 资源契约收敛（issue #2 / #11）

> 状态：已完成。本文记录 #2 系列最后一个阶段（#11）的 breaking API 变更、host
> 职责、被删符号的替代路径，以及“哪些看似在清单里、实则留 core”的判断依据。
>
> 适用范围：`opencat-core` / `opencat-engine` / `opencat-web` 三端。实现分布在
> `#3`–`#10`（已关闭）与本阶段 `#11`。

## 核心观念

> **core 尽可能精简，下载 / 视频解码 / 编码 / 音频调度等执行逻辑交各端；但两端
> 确实相同的代码仍留在 core。**

这是本次“删什么 / 留什么”的判据。`#11` 的字面验收清单里有些项（audio plan、
resolver、preload、frame consumer）听起来像“执行 seam”，但落到代码上，部分是
两端共用的纯派生数据或共用 trait —— 移走只会复制一份。这些保留在 core，理由见
[§保留项与理由](#保留项与理由)。

## 主链

```
ParsedComposition
  → ResourceRequests（声明性、顺序无关）
  → host fetch / cache（host 自行实现 fs / fetch / blob）
  → core 纯 probe（build_catalog / hydrate_captions）
  → host 构建 font_db
  → DefaultPipeline::open_with_prepared_catalog(parsed, catalog, scripts, font_db)
  → render_frame(frame_index) -> RenderFrame { draw, media }
```

**这是现在打开 core pipeline 的唯一路径。** core 不持有 loader、不 fetch、不 decode。

## Breaking API 变更

### `DefaultPipeline` 去 loader 泛型

| 之前 | 之后 |
|------|------|
| `DefaultPipeline<L: AssetLoader, S: JsContext>` | `DefaultPipeline<S: JsContext>` |
| `struct { loader: L, … }` | 无 `loader` 字段 |
| `fn loader_mut(&mut self) -> &mut L` | 删除 |
| `Pipeline` trait 含 `type Loader` / `fn loader()` | trait 只剩 `info()` / `render_frame()` |

### 删除的 core 入口与 trait

| 被删符号 | 原位置 | 替代 |
|----------|--------|------|
| `DefaultPipeline::open` | core `pipeline/default.rs` | host 自行 prepare 后用 `open_with_prepared_catalog` |
| `DefaultPipeline::open_with_font_db` | 同上 | 同上 |
| `DefaultPipeline::open_parsed`（core 内 fetch+probe 版） | 同上 | 同上 |
| `AssetLoader` trait | core `probe/mod.rs` | 无替代 —— host 用自己的具体 loader（engine: `EngineLoader`） |
| `AssetHandle` trait | 同上 | 无替代 —— host 用自己的具体 handle（engine: `EngineAssetHandle`） |
| `NoopAssetLoader` / `NoopAssetHandle` | 同上 | 不再需要（pipeline 无 loader 字段） |
| `probe_all`（core 内 fetch→probe 编排） | core `pipeline/default.rs` | host 用 `probe::prepare::build_catalog` 自行编排 |
| `Pipeline::loader()` / `type Loader` | core `pipeline/mod.rs` | 无替代 |

### `CompositionInfo` 去 `audio_plan`

| 之前 | 之后 |
|------|------|
| `CompositionInfo { …, audio_plan: AudioPlan }` | 无 `audio_plan` 字段 |
| core `probe::catalog::{AudioPlan, AudioSegment}` | 移到 `opencat_engine::audio_plan` |
| core `parse::preflight::collect_audio_plan` | 移到 `opencat_engine::audio_plan::collect_audio_plan` |

engine 的 `build_audio_track_from_pipeline` 现在本地 `collect_audio_plan(pipeline.composition())`
再解码混音 —— 音频执行本就归 host，core 只需暴露 composition。

### `FrameConsumer` / `RenderSessionHeader` 移出 core

| 之前 | 之后 |
|------|------|
| core `platform::frame_consumer`（trait + struct） | 文件删除 |
| `opencat_core::platform::frame_consumer::FrameConsumer` | `opencat_engine::consumer::FrameConsumer` / `opencat_web::consumer::FrameConsumer`（各 host 本地定义，形状一致） |
| `RenderSessionHeader` | 同上，各 host 本地 |

两个 host 的 trait 定义故意保持同形，以便 Skia 与 CanvasKit 两路消费同一个
`RenderFrame` 契约。core 不再调用 `FrameConsumer`。

### `AssetPathStore` 移到 engine

仅 engine 用作 `AssetId → 文件系统路径` 表；web 零引用。

| 之前 | 之后 |
|------|------|
| `opencat_core::resource::AssetPathStore` | `opencat_engine::resource::AssetPathStore`（同文件平移，逻辑不变） |

### 渲染管线 `BlobStore` 死字段

`RenderCtx.blob_store: Option<&dyn BlobStore>` 及 `render_frame_with_state` /
`render_frame` 的 `blob_store` 参数在生产路径上恒为 `None` 且从不 `.read()`，已删
除字段与透传。**`BlobStore` trait 本身保留**（web 的 BlobStore 结构仍 impl 它，
属两端共用，见下）。

### 死代码清理

`resource::protocol::{IndexedResourceProvider, ByteStore}`（仅注释/内部引用，无 host
使用）已删。`TypefaceRequest` 经核实是 `MapResourceProvider` 字体表的活用 key，
**保留**。

## Host 职责（打开 pipeline 前必须完成）

调用 `open_with_prepared_catalog` 前，host 自行完成：

1. **fetch / cache** 所有 `ResourceRequests` 声明的资源（image/video/audio/subtitle/Lottie）。
2. **build_catalog**：用 core 的纯 `probe::prepare::build_catalog(&requests, &bytes)`
   跑 image/video/Lottie probe，得到 `ResourceCatalog`。probe 失败允许 catalog 缺项。
3. **hydrate_captions**：host 拿到 SRT 字节后调 core 的纯 `hydrate_captions`，已有
   entries 不覆盖、缺失保持空。
4. **font database**：host 取字体字节，构建 `Arc<fontdb::Database>` 注入。

参考实现：`opencat-engine::pipeline::open_parsed_host_owned`（engine 的 host 链）。

## 保留项与理由

下列项在 `#11` 字面清单中出现过，但经探索确认是 **engine 与 web 两端共用** 的
抽象或纯派生数据。移走只会复制一份，违反“两端相同代码留 core”原则，故保留：

| 符号 | 位置 | 保留理由 |
|------|------|----------|
| `ResourceProvider` / `MapResourceProvider` / `ResourceLookup` / `TypefaceRequest` | core `resource/protocol.rs` | 两端 Skottie / CanvasKit Lottie bundle 的统一 `(path,name)→bytes` 协议，两端 hydrate 都用它 |
| `ExternalResourceManifest` / `build_manifest` / `collect_external_manifest` | core `resource/manifest.rs`、`parse/preflight.rs` | 纯派生元数据，两端都用它 hydrate provider |
| `materialize` / `host_bridge`（`hydrate_provider_from_bytes` 等） | core `resource/materialize.rs`、`host_bridge.rs` | 依赖上面的 manifest+provider，两端共用 |
| `AssetResolver` / `UrlFetcher` / `AssetSink` / `*Meta` | core `resource/resolver.rs` | 两端各自实现 trait，但 trait 定义与解析协议共用 |
| `preload_all` / `preload_lottie` | core `resource/preload.rs`、`preload_lottie.rs` | 纯编排，两端都调用 |
| `BlobStore` trait（仅 trait，不含已删死字段） | core `resource/blob_store.rs` | web 的 `BlobStore` 结构 impl 它，两端共用 |
| `VideoPreviewQuality` | core `media/types.rs` | 两端视频解码器共用的质量枚举 |
| `FrameConsumer` / `RenderSessionHeader`（trait 形状） | 现各 host 本地 | 形状保持一致；core 不再定义，但 trait 契约由两端共同维护 |

## 验证结果

- **opencat-core**：`cargo test -p opencat-core` → **540 passed, 0 failed**。
- **opencat-engine**：`cargo test -p opencat-engine` → **51 passed, 0 failed**
  （3 个 `inspect::browser_layout_tests::chromedriver_*` 失败为 **既有环境问题**：
  需 ChromeDriver + 已构建 JS 模块；在改动前的基线提交 `7ea92d3` 上同样失败，
  与本次重构无关）。
- **clippy**（core + engine）：`120` 条 warning，**低于基线的 `131`**（删代码后净
  减 11 条），未引入新 warning。
- **opencat-web**（wasm32）：`cargo build -p opencat-web --target wasm32-unknown-unknown`
  通过。
- **web Vitest**：`npx vitest run` → **36 passed (9 files)**，与 `#10` 基线一致。
- **web TypeScript**：`npx tsc --noEmit` 通过。
- **零引用核查**：全仓 grep 确认 `NoopAssetLoader`、`AssetLoader`、`AssetHandle`、
  `IndexedResourceProvider`、core `probe_all`、core `open_parsed`/`open_with_font_db`/
  `open`、`CompositionInfo.audio_plan`、core `frame_consumer`、渲染管线 `blob_store`
  字段 —— 全部归零（engine 内的 `open_parsed_host_owned` 是 engine 自己的 host 链
  函数，与已删的 core `open_parsed` 同名但无关）。

### SSIM 跨提交回归

`#11` 验收要求对“实现前固定提交”与当前提交以相同 renderer / 设计 / 字体 / 资源 /
分辨率 / 帧率抽帧做 SSIM，目标 1.0。

- 本次重构是纯结构改动：删除 core 执行 seam、移动类型、删除恒为 `None` 的死字段。
  core 的渲染内核（layout / shaping / Draw IR / FrameMediaPlan 生成）一行未动；
  engine/web 消费的 `RenderFrame` 契约字段不变。
- **SSIM = 1.0（逐帧，零回归）**。对照基线 `7ea92d3` 与当前 `311a58b`，ffmpeg SSIM
  （YUV All）对两个示例均为 `min=1.000000, avg=1.000000, max=1.000000`：
  - `examples/xhs-neo-brutalism.xml`：543 帧，输出文件字节级相同
    （md5 `51378d7b52351930b2546006f1197cb6`）。
  - `examples/profile-showcase.jsonl`：414 帧，输出文件字节级相同
    （md5 `68bd9da1b25afb4fc484300cda258462`）。
- 说明：skia 源码构建需联网（`git-sync-deps` / `fetch-gn` 取自 GitHub），本沙箱无
  网络，改用 `SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz` 缓存的预编译 skia
  二进制完成 release 构建 + 渲染；在此前提下回归数字有效。

## 实现提交

- 改动分布在 `opencat-core`、`opencat-engine`、`opencat-web`、`opencat` 四个 crate。
- 新增：`crates/opencat-engine/src/audio_plan.rs`、
  `crates/opencat-engine/src/resource/path_store.rs`（从 core 平移）。
- 删除：`crates/opencat-core/src/platform/frame_consumer.rs`、
  `crates/opencat-core/src/resource/path_store.rs`。
