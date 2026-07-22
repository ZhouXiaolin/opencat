# 开发指南

## Tailwind + Taffy + ChromeDriver 布局对齐

确保 Rust Taffy 布局引擎与真实 Chrome 浏览器的 CSS 布局行为一致。

### 原理

每条 fixture 是一个带 `data-oc-id` 属性的 HTML 片段，Tailwind class 写在 `class=""` 中：

1. 收集所有 class name → 调用 `@tailwindcss/node`（通过 bun）编译出 CSS
2. 生成完整 HTML 文档，在 ChromeDriver 中打开
3. 通过 WebDriver `getBoundingClientRect()` 获取每个节点的位置尺寸
4. Rust 端用相同 class name 走 `parse_class_name` → Taffy 布局 → `collect_frame_layout_rects`
5. 对比两边的 rect 集合：id 必须完全匹配，x/y/width/height 在容差内（文本行高 2px，其他 1px）

### 测试

```bash
# 自动生成 fixture 覆盖 Tailwind v4.2.2 的所有 layout utility（71 组，505 个候选）
cargo test chromedriver_tailwind_extended_flex_layout_matches_taffy

# 手写集成 fixture（复杂多 utility 组合）
cargo test chromedriver_tailwind_layout_matches_taffy

# 检查 fixture 生成器是否覆盖了所有 utility
cargo test generated_layout_fixture_templates_cover_utilities_manifest
```

位置：`crates/opencat-engine/src/inspect/tests/tailwind_layout/`

依赖：ChromeDriver、Chrome、`crates/opencat-engine/testsupport/` 中的 bun 依赖。

---

## Engine / Web 像素对齐（SSIM frame oracle）

逐帧对比 **原生 engine（Skia）** 与 **web（WASM + CanvasKit）**，用 SSIM 衡量结构相似度。

### 流程

1. Engine：`DefaultPipeline::render_frame` → RGBA（基准）
2. Headless Chrome 经 ChromeDriver 打开 `web/test-oracle.html`
3. Web：`open_design` → `prepareCatalogVideoSources` → 注入视频帧 →
   `build_frame_ir` → CanvasKit 绘制 → `readPixels` → RGBA
4. `compute_ssim_rgba` 调用 `ffmpeg ssim`
5. 阈值：**≥ 0.99**（静态 / 管线帧），**≥ 0.97**（含活跃视频的帧）

失败帧产物：

```text
target/opencat-web-oracle/<stem>-frame-NNNN/{engine,web,diff}.png
```

### 前置条件

| 依赖 | 说明 |
|------|------|
| Chrome + ChromeDriver | 主版本一致；可自动探测，或设 `CHROME_BIN` / `CHROMEDRIVER_BIN` |
| FFmpeg | `PATH` 中有 `ffmpeg`（ssim filter） |
| Node / npm（或 bun） | 构建 web facade |
| Dev app 依赖 | `cd web && bun install`（或 npm）— oracle 静态服务需要 CanvasKit + `web-demuxer` |
| **:8080** 媒体服务 | 如 `examples/profile-showcase.jsonl` 会请求 `http://127.0.0.1:8080/mp4/...` |

本地媒体示例：

```bash
# 在包含 mp4/ png/ mp3/ 的目录
python3 -m http.server 8080
```

### 构建 web facade（改过 JS/WASM 后必须重编）

```bash
cd crates/opencat-web/web
npm run build          # wasm-pack + vite + types；会把 web-demuxer.wasm 拷进 dist/
# 仅 TS 变更时：
# npm run build:lib && npm run build:types
```

静态路由映射：

- `/test-oracle.html` → `web/test-oracle.html`
- `/wasm/*` → `crates/opencat-web/web/dist/*`（含 worker 与 `web-demuxer.wasm`）
- `/canvaskit/*` → `web/node_modules/canvaskit-wasm/bin/full/*`
- `/assets/*`、`/fonts/*` → 仓库资源

### 运行测试

Oracle 测试均为 `#[ignore]`（依赖 ChromeDriver + 已构建 facade），必须加 `--ignored`。

```bash
# 冒烟：profile-showcase 第 0 帧（无视频）
cargo test chromedriver_profile_showcase_frame_matches_engine \
  --package opencat-engine --lib -- --ignored --nocapture

# 全量多帧：0–413 步进 10（覆盖 scene2/3 视频）
cargo test chromedriver_profile_showcase_all_frames_matches_engine \
  --package opencat-engine --lib -- --ignored --nocapture

# 其它单帧 oracle（按名称过滤）
cargo test chromedriver_ --package opencat-engine --lib -- --ignored --nocapture
# 包含：
#   chromedriver_alipay_finance_homepage_first_frame_matches_engine
#   chromedriver_caption_frame_matches_engine
#   chromedriver_custom_fonts_frame_matches_engine
#   chromedriver_lottie_frame_matches_engine
#   chromedriver_color_emoji_frame_matches_engine
```

### CLI：自定义间隔 / 输出目录

```bash
cargo build --bin opencat-web-compare --release
./target/release/opencat-web-compare examples/profile-showcase.jsonl \
  --out-dir out/compare-mp4 \
  --interval-secs 0.5
```

### 环境变量

| 变量 | 用途 | 默认 |
|------|------|------|
| `CHROME_BIN` | Chrome 可执行路径 | 自动探测 |
| `CHROMEDRIVER_BIN` | chromedriver 路径 | 自动探测 |
| `CHROMEDRIVER_URL` | 远程 WebDriver（不启本地） | 未设置 |
| `MIN_SSIM` | 严格 SSIM（当前代码中为常量 `0.99`） | `0.99` |
| `VIDEO_MIN_SSIM` | 视频帧 SSIM（当前代码中为常量 `0.97`） | `0.97` |

> 说明：`web_frame_oracle.rs` 内阈值目前是编译期常量；表中 env 为工具链约定/预留。

### 代码位置

| 路径 | 作用 |
|------|------|
| `crates/opencat-engine/src/inspect/browser.rs` | ChromeDriver harness、静态服务、SSIM |
| `crates/opencat-engine/src/inspect/tests/web_frame_oracle.rs` | Oracle 用例 |
| `web/test-oracle.html` | 浏览器入口：open design、prepare 视频、绘制 IR |
| `crates/opencat-web/web/src/media/video-frame-injector.ts` | `prepareCatalogVideoSources` + inject |
| `crates/opencat-web/web/dist/` | 构建产物，挂载在 `/wasm/` |

### Host 视频契约（web）

`open_design` / `openDesign` 之后、调用 `injectVideoFramesForRender` **之前**，host 必须执行 `prepareCatalogVideoSources(catalogJson)`。否则 WebCodecs 收不到源，所有 `ImageRef::VideoFrame` 会画成空白（大视频区域 SSIM 会断崖下跌）。
