# Implementation Notes: JSONL Normalization

## Task
Normalize JSONL example files - consolidate scripts, migrate `ctx.getCanvas()` to `ctx.getCanvasById()`.

## Findings

### 1. `ctx.getCanvas()` Migration
- **No `ctx.getCanvas()` calls found** in any JSONL file under `json/`
- The only canvas-using file (`profile-showcase.jsonl`) already uses `ctx.getCanvasById('s1-canvas')`
- The runtime (`canvas_api.js:1154`) already throws an error for `ctx.getCanvas()` telling users to use `getCanvasById()`
- **No changes needed** for this part

### 2. Script Consolidation
- 3 files have multiple script nodes:
  - `ecommerce.jsonl`: 3 scripts (parentIds: `login`, `home`, `product`)
  - `opencat-promo.jsonl`: 4 scripts (parentIds: `scene1`, `scene2`, `scene3`, `root`)
  - `kepler-laws/kepler-laws.jsonl`: 10 scripts (parentIds: `s1-bg`, `scene1`, `s2-canvas`, `scene2`, `s3-canvas`, `scene3`, `s4-bg`, `scene4`, `s5-bg`, `scene5`)
- **All scripts have unique parentIds** - no file has multiple scripts sharing the same parent
- Scripts are scoped to their parent scene/div, so consolidating would break scene-level scoping
- **No consolidation performed** - would change behavior incorrectly

### 3. Decision: No Changes Needed
After thorough analysis:
- All `ctx.getCanvas()` calls already migrated
- All multi-script files have valid reasons for multiple scripts (different parent scopes)
- The JSONL files are already in their normalized form

## Tradeoffs
- **Not consolidating**: The spec asked for "one script per composition" but doing so would require moving scene-scoped animations to a root-level script with explicit scene activation logic, which is a significant behavioral change and potentially fragile. The current per-scene script pattern is the idiomatic approach.
