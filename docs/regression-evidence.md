# Regression Evidence — Cross-Platform Regression Gate (#48) / Explicit Lifecycle (#12 / #25)

> This document records two verification passes:
> 1. Issue #48 re-verification on the current `issue-48-engine-web-cross-platform-regression-gate` branch
> 2. Original Issue #25 verification (preserved below)

## Run 1 — Issue #48 Cross-Platform Regression Gate

> Re-run of the full oracle suite on the `issue-48-engine-web-cross-platform-regression-gate`
> branch. SSIM thresholds, test environment, and oracles are identical to the
> Issue #25 run. This is the canonical evidence artifact for Issue #48 AC 5–8.

### Environment

| Component | Version |
|-----------|---------|
| Date | 2026-07-23 |
| Branch | `issue-48-engine-web-cross-platform-regression-gate` (on `d6aab44` + `ad6bfd2`) |
| Google Chrome | 150.0.7871.181 |
| ChromeDriver | 150.0.7871.128 |
| ffmpeg | 7.1.1-1ubuntu4.2 |
| Media host | `miniserve` on `127.0.0.1` (dynamic port) |

### Thresholds (from `web_frame_oracle.rs`)

- Strict floor: **SSIM ≥ 0.990**
- Video-active relaxation: **SSIM ≥ 0.970**
- Lottie: **SSIM ≥ 0.985**

### Single-frame oracles

All 6 single-frame oracles pass:

| Oracle (`#[ignore]` test) | Composition | Frame | Resolution | SSIM | Band |
|---------------------------|-------------|------:|-----------:|-----:|------|
| `chromedriver_caption_frame_matches_engine` | `web-oracle-caption.jsonl` | 0 | 480×240 | **0.992731** | strict ✓ |
| `chromedriver_custom_fonts_frame_matches_engine` | `web-oracle-font.xml` | 0 | 480×240 | **0.998727** | strict ✓ |
| `chromedriver_color_emoji_frame_matches_engine` | `web-oracle-emoji.xml` | 0 | 96×80 | **1.000000** | strict ✓ |
| `chromedriver_lottie_frame_matches_engine` | `lottie-cat-loader.xml` | 125 | 400×300 | **0.986303** | lottie ✓ |
| `chromedriver_profile_showcase_frame_matches_engine` | `profile-showcase.jsonl` | 0 | 1280×720 | **0.998217** | strict ✓ |
| `chromedriver_alipay_finance_homepage_first_frame_matches_engine` | `alipay-finance-homepage.jsonl` | 0 | 390×844 | **0.996501** | strict ✓ |

Color-emoji lands at 1.000000 — the generated-image full-RGB path matches the engine exactly.

### Multi-frame oracle

`chromedriver_profile_showcase_all_frames_matches_engine` — 42 frames sampled
across the full timeline (frames 0–413, step 10; strict 0.990, video 0.970):

- **Result: 42/42 passed.**
- **Min SSIM = 0.995452** (frame 250).
- **Max SSIM = 0.999837** (frame 320).
- No frame required the relaxed video band — every sampled frame cleared the
  strict 0.990 floor, including the video scenes.

Per-frame SSIM distribution (all ≥ 0.995):

```
f000 0.998217  f010 0.997390  f020 0.997616  f030 0.996114  f040 0.997303
f050 0.997455  f060 0.997433  f070 0.997454  f080 0.997444  f090 0.997427
f100 0.998324  f110 0.999648  f120 0.999048  f130 0.998639  f140 0.998632
f150 0.998828  f160 0.998830  f170 0.998829  f180 0.998832  f190 0.998831
f200 0.997621  f210 0.997204  f220 0.995859  f230 0.996071  f240 0.995614
f250 0.995452  f260 0.995838  f270 0.996618  f280 0.996183  f290 0.995982
f300 0.996159  f310 0.996723  f320 0.999837  f330 0.999040  f340 0.998778
f350 0.998306  f360 0.998296  f370 0.998306  f380 0.998298  f390 0.998269
f400 0.998324  f410 0.998316
```

Artifacts (per-frame PNGs + SSIM) are written under `target/opencat-web-oracle/`
during the run.

### Suite summaries

- `cargo test -p opencat-core -p opencat-engine -p opencat --lib` → **69 passed, 0 failed, 0 ignored** (all oracle tests executed inline).
- `cargo test -p opencat-engine --lib -- --ignored web_frame_oracle --nocapture` → **7 passed, 0 failed** (SSIM evidence above).
- `cargo clippy -p opencat-core -p opencat-engine -p opencat --lib` → finished, no errors (pre-existing warnings only).
- `cd web && bun run test && bunx tsc --noEmit && bun run build` → passed.
- Facade `cd crates/opencat-web/web && bun run build:types && bunx vite build` → ok.

### AC coverage

| AC | Description | Status |
|----|-------------|--------|
| AC 1 | Rust test suite passes (core + engine + opencat) | ✓ 69 passed |
| AC 2 | Clippy clean on core + engine + opencat lib | ✓ |
| AC 3 | Web root ts: tests, typecheck, build | ✓ |
| AC 4 | Crate web facade: types, vite build | ✓ |
| AC 5 | Single-frame oracle: SSIM ≥ threshold | ✓ (6/6) |
| AC 6 | OCIR byte determinism (`encode_non_trivial_frame_is_byte_deterministic`) | ✓ |
| AC 7 | Multi-frame oracle: 42/42 above threshold | ✓ |
| AC 8 | CI workflow automates regression gate | ✓ (`.github/workflows/regression.yml`) |

---

## Run 2 — Original Issue #25 (Explicit Lifecycle)

> Recorded run of the cross-platform engine↔browser SSIM oracles after the
> contract phase (#24). Numbers below are the artifact for issue #25's
> acceptance criterion "_engine/web frame output passes the existing browser
> oracle/SSIM threshold; differences clearly recorded_". Re-run commands are in
> [`MIGRATION.md`](MIGRATION.md#verification-map-issue-25).

### Environment

| Component | Version |
|-----------|---------|
| Date | 2026-07-22 |
| Branch | `worktree-issue-25-regression-migration` (on `b07aa14` + working tree) |
| Google Chrome | 150.0.7871.128 |
| ChromeDriver | 150.0.7871.128 |
| ffmpeg | 7.1.1-1ubuntu4.2 |
| Media host | `miniserve` on `127.0.0.1:8080` (`/mp4 /mp3 /png`) |

## Thresholds (from `web_frame_oracle.rs`)

- Strict floor: **SSIM ≥ 0.990**
- Video-active relaxation: **SSIM ≥ 0.970**
- Lottie: **SSIM ≥ 0.985**

## Single-frame oracles

| Oracle (`#[ignore]` test) | Composition | Frame | Resolution | SSIM | Band |
|---------------------------|-------------|------:|-----------:|-----:|------|
| `chromedriver_caption_frame_matches_engine` | `web-oracle-caption.jsonl` | 0 | 480×240 | **0.992731** | strict ✓ |
| `chromedriver_custom_fonts_frame_matches_engine` | `web-oracle-font.xml` | 0 | 480×240 | **0.998727** | strict ✓ |
| `chromedriver_color_emoji_frame_matches_engine` | `web-oracle-emoji.xml` | 0 | 96×80 | **1.000000** | strict ✓ |
| `chromedriver_lottie_frame_matches_engine` | `lottie-cat-loader.xml` | 125 | 400×300 | **0.986303** | lottie ✓ |
| `chromedriver_profile_showcase_frame_matches_engine` | `profile-showcase.jsonl` | 0 | 1280×720 | **0.998217** | strict ✓ |

Color-emoji lands at 1.000000 — the generated-image full-RGB path matches the
engine exactly.

## Multi-frame oracle

`chromedriver_profile_showcase_all_frames_matches_engine` — 42 frames sampled
across the full timeline (frames 0–413, step 10; strict 0.990, video 0.970):

- **Result: 42/42 passed.**
- **Min SSIM = 0.995452** (frame 250).
- **Max SSIM = 0.999837** (frame 320).
- No frame required the relaxed video band — every sampled frame cleared the
  strict 0.990 floor, including the video scenes.

Per-frame SSIM distribution (all ≥ 0.995):

```
f000 0.998217  f010 0.997390  f020 0.997616  f030 0.996114  f040 0.997303
f050 0.997455  f060 0.997433  f070 0.997454  f080 0.997444  f090 0.997427
f100 0.998324  f110 0.999648  f120 0.999048  f130 0.998639  f140 0.998632
f150 0.998828  f160 0.998830  f170 0.998829  f180 0.998832  f190 0.998831
f200 0.997621  f210 0.997204  f220 0.995859  f230 0.996071  f240 0.995614
f250 0.995452  f260 0.995838  f270 0.996618  f280 0.996183  f290 0.995982
f300 0.996159  f310 0.996723  f320 0.999837  f330 0.999040  f340 0.998778
f350 0.998306  f360 0.998296  f370 0.998306  f380 0.998298  f390 0.998269
f400 0.998324  f410 0.998316
```

Artifacts (per-frame PNGs + SSIM) are written under `target/opencat-web-oracle/`
during the run.

## DrawOp wire parity

| Side | Test | Result |
|------|------|--------|
| Rust encoder | `paint_path_string_effect_generated_fields_round_trip_in_core` | ok |
| Rust fixture writer | `write_ts_roundtrip_fixture_bytes` → `web/src/fixtures/ocir/roundtrip_v4.ocir` | ok, in sync (no diff vs committed) |
| TypeScript decoder | `draw-ir.test.ts` (15 tests, vitest) | ok |

The committed fixture is byte-identical to what core regenerates, so the TS
decoder is asserting against the current Rust encoder.

## Feature coverage map (parent epic #12 acceptance points)

| Feature | Cross-platform evidence |
|---------|------------------------|
| Static image | profile-showcase frames (static scenes) SSIM ≥ 0.995 |
| Video | profile-showcase multi-frame oracle, 42 frames, min 0.995452 |
| Lottie | `chromedriver_lottie_frame_matches_engine` 0.986303 (see fix below) |
| Captions / subtitles | `chromedriver_caption_frame_matches_engine` 0.992731 |
| Custom fonts | `chromedriver_custom_fonts_frame_matches_engine` 0.998727 |
| Color emoji | `chromedriver_color_emoji_frame_matches_engine` 1.000000 |
| Audio plan | core `media::audio_plan` unit tests; web `playback.test.ts` (7); engine `build_audio_track_from_pipeline` |
| Script isolation (one realm per pipeline) | engine `runtime::script_runtime_tests` (`separate_realms_do_not_share_js_globals`, etc.) |

## Regression found and fixed: web Lottie byte key

Running the Lottie oracle surfaced a real bug, fixed in
`crates/opencat-web/src/resource/wasm_api.rs`:

- **Before:** `preload_assets` stored the Lottie primary JSON only under the
  bundle id `lottie:{element_id}`. Core's probe looked Lottie bytes up
  by the path/url-derived probe key, so it got a miss → empty bytes → the
  Lottie scene rendered blank → SSIM collapse.
- **After:** the BlobStore now holds the primary JSON under **both** keys — the
  bundle id (for Skottie / `FrameMediaPlan` / DrawOp) and the path/url probe key
  (for host metadata extraction). This mirrors the lookup contract the engine satisfies
  through its separate handle map.
- **Evidence:** Lottie oracle recovered to **0.986303** (≥ 0.985 band) after the
  fix; was failing before.

## Suite summaries

- `cargo test -p opencat-core -p opencat-engine -p opencat --lib` → **61 passed, 0 failed, 7 ignored** (the 7 ignored are the chromedriver oracles above).
- `cargo clippy -p opencat-core -p opencat-engine -p opencat --lib` → finished, no errors (pre-existing warnings only, none in scope of #25).
- `cd web && bun run test && bunx tsc --noEmit && bun run build` → **47 tests passed**, tsc 0, build ok.
- Facade `cd crates/opencat-web/web && bun run build` → ok.
