<div align="center">

# OpenCat

### 用 XML 写视频，Rust 渲染，一行命令出 MP4。

<p align="center">
  <img alt="Rust" src="https://img.shields.io/badge/Rust-edition_2024-ce422b?style=flat-square" />
  <img alt="Skia" src="https://img.shields.io/badge/Skia-GPU-24c8db?style=flat-square" />
  <img alt="FFmpeg" src="https://img.shields.io/badge/FFmpeg-encode-2c5282?style=flat-square" />
  <img alt="WASM" src="https://img.shields.io/badge/WASM-CanvasKit-805ad5?style=flat-square" />
  <img alt="license" src="https://img.shields.io/badge/license-MIT-2f855a?style=flat-square" />
  <img alt="Stars" src="https://img.shields.io/github/stars/ZhouXiaolin/opencat?style=flat-square&color=805ad5" />
  <a href="https://zread.ai/ZhouXiaolin/opencat"><img alt="zread" src="https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README_ZH.md">中文</a>
</p>

  <video width="60%" controls autoplay loop muted playsinline src="https://github.com/user-attachments/assets/e3263ec6-a36c-4b73-ba4c-c0e6b64b86ee"></video>

</div>

XML 定义场景、动画与布局，Skia GPU 加速渲染，FFmpeg 编码输出 MP4 —— 跨机器、跨平台、确定性一致。告别 Chromium 快照、Puppeteer 和笨重的 Web 渲染管线。

写一个视频，就是写一个 XML 文件（下面是 [`examples/k3-promo.xml`](examples/k3-promo.xml) 的场景 A，即上方示例）：

```xml
<opencat width="1920" height="1080" fps="30" duration="16.4667">
  <fonts default="k3-sans">
    <font id="k3-sans" family="Inter" path="assets/Inter-Regular.ttf" role="sans" />
  </fonts>
  <template name="stage-canvas"><canvas id="$id" class="absolute inset-0 w-[1920px] h-[1080px]"></canvas></template>

  <div id="root" class="relative w-[1920px] h-[1080px] bg-[#010101] overflow-hidden">
    <div id="sceneA" class="absolute inset-0 z-[2] opacity-0">
      <div id="azoom" class="absolute inset-0 [transform-origin:50%_50%]">
        <div id="afield" class="absolute inset-0"><stage-canvas id="afield-canvas" /></div>
        <div id="atitlegrp" class="absolute left-[755px] top-[444px] w-[467px] h-[185px] opacity-0">
          <div id="a-sel" class="absolute inset-0 border-[1px] border-white/25"></div>
          <text id="aline1" class="absolute text-[#d8d8da] text-[63.6426px] tracking-[-0.8px] leading-none">Every token</text>
        </div>
      </div>
    </div>
  </div>
  <script>
    var T = ctx.time;                       // 秒，覆盖整个 16.4667s
    ctx.getNode('sceneA').opacity(T >= 0.75 && T < 4.62 ? 1 : 0);
    if (T >= 0.75 && T < 4.62) {
      ctx.timeline()
        .set('azoom', { scale: 1.07 }, 0.933)
        .to('azoom', { scale: 0.9417, duration: 3.667, ease: 'none' }, 0.933)
        .fromTo('atitlegrp', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power1.inOut' }, 3.03);
    }
  </script>
</opencat>
```

```bash
cargo run --bin opencat -- examples/k3-promo.xml
```

MP4 已生成。不需要浏览器、不需要截图、不需要任何图形界面。


## Why OpenCat

| | OpenCat | Remotion / HyperFrames |
|---|---------|------------------------|
| **渲染方式** | Rust 原生 GPU (Skia) | Chrome snapshot |
| **渲染速度** | 10x | 基准 |
| **部署环境** | 任意环境 / 纯 CLI | 需要 Chromium |
| **动画** | 自研 GSAP 兼容 API（覆盖 80%+） | 直接使用 GSAP / anime.js |
| **浏览器渲染** | WASM + CanvasKit | 原生 |
| **确定性输出** | ✅ 跨机器一致 | ❌ |
| **AI 友好** | XML/JSONL 声明式 | JSX/HTML，较复杂 |

Remotion 复用了 Web 生态，但 Chrome snapshot 的先天缺陷无法绕过 —— GPU 受限、内存开销大、帧率上不去、部署必须带 Chromium。OpenCat 原生调用 GPU 和 FFmpeg，性能上限不在一个量级。

## Capabilities

### 声明式动画，GSAP 级表达

```js
// 声明式 timeline —— examples/k3-promo.xml 场景 A
ctx.timeline()
  .set('azoom', { scale: 1.07 }, 0.933)
  .to('azoom', { scale: 0.9417, duration: 3.667, ease: 'none' }, 0.933)
  .to(['afield', 'afieldp'], { opacity: 0.35, duration: 0.4333, ease: 'power1.inOut' }, 1.9667)
  .fromTo('apwrap', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power1.inOut' }, 3.03);

// GSAP 形态的逐字符文本：splitText 拆分后按 power2.out 曲线归位
var parts = ctx.splitText('otxt', { type: 'chars' });
var tl = ctx.timeline();
parts.forEach(function (p, i) {
  tl.set(p, { opacity: 0 }, 7.35)
    .set(p, { opacity: 1 }, 7.5333)
    .fromTo(p, { x: (i - 6.5) * 21 }, { x: 0, duration: 0.43, ease: 'power2.out' }, 7.5663);
});
```

### 多场景 + 转场

```xml
<tl id="main-tl">
  <div id="scene1" duration="4">...</div>
  <transition from="scene1" to="scene2" effect="fade" duration="0.6" />
  <div id="scene2" duration="4">...</div>
</tl>
```

内置 fade / slide / wipe / clock_wipe / iris / light_leak，支持自定义 GLSL 着色器。

### XML 模板 —— 带插槽和变量的可复用组件

用 `<template>` 定义组件，用 `$variable` 参数化，用 `<slot>` 组合内容。`k3-promo` 把六个公共构件从节点树中提出来——解析一次，渲染前展开为普通节点：

```xml
<template name="stage-canvas"><canvas id="$id" class="absolute inset-0 w-[1920px] h-[1080px]"></canvas></template>
<template name="cdot"><div id="$id" class="absolute w-[$s] h-[$s] bg-[$c]"></div></template>
<template name="agent-pill"><div id="$id" class="absolute opacity-0"><div id="$id-box" class="px-[14px] py-[10px] border-[1.5px] border-[#dcdcde]">…</div></div></template>

<!-- 调用点只传差异参数 -->
<stage-canvas id="afield-canvas" />
<cdot id="a-tl" s="13px" c="#d8d8da" />
<agent-pill id="ap1" t="Agentic" />
```

模板在解析时展开 —— 零运行时开销、完全可组合、支持嵌套。`<slot>` 从调用点注入内容：

```xml
<template name="card">
  <div class="w-[400px] rounded-xl bg-$bg p-6"><h2 class="text-$titleColor">$title</h2><slot name="body" /></div>
</template>
<card bg="white" titleColor="gray-900" title="Hello">
  <slot name="body"><p class="text-gray-500">卡片内容。</p></slot>
</card>
```

### 浏览器内 WASM 渲染

```ts
import { initWasm, preloadAssets, getRendererOrThrow, exportMp4 } from 'opencat-web';

await initWasm();
const catalog = await preloadAssets(xmlContent);
const renderer = getRendererOrThrow();
renderer.build_frame(xmlContent, frameNumber, canvas, catalog);
await exportMp4({ /* ... */ });
```

纯 WASM + CanvasKit，无需服务器。

### HTML in Canvas — Subtree Texture Sampling

`<canvas>` 节点的子树内容可实时纹理化，传入逐像素效果做后处理。效果用 **JS lambda** 表达（只被编译、从不被执行——Rust 解析后自动生成 SKSL 或派发到 CPU 渲染，自身不固化任何效果算法）。离屏 surface（`ctx.createSurface`）同时是**render target**，承载"构建一次、逐帧采样"的数据。`examples/k3-promo.xml` 场景 H 的溶解距离场完全在 JS 里构建：

```js
// 1) 文字锁排一次性栅格化进离屏 surface
var ox = ctx.createSurface('k3dis-off', 1920, 1080);
ox.fillStyle = '#fff'; ox.fillText('K3. Now Open', 962, 586);

// 2) 用 pixel 类 lambda 播种 mask + 距离场，以 surface child 采样那个 surface
var field = ctx.createSurface('k3dis-field', 1460, 222);
field.runEffect(
  (uv, src) => {
    const c = src.eval(uv + [232, 430]);
    const m = byte(c.a * 255 + 0.5);
    const d = m > 120 ? 0 : 3000;                      // R=mask, G/B=dist 低/高字节
    return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1];
  }, null, null, [{ __opencatShader: 'surface', id: 'k3dis-off' }]);

// 3) chamfer 扫描——scan 类 lambda 就地遍历；get(dx,dy) 读 in-progress 缓冲，
//    遍历顺序由 executor 拥有（forward/backward）
field.scanPass((get) => {
  const at = (n) => byte(n.g * 255 + 0.5) + byte(n.b * 255 + 0.5) * 256;
  const c = get(0, 0), d = at(c);
  const m = min(min(d, at(get(-1, 0)) + 3), min(at(get(0, -1)) + 3,
              min(at(get(-1, -1)) + 4, at(get(1, -1)) + 4)));
  return [c.r, m % 256 / 255, floor(m / 256) / 255, 1];
}, 'forward');

// 4) 每帧：lambda 直接采样 field surface——无需每帧烘焙
var DIS = CK.Effect.fromLambda((uv, field, u) => {
  const c = field.eval(uv);
  const d = (byte(c.g * 255 + 0.5) + 256 * byte(c.b * 255 + 0.5)) / 3;
  /* scramble 噪点 · dust · glow → straight RGBA */
}, { uniforms: [['x0','float'], ['f','float'], ['ep','float'], ['pa','float4']] });
```

画布内 HTML 子树的任意布局、图片、文本、视频 → 纹理 → 着色器 → 输出。像素缓冲永不跨 JS 桥，Rust 侧只有**通用执行器**：`surface.runEffect` 用 pixel 类 lambda 重绘，`surface.scanPass` 用 scan 类 lambda 就地扫描，`surface.bake(key)` 把像素注册为帧级图像供着色器采样。需要原始 SKSL 控制力时仍可用 `CK.RuntimeEffect.Make(sksl)` 手写着色器。参考：`examples/k3-promo.xml` 场景 H；指南：`skill/references/canvaskit.md`。

### 更多能力

- **Tailwind 式布局**：`class="flex items-center justify-center gap-4"`
- **音频混音**：多轨道，场景级挂载，自动混音输出
- **字幕引擎**：SRT 解析，跨场景持久化显示
- **Lucide 图标库**：2000+ 开箱即用
- **确定性渲染**：`value = f(time)`，跨机器一致

## Quick start

```bash
# 渲染 MP4（上方示例）
cargo run --bin opencat -- examples/k3-promo.xml

# 桌面播放器实时预览（macOS / Windows）
cargo run --bin opencat-see -- path/to/input.xml
```

> Web (WASM)：`cd crates/opencat-web/web && npm run build`，浏览器需要 `Cross-Origin-Isolated` 环境。

### Engine / Web 像素对齐（k3diff 像素指标）

用 ChromeDriver + 硬像素指标（`mae`/`maxd`/`p8`，非 SSIM）对比原生 Skia 与 WASM CanvasKit 逐帧输出。也可用 `--reference` 把 web 渲染直接和原始参考视频比对。完整步骤见 **[开发指南](DEVELOPMENT_ZH.md#engine--web-像素对齐k3diff-frame-oracle)**。

```bash
# 1) examples 依赖的媒体（profile-showcase 会请求 http://127.0.0.1:8080/...）
#    将 mp4/png/mp3 目录用静态服务挂到 :8080

# 2) 构建 web facade（wasm + JS + web-demuxer.wasm → dist/）
cd crates/opencat-web/web && npm run build && cd -

# 3) 多帧 oracle（0–413，步进 10）— 需要 Chrome + chromedriver + ffmpeg
cargo test chromedriver_profile_showcase_all_frames_matches_engine \
  --package opencat-engine --lib -- --ignored --nocapture
```

失败帧产物：`target/opencat-web-oracle/<stem>-frame-NNNN/{engine,web,diff}.png`。

<details>
<summary><strong>Architecture</strong></summary>

```
XML ──→ Taffy 布局 ──→ Skia 渲染 ──→ 编码 → MP4
              ↑
         QuickJS 动画脚本
```

**双管道：** Rust (GPU) + FFmpeg → MP4 | WASM + CanvasKit (WebGL) → Canvas / MP4

**增量渲染：** Resolve → Layout → Display，Merkle Tree 跳过不变子树 + Scene Snapshot 零计算复用。

```
opencat
├── crates/
│   ├── opencat-core/      # 布局 (Taffy)、文字 (cosmic-text)、字体
│   ├── opencat-engine/    # Skia 渲染、FFmpeg 编码、QuickJS 脚本
│   ├── opencat-web/       # WASM：浏览器端渲染 + 导出
│   └── opencat/           # CLI 入口
├── web/                   # Web 视频编辑器
└── examples/                  # 示例 XML 文件
```

</details>

## 编译指南

### 前置依赖

- **Rust 工具链**（edition 2024）。通过 [rustup](https://rustup.rs/) 安装：
  ```bash
  rustup install nightly  # edition 2024 需要 nightly（截至 2025 年初）
  ```

- **FFmpeg 开发库**（用于 MP4 编码）。[`ffmpeg-next`](https://crates.io/crates/ffmpeg-next) 通过 `pkg-config` 自动查找 FFmpeg。

  <details open>
  <summary><strong>Linux（Ubuntu / Debian）</strong></summary>

  ```bash
  sudo apt install \
    libavcodec-dev libavformat-dev libavutil-dev \
    libavfilter-dev libswscale-dev
  ```

  最低版本要求：FFmpeg 6.x。验证：

  ```bash
  ffmpeg -version
  ```

  当前环境已验证：**FFmpeg 7.1.1**，dev 包已全部安装。

  </details>

  <details>
  <summary><strong>macOS</strong></summary>

  ```bash
  brew install ffmpeg
  ```

  Homebrew 安装后需要用 `FFMPEG_DIR` 指定 ffmpeg 的 lib 路径：

  ```bash
  # Apple Silicon（M1/M2/M3/M4）
  export FFMPEG_DIR=/opt/homebrew

  # Intel Mac
  export FFMPEG_DIR=/usr/local
  ```

  > 如果 `pkg-config` 不能自动找到 ffmpeg lib，可以用 `FFMPEG_DIR` 来指定路径。建议写入 shell 配置（`~/.zshrc` / `~/.bashrc`）持久化。

  验证：

  ```bash
  ls $FFMPEG_DIR/lib/libavcodec.*
  ```

  </details>

  <details>
  <summary><strong>Windows</strong></summary>

  从 [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) 下载 FFmpeg dev 包，或 `vcpkg install ffmpeg`。然后设置：

  ```powershell
  $env:FFMPEG_DIR = "C:\path\to\ffmpeg"
  ```

  </details>

- **OpenGL / EGL 开发库**（Linux，Skia GPU 渲染需要）：

  ```bash
  sudo apt install libegl-dev libgles-dev libgl1-mesa-dev libx11-dev
  ```

  macOS 使用系统 SDK 自带的 Metal，无需额外安装。Windows 通过系统驱动提供 OpenGL。

- **Fontconfig 开发库**（Linux）：

  ```bash
  sudo apt install libfontconfig-dev
  ```

### Skia

Skia 通过 [`skia-safe`](https://crates.io/crates/skia-safe) 引入，并启用了 **`binary-cache`** 特性。构建时 `skia-bindings` 会自动下载预编译的 Skia 二进制文件，无需本地编译或手动下载静态包。

- **Linux**：`gl` 后端（OpenGL）
- **macOS**：`metal` 后端（Metal）
- **额外**：`skottie` 支持 Lottie 动画

预编译二进制文件首次构建后会缓存到 `~/.cargo/skia-binaries/`。

> 如果下载失败（如代理环境），可设置 `HTTP_PROXY` / `HTTPS_PROXY`，或手动将二进制文件放到构建脚本提示的路径。

### 构建命令

**CLI（MP4 渲染）：**

```bash
cargo build --release --bin opencat
```

二进制文件在 `target/release/opencat`。渲染视频：

```bash
cargo run --release --bin opencat -- examples/profile-showcase.xml
```

**桌面预览播放器（macOS / Windows）：**

```bash
cargo run --release --bin opencat-see -- path/to/input.xml
```

**Web（WASM）：**

```bash
cd crates/opencat-web && npm run build
```

需要 `wasm-pack` 和 `Cross-Origin-Isolated` 浏览器环境才能运行。

### 验证

确认构建正确地链接了 FFmpeg 和 Skia：

```bash
cargo run --bin opencat -- --version
```

标准配置下无需设置 `ffmpegDir` 或 `SKIA_BINARIES_URL` 等环境变量——一切通过 `pkg-config` 和 `binary-cache` 自动完成。如果使用了非标准路径安装 FFmpeg，在构建前设置 `FFMPEG_DIR` 即可。

## Who is it for

- **AI 视频管线**：模型输出 XML，引擎渲染视频，接入成本最低
- **Web 应用**：浏览器内集成视频渲染 / 编辑，无需服务器
- **程序化动画**：确定性 GPU 加速渲染，跨机器输出一致
- **批量生产**：模板化视频，换数据 = 换 XML

## Reference

- [XML 格式参考](skill/references/opencat.md)
- [动画系统](skill/references/animations.md)
- [转场效果](skill/references/transitions.md)
- [Canvas API](skill/references/canvaskit.md)
- [模板系统](skill/references/templates.md)
- [设计原则](skill/references/design-principles.md)
- [开发指南](DEVELOPMENT_ZH.md)
- [架构文档](ARCHITECTURE_ZH.md)
- [迁移指南](docs/MIGRATION.md) — 显式 lifecycle、HostInputs、AudioPlan、RenderFrame、OCIR v5

## Community

- Bug / 功能建议 → [提 Issue](https://github.com/ZhouXiaolin/opencat/issues)
- Linux Do 社区 → [OpenCat 社区讨论](https://linux.do/t/topic/2090262/7)

## Star History

[![Star History](https://api.star-history.com/svg?repos=ZhouXiaolin/opencat&type=Date)](https://www.star-history.com/#ZhouXiaolin/opencat&Date)

## License

MIT License
