# Task 15: Hidden Descendant Script Mutations And Nested Canvas Rules

## Decisions Not In Spec

### 1. `seed_text_sources_for_visible_subtree` name kept unchanged
The spec suggested renaming to `seed_text_sources_for_script_targets`, but the function is called from `push_script_scope_for_visible_subtree` and other places. Renaming would require updating ~9 call sites and would be a larger refactor than justified. The Canvas hidden children traversal was added as a new arm in the existing match without renaming.

### 2. Fixed `build_display_node` to populate draw_slot's `hidden_subtree`
The spec assumed the draw_slot's `hidden_subtree` would be populated elsewhere (compositor steps), but the `build_display_node` in `display/build.rs` always set it to `Vec::new()`. Without this fix, `DrawScriptDisplayItem.hidden_subtree` was empty even when a canvas had hidden children, making `DrawSubtreePicture` commands silently render nothing.

The fix moved `build_hidden_subtree()` before draw_slot construction and cloned the result into the draw_slot's `hidden_subtree` field.

### 3. Tests use Composition builder API, not markup parsing
The spec provided tests using markup format (`<opencat>...</opencat>`) with `parse::markup::parse()` and helpers `render_single_frame_from_parsed` / `assert_pixel_rgba`. These helpers don't exist. Tests were adapted to use the existing `Composition::new()` builder API and `render_frame_rgba()` / `pixel_rgba()` helpers.

### 4. JS API is higher-level than Rust bindings
The Rust bindings expose `__canvas_draw_picture(id, owner_id, x, y)` directly, but JS runtime (`canvas_api.js`) provides `ctx.getCanvasById(id).drawPicture(handle, x, y)` with `getSubTree()` as a convenience wrapper. The spec tests use these higher-level JS APIs.

## Changes Made

### File: `crates/opencat-core/src/resolve/resolve.rs`
- **Line ~909**: Added `NodeKind::Canvas(canvas)` case to `seed_text_sources_for_visible_subtree` that traverses `canvas.hidden_children_ref()` and recursively seeds text sources for hidden descendants.

### File: `crates/opencat-core/src/display/build.rs`
- **Line ~84-95**: Moved `build_hidden_subtree()` call before draw_slot construction. Changed draw_slot's `hidden_subtree` from `Vec::new()` to `hidden_subtree.clone()` so that `DrawSubtreePicture` commands in the draw slot can access the actual hidden children display items during rendering.

### File: `crates/opencat-engine/src/render.rs`
- Added three tests (see below)

## Already Complete (from previous tasks)

### Step 1: `collect_visual_script_targets` traverses hidden children
Already implemented at resolve.rs:1148-1151. The function already calls `canvas.hidden_children_ref()` and registers hidden descendants as visual targets.

### Step 4: Recursion guard
Already implemented in `render/helpers.rs:1107-1112`. The `execute_draw_subtree_picture` function checks `ctx.hidden_picture_stack.contains(owner_id)` before rendering and returns `RenderError::InvalidArgument` if recursive.

### Step 3: Nested canvas picture rule
Architecturally correct: each canvas's `build_hidden_subtree` only includes its own `hidden_children`. Nested canvas hidden children are stored separately and only rendered when their own `DrawSubtreePicture` is processed.

## Tradeoffs
- Keeping `hidden_subtree` as a separate field on both `DisplayNode` and `DrawScriptDisplayItem` creates redundancy, but the architecture separates the node's tree-level hidden children from the draw slot's rendering context. Consolidating would require significant refactoring of the compositor and annotation layers.
