// Offscreen-canvas pixel readback for the wasm realm: the recorded canvas
// commands are compiled + OCIR-encoded on the wasm side
// (`__opencatRasterCanvas`), and this hook replays the resulting frame
// through the same CanvasKit interpreter the main frame uses, then reads the
// pixels back. The engine's counterpart rasterizes with skia_safe — same
// family, same paint/AA semantics as the CanvasKit path.
//
// Pixel format contract: RGBA, **premultiplied** (engine `rasterize_offscreen`
// rasterizes into an RGBA8888/Premul ImageInfo; getImageData consumers see
// the same bytes).

import type { CanvasKit, Surface } from 'canvaskit-wasm';
import { renderEncodedDrawFrame } from './draw-ir';

type CanvasKitGlobal = typeof globalThis & { __canvasKit?: CanvasKit };

export interface RasterReplay {
  (id: string, w: number, h: number, encoded: Uint8Array): Uint8Array | null;
}

/// Install `globalThis.__ocRasterCanvasReplay`. Called by the wasm native
/// `__opencatRasterCanvas` after it compiles + encodes the canvas commands.
export function installRasterCanvasReplay(): void {
  const g = globalThis as CanvasKitGlobal & { __ocRasterCanvasReplay?: RasterReplay };
  if (g.__ocRasterCanvasReplay) return;
  g.__ocRasterCanvasReplay = (id, w, h, encoded): Uint8Array | null => {
    void id;
    const CK = g.__canvasKit;
    if (!CK) return null;
    // CPU raster surface — readPixels must see the drawn result without a
    // GPU round-trip (headless SwiftShader has no WebGL surface readback).
    const surface: Surface | null = CK.MakeSurface(w, h);
    if (!surface) return null;
    try {
      renderEncodedDrawFrame(encoded, surface.getCanvas(), CK, { surface });
      surface.flush();
      const image = surface.makeImageSnapshot();
      if (!image) return null;
      const pixels = image.readPixels(0, 0, {
        width: w,
        height: h,
        colorType: CK.ColorType.RGBA_8888,
        alphaType: CK.AlphaType.Premul,
        colorSpace: CK.ColorSpace.SRGB,
      });
      image.delete();
      if (!pixels || !(pixels instanceof Uint8Array)) return null;
      return pixels;
    } finally {
      surface.delete();
    }
  };
}
