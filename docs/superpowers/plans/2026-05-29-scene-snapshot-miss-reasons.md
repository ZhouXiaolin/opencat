# Scene Snapshot Miss Reasons Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add explainable scene snapshot miss reason counters while preserving current cache behavior.

**Architecture:** Introduce a small scene snapshot decision enum in `pipeline/frame.rs`, emit one miss reason event for every scene snapshot miss, aggregate it into `BackendProfile`, and print reason averages in profile output.

**Tech Stack:** Rust, existing tracing profile events, `cargo test`, opencat profile command.

---

## Chunk 1: Scene Snapshot Miss Reason Profile

**Files:**

- Modify: `crates/opencat-core/src/pipeline/frame.rs`
- Modify: `crates/opencat-core/src/profile/mod.rs`
- Modify: `crates/opencat-core/src/profile/aggregator.rs`
- Modify: `crates/opencat-core/src/profile/output.rs`

- [x] **Step 1: Write failing aggregation/output tests**

Add tests proving scene snapshot miss reason events are aggregated and printed.

- [x] **Step 2: Verify tests fail**

Run:

```bash
cargo test -p opencat-core profile::aggregator::tests::count_events_record_scene_snapshot_miss_reasons profile::output::tests::text_output_contains_expected_sections
```

Expected: fail because miss reason fields do not exist yet.

- [x] **Step 3: Implement profile fields and aggregation**

Add backend fields for `plan_blocked`, `empty`, `viewport_changed`, and `root_fingerprint_changed` reason counts. Aggregate events under `kind = "cache"`, `name = "scene_snapshot_miss"`.

- [x] **Step 4: Implement scene snapshot decision enum**

Replace the boolean helper with a decision helper that returns hit or miss reason. Keep the render path behavior identical.

- [x] **Step 5: Verify targeted tests**

Run:

```bash
cargo test -p opencat-core scene_snapshot
cargo test -p opencat-core profile::aggregator::tests::count_events_record_scene_snapshot_miss_reasons
cargo test -p opencat-core profile::output::tests::text_output_contains_expected_sections
```

- [x] **Step 6: Verify full package and profile**

Run:

```bash
cargo test -p opencat-core
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

- [x] **Step 7: Commit if profile is stable**

Commit only the files touched for this chunk.

## Implementation Results

Verification:

```bash
cargo test -p opencat-core
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Latest result:

```text
cargo test -p opencat-core: 411 passed, 0 failed, 1 ignored
profile frames: 414
avg ms/frame: script 0.65, resolve 0.65, layout 0.06, display 0.06, backend 0.25
scene_snapshot_hit 0.00, scene_snapshot_miss 1.00
scene_snapshot_miss_plan_blocked 0.79
scene_snapshot_miss_empty 0.00
scene_snapshot_miss_viewport_changed 0.00
scene_snapshot_miss_root_fingerprint_changed 0.21
node_own_hit 9.55, node_own_record 0.06, node_own_evict 0.00
```

Interpretation:

- Whole-scene snapshot misses are not a cache warmup or viewport problem.
- Most misses are caused by the scene plan blocking whole-frame reuse.
- The remaining misses are caused by root recorded subtree changes.
- Layout/display/analyze/node-own metrics stayed comparable to the previous profile.
