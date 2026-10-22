import { afterEach, describe, expect, test, vi } from 'vitest';
import * as wasm from '../../crates/opencat-web/web/src/wasm';
import * as drawIr from '../../crates/opencat-web/web/src/draw-ir';
import * as injector from '../../crates/opencat-web/web/src/media/video-frame-injector';

// 依赖隔离用命名空间 spyOn:vi.doMock 依赖 hoisting,在 bun 的 vitest
// 运行时里会被忽略(动态 import 拿到真实模块)。globals 仍用 stubGlobal。
describe('PNG frame export', () => {
  afterEach(() => {
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
      open_design: vi.fn(async () => {}),
      build_frame_ir: vi.fn(() => ({ ir: new Uint8Array(), mediaPlan: '{}' })),
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

    vi.spyOn(wasm, 'getRendererOrThrow').mockReturnValue(renderer as any);
    vi.spyOn(drawIr, 'renderEncodedDrawFrame').mockImplementation((() => undefined) as any);
    vi.spyOn(injector, 'prepareCatalogVideoSources').mockResolvedValue(undefined);
    vi.spyOn(injector, 'injectVideoFramesForRender').mockResolvedValue(undefined);

    const { exportPngFrame } = await import('../../crates/opencat-web/web/src/media/exporter');

    await exportPngFrame(
      'composition',
      previewCanvas as any,
      { width: 640, height: 360, fps: 30, duration: 1 / 30 },
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
