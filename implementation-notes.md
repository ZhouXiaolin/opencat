# Implementation Notes — Task 11: Replace `ctx.getCanvas()` With `ctx.getCanvasById(id)`

## What changed

### `canvas_api.js`
- **Removed** the old `ctx.getCanvas()` implementation (which read `ctx.__currentCanvasTarget` and returned a no-op Proxy when no target was set)
- **Added** `ctx.getCanvas()` that throws: `"ctx.getCanvas is not available; use ctx.getCanvasById(id)"`
- **Added** `ctx.getCanvasById(id)` that validates the id via `assertCanvasTarget()` and returns a canvas drawing object
- **Added** `assertCanvasTarget(id, apiName)` — local helper in the canvas_api IIFE that mirrors `assertVisualTarget` from `node_style.js` (checks `ctx.__targetRegistry.visual`)

### All usage sites migrated
| File | Old | New |
|------|-----|-----|
| `examples/typewriter_canvas.rs` | `ctx.getCanvas()` | `ctx.getCanvasById('typewriter-canvas')` |
| `examples/pendulum_canvas.rs` | `ctx.getCanvas()` | `ctx.getCanvasById('pendulum-canvas')` |
| `examples/compare_transitions.rs` (A) | `ctx.getCanvas()` | `ctx.getCanvasById('compare-canvas-a')` |
| `examples/compare_transitions.rs` (B) | `ctx.getCanvas()` | `ctx.getCanvasById('compare-canvas-b')` |
| `examples/video_playback.rs` | `ctx.getCanvas()` | `ctx.getCanvasById('scene-one-canvas')` |
| `crates/opencat-engine/src/render.rs` | `ctx.getCanvas()` | `ctx.getCanvasById('canvas')` |
| `json/kepler-laws/s1-canvas.js` | `ctx.getCanvas()` | `ctx.getCanvasById('s1-bg')` |
| `json/kepler-laws/s2-canvas.js` | `ctx.getCanvas()` | `ctx.getCanvasById('s2-canvas')` |
| `json/kepler-laws/s3-canvas.js` | `ctx.getCanvas()` | `ctx.getCanvasById('s3-canvas')` |
| `json/kepler-laws/stars-bg.js` | `ctx.getCanvas()` | `ctx.getCanvasById(ctx.__currentCanvasTarget)` |
| `json/profile-showcase.jsonl` | `ctx.getCanvas()` | `ctx.getCanvasById('s1-canvas')` |

## Design decision: `stars-bg.js` shared script

`stars-bg.js` is used as a shared background script referenced by two different canvas nodes (`s4-bg` in scene4, `s5-bg` in scene5). Since the script cannot hardcode a single id, it uses `ctx.getCanvasById(ctx.__currentCanvasTarget)`.

**Tradeoff:** This exposes the internal `__currentCanvasTarget` field to user scripts. A cleaner API would be a `ctx.getOwnCanvas()` method, but that was out of scope for this task. The alternative (duplicating the script) would violate DRY.

## Design decision: duplicated `assertCanvasTarget` vs sharing

`assertCanvasTarget` in `canvas_api.js` is a near-copy of `assertVisualTarget` in `node_style.js`. Both are defined inside their respective IIFEs and don't share scope. Options considered:

1. **Duplicate in both files** (chosen) — simple, no coupling between runtime files
2. **Attach to `ctx`** — would work but pollutes the ctx API surface
3. **Extract to shared file** — would require changes to the runtime loading order

## Not changed

- `pendulum_canvas.rs` line 98 — contains `ctx.getCanvas()` in a descriptive text string shown to users, not executable code
- `crates/opencat-web/web/src/media/exporter.ts` — uses `surface.getCanvas()` (Skia Surface API), not `ctx.getCanvas()`
