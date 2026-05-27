# Merkle Cache Semantics Implementation Plan

> **For agentic workers:** REQUIRED: Use superpowers:subagent-driven-development (if subagents available) or superpowers:executing-plans to implement this plan. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add unified Element input Merkle fingerprints, then consume them in Layout and Display cache paths for fast subtree equality checks.

**Architecture:** Resolve remains responsible for canonical input semantics only. Layout consumes Element input fingerprints and adds layout-output semantics. Display/Render continue to compute final snapshot keys from display-time facts such as bounds, clip, and child composite placement.

**Tech Stack:** Rust 2024, `ahash`, Taffy layout, existing opencat-core tests.

---

## Chunk 1: Element Input Merkle

### Task 1: Add semantic fingerprint data to resolved elements

**Files:**
- Create: `crates/opencat-core/src/semantic/mod.rs`
- Create: `crates/opencat-core/src/semantic/fingerprint.rs`
- Modify: `crates/opencat-core/src/lib.rs`
- Modify: `crates/opencat-core/src/resolve/tree.rs`
- Modify: `crates/opencat-core/src/resolve/resolve.rs`
- Test: `crates/opencat-core/src/semantic/fingerprint.rs`

- [ ] Add `ElementInputFingerprints` with `structure_subtree`, `layout_input_subtree`, `paint_input_subtree`, `composite_input_subtree`, and `node_count`.
- [ ] Add `compute_element_input_fingerprints(&mut ElementNode)` that walks children first and computes subtree hashes.
- [ ] Run it once at the end of `resolve_ui_tree_with_script_cache`.
- [ ] Add tests showing paint-only changes do not affect layout input hash, and child changes affect ancestor subtree hash.

## Chunk 2: Layout Fast Path

### Task 2: Make LayoutSession consume layout input Merkle

**Files:**
- Modify: `crates/opencat-core/src/layout/mod.rs`

- [ ] Store `layout_input_subtree` and `node_count` in `CachedLayoutNode`.
- [ ] In `update_cached_subtree`, if the cached subtree hash equals the element subtree hash, count all subtree nodes as reused and return without descending.
- [ ] Keep existing local layout/raster/composite hashes for dirty classification while migrating.
- [ ] Add tests for unchanged large subtree skip and single child layout change.

## Chunk 3: Display/Render Key Alignment

### Task 3: Reuse Element paint/composite input fingerprints where display-time data allows

**Files:**
- Modify: `crates/opencat-core/src/display/tree.rs`
- Modify: `crates/opencat-core/src/display/build.rs`
- Modify: `crates/opencat-core/src/analyze/annotation.rs`
- Modify: `crates/opencat-core/src/analyze/fingerprint/mod.rs`

- [ ] Carry element input fingerprints into DisplayNode/AnnotatedDisplayNode.
- [ ] Use paint input fingerprints as the paint semantic base, mixing in display-only bounds and clip.
- [ ] Keep snapshot fingerprints layout-aware by hashing child translation and display bounds.
- [ ] Preserve the existing rule that dirty descendant composite disables subtree snapshot cache.
