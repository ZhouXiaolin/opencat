/// The wasm realm's canvas_api shim overwrites `globalThis.OffscreenCanvas`
/// with a 2d-only recording face — the browser realm IS the page, so the
/// overwrite is global (engine realms are isolated V8 worlds and don't have
/// this hazard). The shim stashes the real browser class before overwriting
/// (canvas_api.js `__ocRealOffscreenCanvas`); page-world TS consumers that
/// need actual pixels/GL must construct through this alias.
type RealCtor = new (width: number, height: number) => OffscreenCanvas;

export const RealOffscreenCanvas: RealCtor =
  ((globalThis as unknown as Record<string, unknown>).__ocRealOffscreenCanvas as RealCtor | undefined) ??
  OffscreenCanvas;
