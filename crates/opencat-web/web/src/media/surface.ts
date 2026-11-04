import type { CanvasKit, ColorSpace, Surface, WebGLOptions } from 'canvaskit-wasm';

export function createSurfaceWithFallback(
  CK: CanvasKit,
  canvas: HTMLCanvasElement | OffscreenCanvas,
  colorSpace?: ColorSpace,
  opts?: WebGLOptions,
): Surface | null {
  if (typeof CK.MakeWebGLCanvasSurface === 'function') {
    try {
      const surface = CK.MakeWebGLCanvasSurface(canvas, colorSpace, opts);
      if (surface) return surface;
    } catch (err) {
      console.warn(
        'CanvasKit: WebGL surface creation failed, falling back to software surface.',
        err,
      );
    }
  }

  if (typeof CK.MakeSWCanvasSurface === 'function') {
    try {
      return CK.MakeSWCanvasSurface(canvas) ?? null;
    } catch (err) {
      console.warn('CanvasKit: software surface creation failed.', err);
      return null;
    }
  }

  return null;
}

// A CanvasKit `Surface` is backed by a WebGL context/EGL framebuffer that
// `surface.delete()` does NOT fully release in headless Chrome: creating a new
// canvas surface per frame accumulates ~one frame buffer (1920×1080×4 = 8 MB)
// of GPU-process memory per frame until the GPU process SIGTRAPs
// (`exit_code=133`, "GPU state invalid" → the tab's WebGL context is lost).
// Reuse one surface per canvas instead — a long render loop is streaming by
// nature and should keep exactly one framebuffer alive.
const reusableSurfaces = new WeakMap<HTMLCanvasElement | OffscreenCanvas, Surface>();

/** Get or create the reusable CanvasKit surface bound to `canvas`. */
export function getReusableSurface(
  CK: CanvasKit,
  canvas: HTMLCanvasElement | OffscreenCanvas,
  colorSpace?: ColorSpace,
  opts?: WebGLOptions,
): Surface | null {
  const existing = reusableSurfaces.get(canvas);
  if (existing) return existing;
  const surface = createSurfaceWithFallback(CK, canvas, colorSpace, opts);
  if (surface) reusableSurfaces.set(canvas, surface);
  return surface;
}

/** Drop the cached surface for `canvas` (e.g. after a size or context change). */
export function releaseReusableSurface(canvas: HTMLCanvasElement | OffscreenCanvas): void {
  const surface = reusableSurfaces.get(canvas);
  if (!surface) return;
  reusableSurfaces.delete(canvas);
  try {
    surface.delete();
  } catch {
    /* ignore CanvasKit teardown failures */
  }
}
