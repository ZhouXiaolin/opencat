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

`RenderCache` currently has correct behavior but unclear names:

- `subtree_snapshots` sounds like it stores a whole subtree, but the render path records only the parent node's own item and open clip; children are rendered dynamically afterward.
- `parent_own_segments` is the cache that profile shows as frequently reused.
- `segments` is a shared untyped `u64 -> CachedDrawSegment` artifact store used by different cache concepts.

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
    Scene(u64),
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

- `parent_own_segment hit/first_record/replaced` -> `node_own_segment hit/miss/record/replaced`
- `parent_own_cache_*` -> `node_own_cache_*`
- `subtree_snapshot_request_after_analyze_*` may remain, but it should be labeled as planning:
  - `node_cache_request_after_analyze_fresh`
  - `node_cache_request_after_analyze_reused`

Remove or redefine:

- `analyze_composite_blocked_subtrees`
- `analyze_composite_blocked_nodes`
- `subtree_snapshot_request_after_analyze_composite_blocked`

Composite dirty no longer blocks analysis reuse, so these counters currently describe a state that should not occur.

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
