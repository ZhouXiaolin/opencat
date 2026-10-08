import { afterEach, describe, expect, test, vi } from 'vitest';
import {
  createSurfaceWithFallback,
  getReusableSurface,
  releaseReusableSurface,
} from '../../crates/opencat-web/web/src/media/exporter';

describe('createSurfaceWithFallback', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  test('returns the WebGL surface when MakeWebGLCanvasSurface succeeds', () => {
    const canvas = { width: 100, height: 100 };
    const surface = { kind: 'webgl' };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn(() => surface),
      MakeSWCanvasSurface: vi.fn(() => ({ kind: 'sw' })),
    };

    const result = createSurfaceWithFallback(CK as any, canvas as any);

    expect(result).toBe(surface);
    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledWith(canvas, undefined, undefined);
    expect(CK.MakeSWCanvasSurface).not.toHaveBeenCalled();
  });

  test('falls back to MakeSWCanvasSurface when MakeWebGLCanvasSurface throws', () => {
    const canvas = { width: 100, height: 100 };
    const swSurface = { kind: 'sw' };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn(() => {
        throw new Error('failed to create webgl context: err 0');
      }),
      MakeSWCanvasSurface: vi.fn(() => swSurface),
    };
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});

    const result = createSurfaceWithFallback(CK as any, canvas as any);

    expect(result).toBe(swSurface);
    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledOnce();
    expect(CK.MakeSWCanvasSurface).toHaveBeenCalledWith(canvas);
    expect(warn).toHaveBeenCalled();
  });

  test('falls back to MakeSWCanvasSurface when MakeWebGLCanvasSurface returns null', () => {
    const canvas = { width: 100, height: 100 };
    const swSurface = { kind: 'sw' };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn(() => null),
      MakeSWCanvasSurface: vi.fn(() => swSurface),
    };

    const result = createSurfaceWithFallback(CK as any, canvas as any);

    expect(result).toBe(swSurface);
    expect(CK.MakeSWCanvasSurface).toHaveBeenCalledWith(canvas);
  });

  test('returns null when both WebGL and software surface creation fail', () => {
    const canvas = { width: 100, height: 100 };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn(() => {
        throw new Error('failed to create webgl context: err 0');
      }),
      MakeSWCanvasSurface: vi.fn(() => null),
    };
    const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});

    const result = createSurfaceWithFallback(CK as any, canvas as any);

    expect(result).toBeNull();
    expect(warn).toHaveBeenCalled();
  });

  test('passes colorSpace and opts through to MakeWebGLCanvasSurface', () => {
    const canvas = { width: 100, height: 100 };
    const surface = { kind: 'webgl' };
    const colorSpace = { id: 'srgb' };
    const opts = { alphaType: 'premul' };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn(() => surface),
      MakeSWCanvasSurface: vi.fn(),
    };

    createSurfaceWithFallback(CK as any, canvas as any, colorSpace as any, opts as any);

    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledWith(canvas, colorSpace, opts);
  });
});

describe('getReusableSurface', () => {
  // Regression: a render loop that created a fresh canvas surface per frame
  // leaked ~one framebuffer (1920×1080×4 = 8 MB) of GPU-process memory per
  // frame and SIGTRAP'd the GPU process around frame ~380. The cached surface
  // must be created exactly once per canvas and be the same object each call.
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  test('creates the surface once and returns the same object on later calls', () => {
    const canvas = { width: 1920, height: 1080 };
    const surface = { kind: 'webgl', deleted: 0, delete() { this.deleted++; } };
    const CK = { MakeWebGLCanvasSurface: vi.fn(() => surface) };

    const a = getReusableSurface(CK as any, canvas as any);
    const b = getReusableSurface(CK as any, canvas as any);

    expect(a).toBe(surface);
    expect(b).toBe(surface);
    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledOnce();
  });

  test('caches per canvas, not globally', () => {
    const canvasA = { width: 1920, height: 1080 };
    const canvasB = { width: 1920, height: 1080 };
    const surfaceA = { kind: 'a' };
    const surfaceB = { kind: 'b' };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn()
        .mockReturnValueOnce(surfaceA)
        .mockReturnValueOnce(surfaceB),
    };

    expect(getReusableSurface(CK as any, canvasA as any)).toBe(surfaceA);
    expect(getReusableSurface(CK as any, canvasB as any)).toBe(surfaceB);
    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledTimes(2);
  });

  test('releaseReusableSurface drops and deletes the cached surface', () => {
    const canvas = { width: 1920, height: 1080 };
    let deleted = 0;
    const first = { kind: 'first', delete() { deleted++; } };
    const second = { kind: 'second', delete() { deleted++; } };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn()
        .mockReturnValueOnce(first)
        .mockReturnValueOnce(second),
    };

    expect(getReusableSurface(CK as any, canvas as any)).toBe(first);
    releaseReusableSurface(canvas as any);
    expect(deleted).toBe(1);

    expect(getReusableSurface(CK as any, canvas as any)).toBe(second);
    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledTimes(2);
  });

  test('returns null and caches nothing when creation fails', () => {
    const canvas = { width: 1920, height: 1080 };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn(() => null),
      MakeSWCanvasSurface: vi.fn(() => null),
    };

    // A failed creation must not be cached, so a later retry re-attempts it.
    expect(getReusableSurface(CK as any, canvas as any)).toBeNull();
    expect(getReusableSurface(CK as any, canvas as any)).toBeNull();
    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledTimes(2);
  });
});
