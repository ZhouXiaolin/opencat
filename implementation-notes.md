# Merkle Cache Implementation Notes

## Issue 1: Unconditional import of `AnalyzeReuseState` in dispatch.rs
- **Decision:** Gate the import with `#[cfg(feature = "profile")]` since it's only used inside profile-guarded blocks (lines 313-316).
- **Tradeoff:** None. Clean separation.

## Issue 4: Collision event rename (combined with Issue 2)
- **Decision:** Rename from `"collision_rejected"` to `"replaced"` in dispatch.rs. Rename BackendProfile field from `subtree_snapshot_collision_rejected` to `subtree_snapshot_artifact_replaced` to match the new semantics.
- **Tradeoff:** The new name "replaced" is more semantically accurate for `report.replaced` (which fires when a cache entry is overwritten, regardless of whether it's a true hash collision or a re-recording).
- **Chain of renames:**
  - dispatch.rs: `result = "collision_rejected"` → `result = "replaced"`
  - profile/mod.rs: field `subtree_snapshot_collision_rejected` → `subtree_snapshot_artifact_replaced`
  - aggregator.rs: match arm `"collision_rejected"` → `"replaced"`, field reference updated
  - output.rs: label `subtree_collision_rejected` → `subtree_artifact_replaced`, field reference updated
  - pipeline/default.rs: variable + field reference updated

## Issue 2: Field name prefix (absorbed into Issue 4)
- **Decision:** The original Issue 2 asked to add `artifact_` prefix to `subtree_snapshot_collision_rejected`. Since Issue 4 renames the concept entirely to "replaced", the new field name `subtree_snapshot_artifact_replaced` includes the `artifact_` prefix naturally.
- **No separate change needed** — handled by Issue 4's rename.

## Issue 3: Unused counter variables in regression test
- **Decision:**
  - Add cross-field assertion: `analyze_recorded_hit_nodes == analyze_snapshot_eligibility_hit_nodes + analyze_composite_blocked_nodes`
  - Remove `let _` for `analyze_composite_blocked_nodes` (now used in assertion)
  - Remove `let _` for `subtree_snapshot_artifact_evicted_or_absent` (already used in cross-field assertions at lines 543/548)
  - Remove `let _` for `subtree_snapshot_collision_rejected` (renamed to `subtree_snapshot_artifact_replaced`, now unused but the variable extraction is kept in case it's useful for debugging — actually removing the `let _` binding and the variable extraction entirely)
  - Remove `let _` for `subtree_snapshot_request_after_analyze_composite_blocked` (kept as `let _` since there's no meaningful assertion — actually removing it too)
- **Tradeoff:** Decided to keep variable extractions that are used in assertions and remove only the truly unused ones. For the ones we can assert on, we add meaningful assertions.

## Issue 5: Fragile relative path
- **Decision:** Replace `include_str!("../../../../json/profile-showcase.jsonl")` with `include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../json/profile-showcase.jsonl"))`.
- **Tradeoff:** `CARGO_MANIFEST_DIR` resolves to the absolute path of the crate's Cargo.toml directory. From `crates/opencat-core/`, going up two levels then into `json/` gives the same result as the 4-level relative path but is more resilient to file moves within the crate source tree.

## Issue 6: Untested `structure_rebuild=true` path
- **Decision:** Add a test `structure_rebuild_clears_fingerprint_history` that verifies `previous(true)` returns empty map and discards history, so subsequent calls treat everything as fresh.
- **Tradeoff:** Test directly exercises `compute_display_tree_fingerprints_with_history` with `structure_rebuild=true` on second call, verifying that nodes are NOT reused from history (reuse state is Fresh, not ReusedFromHistory).

## Issue 7: Function rename for dual-purpose function
- **Decision:** Rename `copy_subtree_analysis_from_history` → `copy_subtree_analysis_and_mark_reused` to reflect that it both copies analysis AND sets reuse state.
- **Tradeoff:** Longer name but more accurate. Two call sites updated (line 322 + recursive line 418).

## Issue 8: DefaultHasher → ahash consistency cleanup
- **Decision:** Migrate all remaining `std::collections::hash_map::DefaultHasher` uses in `opencat-core` to `ahash::AHasher` for consistency with the Merkle fingerprint chain.
- **Files changed:**
  - `script/host.rs:12-16`: `driver_id_from_source()` — script driver ID hashing (ephemeral, no persistence impact)
  - `ir/asset_id.rs:1,33-37`: `stable_hash()` — cache directory file name generation
  - `ir/draw_op.rs:759-776`: test `script_runtime_effect_hash_differs_by_sksl`
- **Tradeoff:** `stable_hash` is used by `opencat-engine/src/resource/utils.rs` to generate cache file names (`{:016x}.bin`). The hasher change produces different hash values, so existing resource cache files will not be found after this change (cache miss, not corruption). The system will re-process and re-cache assets transparently. This is acceptable since the cache is a performance optimization, not a correctness requirement.

## Issue 9: Add Display-layer recorded subtree identity metrics
- **Decision:** Add `display_recorded_subtree_identical_subtrees/nodes` as a separate Display-layer metric in the profile output. These values mirror `analyze_recorded_hit_subtrees/nodes` (same source: `analyze_stats.recorded_hit_*`) but are displayed under a "display avg/frame" section to make the three-layer diagnostic chain explicit.
- **Why:** The spec requires the profile to answer "how many display recorded subtrees stayed identical" at the Display layer. Previously this was only visible as `analyze_recorded_hit_*` under the Analyze section, which conflated Display identity with Analyze eligibility.
- **Files changed:**
  - `profile/mod.rs`: Added `display_recorded_subtree_identical_subtrees/nodes` fields to `FrameProfile`
  - `pipeline/frame.rs`: Emit `display` kind events with `target: "render.display"`
  - `profile/layer.rs`: Added `"render.display"` to the `on_event` target whitelist
  - `profile/aggregator.rs`: Added match arms for `"display"` kind events + test
  - `profile/output.rs`: Added "display avg/frame" section + test assertions
- **Tradeoff:** The Display metric is computed in the Analyze layer (where the history comparison happens) rather than the Display layer itself. This is architecturally correct because the Display layer only builds fingerprints; the comparison requires the previous frame's history which lives in Analyze.

## Issue 10: Remove composite dimension from Layout layer (Level B convergence)
- **Motivation:** Audit revealed `composite_input_subtree` + `composite_input_local` in `ElementInputFingerprints` were duplicating a signal that Analyze's `CompositeSig` + `CompositeHistory` already tracked authoritatively. Layout doesn't perform composite work (transform/opacity/blur are draw-time concerns), so tracking composite in Layout was a layer-attribution error.
- **Two-step process:**
  - **Level A first attempt:** Drop only `composite_input_subtree` from the 4-way equality check (3-way: structure/layout/paint). FAILED — broke `composite_dirty_nodes` counting because `composite_input_subtree` was secretly serving as a "descent trigger" (forcing recursion past the 3-way early-return so per-leaf `composite_input_local` mismatches could be counted). Two layout tests started failing.
  - **Level B (final):** Remove the entire composite dimension from `ElementInputFingerprints` and `CachedLayoutNode`. Move `composite_dirty_nodes` counting to the Analyze layer's existing `mark_display_tree_composite_dirty` pass (which already computes per-node `CompositeSig` deltas).
- **Files changed:**
  - `semantic/fingerprint.rs`: Removed `composite_input_local`, `composite_input_subtree`, `CompositeInputLocal` struct + Hash impl
  - `layout/mod.rs`: Removed `LayoutPassStats.composite_dirty_nodes`, `paint_dirty_nodes()` helper, `CachedLayoutNode.composite_input_local_hash`, the per-node `composite_changed` branch, and the two failing tests `layout_session_marks_opacity_change_as_composite_dirty` / `layout_session_marks_transform_change_as_composite_dirty`
  - `analyze/invalidation.rs`: Added `CompositeDirtyStats` and threaded `composite_dirty_nodes` counter through `mark_display_node_composite_dirty`; `mark_display_tree_composite_dirty` now returns the stats
  - `analyze/compositor.rs`: `SceneRenderPlan::from_layout_pass` now takes `composite_dirty_nodes: usize` as a separate parameter (was `layout_pass.composite_dirty_nodes`)
  - `pipeline/frame.rs`: Captures `composite_dirty_stats` return value, emits as `("analyze", "analyze_composite_dirty_nodes", "count")` event, drops the `("layout", "composite_dirty", "count")` emit
  - `profile/aggregator.rs`: Replaced `("layout", "composite_dirty", "count")` arm with `("analyze", "analyze_composite_dirty_nodes", "count")`
  - `profile/mod.rs`: Renamed `composite_dirty_nodes` → `analyze_composite_dirty_nodes` in `FrameProfile`
  - `profile/output.rs`: Moved `composite_dirty` from "layout avg/frame" line to "analyze avg/frame" line (renamed to `composite_dirty_nodes` for consistency with other analyze fields)
- **Showcase metrics (json/profile-showcase.jsonl, 414 frames):**

| Metric | Before | After | Δ |
|---|---|---|---|
| reused_nodes | 23.2 | 24.6 | +1.4 |
| input_full_hit_subtrees | 4.0 | 2.0 | −2.0 |
| input_full_hit_nodes | 19.6 | 22.7 | **+3.1** |
| layout_skipped_subtrees | 1.7 | 1.7 | 0 |
| raster_dirty | 0.3 | 0.3 | 0 |
| layout.composite_dirty | 1.4 | (moved) | — |
| analyze.composite_dirty_nodes | (new) | 1.4 | ✅ exact equivalent |
| Analyze/Display/Render | (same) | (same) | 0 |

- **Why the input_full_hit_subtrees count went down while nodes went up:** Hit subtrees became deeper. Before: 19.6 nodes / 4.0 subtrees = 4.9 nodes/subtree. After: 22.7 / 2.0 = 11.4 nodes/subtree. Fewer, larger subtrees skipped — exactly what Merkle convergence aims for.
- **What we lost:** Layout-layer `composite_dirty_nodes` stat (replaced by equivalent in Analyze layer with identical numbers in this scene).
- **What we gained:** Clean layer attribution — Layout no longer pretends to track composite; Analyze is the single source of truth via `CompositeSig`. Semantic input fingerprint reduced from 4 dimensions to 3 (structure / layout / paint). `input_merkle_full_hit` now hits deeper subtrees because composite changes no longer break Merkle equality at ancestors.
