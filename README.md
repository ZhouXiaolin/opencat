<div align="center">

# OpenCat

### Write videos in XML, render with Rust, one command to MP4.

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

XML defines scenes, animations, and layouts. Skia GPU renders, FFmpeg encodes to MP4 — deterministic, cross-platform, cross-machine consistent. No Chromium snapshots, no Puppeteer, no bloated Web rendering pipeline.

A video is just an XML file:

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

MP4 ready. No browser, no screenshots, no GUI needed.


## Why OpenCat

| | OpenCat | Remotion / HyperFrames |
|---|---------|------------------------|
| **Render** | Rust native GPU (Skia) | Chrome snapshot |
| **Speed** | 10x | Baseline |
| **Deployment** | Any environment / pure CLI | Requires Chromium |
| **Animation** | Custom GSAP-compatible API (80%+ coverage) | Direct GSAP / anime.js |
| **Browser render** | WASM + CanvasKit | Native |
| **Deterministic** | ✅ Cross-machine consistent | ❌ |
| **AI-friendly** | Declarative XML/JSONL | JSX/HTML, more complex |

Remotion reuses the Web ecosystem, but Chrome snapshot has inherent limitations — constrained GPU access, high memory overhead, capped frame rates, and Chromium required in deployment. OpenCat calls GPU and FFmpeg natively, with an order of magnitude higher performance ceiling.

## Capabilities

### Declarative animation, GSAP-grade API

```js
ctx.fromTo('title', {opacity: 0, y: 30}, {opacity: 1, y: 0, duration: 0.67, ease: 'spring.gentle'});
ctx.to('rocket', {path: 'M100 360 C400 80 880 640 1180 360', duration: 4, ease: 'ease-in-out'});
ctx.from(ctx.splitText('title', {type: 'chars'}), {opacity: 0, y: 20, stagger: 0.07, ease: 'spring.wobbly'});

ctx.timeline({defaults: {duration: 0.6, ease: 'spring.gentle'}})
  .from('title', {opacity: 0, y: 30})
  .from('subtitle', {opacity: 0, y: 18}, '-=0.27');
```

### Multi-scene timelines + transitions

```xml
<tl id="main-tl">
  <div id="scene1" duration="4">...</div>
  <transition from="scene1" to="scene2" effect="fade" duration="0.6" />
  <div id="scene2" duration="4">...</div>
</tl>
```

Built-in: fade / slide / wipe / clock_wipe / iris / light_leak, with custom GLSL shader support.

### XML Templates — reusable components with slots & variables

Define reusable components with `<template>`, parameterize with `$variable`, and compose with `<slot>`:

```xml
<opencat>
  <!-- Define a template -->
  <template name="card">
    <div class="w-[400px] rounded-xl bg-$bg shadow-lg p-6">
      <h2 class="text-xl font-bold text-$titleColor">$title</h2>
      <slot name="body" />
    </div>
  </template>

  <!-- Use it -->
  <card bg="white" titleColor="gray-900" title="Hello">
    <slot name="body">
      <p class="text-gray-500">This is the card content.</p>
    </slot>
  </card>
</opencat>
```

Templates expand at parse time — zero runtime cost, fully composable, and support nesting.

### WASM rendering in the browser

```ts
import { initWasm, preloadAssets, getRendererOrThrow, exportMp4 } from 'opencat-web';

await initWasm();
const catalog = await preloadAssets(xmlContent);
const renderer = getRendererOrThrow();
renderer.build_frame(xmlContent, frameNumber, canvas, catalog);
await exportMp4({ /* ... */ });
```

Pure WASM + CanvasKit, no server required.

### HTML in Canvas — Subtree Texture Sampling

A `<canvas>` node's subtree content can be live-textured and fed into a custom SkSL shader:

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

Any HTML subtree — layout, images, text, video → texture → shader → output.

### More

- **Tailwind-style layout**：`class="flex items-center justify-center gap-4"`
- **Audio mixing**：multi-track, scene-attached, auto-mix to output
- **Subtitle engine**：SRT parsing, cross-scene persistent display
- **Lucide icons**：2000+ icons out of the box
- **Deterministic rendering**：`value = f(time)`, consistent across machines

## Quick start

```bash
# Render MP4
cargo run --bin opencat -- examples/profile-showcase.xml

# Desktop player for live preview (macOS / Windows)
cargo run --bin opencat-see -- path/to/input.xml

# Hello World example
cargo run --example hello_world
```

> Web (WASM): `cd crates/opencat-web && npm run build`, requires `Cross-Origin-Isolated` environment.

<details>
<summary><strong>Architecture</strong></summary>

```
XML ──→ Taffy layout ──→ Skia render ──→ encode → MP4
              ↑
         QuickJS animation scripts
```

**Dual pipeline：** Rust (GPU) + FFmpeg → MP4 | WASM + CanvasKit (WebGL) → Canvas / MP4

**Incremental rendering：** Resolve → Layout → Display，Merkle Tree skips unchanged subtrees + Scene Snapshot zero-cost reuse.

```
opencat
├── crates/
│   ├── opencat-core/      # Layout (Taffy), text (cosmic-text), fonts
│   ├── opencat-engine/    # Skia render, FFmpeg encode, QuickJS script
│   ├── opencat-web/       # WASM: browser render + export
│   └── opencat/           # CLI entry
├── web/                   # Web video editor
└── examples/                  # Example XML files
```

</details>

## Build from source

### Prerequisites

- **Rust toolchain** (edition 2024). Install via [rustup](https://rustup.rs/):
  ```bash
  rustup install nightly  # edition 2024 requires nightly as of early 2025
  ```

- **FFmpeg dev libraries** (for MP4 encoding). The crate [`ffmpeg-next`](https://crates.io/crates/ffmpeg-next) discovers FFmpeg via `pkg-config` — no manual path configuration is needed on standard systems.

  <details open>
  <summary><strong>Linux (Ubuntu / Debian)</strong></summary>

  ```bash
  sudo apt install \
    libavcodec-dev libavformat-dev libavutil-dev \
    libavfilter-dev libswscale-dev
  ```

  Minimum version: FFmpeg 6.x. Verify:

  ```bash
  ffmpeg -version
  ```

  On this system: **FFmpeg 7.1.1** is installed and all dev packages are present.

  </details>

  <details>
  <summary><strong>macOS</strong></summary>

  ```bash
  brew install ffmpeg
  ```

  Homebrew installs ffmpeg to `/opt/homebrew` (Apple Silicon) or `/usr/local` (Intel). Set `FFMPEG_DIR` to point to the Homebrew prefix:

  ```bash
  # Apple Silicon (M1/M2/M3/M4)
  export FFMPEG_DIR=/opt/homebrew

  # Intel Mac
  export FFMPEG_DIR=/usr/local
  ```

  If `pkg-config` cannot automatically find the ffmpeg libs, set `FFMPEG_DIR` to specify the path. Add the export to your shell config (`~/.zshrc` / `~/.bashrc`) to persist it.

  Verify:

  ```bash
  ls $FFMPEG_DIR/lib/libavcodec.*
  ```

  </details>

  <details>
  <summary><strong>Windows</strong></summary>

  Download FFmpeg dev packages from [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) or `vcpkg install ffmpeg`. Then set:

  ```powershell
  $env:FFMPEG_DIR = "C:\path\to\ffmpeg"
  ```

  </details>

- **OpenGL / EGL dev libraries** (Linux, for Skia GPU rendering):

  ```bash
  sudo apt install libegl-dev libgles-dev libgl1-mesa-dev libx11-dev
  ```

  macOS provides Metal via the system SDK (no manual install). Windows provides OpenGL via the system driver.

- **Fontconfig dev library** (Linux):

  ```bash
  sudo apt install libfontconfig-dev
  ```

### Skia

Skia is pulled in via [`skia-safe`](https://crates.io/crates/skia-safe) with the **`binary-cache`** feature enabled. This causes `skia-bindings` to download a pre-built Skia binary at build time — no local compilation or static package download is required.

- **Linux**: `gl` backend (OpenGL)
- **macOS**: `metal` backend (Metal)
- **Extra**: `skottie` for Lottie animation support

The pre-built binaries are cached in `~/.cargo/skia-binaries/` after the first build.

### Build commands

**CLI (MP4 rendering):**

```bash
cargo build --release --bin opencat
```

The binary is at `target/release/opencat`. Render a video:

```bash
cargo run --release --bin opencat -- examples/profile-showcase.xml
```

**Desktop preview player (macOS / Windows):**

```bash
cargo run --release --bin opencat-see -- path/to/input.xml
```

**Hello World:**

```bash
cargo run --example hello_world
```

**Web (WASM):**

```bash
cd crates/opencat-web && npm run build
```

Requires `wasm-pack` and a `Cross-Origin-Isolated` environment to run.

### Verification

Check that the build picked up the correct FFmpeg and Skia versions:

```bash
cargo run --bin opencat -- --version
```

No `ffmpegDir` or `SKIA_BINARIES_URL` environment variables are needed in a standard setup — everything is resolved through `pkg-config` and the `binary-cache` feature. If you do use a non-standard FFmpeg path, set `FFMPEG_DIR` before building.

## Who is it for

- **AI video pipelines**: model outputs XML, engine renders video, lowest integration cost
- **Web apps**: in-browser video rendering/editing, no server needed
- **Procedural animation**: deterministic GPU-accelerated rendering, consistent across machines
- **Batch production**: template-based video, swap data = swap XML

## Reference

- [XML Format Reference](skill/references/opencat.md)
- [Animation System](skill/references/animations.md)
- [Transitions](skill/references/transitions.md)
- [Canvas API](skill/references/canvaskit.md)
- [Templates](skill/references/templates.md)
- [Design Principles](skill/references/design-principles.md)

## Community

- Bugs / Feature requests → [Open an Issue](https://github.com/ZhouXiaolin/opencat/issues)
- Linux Do Discussion → [OpenCat 社区讨论](https://linux.do/t/topic/2090262/7)

## Star History

[![Star History](https://api.star-history.com/svg?repos=ZhouXiaolin/opencat&type=Date)](https://www.star-history.com/#ZhouXiaolin/opencat&Date)

## License

MIT License
