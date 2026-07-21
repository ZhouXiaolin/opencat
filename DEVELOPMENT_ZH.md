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

## SSIM Engine/Web 像素对比

确保 Rust Skia 渲染输出与浏览器 CanvasKit WASM 渲染输出像素一致。

### 原理

1. 引擎用 `DefaultPipeline::render_frame` 渲染指定帧 → RGBA
2. 浏览器通过 ChromeDriver 打开 `web/test-oracle.html`，CanvasKit 解析同源数据 → `readPixels` → RGBA
3. `compute_ssim_rgba` 将两幅 RGBA 写入临时 PNG，调用 `ffmpeg ssim` 计算结构相似度
4. 阈值：普通帧 ≥ 0.99，含视频解码的帧 ≥ 0.97

### 测试

```bash
# 单帧 oracle（指定示例 + 帧号）
cargo test -p opencat-engine --lib -- --ignored web_frame_oracle

# 多帧采样（默认 0–413 帧，步进 10）
cargo test -p opencat-engine --lib -- --ignored profile_showcase_multi_frame_oracle

# CLI 工具：自定义间隔、阈值
cargo build --bin opencat-web-compare --release
./target/release/opencat-web-compare examples/profile-showcase.jsonl \
  --out-dir out/compare-mp4 \
  --interval-secs 0.5
```

位置：`crates/opencat-engine/src/inspect/`

| 文件 | 作用 |
|------|------|
| `browser.rs` | ChromeDriver + 静态服务 + `compute_ssim_rgba` |
| `tests/web_frame_oracle.rs` | 单帧/多帧 oracle 测试 |
| `tests/tailwind_layout/mod.rs` | Tailwind ↔ Taffy 布局对齐测试 |

### 环境变量

| 变量 | 用途 | 默认 |
|------|------|------|
| `CHROME_BIN` | Chrome 可执行路径 | 自动探测 |
| `CHROMEDRIVER_BIN` | chromedriver 路径 | 自动探测 |
| `CHROMEDRIVER_URL` | 远程 WebDriver 端点 | 无（使用本地） |
| `MIN_SSIM` | 严格 SSIM 阈值 | 0.99 |
| `VIDEO_MIN_SSIM` | 视频帧 SSIM 阈值 | 0.97 |
