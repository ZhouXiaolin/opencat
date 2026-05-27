# Merkle Cache Convergence Next Spec

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make cache decisions fully explainable and increasingly O(1) by converging Layout, Display, Analyze, and Render onto explicit Merkle semantics plus explicit policy blockers.

**Architecture:** Merkle fingerprints answer only "is this semantic subtree identical?". Analyze answers "is the identical subtree safe to reuse in this frame?". Render cache answers "is the reusable artifact still present and valid under cache policy?". The next work should preserve this separation instead of merging semantic identity, snapshot eligibility, and backend cache pressure into one opaque hash.

**Tech Stack:** Rust 2024, `ahash`, existing `opencat-core` profile feature, existing `RenderProfileAggregator`, `json/profile-showcase.jsonl`.

---

## Current State

The cache chain is already rooted in Merkle-style subtree fingerprints:

```text
ElementNode / NodeStyle
  -> ElementInputFingerprints
  -> LayoutOutputFingerprint
  -> DisplayRecordedSubtreeFingerprint
  -> AnalyzeFingerprintHistory
  -> Render subtree/item cache
```

The important current files are:

- `crates/opencat-core/src/semantic/fingerprint.rs`: resolved element input Merkle semantics.
- `crates/opencat-core/src/layout/mod.rs`: layout Merkle skip and dirty classification.
- `crates/opencat-core/src/display/tree.rs`: `DisplayRecordedSubtreeFingerprint` carried on display nodes.
- `crates/opencat-core/src/display/build.rs`: bottom-up display recorded subtree fingerprint construction.
- `crates/opencat-core/src/analyze/fingerprint/mod.rs`: centralized display/analyze fingerprint rules.
- `crates/opencat-core/src/analyze/annotation.rs`: Analyze history and Merkle skip table.
- `crates/opencat-core/src/render/dispatch.rs`: backend subtree snapshot and item cache behavior.
- `crates/opencat-core/src/profile/*`: profile aggregation/output.

The latest real showcase run shows the split is useful:

```text
analyze avg/frame:
  merkle_skipped_nodes 20.0
  recorded_hit_nodes 20.5
  snapshot_eligibility_hit_nodes 20.0
  composite_blocked_nodes 0.5
```

This means recorded display semantics are mostly stable, and the small gap is explained by composite eligibility. That is the right shape.

---

## Why Continue

### 1. Fast decisions should be explicitly attributable

Right now Layout and Analyze report Merkle skips, and Render reports backend cache hits/misses. The missing bridge is a direct explanation from "Merkle said reusable" to "Render did or did not hit an artifact".

Without that bridge, a profile can show:

```text
analyze_merkle_skipped_nodes > 0
subtree_snapshot_miss > 0
```

but it is not immediately obvious whether the miss is caused by cache eviction, first record, collision rejection, dirty composite policy, or a semantic key mismatch. The next step is to make every miss/hit attributable.

### 2. Merkle must remain semantic, not policy

The core rule should stay:

```text
Merkle fingerprint == semantic identity
snapshot eligibility == reuse policy
cache hit/miss == artifact availability
```

If we mix cache policy into Merkle fingerprints, fingerprints become unstable and less reusable. If we leave policy unprofiled, we lose explainability. The right fix is not "more hash"; it is explicit policy accounting after the hash says the subtree is identical.

### 3. More O(1) skip opportunities still exist

Analyze now skips stable subtrees by `RenderNodeKey + DisplayRecordedSubtreeFingerprint + has_dirty_descendant_composite`.

Remaining opportunities:

- expose the skip table as a debuggable data structure, not only aggregate counters;
- allow Render to understand when a subtree was semantically skipped by Analyze;
- avoid redundant work in backend paths when Analyze already proved subtree analysis unchanged;
- detect cache pressure separately from semantic churn.

### 4. It prevents regression into old semantics

The old path had several scattered meanings of "display node hash". The new path has named semantic layers. Future changes should be forced through these names:

- `DisplayRecordedFingerprint`
- `DisplayRecordedSubtreeFingerprint`
- `SubtreeSnapshotFingerprint`
- `CompositeSig`

That makes accidental reintroduction of `DisplayNodeFp`, `ClipFp`, or ad hoc `DefaultHasher` easier to catch.

---

## Design Principles

1. **Semantic hashes are pure and centralized.**
   Keep hash construction in `analyze/fingerprint/*` or the existing semantic modules. Do not create one-off hashers in pipeline/render code.

2. **Use `ahash::AHasher` consistently.**
   Do not use `DefaultHasher` in the NodeStyle -> Display -> Analyze -> Render cache chain.

3. **Profile names must describe the decision stage.**
   Use names like `recorded_hit`, `snapshot_eligibility_hit`, `cache_evicted`, `artifact_hit`. Avoid vague names like `fast_hit` or `cache_skip`.

4. **Merkle hit is not equal to render cache hit.**
   Merkle hit means the semantic subtree is identical. Render cache hit means the artifact is still in the backend cache.

5. **Tests should pin decision boundaries, not implementation accidents.**
   Test "recorded hit but composite blocked", "recorded hit but artifact evicted", and "recorded miss after paint change" as separate cases.

---

## Target End State

For any frame in `profile-showcase.jsonl`, we should be able to explain:

```text
Layout:
  how many nodes avoided layout descent because ElementInputFingerprints matched

Display:
  how many display recorded subtrees stayed identical

Analyze:
  how many recorded hits were eligible for analysis reuse
  how many were blocked by composite eligibility

Render:
  how many eligible snapshot artifacts hit
  how many missed because first record / eviction / collision / dirty policy
```

The final profile should answer:

```text
Did semantics change?
  If yes: which layer changed?
  If no: did policy block reuse?
  If policy allowed reuse: did cache capacity/artifact availability block hit?
```

---

## Phase 1: Formalize Analyze Decision Reasons

### Why

Analyze currently has aggregate counters, but the decision itself is still implicit in `compute_node_fingerprint`. The next optimization work will be safer if the skip reason is a named enum. This prevents future code from treating "recorded hit" and "actual skip" as the same thing.

### How

Introduce an internal enum in `crates/opencat-core/src/analyze/annotation.rs`:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AnalyzeFingerprintDecision {
    Miss,
    Reused {
        nodes: usize,
    },
    CompositeBlocked {
        nodes: usize,
    },
}
```

Move the current inline branch:

```rust
if previous recorded_subtree_fingerprint matches {
    if snapshot eligibility matches { skip }
    else { composite blocked }
}
```

into a small helper:

```rust
fn classify_analyze_fingerprint_decision(...) -> AnalyzeFingerprintDecision
```

Keep the actual recursive compute/copy behavior in `compute_node_fingerprint`; the helper should classify, not mutate.

### Files

- Modify: `crates/opencat-core/src/analyze/annotation.rs`

### Tests

- Extend existing `recorded_hit_with_changed_descendant_composite_blocks_parent_skip_only`.
- Add a direct test for:
  - recorded miss after changed `DisplayRecordedSubtreeFingerprint`;
  - recorded hit + same eligibility returns `Reused`;
  - recorded hit + changed eligibility returns `CompositeBlocked`.

### Commands

```bash
cargo test -p opencat-core --lib analyze::annotation
cargo test -p opencat-core --features profile --lib analyze::annotation
```

### Acceptance

- No behavior change in profile numbers.
- Decision names match profile counters one-to-one.
- No new hashing code outside fingerprint modules.

---

## Phase 2: Add Render Artifact Miss Reasons

### Why

Analyze can prove a subtree is semantically reusable, but Render may still miss because the artifact is absent or evicted. Today backend profile reports hit/miss/collision, but does not explicitly separate "semantic reusable but artifact missing" from normal first-time recording.

This matters because optimization choices differ:

- semantic churn means improve Merkle boundaries;
- artifact eviction means tune cache capacity/weight;
- collision rejection means improve identity/collision guard;
- dirty policy means revisit eligibility rules.

### How

Add explicit backend count events in `crates/opencat-core/src/render/dispatch.rs` around subtree snapshot lookup:

```text
subtree_snapshot_artifact_hit
subtree_snapshot_artifact_first_record
subtree_snapshot_artifact_evicted_or_absent
subtree_snapshot_artifact_replaced
```

Do not change the cache key. This phase is observability only.

Wire these events through:

- `crates/opencat-core/src/profile/mod.rs`
- `crates/opencat-core/src/profile/aggregator.rs`
- `crates/opencat-core/src/profile/output.rs`

### Files

- Modify: `crates/opencat-core/src/render/dispatch.rs`
- Modify: `crates/opencat-core/src/profile/mod.rs`
- Modify: `crates/opencat-core/src/profile/aggregator.rs`
- Modify: `crates/opencat-core/src/profile/output.rs`

### Tests

- Add aggregator tests for each of the 4 new event names.
- Add output test ensuring the 4 new labels appear in text output.
- Prefer existing render/cache tests if they can assert emitted profile counters without heavy fixtures.

### Commands

```bash
cargo test -p opencat-core --features profile --lib profile
cargo test -p opencat-core --features profile --lib render
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

### Acceptance

- Existing `subtree_snapshot_hit/miss` remains for continuity.
- New artifact reason counts (hit, first_record, evicted_or_absent, replaced) reconcile with existing hit/miss categories.
- `profile-showcase.jsonl` output clearly explains whether remaining misses are semantic or cache-policy driven.

---

## Phase 3: Connect Analyze Reuse To Render Diagnostics

### Why

The profile should show whether backend work happened despite Analyze already proving the subtree stable. That lets us spot cases where Merkle skip is working but Render still records because cache artifacts are unavailable.

### How

Carry a lightweight per-node reuse marker from Analyze to Render:

```rust
pub enum AnalyzeReuseState {
    Fresh,
    ReusedFromHistory,
    CompositeBlocked,
}
```

Store it alongside `DisplayNodeAnalysis` or in a parallel table on `AnnotatedDisplayTree`. Prefer a parallel table if it keeps `DisplayNodeAnalysis` focused on fingerprints.

Render should not use this marker to change behavior in this phase. It should only emit profile labels when a subtree snapshot request is made:

```text
subtree_snapshot_request_after_analyze (result: fresh/reused/composite_blocked)
```

### Files

- Modify: `crates/opencat-core/src/analyze/annotation.rs`
- Modify: `crates/opencat-core/src/render/dispatch.rs`
- Modify: `crates/opencat-core/src/profile/mod.rs`
- Modify: `crates/opencat-core/src/profile/aggregator.rs`
- Modify: `crates/opencat-core/src/profile/output.rs`

### Tests

- Add annotation tests proving reused/copied subtree children receive `ReusedFromHistory`.
- Add profile aggregator tests for the new request labels.
- If render tests are too expensive, keep this phase as pure profile aggregation plus focused annotation tests.

### Commands

```bash
cargo test -p opencat-core --lib analyze::annotation
cargo test -p opencat-core --features profile --lib
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

### Acceptance

- Render behavior is unchanged.
- Profile can answer: "this subtree was semantically reusable, but backend still recorded because artifact was absent/evicted" using the single `subtree_snapshot_request_after_analyze` event with result values (fresh/reused/composite_blocked).

---

## Phase 4: Remove Redundant Analyze Hashing Paths

### Why

Once decision reasons are explicit and covered by tests, any Analyze path that recomputes a display recorded subtree hash from expanded DisplayNode semantics should be removed or converted to consume `DisplayRecordedSubtreeFingerprint`.

The point is to enforce this invariant:

```text
Display builds recorded subtree semantics once.
Analyze consumes the carried recorded subtree fingerprint.
Analyze does not rederive recorded display subtree identity.
```

### How

Run scans:

```bash
rg -n "DisplayNodeFp|ClipFp|hash_recorded_semantics|hash_display_node_for_hidden_subtree|DefaultHasher|semantics\\.bounds" crates/opencat-core/src -g '*.rs'
rg -n "DisplayRecordedFingerprint::from_display_node|display_recorded_subtree_fingerprint" crates/opencat-core/src -g '*.rs'
```

For each hit:

- keep `DisplayRecordedFingerprint` and `display_recorded_subtree_fingerprint` only where they are the canonical construction path;
- remove ad hoc recorded subtree recomputation from Analyze decision paths;
- keep tests for hidden subtree hashing, because hidden subtree display semantics still need to flow through the same recorded subtree fingerprint.

### Files

- Likely modify: `crates/opencat-core/src/analyze/fingerprint/mod.rs`
- Likely modify: `crates/opencat-core/src/display/build.rs`
- Likely modify tests in the same modules.

### Commands

```bash
cargo test -p opencat-core --lib analyze::fingerprint display::build
cargo test -p opencat-core --features profile --lib
```

### Acceptance

- Scan has no old semantics hits in `layout`, `display`, `analyze`, `render`.
- Hidden subtree paint changes still affect recorded subtree fingerprints.
- No duplicate hash construction for the same semantic layer.

---

## Phase 5: Profile Showcase Budget And Regression Gate

### Why

The real scene is `json/profile-showcase.jsonl`. If a future change claims to improve Merkle cache behavior, it should show up there without relying on hand inspection.

### How

Add a profile regression test or documented script that captures stable summary fields:

```text
frames
layout_merkle_skipped_nodes
analyze_merkle_skipped_nodes
analyze_recorded_hit_nodes
analyze_composite_blocked_nodes
subtree_snapshot_cache_hits
subtree_snapshot_cache_misses
subtree_snapshot_cache_evictions
```

If a full render is too expensive for unit tests, keep the command as a required manual verification in this plan and add a small synthetic profile test for field presence.

### Files

- Modify: `crates/opencat-core/src/pipeline/default.rs`
- Modify: `crates/opencat-core/src/profile/output.rs`
- Optional create: `scripts/profile-showcase.sh`

### Commands

```bash
cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl
```

### Acceptance

- The command completes and writes `out/profile-showcase.mp4`.
- The summary includes all Merkle/analyze/render diagnostic fields.
- Any regression in skip counts or unexplained backend misses is visible in one text block.

---

## Non-Goals

- Do not replace backend cache keys with `DisplayRecordedSubtreeFingerprint`.
  `SubtreeSnapshotFingerprint` has different semantics because it includes descendant composite placement.

- Do not force every cache hit to be a Merkle hit.
  Backend cache may legitimately hit an artifact even when higher-level analysis was recomputed.

- Do not add a cryptographic Merkle hash.
  This is an in-process semantic fingerprint system; `ahash` is acceptable for speed and consistency with current code.

- Do not churn unrelated `DefaultHasher` uses outside the cache chain.

- Do not rename all profile fields away from `merkle`.
  The user-facing concept remains Merkle-style subtree convergence; if naming changes later, do it as a separate compatibility pass.

---

## Final Verification Checklist

- [ ] `cargo fmt`
- [ ] `cargo test -p opencat-core --lib`
- [ ] `cargo test -p opencat-core --features profile --lib`
- [ ] `cargo run --bin opencat --release --features profile -- json/profile-showcase.jsonl`
- [ ] Old semantics scan is clean:

```bash
rg -n "DisplayNodeFp|ClipFp|hash_recorded_semantics|hash_display_node_for_hidden_subtree|DefaultHasher|semantics\\.bounds" crates/opencat-core/src/layout crates/opencat-core/src/display crates/opencat-core/src/analyze crates/opencat-core/src/render -g '*.rs'
```

- [ ] Profile output explains all three layers:
  - semantic identity: Merkle/recorded hits;
  - policy eligibility: snapshot eligibility/composite blocked;
  - artifact availability: backend cache hit/miss/eviction/replaced.

