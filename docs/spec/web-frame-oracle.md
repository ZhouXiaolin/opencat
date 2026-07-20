# Web Frame Oracle — web-vs-engine SSIM regression

The oracle test in
[`crates/opencat-engine/src/inspect/web_frame_oracle_tests.rs`](../../crates/opencat-engine/src/inspect/web_frame_oracle_tests.rs)
renders a single frame of a design **two ways** and compares them with SSIM:

1. **Engine** (ground truth): `render_single_frame_from_jsonl_with_base` — the
   native Rust engine pipeline (Skia), the same path `scripts/compare-ssim.sh`
   treats as canonical for engine-vs-engine regression.
2. **Web** (the system under test): the `opencat-web` wasm crate rendered in a
   headless Chrome via CanvasKit, driven by `web/test-oracle.html`.

Both produce an RGBA buffer for the same `(composition, frame)`. The oracle
writes each to a unique temporary PNG pair and runs
`ffmpeg -filter_complex ssim`, matching the metric `compare-ssim.sh` uses for
whole-video comparison. A frame normally passes when `SSIM(All) >= 0.99` (the
engine-vs-engine target is `1.000000`). The Lottie fixture uses `0.985` because
native Skia and CanvasKit rasterize Skottie's aspect-fit hard edges on opposite
sides of a sub-pixel boundary; its geometry and content still match.

## Why

Issue #8 migrates the web renderer onto the host-owned persistent core pipeline
(the same `DefaultPipeline` the engine uses). The oracle is the end-to-end
proof that the migrated web path renders identically to the engine for a given
frame — covering Draw IR, the prepared-catalog probe chain, caption hydration,
font injection, and the single-render `prepare_frame` media-plan path.

## Running it

The tests are `#[ignore]`d because they need chromedriver + Chrome + the web
facade built.

```bash
cd /home/solaren/Projects/opencat-issue-2
export SKIA_BINARIES_URL=file:///tmp/skia-binaries.tar.gz

# 1. Build the web facade (wasm + vite bundle -> crates/opencat-web/web/dist)
cd crates/opencat-web/web && bun install && bun run build && cd -

# 2. Install the dev app deps (CanvasKit / web-demuxer, served by the oracle)
cd web && bun install && cd -

# 3. Run the oracle (needs chromedriver + google-chrome on PATH, or
#    CHROMEDRIVER_URL/CHROMEDRIVER_BIN/CHROME_BIN env vars)
cargo test -p opencat-engine --lib chromedriver_ -- --ignored --nocapture
```

On failure, per-frame `engine.png` / `web.png` / `diff.png` artifacts are
written under `target/opencat-web-oracle/<stem>-frame-<NNNN>/`.

## Covered designs

- `chromedriver_alipay_finance_homepage_first_frame_matches_engine` —
  `examples/alipay-finance-homepage.jsonl` frame 0.
- `chromedriver_profile_showcase_frame_matches_engine` —
  `examples/profile-showcase.jsonl` frame 0 (covers
  video/image/audio/canvas/icon/transition).
- `chromedriver_caption_frame_matches_engine` —
  `examples/web-oracle-caption.jsonl` frame 0 (covers web subtitle preload and
  core caption hydration).
- `chromedriver_custom_fonts_frame_matches_engine` —
  `examples/web-oracle-font.xml` frame 0 (covers manifest-declared custom font
  fetch and font database injection).
- `chromedriver_lottie_frame_matches_engine` —
  `examples/lottie-cat-loader.xml` frame 125 (covers Lottie bundle preload,
  frame planning, Draw IR string interning, and CanvasKit Skottie replay).

## Status (issue #8)

The migration compiles and all relevant suites pass (core 552, engine render
21, web vitest 28, wasm and engine clippy); engine-vs-engine SSIM stays
`1.000000` for `profile-showcase` and `xhs-neo-brutalism`. With a freshly
rebuilt wasm facade, all browser oracles pass through the host-owned persistent
pipeline:

- `alipay-finance-homepage` frame 0: SSIM `0.996501`
- `profile-showcase` frame 0: SSIM `0.998217`
- caption frame 0: SSIM `0.992731`
- custom font frame 0: SSIM `0.998727`
- Lottie frame 125: SSIM `0.986303`

The Lottie oracle originally trapped in Draw IR encoding because
`LottieRect.bundle_id` was absent from the encoded string table. It now interns
the bundle id and replays through CanvasKit's native `render(canvas, dstRect)`
contract, avoiding both the trap and a previous double-scaling bug. These
failures occurred before or independently of GPU rasterization, and the oracle
forces ANGLE SwiftShader, so they were not caused by host GPU/OpenGL
acceleration. `console_error_panic_hook` remains installed to preserve Rust
panic messages for future browser regressions.

The oracle server proxies `/assets-proxy/*` to the sample asset server at
`127.0.0.1:8080`, matching the web fetch layer's localhost URL rewrite. The
tests remain `#[ignore]`d because they require ChromeDriver, Chrome, ffmpeg,
the built web facade, and (for profile assets) the sample asset server. SSIM
temporary files include a per-comparison id so the five tests are safe to run
in parallel.
