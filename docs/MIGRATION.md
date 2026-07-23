# Migration Guide: Explicit Composition Lifecycle (#12 / #25)

> Status: current after contract phase (#24). This is the breaking migration
> note for hosts that still open compositions through pre-lifecycle seams.
>
> Scope: `opencat-core` / `opencat-engine` / `opencat-web` / `opencat` facade.
> Architecture detail: [`ARCHITECTURE.md`](../ARCHITECTURE.md) (ZH:
> [`ARCHITECTURE_ZH.md`](../ARCHITECTURE_ZH.md)).

## Mental model

```
Input (XML / JSONL)
  → CompositionDraft::parse / from_parsed
  → HostRequirements  (AssetId + kind + logical locator; no I/O)
  → Host: fetch / probe / load fonts / SRT text / script text
  → HostInputs
  → draft.prepare(inputs) → PreparedComposition
  → prepared.open_pipeline(scripts) → DefaultPipeline
  → render_frame(i) → RenderFrame { draw, media }
  → Engine (Skia) | Web (OCIR v4 → CanvasKit)
```

Core is a pure derivation kernel. Ordinary media **bytes never enter prepare**.
Hosts own fetch, cache, decode, platform APIs, and export.

## Production open paths

| Host | Entry | Lifecycle |
|------|-------|-----------|
| Engine / CLI | `opencat_engine::pipeline::open` / `opencat::open` | draft → host fetch → `HostInputs` → prepare → `open_pipeline` |
| Web | `WebRenderer::open_design` | same; JS facade is `opencat.js` |
| Core tests only | `PreparedComposition::open_pipeline` after manual `HostInputs` | `DefaultPipeline::open_with_prepared_catalog` is **`pub(crate)`** |

Do **not** call crate-private open helpers from outside core.

---

## Breaking API replacements

### Lifecycle / open

| Removed / contracted | Replacement |
|----------------------|-------------|
| Public `DefaultPipeline::open_with_prepared_catalog` | `CompositionDraft::prepare` → `PreparedComposition::open_pipeline` (crate-private open remains for lifecycle only) |
| `DefaultPipeline::open` / `open_with_font_db` / core fetch+probe open | Host fetch + `HostInputs` + prepare |
| `HashMapResourceCatalog` | `PreparedResourceCatalog` + host bytes/handles outside core |
| Dual production `ResourceCatalog` names | Metadata catalog only inside core; host loaders keep their own caches |
| Engine/CLI re-exports of core internals | Import `opencat_core` / `opencat_engine` modules directly |
| Long-lived deprecation wrappers on `opencat` | Narrow facade: `open`, `execute_render_frame`, `EnginePipeline`, media helpers |

### Script / session / platform seams

| Removed | Replacement |
|---------|-------------|
| `LiveScriptHost` / `ScriptRunner` / `ScriptRuntimeCache` | One `ScriptRealm` per pipeline (`open_pipeline`); hosts supply `JsContext` backend only |
| `RenderSession` / second layout session | Real pipeline inspect via `pipeline.inspect_frame` / engine inspect |
| `EnginePlatform` mega-trait | Split host modules (`loader`, `executor`, `media`, `consumer`) |
| FrameConsumer-based open paths | `render_frame` → `RenderFrame` → `execute_render_frame` (engine) or OCIR encode (web) |

### Audio / metadata placement (post-#18/#16)

| Topic | Contract |
|-------|----------|
| `AudioPlan` | Owned by **core** (`opencat_core::AudioPlan` / `collect_audio_plan`). Exposed on `CompositionInfo.audio_plan`. Hosts decode/mix only — do not re-walk the tree for offsets. |
| Video duration | `VideoInfoMeta.duration_micros` (`Option<DurationMicros>`), not mixed frame/ms units |
| Web JSON | `{ segments: [{ assetId, startMicros, endMicros, durationMicros }] }` via `WebRenderer.audio_plan()` / `getAudioPlan()` |

---

## HostInputs checklist

Before `prepare`, satisfy every `HostRequirements` request:

| Kind | Host supplies | Core does during prepare |
|------|---------------|--------------------------|
| Image | `insert_image(id, ImageMeta { width, height })` | Fail-fast if missing / zero size |
| Video | `insert_video(id, VideoInfoMeta { width, height, duration_micros })` | Fail-fast if missing layout-critical fields |
| Audio | `insert_audio(id)` (presence) | Catalog presence; plan from composition structure |
| Lottie | `insert_lottie(id, LottieMeta { … })` | Validate meta |
| Subtitle | `insert_subtitle_text(id, srt)` (soft-miss allowed) | Parse SRT, hydrate caption entries |
| Font | base `font_db` + `insert_document_font(id, bytes)` | Merge faces, family map, shaping db |
| Script | `insert_script_text(id, source)` for external scripts | Inject into drivers; one realm per pipeline |

Engine reference: `opencat_engine::pipeline::open` →
`open_parsed_host_owned_with_fonts`. Web reference: `open_design_pipeline` in
`crates/opencat-web/src/wasm_bridge.rs`.

---

## Metadata schema (ordinary media)

Core never reads image/video/audio/Lottie bytes in prepare. Minimum fields:

```text
ImageMeta      { width: u32, height: u32 }
VideoInfoMeta  { width: u32, height: u32, duration_micros: Option<DurationMicros> }
LottieMeta     { fps, in/out frame, size, dependency asset ids, … }
Audio          presence only at prepare; duration optional for host mixers
```

Probe may leave non-critical fields as `None`. Layout/time-critical zeros fail prepare.

Logical locators (`ResourceLocator`) are never real filesystem paths — hosts
join them to document base / VFS / URL themselves.

---

## AudioPlan

```rust
pub struct AudioPlan {
    pub segments: Vec<AudioSegment>,
}
pub struct AudioSegment {
    pub asset: AssetId,
    pub range: DurationRange, // half-open composition micros
}
```

Derived purely from composition attachment (timeline / scene) and transition
offsets. Web preview/export and engine `build_audio_track_from_pipeline` consume
this schedule; they must not invent segment starts from scene trees.

---

## RenderFrame (sole per-frame core→host contract)

```rust
pub struct RenderFrame {
    pub draw: DrawOpFrame,      // typed Skia-compatible IR
    pub media: FrameMediaPlan,  // what host must prepare this frame
}
```

`FrameMediaPlan` buckets (deduplicated):

- `images` — static `ImageRef`
- `video_frames` — canonical `AssetId` + authoritative `time_micros`
- `lottie_bundles` — bundle ids
- `runtime_effects` — SkSL effect refs
- `generated_images` — **full RGBA** for color-emoji (etc.); do **not** read
  `pipeline.generated_images()`

Hosts may cache platform textures by generated-image id; cache epoch is a host
concern (web stamps pipeline epoch into the OCIR envelope).

---

## DrawOp wire protocol (OCIR v4)

Single versioned envelope, encoded only in core (`encode_ir_envelope`):

```text
magic "OCIR" | version u32 (=4) | section_count u32 | pipeline_epoch u32
directory: repeated (section_id u32, offset u32, length u32)
payloads: OPS, F32_POOL, BYTES, BYTE_RANGES, STRINGS_UTF8, STRING_RANGES,
          PAINTS, PATHS, CHILDREN, EFFECTS, SUBTREES, GENERATED_IMAGES
```

- Rust: `opencat_core::ir::{encode_ir_envelope, IR_VERSION, IR_MAGIC}`
- TypeScript: `crates/opencat-web/web/src/draw-ir.ts` decoder (must stay field-locked)
- Cross-language fixture: `web/src/fixtures/ocir/roundtrip_v4.ocir`
  - Written by core test `write_ts_roundtrip_fixture_bytes`
  - Asserted field-by-field in `web/src/draw-ir.test.ts` (`core encoder → TS decoder`)

Do not maintain a second opcode table or envelope layout outside core.

---

## Web host notes

After `open_design` / `openDesign`:

1. Call `prepareCatalogVideoSources(catalogJson)` before injecting frames.
2. Per frame: read media plan → inject video RGBA → `build_frame_ir` → CanvasKit.
3. Use core `audio_plan()` for preview/export mix schedules.

Skipping video prepare blanks every `ImageRef::VideoFrame` (SSIM collapse on
video scenes). See `DEVELOPMENT.md` (Engine / Web pixel alignment).

---

## Facade surface (`opencat` crate)

Intended app/CLI imports:

```rust
use opencat::{
    open, execute_render_frame, EngineDrawExecutor, EnginePipeline,
    EngineLoader, MediaContext, AudioTrack, RqJsContext,
    build_audio_track_from_pipeline, duration_secs_to_frames,
};
```

Everything else: import from `opencat_core` / `opencat_engine` directly.

---

## Verification map (issue #25)

| Acceptance | Evidence |
|------------|----------|
| core/engine workspace tests + clippy | `cargo test -p opencat-core -p opencat-engine -p opencat --lib`; `cargo clippy -p opencat-core -p opencat-engine -p opencat --lib` |
| web bun test / typecheck / build | `cd web && bun run test && bunx tsc --noEmit && bun run build`; facade `cd crates/opencat-web/web && bun run build` |
| Same composition / resources / fonts / resolution / fps / target frames engine↔browser | `#[ignore]` oracles in `crates/opencat-engine/src/inspect/tests/web_frame_oracle.rs` |
| Static image / video / Lottie / captions / custom fonts / color emoji | oracles: alipay, profile-showcase, lottie-cat-loader, web-oracle-caption, web-oracle-font, web-oracle-emoji |
| Audio plan | core `media/audio_plan` unit tests; web `playback.test.ts`; engine `build_audio_track_from_pipeline` |
| Script isolation | engine `runtime::script_runtime_tests::{one_realm_shares…, separate_realms_do_not…}`; resolve isolation tests |
| DrawOp wire field parity | core `paint_path_string_effect_generated_fields_round_trip_in_core` + vitest fixture AC5 |
| SSIM thresholds | still ≥ 0.99; video-active ≥ 0.97; Lottie ≥ 0.985 — artifacts under `target/opencat-web-oracle/` |

### Oracle commands

Prereqs: Chrome + chromedriver (same major), ffmpeg, media on `:8080` for
profile-showcase (`http://127.0.0.1:8080/mp4|mp3|png/...`), web facade built.

```bash
cd crates/opencat-web/web && bun run build

cargo test -p opencat-engine --lib -- --ignored --nocapture \
  chromedriver_caption_frame_matches_engine
cargo test -p opencat-engine --lib -- --ignored --nocapture \
  chromedriver_custom_fonts_frame_matches_engine
cargo test -p opencat-engine --lib -- --ignored --nocapture \
  chromedriver_lottie_frame_matches_engine
cargo test -p opencat-engine --lib -- --ignored --nocapture \
  chromedriver_color_emoji_frame_matches_engine
cargo test -p opencat-engine --lib -- --ignored --nocapture \
  chromedriver_profile_showcase_frame_matches_engine
# multi-frame (video scenes), step 10:
cargo test -p opencat-engine --lib -- --ignored --nocapture \
  chromedriver_profile_showcase_all_frames_matches_engine
```

CLI sample:

```bash
cargo build --bin opencat-web-compare --release
./target/release/opencat-web-compare examples/profile-showcase.jsonl \
  --out-dir out/compare --interval-secs 0.5
```

Regression results for a given release train should be recorded in
[`regression-evidence.md`](regression-evidence.md) with SSIM min/avg and any
frames that used the video tolerance band. The current release-train numbers
(#25) live there now.
