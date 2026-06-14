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
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README_ZH.md">中文</a>
</p>

  <video width="60%" controls autoplay loop muted playsinline src="https://github.com/user-attachments/assets/62ae6af6-095b-4b54-af53-97ba79945a6d"></video>

</div>

XML 定义场景、动画与布局，Skia GPU 加速渲染，FFmpeg 编码输出 MP4 —— 跨机器、跨平台、确定性一致。告别 Chromium 快照、Puppeteer 和笨重的 Web 渲染管线。

写一个视频，就是写一个 XML 文件：

```xml
<opencat width="1920" height="1080" fps="30" duration="3">
  <div id="root" class="relative w-[1920px] h-[1080px] bg-white overflow-hidden">
    <div id="pink-glow" class="absolute inset-0 opacity-0 bg-[radial-gradient(ellipse_80%_80%_at_50%_50%,rgba(234,76,137,0.05)_0%,transparent_70%)]" />
    <div id="logo-container" class="absolute inset-0 flex items-center justify-center">
      <path id="logo-path" class="fill-white stroke-[#EA4C89] stroke-[1.5] stroke-dasharray-[1800] stroke-dashoffset-[1800]" d="..." />
    </div>
    <canvas id="particle-canvas" class="absolute inset-0 pointer-events-none w-[1920px] h-[1080px]" />
  </div>
  <script>
    var tl = ctx.timeline();
    tl.to('logo-path', { strokeDashoffset: 0, duration: 2, ease: 'power2.inOut' }, 0);
    tl.to('logo-path', { fillColor: '#0D0C22', strokeColor: '#0D0C22', duration: 0.3, ease: 'power2.out' }, 2);
    // particles on canvas, scene exit blur...
  </script>
</opencat>
```

```bash
cargo run --bin opencat -- examples/dribbble-logo-animated.xml
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
ctx.fromTo('title', {opacity: 0, y: 30}, {opacity: 1, y: 0, duration: 0.67, ease: 'spring.gentle'});
ctx.set('caption', {opacity: 0});
ctx.to('rocket', {path: 'M100 360 C400 80 880 640 1180 360', duration: 4, ease: 'ease-in-out'});
ctx.from(ctx.splitText('title', {type: 'chars'}), {opacity: 0, y: 20, stagger: 0.07, ease: 'spring.wobbly'});

ctx.timeline({defaults: {duration: 0.6, ease: 'spring.gentle'}})
  .set('caption', {opacity: 0}, 0)
  .from('title', {opacity: 0, y: 30})
  .from('subtitle', {opacity: 0, y: 18}, '-=0.27');
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

`<canvas>` 节点的子树内容可实时纹理化，传入自定义 SkSL 着色器做后处理：

```js
var CK = ctx.CanvasKit;
var c = ctx.getCanvasById('s1-canvas');
var subtree = c.getSubTree();
var subtreeShader = subtree.makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);

var sksl = [
  'uniform shader image;',
  'uniform float  progress;',
  'uniform float  amplitude;',
  'uniform float  frequency;',
  'uniform float  speed;',
  'uniform float  decay;',
  'uniform float  split;',
  'half4 main(float2 xy) {',
  '  float2 uv = xy;',
  '  float dist = distance(uv, center);',
  '  float ripple = sin(dist * frequency - progress * speed);',
  '  float falloff = exp(-dist * decay);',
  '  float disp = ripple * amplitude * falloff;',
  '  float2 dir = normalize(uv - center);',
  '  float2 tangent = float2(-dir.y, dir.x);',
  '  half4 r = image.eval(uv + dir * disp + tangent * split);',
  '  half4 g = image.eval(uv + dir * disp);',
  '  half4 b = image.eval(uv + dir * disp - tangent * split);',
  '  return half4(r.r, g.g, b.b, max(max(r.a, g.a), b.a));',
  '}',
].join('\n');

var effect = CK.RuntimeEffect.Make(sksl);
if (effect) {
  var shader = effect.makeShaderWithChildren([progress, amplitude, frequency, speed, decay, split], [subtreeShader]);
  var paint = new CK.Paint();
  paint.setShader(shader);
  c.drawRect(CK.LTRBRect(0, 0, 360, 480), paint);
}
```

画布内 HTML 子树的任意布局、图片、文本、视频 → 纹理 → 着色器 → 输出。

### 更多能力

- **Tailwind 式布局**：`class="flex items-center justify-center gap-4"`
- **音频混音**：多轨道，场景级挂载，自动混音输出
- **字幕引擎**：SRT 解析，跨场景持久化显示
- **Lucide 图标库**：2000+ 开箱即用
- **确定性渲染**：`value = f(time)`，跨机器一致

## Quick start

```bash
# 渲染 MP4
cargo run --bin opencat -- examples/profile-showcase.xml

# 桌面播放器实时预览（macOS / Windows）
cargo run --bin opencat-see -- path/to/input.xml

# Hello World 示例
cargo run --example hello_world
```

> Web (WASM)：`cd crates/opencat-web && npm run build`，浏览器需要 `Cross-Origin-Isolated` 环境。

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

**Hello World：**

```bash
cargo run --example hello_world
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

- [XML 格式参考](opencat-creator/references/opencat.md)
- [转场效果](opencat-creator/references/transitions.md)
- [动画系统](opencat-creator/references/animations.md)
- [Canvas API](opencat-creator/references/canvaskit.md)
- [文字动画](opencat-creator/references/text-animations.md)

## Community

- Bug / 功能建议 → [提 Issue](https://github.com/ZhouXiaolin/opencat/issues)

## Star History

[![Star History](https://api.star-history.com/svg?repos=ZhouXiaolin/opencat&type=Date)](https://www.star-history.com/#ZhouXiaolin/opencat&Date)

## License

MIT License
