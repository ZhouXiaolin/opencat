# Implementation Notes: Task 2 — Move JSONL Builder To Shared Builder Module

## Decisions

- **`_options` parameter in `build_node_inner`**: Passed `BuildOptions` through but prefixed with `_` since `CanvasChildrenMode::HiddenPictureSubtree` can't be implemented yet (needs `Canvas::hidden_child()` from Task 9). The Canvas arm keeps the existing rejection logic unchanged.
- **Backward-compatible wrappers**: `build_tree()` and `build_tree_with_tl()` kept as thin wrappers that delegate to `_with_options` variants with `BuildOptions::JSONL`. JSONL module uses these wrappers unchanged.
- **Deleted `jsonl/builder.rs`** entirely rather than keeping as re-export — cleaner, no indirection.

## Tradeoffs

- `BuildOptions` uses `const` associated constants (`JSONL`, `MARKUP`) rather than constructors — zero-cost, compile-time known.
- `MARKUP` constant references `HiddenPictureSubtree` which isn't functional yet — it compiles but the canvas arm doesn't branch on it. This is intentional to avoid dead-code warnings; the branching logic arrives in Task 9.

## Files Changed

- **Created**: `crates/opencat-core/src/parse/document/builder.rs` — all builder code moved from `jsonl/builder.rs` plus `BuildOptions`
- **Modified**: `crates/opencat-core/src/parse/document.rs` — added `mod builder` and re-exports
- **Modified**: `crates/opencat-core/src/parse/jsonl/mod.rs` — switched import to `crate::parse::document::*`, added regression test
- **Deleted**: `crates/opencat-core/src/parse/jsonl/builder.rs`
