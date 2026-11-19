// GPU-stream executor entry: run a frame's section-13 streams through the
// browser-native WebGL2 / WebGPU executors, registering their output in
// `generatedImageCanvas` (WebGL2: the stream canvas itself) /
// `generatedImageRgba` (wgpu: CPU readback) before the CanvasKit draw-stream
// replay consumes its `ImageRef::Generated` placeholders.
//
// Mirrors the engine's GpuExecutor split (glow backend / wgpu backend): one
// executor per backend, resource tables persisting across streams (only the
// first frame carries resource creation), one output texture per stream.

import { splitSections } from '../draw-ir/decode';
import { decodeGpuSection } from '../draw-ir/gpu-stream';
import { GlStreamExecutor } from './gl-executor';
import { WgpuStreamExecutor } from './wgpu-executor';

let glExecutor: GlStreamExecutor | null = null;
let wgpuExecutor: WgpuStreamExecutor | null = null;
/// A WebGPU-stream executor init failure is sticky: the design genuinely
/// needs it, so failing half-invisible is worse than failing loud.
let wgpuInitError: string | null = null;

/// Execute one frame's GPU streams. Returns the image ids the executors
/// registered in `generatedImageCanvas` / `generatedImageRgba` (the caller
/// prunes unconsumed ones from the previous frame to bound the maps).
export async function executeGpuStreamsForFrame(irBytes: Uint8Array): Promise<string[]> {
  const gpu = decodeGpuSection(splitSections(irBytes));
  if (!gpu || gpu.streams.length === 0) return [];
  const injected: string[] = [];
  for (const stream of gpu.streams) {
    if (stream.backend === 'webgl2') {
      glExecutor ??= new GlStreamExecutor();
      // WebGL2 registration is synchronous — the canvas slot is already in.
      glExecutor.execute(stream);
    } else {
      if (wgpuInitError) throw new Error(`WebGPU stream executor unavailable: ${wgpuInitError}`);
      wgpuExecutor ??= new WgpuStreamExecutor();
      if (!(await wgpuExecutor.ensureDevice())) {
        wgpuInitError = 'navigator.gpu unavailable (adapter/device request failed)';
        throw new Error(`WebGPU stream executor unavailable: ${wgpuInitError}`);
      }
      wgpuExecutor.execute(stream);
    }
    injected.push(stream.imageId);
  }
  // Readbacks for webgl2 streams are synchronous; webgpu readbacks are
  // copyTextureToBuffer → mapAsync — join them before the draw replay.
  await wgpuExecutor?.settle();
  return injected;
}

/// Tear down executors (page unload / export end).
export function disposeGpuExecutors(): void {
  glExecutor?.dispose();
  glExecutor = null;
  wgpuExecutor?.dispose();
  wgpuExecutor = null;
  wgpuInitError = null;
}
