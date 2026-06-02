import { afterEach, describe, expect, test, vi } from 'vitest';
import { createSurfaceWithFallback } from '../../crates/opencat-web/web/src/media/exporter';

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
