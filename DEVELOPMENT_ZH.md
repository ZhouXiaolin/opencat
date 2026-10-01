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

## Engine / Web 像素对齐（k3diff frame oracle）

逐帧对比 **原生 engine（Skia）** 与 **web（WASM + CanvasKit）**，用**硬像素指标（非 SSIM）**。

> **方法论。** SSIM 太粗：它把一帧压成一个结构相似度标量，会掩盖局部错位。对齐改用 k3diff 的逐帧指标——`mae`（R,G,B 平均绝对误差）、`maxd`（单通道最大差）、以及变化像素比例阶梯 `p2`/`p4`/`p8`/`p16`/`p32`/`p64`/`p128`（与 `tools/k3diff.py` 同阶梯）。原生实现 `compute_pixel_diff_rgba` 已与 `k3diff.py` 在同一对 PNG 上逐字节一致。整片原生对参考视频的比较仍以 `tools/k3diff.py` 为准（见 `out/`）。

### 流程

1. Engine：`DefaultPipeline::render_frame` → RGBA（基准）
2. Headless Chrome 经 ChromeDriver 打开 `web/test-oracle.html`
3. Web：`open_design` → `prepareCatalogVideoSources` → 注入视频帧 →
   `build_frame_ir` → CanvasKit 绘制 → `readPixels` → RGBA
4. `compute_pixel_diff_rgba`（k3diff 语义，不经过 ffmpeg）
5. 门限（编译期 `PixelGate { max_mae, max_p8 }`）：**mae ≤ 1.0、p8 ≤ 0.02**（静态帧）；**mae ≤ 2.0、p8 ≤ 0.03**（含活跃视频帧）。`p8` 主要由字形反走样构成——Skia 与 CanvasKit 在文字边缘的覆盖率差约 1% 像素，且无位置偏移。

失败帧产物：

```text
target/opencat-web-oracle/<stem>-frame-NNNN/{engine,web,diff}.png
```

### 直接与参考视频对齐

engine-vs-web oracle 只能发现 engine/web 的**分歧**，发现不了两者共有的问题（共享解析/渲染核心的 bug 会照样通过）。要拿到更可信的对齐信号，把 wasm headless 渲染**直接和原始参考渲染**（hyperframes-launch `k3-promo.mp4`）比对：

```bash
./target/release/opencat-web-compare examples/k3-promo.xml \
  --out-dir out/k3-web-vs-ref --interval-secs 1 \
  --reference /home/solaren/Projects/hyperframes-launches/k3-promo/k3-promo.mp4
```

`--reference <video>` 用 ffmpeg 解出采样帧，用同一套 k3diff 指标衡量 web 渲染；summary 会记录 `reference:`，两种基准不会混淆。原生 engine 对同一参考的 `mae`（`tools/k3diff.py`）是下界——web 的 `mae` 与之相差仅百分之几，说明 wasm 通路自身没有引入明显误差。

### 前置条件

| 依赖 | 说明 |
|------|------|
| Chrome + ChromeDriver | 主版本一致；可自动探测，或设 `CHROME_BIN` / `CHROMEDRIVER_BIN` |
| FFmpeg | `PATH` 中有 `ffmpeg`（解码参考视频） |
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

> 说明：k3diff 门限（`PixelGate { max_mae, max_p8 }`）是 `web_frame_oracle.rs` 内的编译期常量。`opencat-web-compare` 以 CLI 参数暴露（`--max-mae`、`--max-maxd`、`--frac-threshold`、`--max-frac`），默认值对齐 `STRICT_GATE`。

### 代码位置

| 路径 | 作用 |
|------|------|
| `crates/opencat-engine/src/inspect/browser.rs` | ChromeDriver harness、静态服务、k3diff 像素指标 |
| `crates/opencat-engine/src/inspect/tests/web_frame_oracle.rs` | Oracle 用例 |
| `web/test-oracle.html` | 浏览器入口：open design、prepare 视频、绘制 IR |
| `crates/opencat-web/web/src/media/video-frame-injector.ts` | `prepareCatalogVideoSources` + inject |
| `crates/opencat-web/web/dist/` | 构建产物，挂载在 `/wasm/` |

### Host 视频契约（web）

`open_design` / `openDesign` 之后、调用 `injectVideoFramesForRender` **之前**，host 必须执行 `prepareCatalogVideoSources(catalogJson)`。否则 WebCodecs 收不到源，所有 `ImageRef::VideoFrame` 会画成空白（大视频区域 `mae`/`p8` 会断崖上升）。

---

## 效果 Lambda DSL（`script::effects_lambda`）

逐像素效果以**只被编译、从不执行的 JS lambda** 表达：Rust 用 oxc 解析 lambda 源码，逐条白名单校验，按 uniform spec 类型推断，然后原生重建计算。

- **后端自动派发。** 只用 SKSL 可表达 op 的 lambda 降为 SKSL、走 `RuntimeEffect` 管线；使用精确整数语义（`h01`/`imul`/`byte`）的 lambda 派发到 f64 AST 解释器（rayon 按行并行）、走 `GeneratedImage` 路径。spec 可强制后端（`backend: 'cpu' | 'sksl'`）；CpuOnly lambda 强制 `'sksl'` 是编译错误。scan 类 lambda（`spec.kind: 'scan'`）只在 CPU 解释器**就地单线程**运行——强制 `'sksl'` 是编译错误。
- **像素缓冲永不跨进 JS，Rust 也不固化任何效果算法。** lambda 只见 `uv`/`get`/`rect`/`u.<name>` 与 `child.eval(pos)`；offscreen surface 就是脚本侧 render target——`surface.runEffect`（pixel 类重绘）、`surface.scanPass`（scan 类就地扫描，首参 `get(dx,dy)` 读 in-progress 缓冲，遍历顺序由 executor 拥有）、`surface.bake(key)`（注册为帧级生成图像）。lambda 以 `{__opencatShader:'surface', id}` child 直采其它 surface（session 级，不上 wire）；SKSL 绘制路径遇 surface child 会报错并引导先 bake。
- JS facade：`CK.Effect.fromLambda(fn, spec)` 与 surface render-target 方法 —— 编写指南、白名单与拒绝项见 `skill/references/canvaskit.md`。
- 模块地图：`crates/opencat-core/src/script/effects_lambda/{parse,program,stdlib,lower_sksl,interp,mod}.rs`。
- k3-promo 场景 H（`examples/k3-promo.xml`）是 render-target 通路的参考迁移：mask + chamfer 距离场完全由 JS 编写——seed pixel lambda 经 `surface.runEffect` 采样离屏 surface child，再用两个 chamfer 扫描 lambda 经 `surface.scanPass`（forward/backward）构建进 `k3dis-field` surface，绘制 lambda 以 `{__opencatShader:'surface'}` child 直采——不再每帧烘焙上 wire。全片渲染与迁移前（field 在 Rust 中）**逐字节一致**（同一 mp4 md5）。
- `examples/xxx.xml` 是 SKSL 后端的参考迁移：slide-2 折射玻璃从手写 SKSL 数组迁到 `REFRACT_LAMBDA` + subtree picture child，全片 360 帧与原实现逐位一致。手写 SKSL 返回预乘色而 lambda 返回 straight 色（codegen 自动预乘），故 lambda 返回前把 rgb 除回 alpha（有下界 ≥0.96）。
- 测试：`cargo test -p opencat-core effects_lambda`（白名单拒绝、后端派发、scan 链 vs chamfer oracle 逐位全等、render target bindings 端到端、kind 互斥拒绝）与 `cargo test -p opencat-engine lambda`（SKSL raster 端到端、SKSL vs 解释器一致性、generated child 本地空间采样）。
