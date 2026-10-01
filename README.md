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
  <a href="https://zread.ai/ZhouXiaolin/opencat"><img alt="zread" src="https://img.shields.io/badge/Ask_Zread-_.svg?style=flat&color=00b0aa&labelColor=000000&logo=data%3Aimage%2Fsvg%2Bxml%3Bbase64%2CPHN2ZyB3aWR0aD0iMTYiIGhlaWdodD0iMTYiIHZpZXdCb3g9IjAgMCAxNiAxNiIgZmlsbD0ibm9uZSIgeG1sbnM9Imh0dHA6Ly93d3cudzMub3JnLzIwMDAvc3ZnIj4KPHBhdGggZD0iTTQuOTYxNTYgMS42MDAxSDIuMjQxNTZDMS44ODgxIDEuNjAwMSAxLjYwMTU2IDEuODg2NjQgMS42MDE1NiAyLjI0MDFWNC45NjAxQzEuNjAxNTYgNS4zMTM1NiAxLjg4ODEgNS42MDAxIDIuMjQxNTYgNS42MDAxSDQuOTYxNTZDNS4zMTUwMiA1LjYwMDEgNS42MDE1NiA1LjMxMzU2IDUuNjAxNTYgNC45NjAxVjIuMjQwMUM1LjYwMTU2IDEuODg2NjQgNS4zMTUwMiAxLjYwMDEgNC45NjE1NiAxLjYwMDFaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00Ljk2MTU2IDEwLjM5OTlIMi4yNDE1NkMxLjg4ODEgMTAuMzk5OSAxLjYwMTU2IDEwLjY4NjQgMS42MDE1NiAxMS4wMzk5VjEzLjc1OTlDMS42MDE1NiAxNC4xMTM0IDEuODg4MSAxNC4zOTk5IDIuMjQxNTYgMTQuMzk5OUg0Ljk2MTU2QzUuMzE1MDIgMTQuMzk5OSA1LjYwMTU2IDE0LjExMzQgNS42MDE1NiAxMy43NTk5VjExLjAzOTlDNS42MDE1NiAxMC42ODY0IDUuMzE1MDIgMTAuMzk5OSA0Ljk2MTU2IDEwLjM5OTlaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik0xMy43NTg0IDEuNjAwMUgxMS4wMzg0QzEwLjY4NSAxLjYwMDEgMTAuMzk4NCAxLjg4NjY0IDEwLjM5ODQgMi4yNDAxVjQuOTYwMUMxMC4zOTg0IDUuMzEzNTYgMTAuNjg1IDUuNjAwMSAxMS4wMzg0IDUuNjAwMUgxMy43NTg0QzE0LjExMTkgNS42MDAxIDE0LjM5ODQgNS4zMTM1NiAxNC4zOTg0IDQuOTYwMVYyLjI0MDFDMTQuMzk4NCAxLjg4NjY0IDE0LjExMTkgMS42MDAxIDEzLjc1ODQgMS42MDAxWiIgZmlsbD0iI2ZmZiIvPgo8cGF0aCBkPSJNNCAxMkwxMiA0TDQgMTJaIiBmaWxsPSIjZmZmIi8%2BCjxwYXRoIGQ9Ik00IDEyTDEyIDQiIHN0cm9rZT0iI2ZmZiIgc3Ryb2tlLXdpZHRoPSIxLjUiIHN0cm9rZS1saW5lY2FwPSJyb3VuZCIvPgo8L3N2Zz4K&logoColor=ffffff" /></a>
</p>

<p align="center">
  <a href="README.md">English</a> · <a href="README_ZH.md">中文</a>
</p>

  <video width="60%" controls autoplay loop muted playsinline src="https://github.com/user-attachments/assets/e3263ec6-a36c-4b73-ba4c-c0e6b64b86ee"></video>

</div>

XML defines scenes, animations, and layouts. Skia GPU renders, FFmpeg encodes to MP4 — deterministic, cross-platform, cross-machine consistent. No Chromium snapshots, no Puppeteer, no bloated Web rendering pipeline.

A video is just an XML file (this is scene A of [`examples/k3-promo.xml`](examples/k3-promo.xml), the spot above):

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
    var T = ctx.time;                       // seconds over the whole 16.4667s
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
// Declarative timeline — scene A of examples/k3-promo.xml
ctx.timeline()
  .set('azoom', { scale: 1.07 }, 0.933)
  .to('azoom', { scale: 0.9417, duration: 3.667, ease: 'none' }, 0.933)
  .to(['afield', 'afieldp'], { opacity: 0.35, duration: 0.4333, ease: 'power1.inOut' }, 1.9667)
  .fromTo('apwrap', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power1.inOut' }, 3.03);

// GSAP-shape per-character text: splitText parts settle with a power2.out ease
var parts = ctx.splitText('otxt', { type: 'chars' });
var tl = ctx.timeline();
parts.forEach(function (p, i) {
  tl.set(p, { opacity: 0 }, 7.35)
    .set(p, { opacity: 1 }, 7.5333)
    .fromTo(p, { x: (i - 6.5) * 21 }, { x: 0, duration: 0.43, ease: 'power2.out' }, 7.5663);
});
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

Define reusable components with `<template>`, parameterize with `$variable`, and compose with `<slot>`. `k3-promo` hoists six shared building blocks out of the node tree — parsed once, expanded to plain nodes before rendering:

```xml
<template name="stage-canvas"><canvas id="$id" class="absolute inset-0 w-[1920px] h-[1080px]"></canvas></template>
<template name="cdot"><div id="$id" class="absolute w-[$s] h-[$s] bg-[$c]"></div></template>
<template name="agent-pill"><div id="$id" class="absolute opacity-0"><div id="$id-box" class="px-[14px] py-[10px] border-[1.5px] border-[#dcdcde]">…</div></div></template>

<!-- call sites pass only the differences -->
<stage-canvas id="afield-canvas" />
<cdot id="a-tl" s="13px" c="#d8d8da" />
<agent-pill id="ap1" t="Agentic" />
```

Templates expand at parse time — zero runtime cost, fully composable, and support nesting. A `<slot>` composes markup from the call site:

```xml
<template name="card">
  <div class="w-[400px] rounded-xl bg-$bg p-6"><h2 class="text-$titleColor">$title</h2><slot name="body" /></div>
</template>
<card bg="white" titleColor="gray-900" title="Hello">
  <slot name="body"><p class="text-gray-500">Card content.</p></slot>
</card>
```

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

A `<canvas>` node's subtree content can be live-textured and fed into a per-pixel effect. Effects are authored as **JS lambdas** (compiled, never executed — Rust parses the source, lowers it to SKSL or dispatches it to the CPU renderer, and hardcodes no effect algorithm). Offscreen surfaces (`ctx.createSurface`) double as **render targets** for build-once, sample-per-frame data. `examples/k3-promo.xml` scene H builds its dissolve distance field entirely in JS:

```js
// 1) raster the lockup once into an offscreen surface
var ox = ctx.createSurface('k3dis-off', 1920, 1080);
ox.fillStyle = '#fff'; ox.fillText('K3. Now Open', 962, 586);

// 2) seed a mask + distance field — a pixel-class lambda sampling that surface as a child
var field = ctx.createSurface('k3dis-field', 1460, 222);
field.runEffect(
  (uv, src) => {
    const c = src.eval(uv + [232, 430]);
    const m = byte(c.a * 255 + 0.5);
    const d = m > 120 ? 0 : 3000;                      // R=mask, G/B=dist lo/hi
    return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1];
  }, null, null, [{ __opencatShader: 'surface', id: 'k3dis-off' }]);

// 3) chamfer sweeps — a scan-class lambda, in place; get(dx,dy) reads the in-progress
//    buffer and the executor owns the traversal order (forward/backward)
field.scanPass((get) => {
  const at = (n) => byte(n.g * 255 + 0.5) + byte(n.b * 255 + 0.5) * 256;
  const c = get(0, 0), d = at(c);
  const m = min(min(d, at(get(-1, 0)) + 3), min(at(get(0, -1)) + 3,
              min(at(get(-1, -1)) + 4, at(get(1, -1)) + 4)));
  return [c.r, m % 256 / 255, floor(m / 256) / 255, 1];
}, 'forward');

// 4) each frame: a per-pixel lambda samples the field surface directly — no per-frame bake
var DIS = CK.Effect.fromLambda((uv, field, u) => {
  const c = field.eval(uv);
  const d = (byte(c.g * 255 + 0.5) + 256 * byte(c.b * 255 + 0.5)) / 3;
  /* scramble noise · dust · glow → straight RGBA */
}, { uniforms: [['x0','float'], ['f','float'], ['ep','float'], ['pa','float4']] });
```

Any HTML subtree — layout, images, text, video → texture → shader → output. Pixel buffers never cross the JS bridge and the Rust side holds only a **generic executor**: `surface.runEffect` rewrites pixels with a pixel-class lambda, `surface.scanPass` sweeps in place with a scan-class lambda, and `surface.bake(key)` registers the pixels as a frame image for shader sampling. When you need raw SKSL control, hand-written shaders via `CK.RuntimeEffect.Make(sksl)` still work. Reference: `examples/k3-promo.xml` scene H; guide: `skill/references/canvaskit.md`.

### More

- **Tailwind-style layout**：`class="flex items-center justify-center gap-4"`
- **Audio mixing**：multi-track, scene-attached, auto-mix to output
- **Subtitle engine**：SRT parsing, cross-scene persistent display
- **Lucide icons**：2000+ icons out of the box
- **Deterministic rendering**：`value = f(time)`, consistent across machines

## Quick start

```bash
# Render MP4 (the spot above)
cargo run --bin opencat -- examples/k3-promo.xml

# Desktop player for live preview (macOS / Windows / Linux)
cargo run --bin opencat-see -- path/to/input.xml
```

> Web (WASM): `cd crates/opencat-web/web && npm run build`, requires `Cross-Origin-Isolated` environment.

### Engine / Web alignment (k3diff pixel metrics)

Pixel-compare native Skia vs WASM CanvasKit with ChromeDriver using hard pixel metrics (`mae`/`maxd`/`p8`, not SSIM). Can also compare the web render directly against the original reference video via `--reference`. Full steps: **[Development Guide](DEVELOPMENT.md#engine--web-pixel-alignment-k3diff-frame-oracle)**.

```bash
# 1) media used by examples (profile-showcase loads http://127.0.0.1:8080/...)
#    serve your mp4/png/mp3 tree on :8080

# 2) build web facade (wasm + JS + web-demuxer.wasm → dist/)
cd crates/opencat-web/web && npm run build && cd -

# 3) multi-frame oracle (0–413, step 10) — needs Chrome + chromedriver + ffmpeg
cargo test chromedriver_profile_showcase_all_frames_matches_engine \
  --package opencat-engine --lib -- --ignored --nocapture
```

Failures write `target/opencat-web-oracle/<stem>-frame-NNNN/{engine,web,diff}.png`.

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

  `opencat-see` (the desktop preview player) creates its GL context via **EGL** on Linux, matching the prebuilt Skia backend. It currently requires an **X11** window handle, so on a Wayland session it runs through **XWayland** (set `WAYLAND_DISPLAY=` only if auto-selection fails). Native Wayland (`wl_surface`) support is not wired up yet.

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

**Desktop preview player (macOS / Windows / Linux):**

```bash
cargo run --release --bin opencat-see -- path/to/input.xml
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
- [Development Guide](DEVELOPMENT.md) — Tailwind/Taffy layout alignment & engine/web k3diff pixel comparison
- [Development Guide (Chinese)](DEVELOPMENT_ZH.md)
- [Architecture](ARCHITECTURE.md) — complete rendering pipeline from XML/JSONL to pixels
- [Architecture (Chinese)](ARCHITECTURE_ZH.md)
- [Migration Guide](docs/MIGRATION.md) — explicit lifecycle, HostInputs, AudioPlan, RenderFrame, OCIR v5

## Community

- Bugs / Feature requests → [Open an Issue](https://github.com/ZhouXiaolin/opencat/issues)
- Linux Do Discussion → [OpenCat Community](https://linux.do/t/topic/2090262/7)

## Star History

[![Star History](https://api.star-history.com/svg?repos=ZhouXiaolin/opencat&type=Date)](https://www.star-history.com/#ZhouXiaolin/opencat&Date)

## License

MIT License
