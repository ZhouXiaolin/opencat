import { afterEach, describe, expect, test, vi } from 'vitest';
import { snapshotCanvasToImageBitmap } from '../../crates/opencat-web/web/src/media/exporter';

describe('exporter canvas snapshots', () => {
  afterEach(() => {
    vi.unstubAllGlobals();
  });

  test('uses OffscreenCanvas transferToImageBitmap without PNG blob encoding', async () => {
    const bitmap = {};
    const canvas = {
      transferToImageBitmap: vi.fn(() => bitmap),
      toBlob: vi.fn(),
    };
    const createImageBitmap = vi.fn();
    vi.stubGlobal('createImageBitmap', createImageBitmap);

    await expect(snapshotCanvasToImageBitmap(canvas as any)).resolves.toBe(bitmap);

    expect(canvas.transferToImageBitmap).toHaveBeenCalledOnce();
    expect(canvas.toBlob).not.toHaveBeenCalled();
    expect(createImageBitmap).not.toHaveBeenCalled();
  });

  test('uses createImageBitmap directly for HTML canvas before falling back to PNG blobs', async () => {
    const bitmap = {};
    const canvas = { toBlob: vi.fn() };
    const createImageBitmap = vi.fn(async () => bitmap);
    vi.stubGlobal('createImageBitmap', createImageBitmap);

    await expect(snapshotCanvasToImageBitmap(canvas as any)).resolves.toBe(bitmap);

    expect(createImageBitmap).toHaveBeenCalledWith(canvas);
    expect(canvas.toBlob).not.toHaveBeenCalled();
  });
});
