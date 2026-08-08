
<div align="center">

# OpenCat

### Escribe videos en XML, renderiza con Rust, un comando a MP4.

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

  <video width="60%" controls autoplay loop muted playsinline src="https://github.com/user-attachments/assets/62ae6af6-095b-4b54-af53-97ba79945a6d"></video>

</div>

XML define escenas, animaciones y diseños. Skia GPU renderiza, FFmpeg codifica a MP4: determinista, multiplataforma y consistente entre máquinas. Sin instantáneas de Chromium, sin Puppeteer, sin un pipeline de renderizado Web sobrecargado.

Un video es simplemente un archivo XML:

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

MP4 listo. Sin navegador, sin capturas de pantalla, sin GUI necesaria.


## Por qué OpenCat

| | OpenCat | Remotion / HyperFrames |
|---|---------|------------------------|
| **Render** | Rust native GPU (Skia) | Chrome snapshot |
| **Speed** | 10x | Baseline |
| **Deployment** | Any environment / pure CLI | Requires Chromium |
| **Animation** | Custom GSAP-compatible API (80%+ coverage) | Direct GSAP / anime.js |
| **Browser render** | WASM + CanvasKit | Native |
| **Deterministic** | ✅ Consistente entre máquinas | ❌ |
| **AI-friendly** | XML/JSONL declarativo | JSX/HTML, más complejo |

Remotion reutiliza el ecosistema Web, pero la instantánea de Chrome tiene limitaciones inherentes: acceso a GPU restringido, alta sobrecarga de memoria, tasas de fotogramas limitadas y requiere Chromium en la implementación. OpenCat llama a GPU y FFmpeg de forma nativa, con un límite de rendimiento un orden de magnitud superior.

## Capacidades

### Animación declarativa, API de nivel GSAP

```js
ctx.fromTo('title', {opacity: 0, y: 30}, {opacity: 1, y: 0, duration: 0.67, ease: 'spring.gentle'});
ctx.to('rocket', {path: 'M100 360 C400 80 880 640 1180 360', duration: 4, ease: 'ease-in-out'});
ctx.from(ctx.splitText('title', {type: 'chars'}), {opacity: 0, y: 20, stagger: 0.07, ease: 'spring.wobbly'});

ctx.timeline({defaults: {duration: 0.6, ease: 'spring.gentle'}})
  .from('title', {opacity: 0, y: 30})
  .from('subtitle', {opacity: 0, y: 18}, '-=0.27');
```

### Líneas de tiempo multi-escena + transiciones

```xml
<tl id="main-tl">
  <div id="scene1" duration="4">...</div>
  <transition from="scene1" to="scene2" effect="fade" duration="0.6" />
  <div id="scene2" duration="4">...</div>
</tl>
```

Integrado: fade / slide / wipe / clock_wipe / iris / light_leak, con soporte para shaders GLSL personalizados.

### Plantillas XML — componentes reutilizables con slots y variables

Define componentes reutilizables con `<template>`, parametriza con `$variable` y compón con `<slot>`:

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

Las plantillas se expanden en tiempo de análisis: costo en tiempo de ejecución cero, totalmente composables y admiten anidamiento.

### Renderizado WASM en el navegador

```ts
import { initWasm, preloadAssets, getRendererOrThrow, exportMp4 } from 'opencat-web';

await initWasm();
const catalog = await preloadAssets(xmlContent);
const renderer = getRendererOrThrow();
renderer.build_frame(xmlContent, frameNumber, canvas, catalog);
await exportMp4({ /* ... */ });
```

WASM y CanvasKit puros, sin servidor necesario.

### HTML en Canvas — Muestreo de Textura de Subárbol

El contenido de un subárbol de un nodo `<canvas>` puede texturizarse en vivo y alimentarse a un shader SkSL personalizado:

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

Cualquier subárbol HTML: diseño, imágenes, texto, video → textura → shader → salida.

### Más

- **Diseño estilo Tailwind**: `class="flex items-center justify-center gap-4"`
- **Mezcla de audio**: multi-pista, adjunto a la escena, mezcla automática a la salida
- **Motor de subtítulos**: análisis de SRT, visualización persistente entre escenas
- **Iconos Lucide**: +2000 iconos listos para usar
- **Renderizado determinista**: `value = f(time)`, consistente entre máquinas

## Inicio rápido

```bash
# Renderizar MP4
cargo run --bin opencat -- examples/profile-showcase.xml

# Visor de vista previa de escritorio (macOS / Windows / Linux)
cargo run --bin opencat-see -- path/to/input.xml

# Ejemplo Hello World
cargo run --example hello_world
```

> Web (WASM): `cd crates/opencat-web/web && npm run build`, requiere un entorno `Cross-Origin-Isolated`.

### Alineación Engine / Web (SSIM)

Comparación píxel a píxel del Skia nativo vs WASM CanvasKit con ChromeDriver + SSIM. Pasos completos: **[Guía de Desarrollo](DEVELOPMENT.md#engine--web-pixel-alignment-ssim-frame-oracle)**.

```bash
# 1) multimedia usada por los ejemplos (profile-showcase carga http://127.0.0.1:8080/...)
#    sirva tu árbol de mp4/png/mp3 en :8080

# 2) construir la fachada web (wasm + JS + web-demuxer.wasm → dist/)
cd crates/opencat-web/web && npm run build && cd -

# 3) oráculo multi-frame (0–413, paso 10) — necesita Chrome + chromedriver + ffmpeg
cargo test chromedriver_profile_showcase_all_frames_matches_engine \
  --package opencat-engine --lib -- --ignored --nocapture
```

Los fallos generan `target/opencat-web-oracle/<stem>-frame-NNNN/{engine,web,diff}.png`.

<details>
<summary><strong>Arquitectura</strong></summary>

```
XML ──→ Taffy layout ──→ Skia render ──→ encode → MP4
              ↑
         QuickJS animation scripts
```

**Doble pipeline:** Rust (GPU) + FFmpeg → MP4 | WASM + CanvasKit (WebGL) → Canvas / MP4

**Renderizado incremental:** Resolve → Layout → Display, Merkle Tree omite subárboles sin cambios + reutilización de Escena Snapshot sin costo.

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

## Compilar desde el código fuente

### Requisitos previos

- **Toolchain de Rust** (edición 2024). Instala mediante [rustup](https://rustup.rs/):
  ```bash
  rustup install nightly  # edition 2024 requires nightly as of early 2025
  ```

- **Bibliotecas de desarrollo de FFmpeg** (para codificación MP4). La crate [`ffmpeg-next`](https://crates.io/crates/ffmpeg-next) detecta FFmpeg mediante `pkg-config`, no se necesita configuración manual de ruta en sistemas estándar.

  <details open>
  <summary><strong>Linux (Ubuntu / Debian)</strong></summary>

  ```bash
  sudo apt install \
    libavcodec-dev libavformat-dev libavutil-dev \
    libavfilter-dev libswscale-dev
  ```

  Versión mínima: FFmpeg 6.x. Verificar:

  ```bash
  ffmpeg -version
  ```

  En este sistema: **FFmpeg 7.1.1** está instalado y todos los paquetes de desarrollo están presentes.

  </details>

  <details>
  <summary><strong>macOS</strong></summary>

  ```bash
  brew install ffmpeg
  ```

  Homebrew instala ffmpeg en `/opt/homebrew` (Apple Silicon) o `/usr/local` (Intel). Configura `FFMPEG_DIR` para que apunte al prefijo de Homebrew:

  ```bash
  # Apple Silicon (M1/M2/M3/M4)
  export FFMPEG_DIR=/opt/homebrew

  # Intel Mac
  export FFMPEG_DIR=/usr/local
  ```

  Si `pkg-config` no puede encontrar automáticamente las bibliotecas de ffmpeg, configura `FFMPEG_DIR` para especificar la ruta. Añade la exportación a la configuración de tu shell (`~/.zshrc` / `~/.bashrc`) para que persista.

  Verificar:

  ```bash
  ls $FFMPEG_DIR/lib/libavcodec.*
  ```

  </details>

  <details>
  <summary><strong>Windows</strong></summary>

  Descarga los paquetes de desarrollo de FFmpeg desde [gyan.dev](https://www.gyan.dev/ffmpeg/builds/) o `vcpkg install ffmpeg`. Luego configura:

  ```powershell
  $env:FFMPEG_DIR = "C:\path\to\ffmpeg"
  ```

  </details>

- **Bibliotecas de desarrollo de OpenGL / EGL** (Linux, para renderizado GPU de Skia):

  ```bash
  sudo apt install libegl-dev libgles-dev libgl1-mesa-dev libx11-dev
  ```

  macOS proporciona Metal a través del SDK del sistema (sin instalación manual). Windows proporciona OpenGL a través del controlador del sistema.

  `opencat-see` (el visor de vista previa de escritorio) crea su contexto GL vía EGL en Linux, coincidiendo con el backend de Skia precompilado. Actualmente requiere un identificador de ventana X11, por lo que en una sesión Wayland se ejecuta a través de XWayland (configura `WAYLAND_DISPLAY=` solo si la selección automática falla). El soporte nativo de Wayland (`wl_surface`) aún no está conectado.

- **Biblioteca de desarrollo de Fontconfig** (Linux):

  ```bash
  sudo apt install libfontconfig-dev
  ```

### Skia

Skia se incluye a través de [`skia-safe`](https://crates.io/crates/skia-safe) con la feature **`binary-cache`** habilitada. Esto hace que `skia-bindings` descargue un binario de Skia precompilado en tiempo de construcción, sin necesidad de compilación local o descarga de paquete estático.

- **Linux**: backend `gl` (OpenGL)
- **macOS**: backend `metal` (Metal)
- **Extra**: `skottie` para soporte de animaciones Lottie

Los binarios precompilados se almacenan en caché en `~/.cargo/skia-binaries/` tras la primera construcción.

### Comandos de construcción

**CLI (renderizado MP4):**

```bash
cargo build --release --bin opencat
```

El binario se encuentra en `target/release/opencat`. Renderiza un video:

```bash
cargo run --release --bin opencat -- examples/profile-showcase.xml
```

**Visor de vista previa de escritorio (macOS / Windows / Linux):**

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

Requiere `wasm-pack` y un entorno `Cross-Origin-Isolated` para ejecutarse.

### Verificación

Verifica que la construcción haya detectado las versiones correctas de FFmpeg y Skia:

```bash
cargo run --bin opencat -- --version
```

No se necesitan las variables de entorno `ffmpegDir` o `SKIA_BINARIES_URL` en una configuración estándar, todo se resuelve a través de `pkg-config` y la feature `binary-cache`. Si usas una ruta no estándar de FFmpeg, configura `FFMPEG_DIR` antes de compilar.

## A quién va dirigido

- **Tuberías de video con IA**: el modelo genera XML, el motor renderiza el video, costo de integración mínimo
- **Aplicaciones Web**: renderizado/edición de video en el navegador, sin necesidad de servidor
- **Animación procedural**: renderizado acelerado por GPU determinista, consistente entre máquinas
- **Producción por lotes**: video basado en plantillas, cambiar datos = cambiar XML

## Referencias

- [Referencia de formato XML](skill/references/opencat.md)
- [Sistema de Animación](skill/references/animations.md)
- [Transiciones](skill/references/transitions.md)
- [API de Canvas](skill/references/canvaskit.md)
- [Plantillas](skill/references/templates.md)
- [Principios de Diseño](skill/references/design-principles.md)
- [Guía de Desarrollo](DEVELOPMENT.md) — Alineación de diseño Tailwind/Taffy y comparación SSIM engine/web
- [开发指南](DEVELOPMENT_ZH.md)
- [Arquitectura](ARCHITECTURE.md) — pipeline de renderizado completo desde XML/JSONL a píxeles
- [架构文档](ARCHITECTURE_ZH.md)
- [Guía de Migración](docs/MIGRATION.md) — ciclo de vida explícito, HostInputs, AudioPlan, RenderFrame, OCIR v4

## Comunidad

- Errores / Solicitudes de funciones → [Abrir un Issue](https://github.com/ZhouXiaolin/opencat/issues)
- Discusión en Linux Do → [OpenCat 社区讨论](https://linux.do/t/topic/2090262/7)

## Historial de Estrellas

[![Star History](https://api.star-history.com/svg?repos=ZhouXiaolin/opencat&type=Date)](https://www.star-history.com/#ZhouXiaolin/opencat&Date)

## Licencia

Licencia MIT
