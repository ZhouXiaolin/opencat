# Merkle Cache Design

## Context

The current render profile command is:

```bash
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Baseline from `profile-showcase`:

- Frames: 414
- Average frame cost: script 0.60 ms, resolve 0.61 ms, layout 0.07 ms, display 0.05 ms, backend 0.23 ms
- Layout: 24.0 layout-merkle skipped nodes/frame, 0.7 layout-dirty nodes/frame
- Display: 12.8 display-merkle skipped nodes/frame, 9.9 apply-only patched nodes/frame, 2.9 rebuilt nodes/frame
- Analyze: 20.0 analyze-merkle skipped nodes/frame, 20.0 recorded-hit nodes/frame, 1.4 composite-dirty nodes/frame
- Backend: 0 scene snapshot hits/frame, 0 subtree snapshot hits/frame, 5.64 subtree requests after reused analysis/frame
- Cache pressure: parent-own repeat 9.55/frame, parent-own utilization 55.21

The baseline shows the Merkle paths are already effective. The next step is not to add more cache layers; it is to make the existing cache model more natural, explicit, and easier to reason about.

## Goal

Make the cache system follow one clear Merkle analysis model:

- Structure changes decide whether node identity and child alignment can be reused.
- Layout changes decide whether layout output can be reused.
- Paint changes decide whether recorded visual content can be reused.
- Apply changes decide whether only node-local composite state must be patched.

The implementation should make these boundaries visible in names, cache keys, profile counters, and tests.

## Non-Goals

- Do not change rendering semantics.
- Do not introduce backend-specific Skia picture caching in this phase.
- Do not tune cache capacity before cache identity and counters are clear.
- Do not optimize resource, video, audio, or script caches as part of this change.

## Current Model

`ElementInputFingerprints` already contains four local and subtree Merkle axes:

- `structure_local/subtree`
- `layout_input_local/subtree`
- `paint_input_local/subtree`
- `apply_input_local/subtree`

`DisplayBuildSession` already uses the intended rule:

- `paint_input_subtree + layout_output_subtree + element_id + node_count` match: display content can be reused.
- `apply_input_subtree` also matches: clone the cached `DisplayNode` subtree.
- only `apply_input_subtree` differs: patch node-local apply fields and avoid rebuilding `DisplayItem`.

`AnalyzeFingerprintHistory` reuses analysis when `recorded_subtree_fingerprint` matches for the same render node key.

Before this cleanup, `RenderCache` had correct behavior but unclear names:

- `subtree_snapshots` sounds like it stores a whole subtree, but the render path records only the parent node's own item and open clip; children are rendered dynamically afterward.
- `parent_own_segments` is the cache that profile shows as frequently reused.
- `segments` is a shared untyped `u64 -> CachedDrawSegment` artifact store used by different cache concepts.

After the cleanup, the render artifact cache is intentionally narrower:

- `node_own_segments: BoundedLruCache<u64, CachedNodeOwnIr>` is the only node-own cache entry map.
- `segments: BoundedLruCache<SegmentKey, CachedDrawSegment>` stores typed IR artifacts.
- `item_ranges: BoundedLruCache<u64, CachedDrawRange>` stores item-level picture ranges.
- `last_scene_snapshot` remains the whole-frame snapshot path.
- The old `subtree_snapshots` artifact lookup was removed; `OrderedSceneOp::ReusedSubtree` remains a planning operation, not a second artifact cache.

## Proposed Design

### 1. Formalize Cache Axes

Introduce small type wrappers for cache axes near the fingerprint modules:

- `StructureFingerprint(u64)`
- `LayoutInputFingerprint(u64)`
- `PaintInputFingerprint(u64)`
- `ApplyInputFingerprint(u64)`
- `RecordedFingerprint(u64)`
- `RecordedSubtreeFingerprint(u64)`

This can be incremental. The first implementation may keep the stored fields as `u64`, but new cache APIs should use semantic key types instead of raw `u64` where practical.

### 2. Rename RenderCache Concepts

Rename render cache concepts to match actual behavior:

- `parent_own_segments` -> `node_own_segments`
- `subtree_snapshots` -> either remove in favor of `node_own_segments`, or rename to `subtree_requests` if it remains a planning cache
- `CachedSubtreeIr` -> `CachedNodeOwnIr` for the node-own segment path

The important semantic rule is:

```text
node-own segment = current node's recorded item + open clip state
children are always rendered through ordered scene ops, so their apply state stays dynamic
```

This aligns with the desired Merkle rule: this node's paint affects its own recorded content; descendants keep their own paint/apply identities.

### 3. Use Typed Segment Keys

Replace shared raw segment keys with a typed key:

```rust
enum SegmentKey {
    Item(u64),
    NodeOwn(u64),
}
```

If `BoundedLruCache` requires simple hash keys, derive `Hash + Eq + Clone` on `SegmentKey`.

This avoids semantic collisions and makes profile/debug output meaningful.

### 4. Simplify Subtree Cache Lookup

The render path should use one primary node-own segment cache:

1. Apply current node transform/opacity/backdrop blur dynamically.
2. Compute node-own recorded fingerprint from `RecordedNodeSemantics`.
3. Try `node_own_segments`.
4. On hit, import the node-own segment and render children dynamically.
5. On miss, record only the node-own range, insert it, then render children dynamically.

`snapshot_fingerprint` remains useful for analysis and ordered-scene planning, but should not imply that children are baked into the cached artifact unless the implementation actually records a whole subtree.

### 5. Fix Profile Counters

Rename counters to show real cache behavior:

- `parent_own_segment hit/first_record/replaced` -> `node_own_segment hit/record/replaced`
- `parent_own_cache_*` -> `node_own_cache_*`
- `subtree_snapshot_request_after_analyze_*` may remain, but it should be labeled as planning:
  - `subtree_request_after_fresh`
  - `subtree_request_after_reused`
  - `subtree_request_after_composite_blocked`

Removed render-artifact counters:

- `subtree_snapshot_hit`
- `subtree_snapshot_miss`
- `subtree_artifact_hit`
- `subtree_artifact_first_record`
- `subtree_artifact_evicted_or_absent`
- `subtree_artifact_replaced`
- `subtree_evict/repeat/util`

The analyze-side `composite_blocked_*` counters are still present for visibility, but the current showcase profile reports them as `0.0`.

### 6. Tests

Add focused tests for invariants:

- A node transform change does not change paint or node-own cache key.
- A child transform change does not change parent node-own key.
- A child paint change does not invalidate parent node-own segment.
- A parent paint change invalidates parent node-own segment.
- A parent clip size/radius change invalidates parent node-own segment.
- Apply-only animation increments display apply-only counts but does not force display item rebuild.
- Profile aggregation records node-own hit/miss/record counters.

## Migration Steps

1. Add typed key wrappers and update profile naming tests.
2. Rename profile fields and output text from parent-own/subtree-snapshot wording to node-own wording.
3. Rename `RenderCache` fields and structs while preserving behavior.
4. Collapse redundant subtree snapshot artifact lookup if tests show `node_own_segments` covers the active behavior.
5. Add invariant tests around parent/child paint/apply independence.
6. Re-run the profile baseline and compare the same key metrics.

## Expected Result

After this phase, profile output should make the cache story obvious:

- layout Merkle skips layout work;
- display Merkle skips display construction;
- display apply-only patches node-local composite fields;
- analyze Merkle skips analysis;
- node-own render cache reuses current-node recorded IR;
- children remain dynamically rendered so their apply changes do not poison parent paint caches.

This gives a clean foundation for later capacity tuning and weighted LRU.

## Final Implementation Record

Accepted commits:

- `334af71 refactor: rename parent-own profile counters`
- `2753028 refactor: rename node-own render cache`
- `fe53fce refactor: type render segment keys`
- `7815398 refactor: simplify node-own render cache lookup`

Final render cache layers:

1. Scene snapshot: `last_scene_snapshot`, validated by root recorded subtree fingerprint and viewport.
2. Node-own IR entry: `node_own_segments`, keyed by `DisplayRecordedFingerprint::from_recorded(&node.recorded_semantics())`.
3. Typed segment artifact store: `segments`, keyed by `SegmentKey::Item(_)` or `SegmentKey::NodeOwn(_)`.
4. Item picture ranges: `item_ranges`, keyed by `item_paint_fingerprint`.

Final profile after `cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl`:

```text
frames: 414
avg ms/frame: script 0.64, resolve 0.64, layout 0.06, display 0.06, backend 0.25
layout avg/frame: layout_skipped_nodes 24.0, layout_dirty 0.7, raster_dirty 0.3
display avg/frame: merkle_skipped_nodes 12.8, rebuilt_nodes 2.9, apply_only_patched_nodes 9.9
analyze avg/frame: merkle_skipped_nodes 20.0, recorded_hit_nodes 20.0, composite_dirty_nodes 1.4
backend avg counts/frame: scene_snapshot_hit 0.00, scene_snapshot_miss 1.00, subtree_request_after_fresh 3.97, subtree_request_after_reused 5.64, node_own_hit 9.55, node_own_record 0.06
cache pressure avg/frame: node_own_evict 0.00, node_own_repeat 9.55, node_own_util 55.21
```

The result removes the redundant subtree artifact lookup without changing the effective Merkle behavior: node-own hits and records remain stable, display/analyze skip counts remain comparable, and backend time stays within noise of the baseline.
