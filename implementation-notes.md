# Implementation Notes: Task 14 - Resolve Hidden Canvas Subtree Pictures Lazily

## Decisions Made

### 1. Hidden children resolved eagerly at resolve time
Hidden children are resolved during `resolve_canvas` alongside the canvas node itself. They inherit style from the canvas computed style. This means their script mutations are baked in at resolve time - consistent with how the rest of the tree works.

### 2. Hidden children stored separately from main children
`ElementCanvas.hidden_children` is a separate `Vec<ElementNode>` from `ElementNode.children` (which remains empty for canvas). This keeps the hidden children out of the normal display/layout traversal.

### 3. Display tree carries hidden subtree as pre-built display nodes
Rather than storing raw `ElementNode`s on the display tree, we build a mini display tree for hidden children during `build_display_node`. This keeps the render phase pure (no resolve/layout needed at render time).

The hidden subtree uses the canvas's own bounds as the layout rect for each hidden child (they fill the canvas). This is a simplification - hidden children inherit the full canvas size.

### 4. DrawSubtreePicture expanded lazily during execute_draw_op
When `DrawSubtreePicture` is encountered during draw-script execution, we render the pre-built hidden subtree display nodes directly. The hidden subtree display nodes are stored on the `DrawScriptDisplayItem` (via `hidden_subtree` field).

### 5. Recursion guard stored in a separate vec on RenderCtx
The `hidden_picture_stack: Vec<String>` prevents infinite recursion if a script calls `drawPicture` on the same canvas id. This is lightweight - just string comparisons.

### 6. Hidden children get same layout as canvas
Hidden children are laid out with the same bounds as the canvas itself. They use absolute positioning within the canvas coordinate space. The translation (x, y) from DrawSubtreePicture is applied as a canvas translate during rendering.

## Trade-offs

- **No layout integration for hidden children**: Hidden children don't participate in the normal layout flow. They're given the canvas bounds as their layout rect. This means CSS layout properties like flex won't work for hidden children - they need explicit sizes. This is acceptable since hidden children are meant to be drawn programmatically via scripts.
- **Hidden subtree is built at display build time, not render time**: The spec suggested lazy building during render, but building at display time is simpler and avoids the need for resolve/layout infrastructure in the render phase. The "lazy" part is that the hidden subtree is only *rendered* when `DrawSubtreePicture` is encountered.

## Changes Not in Spec

- Added `hidden_subtree: Vec<HiddenChildDisplayNode>` to `DrawScriptDisplayItem` - needed to carry the pre-built display data to the render phase.
- Added `HiddenChildDisplayNode` struct to `display/tree.rs` - a simplified display node for hidden children (no children, no clips, no transforms from layout).
- Added `hidden_subtree` field to `DisplayNode` and `AnnotatedDisplayNode` for carrying hidden subtree through annotation.
- Added `hidden_picture_stack: Vec<String>` to `RenderCtx` for recursion guard on `DrawSubtreePicture`.
- Added `DrawSubtreePicture { owner_id, x, y }` variant to `DrawOp` enum.
- `resolve_hidden_children` now receives `InheritedStyle` from the owning canvas (not default) - hidden children inherit style correctly.

## Fixes During Completion

- Fixed borrow conflict in `render_draw_script` by removing `let b = &mut ctx.builder;` alias and using `ctx.builder` directly.
- Added missing `hidden_picture_stack` field to `RenderCtx` construction in `pipeline/default.rs`.
- Added missing `hidden_subtree` fields to test helpers in `analyze/fingerprint/mod.rs`.
