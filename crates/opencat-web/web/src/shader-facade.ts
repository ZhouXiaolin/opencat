// Browser-side executor for the IR `shader` section.
//
// Symmetric to `three-facade.ts`: the IR carries a component name + props per
// layer (never WGSL), and this side bakes it with the real shaders library —
// the same way `opencat-shaders` bakes it natively. Pixels land in draw-ir's
// generated-image slot via `provideGeneratedImageRgba`, so the OCIR replay
// composites them at the placeholder's z-site.
//
// Clock: `target.time` is the scene clock `f(t)` the core recorded. The
// library is delta-driven (`renderFrame({ deltaSeconds })`), so we step it by
// the difference from the previous rendered time — frame N is a pure function
// of the deltas before it, which is exactly `animate = g(t)`.
//
// Alpha: canvas 2D readback yields straight (unpremultiplied) RGBA, matching
// `GeneratedImageEntry`'s contract and `three-facade.ts`.

import { provideGeneratedImageRgba } from './draw-ir';

/** One layer of a shader target: component name + props, in declaration order. */
export type ShaderLayerJson = {
  component: string;
  props: Record<string, unknown> | null;
};

/** One `<Shader>` target: the canvas, its box, the scene clock and its layers. */
export type ShaderTargetJson = {
  imageId: string;
  canvasId: string;
  width: number;
  height: number;
  time: number;
  layers: ShaderLayerJson[];
};

/** The subset of `createRendererFromJSON`'s return value this facade uses. */
type PresetRenderer = {
  initialize(canvas: HTMLCanvasElement): Promise<void>;
  updatePreset(preset: { components: unknown[] }): void;
  renderFrame(options: { deltaSeconds: number; waitForGpu?: boolean }): Promise<void>;
  dispose(): void;
};

type PresetModule = {
  createRendererFromJSON(
    preset: { components: unknown[] },
    options?: Record<string, unknown>,
  ): PresetRenderer;
};

type TargetState = {
  renderer: PresetRenderer;
  canvas: HTMLCanvasElement;
  width: number;
  height: number;
  /** Clock value already rendered; the next frame steps by `time - rendered`. */
  rendered: number;
  /** Layer tree signature — a change rebuilds the preset. */
  signature: string;
};

/**
 * Layers fold into a component tree: each later layer consumes the composite
 * of everything before it as its child (that is the filter/warp contract —
 * the library allocates the RTT boundary itself).
 */
function buildComponentTree(layers: ShaderLayerJson[]): unknown {
  let node: unknown = null;
  for (const layer of layers) {
    const components = layer.props ? { props: layer.props } : {};
    node =
      node === null
        ? { type: layer.component, ...components }
        : { type: layer.component, ...components, children: [node] };
  }
  return node;
}

function signatureOf(layers: ShaderLayerJson[]): string {
  return JSON.stringify(layers);
}

let presetModule: Promise<PresetModule> | null = null;

/** Load the shaders library once. Fails loud: a missing lib is a build error,
    not a silently empty frame. `shaders/core` is the GPU entry — the package
    root only re-exports registry helpers. */
function loadPresetModule(): Promise<PresetModule> {
  if (!presetModule) {
    presetModule = import('shaders/core').then(
      (mod) => mod as unknown as PresetModule,
      (err) => {
        throw new Error(
          `shader: cannot load the shaders library (${String(err)}); a <Shader> needs it on the web side`,
        );
      },
    );
  }
  return presetModule;
}

/** Read a canvas back through a blob: that path synchronises with the GPU,
    unlike drawImage/createImageBitmap on a live WebGPU canvas. */
async function readCanvasRgba(
  canvas: HTMLCanvasElement,
  width: number,
  height: number,
): Promise<Uint8Array> {
  const blob = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
  if (!blob) throw new Error('shader: canvas readback returned no blob');
  const bitmap = await createImageBitmap(blob);
  const scratch = document.createElement('canvas');
  scratch.width = width;
  scratch.height = height;
  const ctx = scratch.getContext('2d', { willReadFrequently: true });
  if (!ctx) throw new Error('shader: no 2D context for readback');
  ctx.drawImage(bitmap, 0, 0);
  bitmap.close();
  return new Uint8Array(ctx.getImageData(0, 0, width, height).data.buffer.slice(0));
}

export class ShaderWebExecutor {
  /** One preset renderer per `<Shader>` canvas id. */
  private targets = new Map<string, TargetState>();

  /** Build (or rebuild) the preset renderer for every target this frame. */
  async prepare(targets: ShaderTargetJson[]): Promise<void> {
    for (const target of targets) {
      const signature = signatureOf(target.layers);
      const existing = this.targets.get(target.canvasId);
      if (existing && existing.signature === signature) continue;

      const mod = await loadPresetModule();
      const canvas = existing?.canvas ?? document.createElement('canvas');
      canvas.width = target.width;
      canvas.height = target.height;

      const renderer = mod.createRendererFromJSON({
        components: [buildComponentTree(target.layers)],
      });
      await renderer.initialize(canvas);
      // A browser without WebGPU resolves initialize() and then no-ops every
      // frame — that would composite an empty slot silently, so fail loud here.
      const failure = (renderer as { getFailureReason?: () => unknown }).getFailureReason?.();
      if (failure) {
        throw new Error(`shader: target ${target.canvasId} cannot render — ${String(failure)}`);
      }

      existing?.renderer.dispose();
      this.targets.set(target.canvasId, {
        renderer,
        canvas,
        width: target.width,
        height: target.height,
        // A fresh renderer starts at clock 0, so the first frame advances by
        // the whole target time.
        rendered: 0,
        signature,
      });
    }
  }

  /** Step every target's clock to `target.time` and publish its pixels. */
  async renderFrame(targets: ShaderTargetJson[]): Promise<void> {
    for (const target of targets) {
      const state = this.targets.get(target.canvasId);
      if (!state) throw new Error(`shader: target ${target.canvasId} was not prepared`);

      if (state.width !== target.width || state.height !== target.height) {
        state.canvas.width = target.width;
        state.canvas.height = target.height;
        state.width = target.width;
        state.height = target.height;
        state.renderer.updatePreset({ components: [buildComponentTree(target.layers)] });
      }

      // The library accumulates deltas; clamp at 0 so a frozen clock (a target
      // repeated across frames, `frameLocked` shape) repaints without rewinding.
      const delta = Math.max(0, target.time - state.rendered);
      await state.renderer.renderFrame({ deltaSeconds: delta });
      state.rendered += delta;

      const rgba = await readCanvasRgba(state.canvas, state.width, state.height);
      provideGeneratedImageRgba(
        BigInt(target.imageId),
        state.width,
        state.height,
        rgba,
      );
    }
  }

  dispose(): void {
    for (const state of this.targets.values()) state.renderer.dispose();
    this.targets.clear();
  }
}

/** Page-level singleton: GPU contexts and preset renderers are expensive and
    one document only ever needs one. */
export const shaderExecutor = new ShaderWebExecutor();
