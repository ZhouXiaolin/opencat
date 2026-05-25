# Implementation Notes — canvaskit-js RuntimeEffect

## Spec Review Decisions (2026-05-25)

### A' Approach: Inline IR Variant vs Parallel Tables

**Original spec proposed:** Extend `CanvasMutations` with 4 parallel tables (`script_effects`, `script_uniform_bytes`, `script_uniform_ranges`, `script_children`) + local-ID remap in `execute_draw_op`.

**Review found:** Parallel tables lost during `apply_to_canvas` (only `commands.extend`, no parallel table propagation). `ElementDrawSlot` only carries `Vec<DrawOp>`.

**Three fix options rejected:**
- **A)** Full pipeline parallel table propagation — 5+ files changed, multi-layer ID remapping
- **B)** One-shot merge in `display/build.rs` — fragile
- **C)** Bypass pipeline — breaks existing mutation stack merge

**A' chosen:** Add `DrawOp::ScriptRuntimeEffect { sksl, uniforms_bytes, children, dst }` as an inline intermediate IR variant. Data rides inside the op, naturally survives `commands.extend` with zero pipeline changes.

**Tradeoffs:**
- Pro: No new fields on `CanvasMutations`, `DrawScriptDisplayItem`, `ElementDrawSlot` — all 7 construction sites unchanged
- Pro: No multi-layer ID remap needed — hash computed once in render helpers
- Pro: Follows existing `DrawSubtreePicture` no-op pattern in engine/web
- Con: New IR variant (but follows precedent of `DrawSubtreePicture` as intermediate op)
- Con: `uniforms_bytes: Vec<u8>` + `children: Vec<RuntimeEffectChildRef>` clone per-frame via `commands.extend` — acceptable for short video; long video follow-up can lazy-intern

### Hash Strategy

**Original spec proposed:** JS-side FNV-1a 64-bit (BigInt), pass hi/lo u32 through binding.

**Changed to:** Rust-side `rustc-hash::FxHasher` (already a dependency) on `sksl.as_bytes()`, computed in `execute_draw_op` during ScriptRuntimeEffect → RuntimeEffect translation. `intern_effect` dedup by hash unchanged.

**Tradeoffs:**
- Pro: Zero JS complexity — no `TextEncoder`, no BigInt, no hi/lo split
- Pro: Hash computed once per unique SkSL at render time, then deduped
- Con: Hash not available for JS-side dedup (not needed — JS side only wraps strings)

### Make() Return Value

**Clarified:** `CK.RuntimeEffect.Make(sksl)` only validates non-empty string. Returns null for empty/invalid, otherwise wraps sksl. Actual compile deferred to render `intern_effect`. Fallback in XML is for null-return case only.

### Tile Mode Handling

Engine hardcodes `(Clamp, Clamp)` at `replay.rs:516`. Binding receives tile mode from JS but stores only asset_id in `RuntimeEffectChildRef::Image`. Follow-up: propagate tile mode through `ImageRef` or a wrapper.

### Construction Sites Verified

7 `DrawScriptDisplayItem` construction sites confirmed:
- `display/build.rs:89,238`
- `analyze/compositor.rs:289,384,1067,1099`
- `analyze/fingerprint/mod.rs:817`

With A' approach, none need changes.

### ensurePaint Location

Exists at `canvas_api.js:192`, already guards `instanceof Paint`. Reused in new drawRect shader branch.

### Dependencies

- `rustc-hash = "2.1"` — already in `Cargo.toml`, used for SkSL hash
- `serde_json` — already available, used for `children_json` deserialization in binding

---

## Implementation Pass (2026-05-25)

### RuntimeEffectChildRef did NOT derive Hash

Spec claimed `RuntimeEffectChildRef`, `ShaderSpec`, and `ShaderType` already derived `Hash`. They only had `#[derive(Clone, Debug, PartialEq)]`. Added manual Hash impls in `draw_types.rs` because `f32` fields prevent automatic derivation. These are necessary for `DrawOp`'s manual `Hash` implementation to work.

### Downstream exhaustive matches required arms earlier than spec anticipated

Spec said "don't add stub arms" in Task 1, acknowledging that `cargo check` would fail with non-exhaustive-match errors in `execute_draw_op`, `encode_op`, and `replay_op`. This caused practical problems for subsequent tasks whose tests also couldn't compile. Resolved by:
- Task 1: **Kept only `draw_types.rs` Hash impls**, reverted stub arms from `helpers.rs`, `draw_encoding.rs`, `replay.rs`
- Task 4: Added the proper translation arm in `helpers.rs` (this was the planned fix)
- Task 5: Added `Ok(())` in replay and `unreachable!()` in encoder (the final fix)

All subagents attempted to add temporary stub arms — each was reverted. The crate was non-compilable between Tasks 1 and 5. Acceptable given CI would test only the final state.

### record_canvas_runtime_effect delegation pattern

Code review flagged that `record_canvas_runtime_effect` directly called `canvas_entry(id).commands.push(...)` while `record_draw_picture` delegates to `record_draw_op`. Changed to delegation for consistency.

### for_each_binding! macro requires single-line parameter list

The binding macro's parameter parsing doesn't handle multi-line `(param,)\n` syntax. Reformatted `canvas_runtime_effect_draw` to single-line params matching existing bindings like `canvas_draw_image`.

### to_ne_bytes() vs to_le_bytes()

The binding uses `f32::to_ne_bytes()` to convert uniform floats to bytes. On little-endian platforms (x86_64, wasm32) this is safe. For absolute portability `to_le_bytes()` would be clearer. Not critical — no big-endian targets in the current build matrix.

### No CLI entry point for profile-showcase.xml

The spec assumed `json/profile-showcase.xml` could be rendered via `cargo run --example compare_transitions` or similar. No existing entry point consumes this file — `profile-showcase` isn't referenced in any `.rs` or `.toml` file in the repo. The XML change was applied spec-compliantly but cannot be end-to-end verified without building a renderer entry point.

### children_len accessor

Spec called `b.children_len()` but no such method existed on `DrawOpBuilder`. Added `pub fn children_len(&self) -> usize` next to `push_child` — minimal surface, no internal-state exposure.

### ScriptChildSpec lives in script/helpers.rs (not bindings.rs)

Spec placed `ScriptChildSpec` inline in `bindings.rs`. Moved to `script/helpers.rs` next to existing parse helpers so the `for_each_binding!` macro body stays a one-liner. The binding macro can't host nested type defs cleanly.

### Tile mode is parsed but not propagated

Per spec §6.1, image-child tile mode is hard-coded `(Clamp, Clamp)` in engine. The JS-side `makeShader(tileX, tileY)` and JSON decoding accept tile modes but `to_ir_child_ref()` discards them. Follow-up: extend `ImageRef` or `RuntimeEffectChildRef::Image` to carry tile modes.

### Paint.shader cloned by reference in Paint.copy()

JS-side `Paint.copy()` does a shallow copy of `_shader`. Shaders are immutable handle objects (no mutation methods exposed), so reference sharing is safe and avoids deep-cloning uniforms arrays.

### profile-showcase.xml changes beyond spec

- Audio IDs deduplicated (`bgm` → `bgm0`/`bgm1`/`bgm2`) — the original file had duplicate `id` attributes which is invalid XML
- `c.clear()` removed and `else { c.drawPicture(sb,0,0); }` fallback removed per A' approach (null Make returns fallthrough to hidden child render)
- `ctx.fromTo('s1-decor', ...)` entrance animation removed as the element was repurposed
