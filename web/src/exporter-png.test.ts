import { afterEach, describe, expect, test, vi } from 'vitest';

describe('PNG frame export', () => {
  afterEach(() => {
    vi.resetModules();
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  test('renders and downloads from an isolated export canvas instead of the preview canvas', async () => {
    const previewCanvas = {
      width: 320,
      height: 180,
      toBlob: vi.fn((resolve: (blob: Blob) => void) => resolve(new Blob(['preview']))),
    };
    const exportBlob = new Blob(['png'], { type: 'image/png' });
    const exportCanvas = {
      width: 0,
      height: 0,
      toBlob: vi.fn((resolve: (blob: Blob) => void) => resolve(exportBlob)),
    };
    const surface = {
      getCanvas: vi.fn(() => ({})),
      flush: vi.fn(),
      delete: vi.fn(),
    };
    const CK = {
      MakeWebGLCanvasSurface: vi.fn(() => surface),
    };
    const renderer = {
      plan_video_frames: vi.fn(() => '[]'),
      build_frame_ir: vi.fn(() => new Uint8Array()),
    };
    const anchor = {
      href: '',
      download: '',
      click: vi.fn(),
    };

    vi.stubGlobal('__canvasKit', CK);
    vi.stubGlobal('document', {
      createElement: vi.fn((tag: string) => {
        if (tag === 'canvas') return exportCanvas;
        if (tag === 'a') return anchor;
        throw new Error(`unexpected element: ${tag}`);
      }),
    });
    vi.stubGlobal('URL', {
      createObjectURL: vi.fn(() => 'blob:png'),
      revokeObjectURL: vi.fn(),
    });

    vi.doMock('../../crates/opencat-web/web/src/wasm', () => ({
      getRendererOrThrow: () => renderer,
    }));
    vi.doMock('../../crates/opencat-web/web/src/draw-ir', () => ({
      renderEncodedDrawFrame: vi.fn(),
    }));
    vi.doMock('../../crates/opencat-web/web/src/media/video-frame-injector', () => ({
      injectVideoFramesForRender: vi.fn(async () => {}),
    }));

    const { exportPngFrame } = await import('../../crates/opencat-web/web/src/media/exporter');

    await exportPngFrame(
      'composition',
      previewCanvas as any,
      { width: 640, height: 360, fps: 30, frames: 1 },
      0,
      {},
    );

    expect(document.createElement).toHaveBeenCalledWith('canvas');
    expect(exportCanvas.width).toBe(640);
    expect(exportCanvas.height).toBe(360);
    expect(CK.MakeWebGLCanvasSurface).toHaveBeenCalledWith(exportCanvas, undefined, undefined);
    expect(exportCanvas.toBlob).toHaveBeenCalledOnce();
    expect(previewCanvas.toBlob).not.toHaveBeenCalled();
    expect(anchor.download).toBe('frame_0000.png');
    expect(anchor.click).toHaveBeenCalledOnce();
  });
});
