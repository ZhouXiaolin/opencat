# Development Guide

## Tailwind + Taffy + ChromeDriver Layout Alignment

Ensures Rust's Taffy layout engine matches real Chrome CSS layout behavior.

### How it works

Each fixture is an HTML fragment with `data-oc-id` attributes and Tailwind classes in `class=""`:

1. Collect all class names → compile CSS via `@tailwindcss/node` (through bun)
2. Generate a full HTML document, open in ChromeDriver
3. Read positions via WebDriver `getBoundingClientRect()`
4. Rust side: same class names → `parse_class_name` → Taffy layout → `collect_frame_layout_rects`
5. Compare both rect sets: ids must match exactly, x/y/width/height within tolerance (text line-height 2px, others 1px)

### Tests

```bash
# Auto-generated fixtures covering all Tailwind v4.2.2 layout utilities (71 groups, 505 candidates)
cargo test chromedriver_tailwind_extended_flex_layout_matches_taffy

# Hand-written integration fixtures (complex multi-utility combinations)
cargo test chromedriver_tailwind_layout_matches_taffy

# Verify fixture generator covers all utilities
cargo test generated_layout_fixture_templates_cover_utilities_manifest
```

Location: `crates/opencat-engine/src/inspect/tests/tailwind_layout/`

Dependencies: ChromeDriver, Chrome, bun dependencies in `crates/opencat-engine/testsupport/`.

---

## Engine / Web pixel alignment (SSIM frame oracle)

Compares **native engine (Skia)** vs **web (WASM + CanvasKit)** frame-by-frame with SSIM.

### Pipeline

1. Engine: `DefaultPipeline::render_frame` → RGBA (ground truth)
2. Headless Chrome loads `web/test-oracle.html` via ChromeDriver
3. Web: `open_design` → `prepareCatalogVideoSources` → inject video frames →
   `build_frame_ir` → CanvasKit draw → `readPixels` → RGBA
4. `ffmpeg ssim` via `compute_ssim_rgba`
5. Thresholds: **≥ 0.99** (pipeline / still frames), **≥ 0.97** (frames with active video)

Failing frames write `engine.png` / `web.png` / `diff.png` under:

```text
target/opencat-web-oracle/<stem>-frame-NNNN/
```

### Prerequisites

| Dependency | Notes |
|------------|--------|
| Chrome + ChromeDriver | Same major version; auto-detected or set `CHROME_BIN` / `CHROMEDRIVER_BIN` |
| FFmpeg | `ffmpeg` on `PATH` (SSIM filter) |
| Node / npm (or bun) | Build the web facade |
| Dev app deps | `cd web && bun install` (or npm) — CanvasKit + `web-demuxer` for the oracle server |
| Media server on **:8080** | Compositions such as `examples/profile-showcase.jsonl` load `http://127.0.0.1:8080/mp4/...` |

Serve local media if needed, e.g.:

```bash
# from the directory that contains mp4/ png/ mp3/ used by examples
python3 -m http.server 8080
# or any static server bound to 127.0.0.1:8080
```

### Build the web facade (required before every oracle run that needs a fresh JS/WASM build)

```bash
cd crates/opencat-web/web
npm run build          # wasm-pack + vite + types; copies web-demuxer.wasm into dist/
# or only JS after pure TS changes:
# npm run build:lib && npm run build:types
```

The oracle static server maps:

- `/test-oracle.html` → `web/test-oracle.html`
- `/wasm/*` → `crates/opencat-web/web/dist/*` (includes worker + `web-demuxer.wasm`)
- `/canvaskit/*` → `web/node_modules/canvaskit-wasm/bin/full/*`
- `/assets/*`, `/fonts/*` → repo assets

### Run tests

All oracle tests are `#[ignore]` (need ChromeDriver + built facade). Always pass `--ignored`.

```bash
# Smoke: profile-showcase frame 0 (no video)
cargo test chromedriver_profile_showcase_frame_matches_engine \
  --package opencat-engine --lib -- --ignored --nocapture

# Full multi-frame sweep: frames 0–413 step 10 (covers video scenes 2 & 3)
cargo test chromedriver_profile_showcase_all_frames_matches_engine \
  --package opencat-engine --lib -- --ignored --nocapture

# Other single-frame oracles (filter by name substring)
cargo test chromedriver_ --package opencat-engine --lib -- --ignored --nocapture
# includes:
#   chromedriver_alipay_finance_homepage_first_frame_matches_engine
#   chromedriver_caption_frame_matches_engine
#   chromedriver_custom_fonts_frame_matches_engine
#   chromedriver_lottie_frame_matches_engine
#   chromedriver_color_emoji_frame_matches_engine
```

### CLI: custom interval / output dir

```bash
cargo build --bin opencat-web-compare --release
./target/release/opencat-web-compare examples/profile-showcase.jsonl \
  --out-dir out/compare-mp4 \
  --interval-secs 0.5
```

### Environment variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `CHROME_BIN` | Chrome binary | Auto-detected |
| `CHROMEDRIVER_BIN` | chromedriver path | Auto-detected |
| `CHROMEDRIVER_URL` | Remote WebDriver (skip local spawn) | unset |
| `MIN_SSIM` | Strict SSIM (code constant today: `0.99`) | `0.99` |
| `VIDEO_MIN_SSIM` | Video-active SSIM (code constant today: `0.97`) | `0.97` |

> Note: thresholds in `web_frame_oracle.rs` are currently compile-time constants; env vars in the table are reserved / used by tooling where wired.

### Code map

| Path | Role |
|------|------|
| `crates/opencat-engine/src/inspect/browser.rs` | ChromeDriver harness, static server, SSIM |
| `crates/opencat-engine/src/inspect/tests/web_frame_oracle.rs` | Oracle test cases |
| `web/test-oracle.html` | Browser entry: open design, prepare video, draw IR |
| `crates/opencat-web/web/src/media/video-frame-injector.ts` | `prepareCatalogVideoSources` + inject |
| `crates/opencat-web/web/dist/` | Built facade served at `/wasm/` |

### Host video contract (web)

After `open_design` / `openDesign`, hosts **must** call `prepareCatalogVideoSources(catalogJson)` before `injectVideoFramesForRender`. Otherwise WebCodecs never sees the asset and every `ImageRef::VideoFrame` draws blank (SSIM collapses on large video regions).
