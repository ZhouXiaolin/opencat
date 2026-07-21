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

## SSIM Engine/Web Pixel Comparison

Ensures Rust Skia output matches browser CanvasKit WASM output pixel-for-pixel.

### How it works

1. Engine renders a frame via `DefaultPipeline::render_frame` → RGBA
2. Browser opens `web/test-oracle.html` via ChromeDriver, CanvasKit parses the same source → `readPixels` → RGBA
3. `compute_ssim_rgba` writes both RGBA buffers to temp PNGs, calls `ffmpeg ssim` for structural similarity
4. Thresholds: normal frames ≥ 0.99, video-decoded frames ≥ 0.97

### Tests

```bash
# Single-frame oracle (specific example + frame number)
cargo test -p opencat-engine --lib -- --ignored web_frame_oracle

# Multi-frame sampling (default frames 0–413, step 10)
cargo test -p opencat-engine --lib -- --ignored profile_showcase_multi_frame_oracle

# CLI tool: custom interval, threshold
cargo build --bin opencat-web-compare --release
./target/release/opencat-web-compare examples/profile-showcase.jsonl \
  --out-dir out/compare-mp4 \
  --interval-secs 0.5
```

Location: `crates/opencat-engine/src/inspect/`

| File | Purpose |
|------|---------|
| `browser.rs` | ChromeDriver harness + static server + `compute_ssim_rgba` |
| `tests/web_frame_oracle.rs` | Single / multi-frame oracle tests |
| `tests/tailwind_layout/mod.rs` | Tailwind ↔ Taffy layout alignment tests |

### Environment Variables

| Variable | Purpose | Default |
|----------|---------|---------|
| `CHROME_BIN` | Chrome binary path | Auto-detected |
| `CHROMEDRIVER_BIN` | chromedriver path | Auto-detected |
| `CHROMEDRIVER_URL` | Remote WebDriver endpoint | None (uses local) |
| `MIN_SSIM` | Strict SSIM threshold | 0.99 |
| `VIDEO_MIN_SSIM` | Video frame SSIM threshold | 0.97 |
