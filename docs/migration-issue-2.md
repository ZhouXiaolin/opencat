# 迁移说明：core 资源契约收敛（issue #2 / #11）

> 状态：已完成。本文记录 #2 系列最后一个阶段（#11）的 breaking API 变更、host
> 职责、被删符号的替代路径，以及“哪些看似在清单里、实则留 core”的判断依据。
>
> 适用范围：`opencat-core` / `opencat-engine` / `opencat-web` 三端。实现分布在
> `#3`–`#10`（已关闭）与本阶段 `#11`。

## 核心观念

> **core 尽可能精简，下载 / 视频解码 / 编码 / 音频调度等执行逻辑交各端；但两端
> 确实相同的代码仍留在 core。**

这是本次“删什么 / 留什么”的判据。下载、存储、预加载、解码质量和 seek 策略已经
由 engine/web 的具体模块直接承担，不再为了共享少量代码而在 core 建 host trait。
core 只保留资源声明、稳定 ID、纯 metadata probe、时间映射和渲染派生语义。

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
除字段与透传。core `BlobStore` trait 同样删除；web 保留自己的具体 `BlobStore`。

### 死代码清理

以下 core 执行模块及兼容入口已删除：

- `resource::{resolver, preload, blob_store, host_bridge, manifest, materialize, preload_lottie, protocol}`
- `media::{codec, export, preview, seek}` 与 `platform::{video, frame_consumer}`
- `runtime::session::RenderSession` 及旧 `pipeline::frame::render_frame` 包装

engine 未被消费的 resource provider 构建链直接删除。web 直接用端侧 `BlobStore` 和
稳定 `AssetId` 约定提供 CanvasKit Lottie 依赖，不再经过 core provider/manifest。

core 解析不再读取 SRT、检查字体文件是否存在或读取/下载字体。解析只产生路径和 URL
声明；字幕 hydrate、字体读取和字体数据库装配由 host 完成。

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

core 目前保留的资源/端侧相关 seam 都有稳定语义，或已有两个真实 adapter：

| 符号 | 位置 | 保留理由 |
|------|------|----------|
| `ResourceRequests` / `AssetId` | core `probe/catalog.rs`、`ir/asset_id.rs` | 跨端声明和稳定身份，不包含获取、存储或执行策略 |
| `parse_lottie_meta` / `scan_lottie_dependencies` | core `resource/lottie.rs` | 对已注入 JSON 的纯解析，不读取或下载资源 |
| `VideoFrameRequest` / `VideoFrameTiming` | core `media/types.rs` | 只表达可见性和 composition time 到 media time 的映射；质量、尺寸、seek、decode 在 host |
| `JsContext` | core `script/js_context.rs` | engine RQuickJS 与 web 浏览器是两个真实 adapter；core 只承载一致的脚本调度语义 |
| `DefaultPipeline` 内部帧状态 | core `pipeline/default.rs` | 渲染派生实现细节，不再作为 public `RenderSession` 暴露 |

## 验证结果

- **opencat-core**：`cargo test -p opencat-core` → **522 passed, 0 failed, 1 ignored**。
- **opencat-engine**：`cargo test -p opencat-engine --lib` → **56 passed, 0 failed,
  7 ignored**。Tailwind layout parity suite 已独立到 `inspect/tests/tailwind_layout`：
  由结构化清单固定 Tailwind v4.2.2 的 71 组、505 个候选，CSS 编译依赖由
  `opencat-engine/testsupport/bun.lock` 管理，不再解析 812 KB 的上游 TypeScript
  测试源码。Chrome viewport 使用 CDP 精确设置，并与 engine 共用 Noto Sans SC
  字体契约。
- **clippy**：`cargo clippy -p opencat-core -p opencat-engine --all-targets` exit 0；
  仍有既有 warning，本轮不做无关 warning 清理。
- **desktop / engine / wasm32**：`cargo check -p opencat -p opencat-engine` 与
  `cargo check -p opencat-web --target wasm32-unknown-unknown` 通过。
- **web Vitest**：`bun run test` → **36 passed (9 files)**。
- **web build**：`bun run build`（TypeScript + Vite）通过。
- **最新 SSIM 回归**：使用 main 的参考视频与当前 worktree 最新渲染逐帧比较，均为
  `SSIM All = 1.000000`：
  - `examples/xhs-neo-brutalism.xml`：543 帧，1280x720。
  - `examples/profile-showcase.jsonl`：414 帧，1280x720；资源来自
    `/home/solaren/Documents/resources` 的本地 8080 静态服务。
- **零引用核查**：全仓 grep 确认 `NoopAssetLoader`、`AssetLoader`、`AssetHandle`、
  `IndexedResourceProvider`、core `probe_all`、core `open_parsed`/`open_with_font_db`/
  `open`、`CompositionInfo.audio_plan`、core `frame_consumer`、渲染管线 `blob_store`
  字段 —— 全部归零（engine 内的 `open_parsed_host_owned` 是 engine 自己的 host 链
  函数，与已删的 core `open_parsed` 同名但无关）。

### 回归命令

```bash
ffmpeg -i /path/to/main/out/xhs-neo-brutalism.mp4 \
  -i out/xhs-neo-brutalism.mp4 \
  -lavfi "ssim=stats_file=/tmp/opencat-xhs-ssim.log" -f null -
```

profile 样本使用同样命令，先在资源目录启动 `python3 -m http.server 8080`，再运行
`./target/release/opencat examples/profile-showcase.jsonl`。

## 实现范围

- 改动分布在 `opencat-core`、`opencat-engine`、`opencat-web` 三个 crate。
- engine 新增 host-owned `audio_plan`、`media/seek` 和 `resource/path_store`。
- core 删除资源执行、媒体执行、平台视频与公开 session 模块；web/engine 删除相应
  pass-through adapter，并直接持有各自的具体实现。
