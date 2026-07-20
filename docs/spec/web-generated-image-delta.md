# Web generated-image delta (#10)

> Parent epic: #2 — *Unify core resource metadata and frame render contract*.
> Depends on: #8 (persistent core pipeline on web), #9 (core `GeneratedImageTable`).
> Status: spec, implemented by issue #10.

## Problem

Color-emoji bitmap glyphs are the one image resource core must rasterize
itself: the font database, shaping and glyph-width math all live in core (#9),
so a host cannot re-derive the RGBA without re-implementing core's font
pipeline. The engine consumes these glyphs directly from the pipeline's
`GeneratedImageTable` via `ImageRef::Generated` (#9). On web, #8 left the path
as a **compile-time stub**: the OCIR encoder writes `ImageRef::Generated` with
`tag=2`, but the JS decoder falls through and treats tag=2 as a video ref, and
there is no way for JS to reach the RGBA. Result: color emoji is a no-op on web.

## Goal

Close the core→web color-emoji path so that color emoji renders on web, by
publishing the generated RGBA through the normal per-frame OCIR contract —
**once per glyph per pipeline**, with a pipeline epoch isolating caches across
designs — and without per-glyph `JS→WASM→JS` round-trips or an external
BlobStore/query bridge.

## Contract

### OCIR envelope, version 4

The envelope header gains a `pipeline_epoch: u32` immediately after the
section count. This is the authoritative generation counter for the pipeline
that produced this frame.

```
offset  field
0       magic       "OCIR" (4 bytes)
4       version     u32 LE == 4
8       section_count u32 LE
12      pipeline_epoch u32 LE          <-- NEW (v4)
16      sections[section_count] { id:u32, offset:u32, len:u32 }
…       payloads (4-byte aligned)
```

Version 3 envelopes (no epoch) are rejected by the decoder; v3 was never
released beyond this branch, so this is not an externally observable break.

A new section carries the generated-image delta:

```
SECTION_GENERATED_IMAGES = 12
  count: u32
  repeat count:
    id: u64           // GeneratedImageId (stable glyph cache key)
    width: u32
    height: u32
    rgba_len: u32
    rgba: [u8]        // RGBA_8888, unpremultiplied, width*height*4 bytes
```

### Delta semantics

`WebRenderer` keeps:
- `pipeline_epoch: u32`, bumped on every `open_design` (including the first);
- `published_generated: HashSet<GeneratedImageId>`, cleared on each epoch bump.

Each `build_frame_ir(frame)`:
1. Renders the frame (populating the pipeline's `GeneratedImageTable`).
2. Inspects `render.media.generated_images` — the set of IDs the core needs
   visible this frame.
3. Emits into section 12 only the entries whose id is **not** already in
   `published_generated`, copying RGBA from the pipeline's table. Missing
   entries are skipped (defensive; core always populates the table before
   emitting `ImageRef::Generated`).
4. Adds every id emitted this frame to `published_generated`.

So within one pipeline, each distinct glyph's RGBA is published **exactly
once**, on the first frame that references it. Subsequent frames carry an
empty section 12. When a new design opens, the epoch bumps, the published set
clears, and the next frame republishes everything JS needs.

### `ImageRef::Generated` in Draw IR

Unchanged from #9: `[tag:u8=2, id:u64, reserved:u32]`. The reserved u32 is
kept for layout parity with the core encoder; the decoder reads but ignores
it. The decoder resolves `id` against the `(pipeline_epoch, id)` cache.

## JS decoder (`draw-ir.ts`)

1. `decodeFrame` reads `pipeline_epoch` at offset 12. If the epoch differs
   from the cached epoch, drop every stale generated image (those whose cache
   key carries the old epoch) and update the cached epoch.
2. Section 12 (now a *required* section, since v4 always emits it, possibly
   with `count: 0`) is decoded into a list of `{id, width, height, rgba}`.
   For each entry, build a CanvasKit image via `CK.MakeImage({width, height,
   RGBA_8888, Unpremul, SRGB}, rgba, width*4)` and store it under
   `(epoch, id)` in `generatedImageCache`. This reuses the exact image-creation
   pattern already used for video-frame RGBA fallback.
3. `readImageRef` learns `tag === 2` → `{ type: 'generated', id }`.
4. `resolveImage` handles `'generated'` by looking up `(cachedEpoch, id)` and
   returning the cached `Image` (or `null` if the delta never arrived —
   treated as a no-op draw, matching the engine's defensive skip).

### Cache identity & eviction

The cache is `Map<string, Image>` keyed by ``${epoch}:${id}``. Because epoch
is monotonic and bumps only on `open_design`, eviction is O(stale entries) and
happens exactly when JS observes a new epoch in a frame — no separate
invalidation API is needed.

## Out of scope

- Changing `ImageRef::Generated`'s wire layout (already fixed by #9).
- The core `GeneratedImageTable` or `GeneratedImageId` (already fixed by #9).
- Engine-side generated-image consumption (already done in #9, unchanged).
- Any core audio plan / video plan / loader seam work — that is #11.
- External BlobStore or per-image WASM query bridges for emoji — explicitly
  rejected; the delta is the only path.

## Testing

- Rust: envelope encode/decode round-trip covering epoch, delta dedup across
  frames, and (id, width, height, RGBA) field fidelity. Also a delta-empty
  case (second frame referencing already-published glyphs).
- JS (`draw-ir.test.ts`, new): decode a hand-built v4 envelope with section 12,
  assert `(epoch, id)` cache is populated; assert epoch change evicts stale
  entries; assert `readImageRef` tag=2 yields `{type:'generated', id}`.
- Oracle: a new `web-oracle-emoji.xml` + ignored chromedriver test mirroring
  the engine `color_emoji_glyphs_render_via_generated_image_table` scenario,
  so the web↔engine emoji parity is exercised when run explicitly.
- Regression: non-emoji oracle designs (Alipay, profile-showcase, caption,
  custom-fonts, Lottie) must stay at SSIM ≥ 0.99 vs engine; emoji oracle
  passes once the path is live.
