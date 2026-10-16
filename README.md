<div align="center">

# OpenCat

### Write videos in JSX, render with Rust, one command to MP4.

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

JSX defines scenes, animations, and layouts. Skia GPU renders, FFmpeg encodes to MP4 — deterministic, cross-platform, cross-machine consistent. No Chromium snapshots, no Puppeteer, no bloated Web rendering pipeline.

A video is just a JSX file (this is scene A of [`examples/k3-promo/k3-promo.jsx`](examples/k3-promo/k3-promo.jsx), the spot above):

```jsx
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 16.4667 };

function StageCanvas({ id }) {
  return <canvas id={id} class="absolute inset-0 w-[1920px] h-[1080px]" />;
}

function SceneA() {
  useGSAP((ctx) => {
    var T = ctx.global.time;                 // seconds over the whole 16.4667s
    ctx.getNode('sceneA').opacity(T >= 0.75 && T < 4.62 ? 1 : 0);
    if (T >= 0.75 && T < 4.62) {
      ctx.timeline()
        .set('azoom', { scale: 1.07 }, 0.933)
        .to('azoom', { scale: 0.9417, duration: 3.667, ease: 'none' }, 0.933)
        .fromTo('atitlegrp', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power1.inOut' }, 3.03);
    }
  });
  return (
    <div id="sceneA" class="absolute inset-0 z-[2] opacity-0">
      <div id="azoom" class="absolute inset-0 [transform-origin:50%_50%]">
        <div id="afield" class="absolute inset-0"><StageCanvas id="afield-canvas" /></div>
        <div id="atitlegrp" class="absolute left-[755px] top-[444px] w-[467px] h-[185px] opacity-0">
          <div id="a-sel" class="absolute inset-0 border-[1px] border-white/25"></div>
          <text id="aline1" class="absolute text-[#d8d8da] text-[63.6426px] tracking-[-0.8px] leading-none">Every token</text>
        </div>
      </div>
    </div>
  );
}

export default function K3Promo() {
  return <div id="root" class="relative w-[1920px] h-[1080px] bg-[#010101] overflow-hidden"><SceneA /></div>;
}
```

```bash
cargo run --bin opencat -- examples/k3-promo/k3-promo.jsx
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
| **AI-friendly** | Declarative JSX | JSX/HTML, more complex |

Remotion reuses the Web ecosystem, but Chrome snapshot has inherent limitations — constrained GPU access, high memory overhead, capped frame rates, and Chromium required in deployment. OpenCat calls GPU and FFmpeg natively, with an order of magnitude higher performance ceiling.

## Capabilities

### Declarative animation, GSAP-grade API

```js
// Declarative timeline — scene A of examples/k3-promo/k3-promo.jsx
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

```jsx
<tl id="main-tl">
  <Scene1 />                                             {/* duration="4" on its root */}
  <transition from="scene1" to="scene2" effect="fade" duration="0.6" />
  <Scene2 />                                             {/* duration="4" on its root */}
</tl>
```

Each scene is a component whose root carries the `duration`. A scene's hooks bind to that scene's clock, so `ctx.current.time` starts at 0 when the scene becomes visible — no manual start-time bookkeeping, and transitions do not shift it.

Built-in: fade / slide / wipe / clock_wipe / iris / light_leak, with custom GLSL shader support.

### Components — reusable building blocks

Any JSX function is a component. `k3-promo` hoists six shared building blocks out of the node tree — props replace the old template parameters:

```jsx
function StageCanvas({ id }) {
  return <canvas id={id} class="absolute inset-0 w-[1920px] h-[1080px]" />;
}
function Cdot({ id, s, c }) {
  return <div id={id} class={'absolute w-[' + s + '] h-[' + s + '] bg-[' + c + ']'} />;
}

// call sites pass only the differences
<StageCanvas id="afield-canvas" />
<Cdot id="a-tl" s="13px" c="#d8d8da" />
```

Components compose by nesting, and `props.children` composes markup from the call site:

```jsx
function Card({ bg, title, children }) {
  return <div class={'w-[400px] rounded-xl p-6 ' + bg}>
    <text class="text-[20px] font-bold">{title}</text>
    {children}
  </div>;
}
<Card bg="bg-white" title="Hello">
  <text class="text-gray-500">Card content.</text>
</Card>
```

### WASM rendering in the browser

```ts
import { initWasm, preloadAssets, getRendererOrThrow, exportMp4 } from 'opencat-web';

await initWasm();
const catalog = await preloadAssets(jsxContent);
const renderer = getRendererOrThrow();
renderer.build_frame(jsxContent, frameNumber, canvas, catalog);
await exportMp4({ /* ... */ });
```

Pure WASM + CanvasKit, no server required.

### HTML in Canvas — Subtree Texture Sampling

A `<canvas>` node's subtree content can be live-textured and fed into a per-pixel effect. Effects are authored as **JS lambdas** (compiled, never executed — Rust parses the source, lowers it to SKSL or dispatches it to the CPU renderer, and hardcodes no effect algorithm). Offscreen targets (`ly.surface`) hold build-once, sample-per-frame data. **The `ly` path exposes no CanvasKit** — no `Paint`, no `drawRect`, no `TileMode`; the engine picks Skia or CPU by kernel capability. `examples/k3-promo/k3-promo.jsx` scene H builds its dissolve distance field entirely in JS:

```js
// 1) raster the lockup once into an offscreen surface
var ox = ly.surface(1920, 1080);
ox.fillStyle = '#fff'; ox.fillText('K3. Now Open', 962, 586);

// 2) seed a mask + distance field — a pixel kernel sampling that surface as a source
var field = ly.surface(1460, 222);
const seed = ly.pixel((uv, src) => {
  const c = src.eval(uv + [232, 430]);
  const m = byte(c.a * 255 + 0.5);
  const d = m > 120 ? 0 : 3000;                      // R=mask, G/B=dist lo/hi
  return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1];
});
field.apply(seed, { sources: [ox] });

// 3) chamfer sweeps — a scan kernel, in place; get(dx,dy) reads the in-progress
//    buffer and the traversal direction rides on the kernel declaration
field.sweep(ly.scan((get) => {
  const at = (n) => byte(n.g * 255 + 0.5) + byte(n.b * 255 + 0.5) * 256;
  const c = get(0, 0), d = at(c);
  const m = min(min(d, at(get(-1, 0)) + 3), min(at(get(0, -1)) + 3,
              min(at(get(-1, -1)) + 4, at(get(1, -1)) + 4)));
  return [c.r, m % 256 / 255, floor(m / 256) / 255, 1];
}, { dir: 'forward' }));

// 4) each frame: draw a pixel kernel that samples the field surface directly
var DIS = ly.pixel((uv, field, u) => {
  const c = field.eval(uv);
  const d = (byte(c.g * 255 + 0.5) + 256 * byte(c.b * 255 + 0.5)) / 3;
  /* scramble noise · dust · glow → straight RGBA */
}, { uniforms: { x0: 'float', f: 'float', ep: 'float', pa: 'float4' } });
ly.draw(canvas, DIS, {
  rect: [232, 230, 1460, 222],
  uniforms: { x0: 232, f: 420, ep: 60, pa: [.9, .8, .7, .3] },
  sources: [field],
});
```

Any HTML subtree — layout, images, text, video → texture → shader → output. Pixel buffers never cross the JS bridge and the Rust side holds only a **generic executor**: `target.apply` rewrites pixels with a pixel kernel, `target.sweep` sweeps in place with a scan kernel, and `ly.draw` bakes a surface on demand for the draw path. The effect-kernel language is **Liangyi** (`ly.*` — `crates/opencat-liangyi`): JS-hosted, compiled never executed, dispatched to SKSL or a CPU interpreter by capability. When you need raw SKSL control, hand-written shaders via `CK.RuntimeEffect.Make(sksl)` still work. Reference: `examples/k3-promo/k3-promo.jsx` scene H; guide: `skills/opencat-creator/references/canvaskit.md`.

### More

- **Tailwind-style layout**：`class="flex items-center justify-center gap-4"`
- **Audio mixing**：multi-track, scene-attached, auto-mix to output
- **Subtitle engine**：SRT parsing, cross-scene persistent display
- **Lucide icons**：2000+ icons out of the box
- **Deterministic rendering**：`value = f(time)`, consistent across machines

## Quick start

```bash
# Render MP4 (the spot above)
cargo run --bin opencat -- examples/k3-promo/k3-promo.jsx

# Desktop player for live preview (macOS / Windows / Linux)
cargo run --bin opencat-see -- path/to/input.jsx
```

> Web (WASM): `cd crates/opencat-web/web && npm run build`, requires `Cross-Origin-Isolated` environment.

### Engine / Web alignment (k3diff pixel metrics)

Pixel-compare native Skia vs WASM CanvasKit with ChromeDriver using hard pixel metrics (`mae`/`maxd`/`p8`, not SSIM), via `tools/k3diff.py`. Can also compare the web render directly against the original reference video with `opencat-web-compare --reference`.

```bash
# 1) media used by the oracle compositions (loads http://127.0.0.1:8080/...)
#    serve your mp4/png/mp3 tree on :8080

# 2) build web facade (wasm + JS + web-demuxer.wasm → dist/)
cd crates/opencat-web/web && npm run build && cd -

# 3) multi-frame oracle — needs Chrome + chromedriver + ffmpeg
cargo test chromedriver_ --package opencat-engine --lib -- --ignored --nocapture
```

Failures write `target/opencat-web-oracle/<stem>-frame-NNNN/{engine,web,diff}.png`.

<details>
<summary><strong>Architecture</strong></summary>

```
JSX ──→ Taffy layout ──→ Skia render ──→ encode → MP4
              ↑
         V8 animation scripts
```

**Dual pipeline：** Rust (GPU) + FFmpeg → MP4 | WASM + CanvasKit (WebGL) → Canvas / MP4

**Incremental rendering：** Resolve → Layout → Display，Merkle Tree skips unchanged subtrees + Scene Snapshot zero-cost reuse.

```
opencat
├── crates/
│   ├── opencat-core/      # Layout (Taffy), text (cosmic-text), fonts
│   ├── opencat-engine/    # Skia render, FFmpeg encode, V8 script
│   ├── opencat-web/       # WASM: browser render + export
│   └── opencat/           # CLI entry
├── web/                   # Web video editor
└── examples/                  # Example JSX files
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

### Windows: two required linker flags

The pre-built Skia and V8 binaries are built with **incompatible C runtimes**, so a plain
`cargo build` on Windows fails to link. Both fixes go in `~/.cargo/config.toml`
(or `.cargo/config.toml` in the repo):

```toml
[target.x86_64-pc-windows-msvc]
rustflags = [
  "-C", "link-arg=/STACK:16777216",
  "-C", "link-arg=/FORCE:MULTIPLE",
]
```

**`/STACK:16777216`** — `ffmpeg-sys-next`'s build script runs bindgen over the full FFmpeg
header set, which recurses deeper than the 1 MiB Windows default main-thread stack
(`SizeOfStackReserve` in the PE header). Without this the build script dies with
`STATUS_STACK_OVERFLOW` (exit `0xc00000fd`).

**`/FORCE:MULTIPLE`** — `rusty_v8.lib` carries a `/DEFAULTLIB:libcmt.lib` directive
(static CRT); `skia.lib` carries `/DEFAULTLIB:msvcrt.lib` + `msvcprt.lib` (dynamic CRT).
Both directives are baked into the prebuilt binaries and cannot be changed by Cargo
features or environment variables. Each library defines its own `std::terminate`, so the
link fails with `LNK2005` / `LNK1169`. This flag tells the linker to pick one
implementation instead of erroring.

> **Known risk:** the two STL implementations are mixed in one executable. Ordinary
> rendering and JS `try`/`catch` are verified working, but the behaviour of C++ exceptions
> crossing between the Skia and V8 sides is undefined. If you hit inexplicable crashes,
> suspect this flag first. The clean fix is building V8 from source with a dynamic CRT
> (`EXTRA_GN_ARGS="is_component_build=true"`), which takes hours and is not wired up yet.

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
cargo run --release --bin opencat -- examples/k3-promo/k3-promo.jsx
```

**Desktop preview player (macOS / Windows / Linux):**

```bash
cargo run --release --bin opencat-see -- path/to/input.jsx
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

- **AI video pipelines**: model outputs JSX, engine renders video, lowest integration cost
- **Web apps**: in-browser video rendering/editing, no server needed
- **Procedural animation**: deterministic GPU-accelerated rendering, consistent across machines
- **Batch production**: component-based video, swap props = swap a new video

## Reference

- [JSX Format Reference](skills/opencat-creator/references/jsx-format.md)
- [Animation System](skills/opencat-creator/references/animations.md)
- [Transitions](skills/opencat-creator/references/transitions.md)
- [Canvas API](skills/opencat-creator/references/canvaskit.md)
- [Three.js Scenes](skills/opencat-creator/references/three.md)
- [Templates](skills/opencat-creator/references/templates.md)
- [Design Principles](skills/opencat-creator/references/design-principles.md)

## Community

- Bugs / Feature requests → [Open an Issue](https://github.com/ZhouXiaolin/opencat/issues)
- Linux Do Discussion → [OpenCat Community](https://linux.do/t/topic/2090262/7)

## Star History

[![Star History](https://api.star-history.com/svg?repos=ZhouXiaolin/opencat&type=Date)](https://www.star-history.com/#ZhouXiaolin/opencat&Date)

## License

MIT License
