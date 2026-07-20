# SSIM 渲染对齐流程

本文记录 OpenCat 渲染重构时使用的两层视觉回归方法：

1. 使用 `scripts/compare-ssim.sh` 对齐当前分支与 `main` 的原生
   `opencat` CLI 输出，证明 core/engine 重构没有改变整段视频。
2. 使用 ChromeDriver browser oracle 对齐 Web CanvasKit 与原生 engine 的
   指定帧，证明 WASM、资源加载和 Draw IR replay 与 engine 一致。

两层检查解决的问题不同。CLI 对齐是 native-vs-native 的整视频基线，Web
对齐是 web-vs-native 的端到端单帧验证。涉及 pipeline、资源协议、Draw IR、
字体、字幕、Lottie 或视频解码的改动，应该先通过第一层，再通过第二层。

## 1. 当前分支 CLI 对齐 main CLI

### 比较模型

- Reference：`main` checkout 构建的 release `opencat`，渲染目标 example。
- Test：功能分支 worktree 构建的 release `opencat`，渲染同一个 example。
- Metric：FFmpeg `ssim` filter，逐帧比较两个 MP4 的视频流。
- 目标：renderer/pipeline 等价重构通常要求 `min/avg/max = 1.000000`。

SSIM 只验证视频像素，不验证音频内容。脚本会打印 reference 的帧数和 stream
类型，音频协议或编码变更仍需额外测试。

### 环境约定

当前脚本的目录语义是：

- `MAIN_DIR` 是脚本所在 checkout 的仓库根目录。
- `WORKTREE_DIR` 当前固定为 `/home/solaren/Projects/opencat-issue-2`。
- Reference 位于 `$MAIN_DIR/out/<stem>.mp4`。
- Test 位于 `$WORKTREE_DIR/out/<stem>.mp4`。
- 报告写入 `$MAIN_DIR/out/compare-<stem>/`。

因此应从 main checkout 调用 main checkout 内的脚本。如果换了 worktree 路径，
先同步修改脚本中的 `WORKTREE_DIR`，并确认 reference/test 没有指向同一个文件。

依赖：

- Rust toolchain 与 release 构建依赖。
- `ffmpeg`、`ffprobe`。
- example 所需的本地字体和资源服务。
- 离线/沙箱环境设置
  `SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz`。

### 执行步骤

先在 main checkout 生成 reference：

```bash
cd /home/solaren/Projects/opencat
export SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz

cargo run --release --features profile -- examples/profile-showcase.jsonl
```

然后仍在 main checkout 运行比较脚本。脚本会自行构建并运行分支 worktree 的
CLI：

```bash
./scripts/compare-ssim.sh examples/profile-showcase.jsonl
```

另一个常用基线：

```bash
cargo run --release --features profile -- examples/xhs-neo-brutalism.xml
./scripts/compare-ssim.sh examples/xhs-neo-brutalism.xml
```

脚本内部执行四步：

1. 检查 main reference 是否存在并读取帧数/stream。
2. 在分支 worktree release 构建 `opencat`。
3. 用分支 CLI 渲染同一个 example。
4. 运行 FFmpeg SSIM，汇总逐帧 `min/max/avg`。

### 输出与判定

以 `profile-showcase` 为例：

```text
out/compare-profile-showcase/ssim_stats.txt
out/compare-profile-showcase/ssim_report.txt
```

- `ssim_stats.txt`：每一帧的 Y/U/V/All SSIM。
- `ssim_report.txt`：完整 FFmpeg 日志和全局摘要。
- `SSIM All = 1.000000`：像素完全一致。
- 低于 `1.000000`：先定位最低帧，再确认是预期 raster 差异还是回归。

脚本当前负责报告，不会因为 SSIM 低于阈值自动返回失败。提交前必须人工检查
汇总值；对纯重构，默认不接受“接近 1”代替 `1.000000`。

### 常见误差来源

- main reference 是旧文件，没有由当前 main commit 重新生成。
- main 和 worktree 使用了不同 example、字体、资源文件或资源服务。
- 两个视频的尺寸、fps、帧数、codec pipeline 不一致。
- profile example 的 `127.0.0.1:8080` 资源服务未启动或内容不同。
- 脚本从分支 worktree 运行，导致 `MAIN_DIR` 不是 main checkout。
- 输出路径残留，比较到了上一次运行的 MP4。

## 2. Web ChromeDriver 对齐 engine

### 比较模型

Browser oracle 位于
`crates/opencat-engine/src/inspect/web_frame_oracle_tests.rs`，对同一个
`(composition, frame)` 执行两条路径：

- Engine reference：原生 pipeline + Skia 输出 RGBA。
- Web test：WASM persistent pipeline 生成 Draw IR，Chrome 中由 CanvasKit replay
  并输出 RGBA。

测试把两份 RGBA 写成唯一命名的临时 PNG，再调用 FFmpeg `ssim`。失败时保留：

```text
target/opencat-web-oracle/<stem>-frame-<NNNN>/engine.png
target/opencat-web-oracle/<stem>-frame-<NNNN>/web.png
target/opencat-web-oracle/<stem>-frame-<NNNN>/diff.png
```

### 环境准备

```bash
cd /home/solaren/Projects/opencat-issue-2
export SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz
export CHROME_BIN=/usr/bin/google-chrome

cd crates/opencat-web/web
bun install
bun run build

cd ../../../web
bun install
cd ..
```

还需要：

- `chromedriver` 与 Chrome major version 匹配。
- `ffmpeg` 可执行。
- profile 资源服务监听 `127.0.0.1:8080`。
- 也可通过 `CHROMEDRIVER_BIN` 指定 binary，或通过
  `CHROMEDRIVER_URL` 复用已启动的 WebDriver。

Oracle 自带静态服务器，提供 Web facade、CanvasKit、字体和 assets；
`/assets-proxy/*` 会代理到 `127.0.0.1:8080`，与 Web fetch 层的 localhost
改写保持一致。

### 执行全部 oracle

```bash
export SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz
export CHROME_BIN=/usr/bin/google-chrome

cargo test -p opencat-engine --lib chromedriver_ -- --ignored --nocapture
```

执行单个用例可缩短调试闭环：

```bash
cargo test -p opencat-engine --lib \
  chromedriver_lottie_frame_matches_engine -- --ignored --nocapture
```

### 当前覆盖与阈值

| Oracle | 覆盖 | Frame | 当前 SSIM | 阈值 |
| --- | --- | ---: | ---: | ---: |
| Alipay | Draw IR、文本、icon、canvas | 0 | 0.996501 | 0.990 |
| Profile showcase | image、video、audio、transition | 0 | 0.998217 | 0.990 |
| Caption | Web subtitle preload、core hydrate | 0 | 0.992731 | 0.990 |
| Custom font | manifest font fetch、font DB injection | 0 | 0.998727 | 0.990 |
| Lottie | bundle preload、frame plan、Skottie replay | 125 | 0.986303 | 0.985 |

Lottie 使用单独阈值，是因为 native Skia 与 CanvasKit 在 aspect-fit 的硬边缘上
存在亚像素 raster 差异。它仍然要求几何、内容和帧时序一致，不能用更低阈值
掩盖资源缺失、缩放错误或空白帧。

### Chrome/OpenGL 说明

Oracle 固定使用：

```text
--use-gl=angle
--use-angle=swiftshader
--enable-unsafe-swiftshader
```

因此该流程不依赖 Chrome 的硬件加速，也不走 AMD GPU driver。此前 Lottie 的
`RuntimeError: unreachable` 发生在 Draw IR 编码阶段，根因是
`LottieRect.bundle_id` 未进入字符串表；缩小渲染则是 CanvasKit Skottie 被二次
scale。两者都不是 OpenGL/AMD 硬件加速问题。

### 新增或修改 Web 资源路径时

至少完成以下步骤：

1. 增加最小 fixture，确保缺少目标资源时 oracle 会明确变红。
2. 为 fixture 增加 `chromedriver_*_matches_engine` 测试并选择有内容的稳定帧。
3. 如 engine path 与 browser URL 不同，在 `web_source_for_oracle` 做最小转换。
4. 重新构建 `crates/opencat-web/web`；oracle 服务读取 `dist/`，不会自动重建。
5. 先跑单个红灯，再跑全部 `chromedriver_` oracle。
6. 失败时先看三张 artifact，再区分资源缺失、布局/时序、Draw IR 或 raster 差异。

## 3. 提交前建议顺序

```text
core/engine 单元测试
        ↓
main CLI vs branch CLI：整视频 SSIM = 1.000000
        ↓
构建 WASM/Web facade
        ↓
Web vs engine：ChromeDriver 单帧 oracle 全通过
        ↓
Web tests、TypeScript、cargo clippy、git diff --check
```

这套顺序能先证明 native ground truth 没变，再把剩余差异限定在 Web adapter、
浏览器资源 runtime 或 CanvasKit replay，避免同时调试两条变化中的渲染链路。

Web oracle 的实现和 issue #8 当前基线另见
[`docs/spec/web-frame-oracle.md`](spec/web-frame-oracle.md)。
