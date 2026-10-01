
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

  <video width="60%" controls autoplay loop muted playsinline src="https://github.com/user-attachments/assets/e3263ec6-a36c-4b73-ba4c-c0e6b64b86ee"></video>

</div>

XML define escenas, animaciones y diseños. Skia GPU renderiza, FFmpeg codifica a MP4: determinista, multiplataforma y consistente entre máquinas. Sin instantáneas de Chromium, sin Puppeteer, sin un pipeline de renderizado Web sobrecargado.

Un video es simplemente un archivo XML (esta es la escena A de [`examples/k3-promo.xml`](examples/k3-promo.xml), el spot de arriba):

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
    var T = ctx.time;                       // segundos a lo largo de los 16.4667s
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
// Línea de tiempo declarativa — escena A de examples/k3-promo.xml
ctx.timeline()
  .set('azoom', { scale: 1.07 }, 0.933)
  .to('azoom', { scale: 0.9417, duration: 3.667, ease: 'none' }, 0.933)
  .to(['afield', 'afieldp'], { opacity: 0.35, duration: 0.4333, ease: 'power1.inOut' }, 1.9667)
  .fromTo('apwrap', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power1.inOut' }, 3.03);

// Texto por carácter estilo GSAP: los parts de splitText se asientan con power2.out
var parts = ctx.splitText('otxt', { type: 'chars' });
var tl = ctx.timeline();
parts.forEach(function (p, i) {
  tl.set(p, { opacity: 0 }, 7.35)
    .set(p, { opacity: 1 }, 7.5333)
    .fromTo(p, { x: (i - 6.5) * 21 }, { x: 0, duration: 0.43, ease: 'power2.out' }, 7.5663);
});
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

Define componentes reutilizables con `<template>`, parametriza con `$variable` y compón con `<slot>`. `k3-promo` extrae seis bloques compartidos del árbol de nodos — se analizan una vez y se expanden a nodos planos antes de renderizar:

```xml
<template name="stage-canvas"><canvas id="$id" class="absolute inset-0 w-[1920px] h-[1080px]"></canvas></template>
<template name="cdot"><div id="$id" class="absolute w-[$s] h-[$s] bg-[$c]"></div></template>
<template name="agent-pill"><div id="$id" class="absolute opacity-0"><div id="$id-box" class="px-[14px] py-[10px] border-[1.5px] border-[#dcdcde]">…</div></div></template>

<!-- los puntos de llamada solo pasan las diferencias -->
<stage-canvas id="afield-canvas" />
<cdot id="a-tl" s="13px" c="#d8d8da" />
<agent-pill id="ap1" t="Agentic" />
```

Las plantillas se expanden en tiempo de análisis: costo en tiempo de ejecución cero, totalmente composables y admiten anidamiento. Un `<slot>` compone contenido desde el punto de llamada:

```xml
<template name="card">
  <div class="w-[400px] rounded-xl bg-$bg p-6"><h2 class="text-$titleColor">$title</h2><slot name="body" /></div>
</template>
<card bg="white" titleColor="gray-900" title="Hello">
  <slot name="body"><p class="text-gray-500">Contenido de la tarjeta.</p></slot>
</card>
```

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

El contenido de un subárbol de un nodo `<canvas>` puede texturizarse en vivo y alimentarse a un efecto por píxel. Los efectos se escriben como **lambdas de JS** (compiladas, nunca ejecutadas — Rust analiza el código, lo baja automáticamente a SKSL o lo despacha al renderizador de CPU, y no codifica rígidamente ningún algoritmo de efecto). Las superficies offscreen (`ctx.createSurface`) sirven también como **render targets** para datos que se construyen una vez y se muestrean por fotograma. La escena H de `examples/k3-promo.xml` construye todo su campo de distancia de disolución en JS:

```js
// 1) rasteriza el lockup una sola vez en una superficie offscreen
var ox = ctx.createSurface('k3dis-off', 1920, 1080);
ox.fillStyle = '#fff'; ox.fillText('K3. Now Open', 962, 586);

// 2) siembra una máscara + campo de distancia — lambda de clase pixel que muestrea
//    esa superficie como child
var field = ctx.createSurface('k3dis-field', 1460, 222);
field.runEffect(
  (uv, src) => {
    const c = src.eval(uv + [232, 430]);
    const m = byte(c.a * 255 + 0.5);
    const d = m > 120 ? 0 : 3000;                      // R=mask, G/B=dist lo/hi
    return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1];
  }, null, null, [{ __opencatShader: 'surface', id: 'k3dis-off' }]);

// 3) barridos chamfer — lambda de clase scan, in situ; get(dx,dy) lee el búfer en
//    progreso y el ejecutor posee el orden de recorrido (forward/backward)
field.scanPass((get) => {
  const at = (n) => byte(n.g * 255 + 0.5) + byte(n.b * 255 + 0.5) * 256;
  const c = get(0, 0), d = at(c);
  const m = min(min(d, at(get(-1, 0)) + 3), min(at(get(0, -1)) + 3,
              min(at(get(-1, -1)) + 4, at(get(1, -1)) + 4)));
  return [c.r, m % 256 / 255, floor(m / 256) / 255, 1];
}, 'forward');

// 4) cada fotograma: una lambda muestrea la superficie del campo directamente — sin bake
var DIS = CK.Effect.fromLambda((uv, field, u) => {
  const c = field.eval(uv);
  const d = (byte(c.g * 255 + 0.5) + 256 * byte(c.b * 255 + 0.5)) / 3;
  /* ruido scramble · dust · glow → RGBA straight */
}, { uniforms: [['x0','float'], ['f','float'], ['ep','float'], ['pa','float4']] });
```

Cualquier subárbol HTML: diseño, imágenes, texto, video → textura → shader → salida. Los búferes de píxeles nunca cruzan el puente JS y el lado Rust solo tiene un **ejecutor genérico**: `surface.runEffect` reescribe con una lambda de clase pixel, `surface.scanPass` recorre in situ con una lambda de clase scan y `surface.bake(key)` registra los píxeles como imagen del fotograma para muestreo por shader. Si necesitas control directo de SKSL, los shaders escritos a mano con `CK.RuntimeEffect.Make(sksl)` siguen funcionando. Referencia: `examples/k3-promo.xml` escena H; guía: `skill/references/canvaskit.md`.

### Más

- **Diseño estilo Tailwind**: `class="flex items-center justify-center gap-4"`
- **Mezcla de audio**: multi-pista, adjunto a la escena, mezcla automática a la salida
- **Motor de subtítulos**: análisis de SRT, visualización persistente entre escenas
- **Iconos Lucide**: +2000 iconos listos para usar
- **Renderizado determinista**: `value = f(time)`, consistente entre máquinas

## Inicio rápido

```bash
# Renderizar MP4 (el spot de arriba)
cargo run --bin opencat -- examples/k3-promo.xml

# Visor de vista previa de escritorio (macOS / Windows / Linux)
cargo run --bin opencat-see -- path/to/input.xml
```

> Web (WASM): `cd crates/opencat-web/web && npm run build`, requiere un entorno `Cross-Origin-Isolated`.

### Alineación Engine / Web (métricas de píxel k3diff)

Comparación píxel a píxel del Skia nativo vs WASM CanvasKit con ChromeDriver usando métricas de píxel duras (`mae`/`maxd`/`p8`, no SSIM). También puede comparar el render web directamente contra el video de referencia original con `--reference`. Pasos completos: **[Guía de Desarrollo](DEVELOPMENT.md#engine--web-pixel-alignment-k3diff-frame-oracle)**.

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
- [Guía de Desarrollo](DEVELOPMENT.md) — Alineación de diseño Tailwind/Taffy y comparación k3diff engine/web
- [Guía de Desarrollo (chino)](DEVELOPMENT_ZH.md)
- [Arquitectura](ARCHITECTURE.md) — pipeline de renderizado completo desde XML/JSONL a píxeles
- [Arquitectura (chino)](ARCHITECTURE_ZH.md)
- [Guía de Migración](docs/MIGRATION.md) — ciclo de vida explícito, HostInputs, AudioPlan, RenderFrame, OCIR v5

## Comunidad

- Errores / Solicitudes de funciones → [Abrir un Issue](https://github.com/ZhouXiaolin/opencat/issues)
- Discusión en Linux Do → [Comunidad OpenCat](https://linux.do/t/topic/2090262/7)

## Historial de Estrellas

[![Star History](https://api.star-history.com/svg?repos=ZhouXiaolin/opencat&type=Date)](https://www.star-history.com/#ZhouXiaolin/opencat&Date)

## Licencia

Licencia MIT
