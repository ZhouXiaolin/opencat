# Scene Snapshot Miss Reasons

## Context

The Merkle cache cleanup made render artifacts explicit:

- scene snapshot caches the whole frame;
- node-own segments cache only a node's own recorded IR;
- ordered subtree reuse is an analyze/compositor planning decision.

The showcase profile currently reports `scene_snapshot_hit 0.00` and `scene_snapshot_miss 1.00`, but it does not explain why the whole-frame cache misses.

## Goal

Make scene snapshot cache decisions explainable without changing rendering semantics.

Each scene snapshot miss should record one reason:

- `plan_blocked`: the scene plan disallows the whole-frame cache;
- `empty`: there is no prior scene snapshot;
- `viewport_changed`: the cached snapshot was recorded at a different viewport;
- `root_fingerprint_changed`: the root recorded subtree fingerprint differs.

## Non-Goals

- Do not change scene snapshot reuse semantics.
- Do not add a new cache layer.
- Do not tune cache capacity.
- Do not change node-own cache keys.

## Design

Replace the internal boolean-only scene snapshot reuse check with a small decision enum. The render path still emits the existing `scene_snapshot hit/miss` counters, and on miss it also emits a reason counter. Profile aggregation and text output summarize those reason counters.

This gives the next optimization round a precise answer for whether the whole-frame cache is unavailable because the scene is dynamic, the viewport changed, or the cache is simply cold.

## Verification

Run:

```bash
cargo test -p opencat-core
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

Accept the change if tests pass, profile completes, existing skip/cache counts remain comparable, and profile output exposes non-zero scene snapshot miss reason counters.
