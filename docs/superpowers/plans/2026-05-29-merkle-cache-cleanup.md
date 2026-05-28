# Merkle Cache Cleanup Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make the render cache system expose one clear Merkle model around structure, layout, paint, and apply, while preserving rendering behavior and comparing profile output after every implementation round.

**Architecture:** Keep the existing pipeline shape intact: resolve -> layout -> display -> analyze -> render IR -> engine replay. First clean profile names and tests so measurement is trustworthy, then rename render cache concepts from parent-own/subtree wording to node-own wording, then introduce typed segment keys, and only then consider removing redundant subtree snapshot lookup. Each round must preserve current output behavior and include a profile comparison against the `profile-showcase` baseline.

**Tech Stack:** Rust workspace, `cargo test`, `cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl`, tracing profile events, existing `BoundedLruCache`.

---

## Baseline

Use this profile output as the baseline for round-by-round comparison:

```text
frames: 414
avg ms/frame: script 0.60, resolve 0.61, layout 0.07, display 0.05, backend 0.23
layout avg/frame: layout_skipped_nodes 24.0, layout_dirty 0.7, raster_dirty 0.3
display avg/frame: merkle_skipped_nodes 12.8, rebuilt_nodes 2.9, apply_only_patched_nodes 9.9
analyze avg/frame: merkle_skipped_nodes 20.0, recorded_hit_nodes 20.0, composite_dirty_nodes 1.4
backend avg counts/frame: scene_snapshot_hit 0.00, subtree_snapshot_hit 0.00, subtree_request_after_reused 5.64
cache pressure avg/frame: parent_own_repeat 9.55, parent_own_util 55.21
```

Round acceptance rule:

- Rendering command must complete.
- Tests for touched modules must pass.
- Profile should not regress materially: no obvious increase in `backend ms/frame`, `display rebuilt_nodes`, or cache evictions.
- If the round is mostly naming/profile cleanup, the expected good result is equivalent behavior with clearer counters.
- If the round changes cache structure, the expected good result is equal or lower misses/records and no semantic regression.
- Commit only after the round passes the tests and profile comparison.

## File Map

- `docs/superpowers/specs/2026-05-29-merkle-cache-design.md`: approved design reference.
- `crates/opencat-core/src/profile/mod.rs`: profile data structs and backend counter fields.
- `crates/opencat-core/src/profile/aggregator.rs`: maps tracing events into profile counters.
- `crates/opencat-core/src/profile/output.rs`: human-readable profile text.
- `crates/opencat-core/src/ir/cache.rs`: render cache structs, cache field names, and segment artifact store.
- `crates/opencat-core/src/render/dispatch.rs`: render cache lookup/record path and tracing event names.
- `crates/opencat-core/src/render/builder.rs`: segment import/snapshot behavior; should only change if typed keys require local adjustments.
- `crates/opencat-core/src/analyze/fingerprint/mod.rs`: recorded/node fingerprint helpers and tests.
- `crates/opencat-core/src/analyze/annotation.rs`: analysis reuse state and Merkle skip tests.
- `crates/opencat-core/src/display/build.rs`: display apply-only path tests.
- `crates/opencat-core/src/pipeline/frame.rs`: profile event emission names for layout/display/analyze.

## Chunk 1: Profile Counter Cleanup

Purpose: make measurement match current behavior before structural changes.

**Files:**
- Modify: `crates/opencat-core/src/profile/mod.rs`
- Modify: `crates/opencat-core/src/profile/aggregator.rs`
- Modify: `crates/opencat-core/src/profile/output.rs`
- Modify: `crates/opencat-core/src/render/dispatch.rs`

- [ ] **Step 1: Write failing profile aggregation tests**

Add tests in `crates/opencat-core/src/profile/aggregator.rs` for these event mappings:

```rust
#[test]
fn count_events_record_node_own_segment_activity() {
    let mut aggregator = RenderProfileAggregator::default();

    aggregator.record_count(ProfileCountEvent {
        frame: 1,
        kind: "cache",
        name: "node_own_segment",
        result: "hit",
        amount: 2,
    });
    aggregator.record_count(ProfileCountEvent {
        frame: 1,
        kind: "cache",
        name: "node_own_segment",
        result: "record",
        amount: 3,
    });
    aggregator.record_count(ProfileCountEvent {
        frame: 1,
        kind: "cache",
        name: "node_own_segment",
        result: "replaced",
        amount: 1,
    });

    let summary = aggregator.finish();
    let backend = &summary.frames[&1].backend;
    assert_eq!(backend.node_own_segment_hits, 2);
    assert_eq!(backend.node_own_segment_records, 3);
    assert_eq!(backend.node_own_segment_replaced, 1);
}
```

Expected first run: fail because `node_own_*` fields do not exist.

- [ ] **Step 2: Run failing test**

Run:

```bash
cargo test -p opencat-core profile::aggregator::tests::count_events_record_node_own_segment_activity
```

Expected: compile failure or test failure for missing fields/mapping.

- [ ] **Step 3: Add node-own profile fields**

In `BackendProfile`, replace or add fields:

```rust
pub node_own_segment_hits: usize,
pub node_own_segment_records: usize,
pub node_own_segment_replaced: usize,
pub node_own_cache_evictions: usize,
pub node_own_cache_record_repeats: usize,
pub node_own_cache_capacity_utilization: usize,
```

Keep old fields only temporarily if needed for compile; prefer replacing all current references in this chunk.

- [ ] **Step 4: Rename event mappings**

In `RenderProfileAggregator::record_count`, map:

- `("cache", "node_own_segment", "hit")`
- `("cache", "node_own_segment", "record")`
- `("cache", "node_own_segment", "replaced")`
- `("eviction", "node_own", "count")`
- `("repeat", "node_own", "count")`
- `("utilization", "node_own", "count")`

Remove or stop printing parent-own fields.

- [ ] **Step 5: Update render event names**

In `render/dispatch.rs`, change event names:

- `parent_own_segment` -> `node_own_segment`
- first-record event should use `result = "record"` rather than `first_record`
- `record_cache_pressure("parent_own", ...)` -> `record_cache_pressure("node_own", ...)`

- [ ] **Step 6: Update profile text output**

In `profile/output.rs`, replace `parent_own_*` labels with:

```text
node_own_evict, node_own_repeat, node_own_util
```

Add backend count fields for `node_own_hit`, `node_own_record`, `node_own_replaced` if useful near the cache counts line.

- [ ] **Step 7: Run tests**

Run:

```bash
cargo test -p opencat-core profile::aggregator
```

Expected: all profile aggregator tests pass.

- [ ] **Step 8: Run profile comparison**

Run:

```bash
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Expected:

- command completes;
- timing roughly matches baseline;
- output now reports `node_own_*` counters instead of `parent_own_*`;
- semantic counters remain equivalent to baseline.

- [ ] **Step 9: Commit if accepted**

If tests pass and profile output is equivalent with clearer labels:

```bash
git add crates/opencat-core/src/profile/mod.rs crates/opencat-core/src/profile/aggregator.rs crates/opencat-core/src/profile/output.rs crates/opencat-core/src/render/dispatch.rs
git commit -m "refactor: rename parent-own profile counters"
```

## Chunk 2: Rename RenderCache Node-Own Concepts

Purpose: align code names with actual cache semantics while preserving behavior.

**Files:**
- Modify: `crates/opencat-core/src/ir/cache.rs`
- Modify: `crates/opencat-core/src/render/dispatch.rs`
- Modify: any tests or call sites found by `rg "parent_own|CachedSubtreeIr"`

- [ ] **Step 1: Locate current references**

Run:

```bash
rg -n "parent_own|CachedSubtreeIr|subtree_snapshots" crates/opencat-core/src crates/opencat-engine/src
```

Expected: references are mainly in `ir/cache.rs`, `render/dispatch.rs`, tests, and profile output already handled in chunk 1.

- [ ] **Step 2: Write/adjust cache struct tests**

In `crates/opencat-core/src/ir/cache.rs`, update tests to assert node-own naming:

```rust
#[test]
fn node_own_segments_can_insert_and_lookup() {
    let mut cache = RenderCache::new(2, 2, 2, 2);
    let entry = CachedNodeOwnIr {
        segment_key: 10,
        consecutive_hits: 0,
        recorded_bounds: DisplayRect::default(),
    };
    cache.node_own_segments.insert(1, entry.clone());
    assert_eq!(
        cache.node_own_segments.get_cloned(&1).unwrap().segment_key,
        10
    );
}
```

Expected first run: fail until names are changed.

- [ ] **Step 3: Rename structs and fields**

In `ir/cache.rs`:

- `CachedSubtreeIr` -> `CachedNodeOwnIr`
- `parent_own_segments` -> `node_own_segments`

Keep `subtree_snapshots` unchanged in this chunk unless required for compile.

- [ ] **Step 4: Update render dispatch imports and usage**

In `render/dispatch.rs`:

- import `CachedNodeOwnIr`;
- replace `cache.parent_own_segments` with `cache.node_own_segments`;
- replace temporary variable names `own_snapshot` where useful with `node_own_entry`.

Behavior should remain the same.

- [ ] **Step 5: Run focused tests**

Run:

```bash
cargo test -p opencat-core ir::cache
cargo test -p opencat-core profile::aggregator
```

Expected: pass.

- [ ] **Step 6: Run profile comparison**

Run:

```bash
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Expected:

- command completes;
- `node_own_*` counts remain equivalent to chunk 1;
- no backend/display regression.

- [ ] **Step 7: Commit if accepted**

```bash
git add crates/opencat-core/src/ir/cache.rs crates/opencat-core/src/render/dispatch.rs
git commit -m "refactor: rename node-own render cache"
```

## Chunk 3: Introduce Typed Segment Keys

Purpose: prevent different cache concepts from sharing an untyped `u64` artifact namespace.

**Files:**
- Modify: `crates/opencat-core/src/ir/cache.rs`
- Modify: `crates/opencat-core/src/render/dispatch.rs`
- Modify: tests in `crates/opencat-core/src/ir/cache.rs`

- [ ] **Step 1: Write failing typed-key tests**

In `ir/cache.rs`, add:

```rust
#[test]
fn segment_keys_keep_item_and_node_own_namespaces_separate() {
    assert_ne!(SegmentKey::Item(42), SegmentKey::NodeOwn(42));
}
```

Expected first run: fail because `SegmentKey` does not exist.

- [ ] **Step 2: Add `SegmentKey`**

In `ir/cache.rs`:

```rust
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub enum SegmentKey {
    Item(u64),
    NodeOwn(u64),
}
```

Do not add `Scene` yet unless there is an actual segment artifact for scene snapshots.

- [ ] **Step 3: Change segment artifact store**

Change:

```rust
pub segments: BoundedLruCache<u64, CachedDrawSegment>,
```

to:

```rust
pub segments: BoundedLruCache<SegmentKey, CachedDrawSegment>,
```

Change `CachedNodeOwnIr.segment_key` and `CachedDrawRange.segment_key` to `SegmentKey`.

- [ ] **Step 4: Update item cache path**

In `render_display_item_cached`:

```rust
let segment_key = SegmentKey::Item(cache_key);
cache.segments.insert(segment_key.clone(), segment);
```

Lookups must pass `&cached_range.segment_key`.

- [ ] **Step 5: Update node-own cache path**

In `render_cached_subtree`:

```rust
let segment_key = SegmentKey::NodeOwn(own_key);
```

Use this for `node_own_segments` entries and segment insertion.

If `subtree_snapshots` still stores a node-own artifact, use `SegmentKey::NodeOwn(own_key)` rather than a subtree key. This reveals whether `subtree_snapshots` is redundant.

- [ ] **Step 6: Run focused tests**

Run:

```bash
cargo test -p opencat-core ir::cache
cargo test -p opencat-core profile::aggregator
```

Expected: pass.

- [ ] **Step 7: Run broader core tests**

Run:

```bash
cargo test -p opencat-core
```

Expected: pass.

- [ ] **Step 8: Run profile comparison**

Run:

```bash
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Expected:

- command completes;
- cache hit/miss counts equivalent to chunk 2;
- no material timing regression.

- [ ] **Step 9: Commit if accepted**

```bash
git add crates/opencat-core/src/ir/cache.rs crates/opencat-core/src/render/dispatch.rs
git commit -m "refactor: type render segment keys"
```

## Chunk 4: Evaluate and Collapse Redundant Subtree Snapshot Lookup

Purpose: simplify render cache lookup if `node_own_segments` fully covers the artifact behavior.

**Files:**
- Modify: `crates/opencat-core/src/ir/cache.rs`
- Modify: `crates/opencat-core/src/render/dispatch.rs`
- Modify: `crates/opencat-core/src/profile/mod.rs`
- Modify: `crates/opencat-core/src/profile/aggregator.rs`
- Modify: `crates/opencat-core/src/profile/output.rs`

- [ ] **Step 1: Inspect profile after chunk 3**

Compare:

- `node_own_segment_hits`
- `node_own_segment_records`
- `subtree_snapshot_cache_hits`
- `subtree_snapshot_cache_misses`
- `subtree_snapshot_artifact_hits`
- `subtree_snapshot_artifact_first_record` or renamed equivalent

If subtree snapshot counters are still useful for planning visibility, keep them but rename to planning counters. If they are always duplicating node-own behavior, remove the artifact cache path.

- [ ] **Step 2: Write behavior tests before deletion**

Add or preserve tests proving:

- child paint changes do not invalidate parent node-own segment;
- parent paint changes invalidate parent node-own segment;
- child transform changes do not invalidate parent node-own segment.

Prefer pure fingerprint/cache-key tests in `analyze/fingerprint/mod.rs` or `render/dispatch.rs` helper tests. If no suitable public helper exists, extract a small pure function:

```rust
fn node_own_segment_key(node: &AnnotatedDisplayNode) -> u64 {
    DisplayRecordedFingerprint::from_recorded(&node.recorded_semantics()).0
}
```

Expected first run: fail if helper does not exist.

- [ ] **Step 3: Remove redundant artifact lookup only if tests support it**

If `subtree_snapshots` only points at the same node-own segment artifact:

- remove `subtree_snapshots` from `RenderCache`;
- remove `CachedSubtreeIr` remnants;
- remove `subtree_snapshot_artifact_*` events;
- keep ordered-scene `CachedSubtree` as a planning operation if still useful.

If removal creates ambiguity or profile loses useful visibility, stop and keep the cache with clearer names instead.

- [ ] **Step 4: Run focused tests**

Run:

```bash
cargo test -p opencat-core analyze::fingerprint
cargo test -p opencat-core ir::cache
cargo test -p opencat-core profile::aggregator
```

Expected: pass.

- [ ] **Step 5: Run core tests**

Run:

```bash
cargo test -p opencat-core
```

Expected: pass.

- [ ] **Step 6: Run profile comparison**

Run:

```bash
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Expected:

- command completes;
- output explains node-own render cache clearly;
- `backend ms/frame` does not regress materially;
- `display rebuilt_nodes` and `analyze skipped nodes` stay comparable to baseline;
- cache pressure is not worse.

- [ ] **Step 7: Commit if accepted**

If profile is equivalent or clearer without regression:

```bash
git add crates/opencat-core/src
git commit -m "refactor: simplify node-own render cache lookup"
```

If profile regresses or the code becomes less clear, revert only this chunk's local edits and do not commit.

## Chunk 5: Documentation and Final Profile Record

Purpose: make the new cache language durable.

**Files:**
- Modify: `docs/superpowers/specs/2026-05-29-merkle-cache-design.md`
- Modify: this plan file if implementation discoveries changed the plan
- Optional create: `docs/render-cache.md` if the project wants a non-superpowers design note

- [ ] **Step 1: Update design doc with actual final names**

Record final accepted names:

- cache fields;
- profile labels;
- segment key variants;
- any retained subtree planning counters.

- [ ] **Step 2: Run final verification**

Run:

```bash
cargo test -p opencat-core
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Expected: tests pass and profile completes.

- [ ] **Step 3: Commit docs if changed**

```bash
git add docs/superpowers/specs/2026-05-29-merkle-cache-design.md docs/superpowers/plans/2026-05-29-merkle-cache-cleanup.md
git commit -m "docs: record merkle cache cleanup results"
```

## Execution Notes

- Do not touch `implementation-notes.md` or `json/four-corners.xml` unless Solaren explicitly asks; they were pre-existing untracked files.
- Do not use destructive git commands.
- Each implementation chunk should be a separate commit only after tests and profile comparison.
- Keep behavior-preserving rename chunks small. If a rename chunk causes a profile regression, treat it as a bug in instrumentation or lookup, not as acceptable churn.
- Because the available sub-agent tool requires explicit user permission for agent delegation, implement this plan locally with `executing-plans` unless Solaren explicitly asks to use sub-agents.
