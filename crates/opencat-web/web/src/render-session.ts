/// 打开 design → 逐帧产 canvas 的半环唯一实现。
///
/// 两个消费方（dev 应用侧 `exportMp4` 的 ExportClip、oracle harness 页的
/// 采样/整段通道）共用：open-once 管线、comp 边界检查、可复用 CanvasKit
/// surface 全部在这里，消费方只剩各自的编码 adapter（WebAV / 裸
/// WebCodecs+MP4Box）。
///
/// 这里只管生产渲染路径。native WebGPU 的 warmup/settle 探针时序是 oracle
/// 的 native 探针专属语义，住在 oracle harness 页
/// （`crates/opencat-web-oracle/web/test-oracle.html`），不进生产 exporter。

import type { CanvasKit, Surface } from 'canvaskit-wasm';
import type { FrameMediaPlanWire } from './generated/media-plan.schema.generated';
import { getRendererOrThrow, clearBlobs } from './wasm';
import { clearVideoCache } from './media/video-decoder';
import { injectVideoFramesForRender, prepareCatalogVideoSources } from './media/video-frame-injector';
import { renderEncodedDrawFrame } from './draw-ir';
import { warmStaticSvgImage } from './draw-ir/ck-adapter';
import { generatedImageCanvas, generatedImageRgba } from './draw-ir/decode';
import { executeGpuStreamsForFrame, disposeGpuExecutors } from './gpu/executors';
import { getReusableSurface, releaseReusableSurface } from './media/surface';
import type { CompositionInfo } from './types';

type CanvasKitGlobal = typeof globalThis & { __canvasKit?: CanvasKit };

export interface RenderSessionOptions {
  /// 逐帧注入解码 VideoFrame 的参数分叉：oracle 页 headless SwiftShader
  /// 无法 GPU 纹理导入，需要 `'source'`（CPU RGBA 直出）；dev 应用侧不传。
  frameOutput?: 'source';
}

// ── 帧预算 / 内存门禁 ──
// 浏览器侧是流式渲染：每帧的 GL/wgpu 回读、generated 像素、IR 解码都是
// 一次性消费。任何一处滞留（执行器卡死、缓存失控）都应该显式终止管线并
// 带出诊断，而不是静默累积直到 chrome 被系统 OOM 击杀。三个门都可用 URL
// query 覆盖：?frameBudgetMs= / ?heapCapMB= / ?wasmCapMB=。注意 heap 门
// 只测 JS 堆（performance.memory，Chrome 专有），wasm 门只测 core wasm
// 线性内存——canvaskit 自身的 wasm 堆在其实例内部，取不到引用。
const FRAME_BUDGET_MS_DEFAULT = 30_000;
const HEAP_CAP_MB_DEFAULT = 2048;
const WASM_CAP_MB_DEFAULT = 4096;

let gateLimits: { budgetMs: number; heapCapBytes: number; wasmCapBytes: number } | null = null;

function getGateLimits(): { budgetMs: number; heapCapBytes: number; wasmCapBytes: number } {
  gateLimits ??= (() => {
    const q = new URLSearchParams(location.search);
    const num = (name: string, dfltMb: number, dfltMs?: number) => {
      const raw = q.get(name);
      const v = raw ? Number(raw) : NaN;
      return Number.isFinite(v) && v > 0 ? v : (dfltMs ?? dfltMb);
    };
    return {
      budgetMs: num('frameBudgetMs', FRAME_BUDGET_MS_DEFAULT, FRAME_BUDGET_MS_DEFAULT),
      heapCapBytes: num('heapCapMB', HEAP_CAP_MB_DEFAULT) * 1024 * 1024,
      wasmCapBytes: num('wasmCapMB', WASM_CAP_MB_DEFAULT) * 1024 * 1024,
    };
  })();
  return gateLimits;
}

function assertFrameGates(frame: number, elapsedMs: number): void {
  const { budgetMs, heapCapBytes, wasmCapBytes } = getGateLimits();
  if (elapsedMs > budgetMs) {
    throw new Error(`[gate] frame ${frame} exceeded budget: ${Math.round(elapsedMs)}ms > ${budgetMs}ms (tune ?frameBudgetMs=)`);
  }
  const mem = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory;
  const heap = mem?.usedJSHeapSize;
  if (heap !== undefined && heap > heapCapBytes) {
    throw new Error(`[gate] frame ${frame} JS heap ${Math.round(heap / 1048576)}MB > cap ${Math.round(heapCapBytes / 1048576)}MB (tune ?heapCapMB=)`);
  }
  const wasmBytes = (globalThis as { __opencatWasmMemory?: WebAssembly.Memory }).__opencatWasmMemory?.buffer.byteLength;
  if (wasmBytes !== undefined && wasmBytes > wasmCapBytes) {
    throw new Error(`[gate] frame ${frame} core wasm memory ${Math.round(wasmBytes / 1048576)}MB > cap ${Math.round(wasmCapBytes / 1048576)}MB (tune ?wasmCapMB=)`);
  }
}

export class RenderSession {
  readonly canvas: HTMLCanvasElement | OffscreenCanvas;
  readonly envelope: string;
  readonly comp: CompositionInfo;
  /// 帧数边界（oracle 页显式取 meta.frames，dev 侧 fps×duration 上取整——
  /// 两侧换算口径不同，由调用方定）。
  readonly frames: number;
  private readonly frameOutput?: 'source';
  private surfaceSize: { w: number; h: number } = { w: 0, h: 0 };
  /// GPU-stream injections from the previous frame that the draw replay
  /// never consumed (invisible placeholders). Pruned each frame — an
  /// unconsumed 1080p RGBA slot is ~8MB, and a multi-minute export would
  /// otherwise pin hundreds of them (the export-OOM class of bug).
  private gpuInjectedKeys: string[] = [];

  private constructor(
    canvas: HTMLCanvasElement | OffscreenCanvas,
    envelope: string,
    comp: CompositionInfo,
    frames: number,
    opts: RenderSessionOptions,
  ) {
    this.canvas = canvas;
    this.envelope = envelope;
    this.comp = comp;
    this.frames = frames;
    this.frameOutput = opts.frameOutput;
  }

  /// 打开常驻管线（fetch 资源 → catalog → open → WebCodecs 视频源）。
  /// create 一步到位：半环使用方不需要感知"打开"这个阶段。
  static async create(
    canvas: HTMLCanvasElement | OffscreenCanvas,
    envelope: string,
    comp: CompositionInfo,
    frames: number,
    opts: RenderSessionOptions = {},
  ): Promise<RenderSession> {
    const session = new RenderSession(canvas, envelope, comp, frames, opts);
    await session.open();
    return session;
  }

  private async open(): Promise<void> {
    clearBlobs();
    await clearVideoCache();
    const renderer = getRendererOrThrow();
    const catalogJson = await renderer.open_design(this.envelope);
    await prepareCatalogVideoSources(catalogJson);
    // Native WebGPU（shaders 语料）初始化走真实异步：requestAdapter/
    // requestDevice 在浏览器任务队列解析，首绘在 initialize().then。
    // realm 帧内 drain 只有微任务，推不动宏任务——open 后先泵事件
    // 循环再建帧 IR，否则（尤其单帧案）帧 0 读到空交换链。30 轮足够
    // 覆盖 adapter→device→管线→首绘提交的十余跳异步；对非 WebGPU
    // 语料只是推进已有异步加载，无语义影响。
    for (let i = 0; i < 30; i++) await new Promise((r) => setTimeout(r, 0));
  }

  /// 渲染一帧进可复用 surface。像素读回留给调用方——那是昂贵的、
  /// 传输特定的步骤（hex 读回 / VideoFrame 截取 / ImageBitmap 快照）。
  async drawFrame(frame: number): Promise<{ comp: CompositionInfo; surface: Surface; CK: CanvasKit; canvas: HTMLCanvasElement | OffscreenCanvas }> {
    const comp = this.comp;
    if (frame < 0 || frame >= this.frames) {
      throw new Error(`frame ${frame} out of range 0..${this.frames - 1}`);
    }
    const gateStart = performance.now();

    this.canvas.width = comp.width;
    this.canvas.height = comp.height;
    // A previous frame at a different size invalidates the cached surface.
    if (this.surfaceSize.w !== comp.width || this.surfaceSize.h !== comp.height) {
      releaseReusableSurface(this.canvas);
      this.surfaceSize = { w: comp.width, h: comp.height };
    }

    const renderer = getRendererOrThrow();
    // oracle 实时通道（服务端墙钟是唯一真时标——页内时钟全被 realm shim
    // 接管）。默认关：/oracle-log 端点只在开发 oracle 静态服务存在，生产
    // dist 里无条件发只会打向宿主页面的同源 404。oracle 页以
    // `?oraclelog` 开启（test-oracle.html 注入 __ocOracleLog）。
    const ocLog = (m: string) => {
      if ((globalThis as unknown as Record<string, unknown>).__ocOracleLog !== true) return;
      try {
        fetch('/oracle-log', { method: 'POST', body: m }).catch(() => {});
      } catch { /* ignore */ }
    };
    ocLog(`rs f${frame} ir-start`);
    const result = renderer.build_frame_ir(frame);
    ocLog(`rs f${frame} ir-done`);
    if ((globalThis as unknown as Record<string, unknown>).__ocOracleLog === true) {
      ocLog(`rs f${frame} ir-shape envelopeBytes=${result.ir.byteLength}`);
    }

    // GPU streams (OCIR section 13): browser-native WebGL2/WebGPU replay of
    // the recorded streams, output registered into generatedImageCanvas
    // (WebGL2: the stream canvas itself) / generatedImageRgba (wgpu: CPU
    // readback) before the CanvasKit replay consumes the Generated
    // placeholders. Unconsumed injections from the previous frame are
    // pruned first (invisible placeholders otherwise leak their slot per
    // frame — and a stale canvas slot would feed this frame's placeholder
    // with last frame's back-buffer content).
    for (const key of this.gpuInjectedKeys) {
      generatedImageRgba.delete(key);
      generatedImageCanvas.delete(key);
    }
    ocLog(`rs f${frame} gl-start`);
    this.gpuInjectedKeys = await executeGpuStreamsForFrame(result.ir);
    ocLog(`rs f${frame} gl-done`);

    // SVG 位图资产先走浏览器 img 预解码（CanvasKit 无 svg codec）——
    // resolveImage 是同步的，预热必须在回放前完成；命中后零开销。
    // wire 契约类型由 schema_gen 单源生成（core ir/media_plan.rs wire_spec）。
    try {
      const plan = JSON.parse(result.mediaPlan) as FrameMediaPlanWire;
      for (const entry of plan.images ?? []) {
        await warmStaticSvgImage(entry.assetId);
      }
    } catch {
      // plan 解析失败交给后续阶段的既有报错路径。
    }

    // Headless SwiftShader often cannot import VideoFrame textures via
    // makeImageFromTextureSource. Inject as CPU RGBA so draw-ir can
    // MakeImage() without a GPU texture path.
    await injectVideoFramesForRender({
      mediaPlan: result.mediaPlan,
      quality: 'exact',
      ...(this.frameOutput ? { frameOutput: this.frameOutput } : {}),
    });

    const CK = (globalThis as CanvasKitGlobal).__canvasKit;
    if (!CK) throw new Error('CanvasKit is not initialized');
    // preserveDrawingBuffer keeps the WebGL back buffer readable after
    // `flush()`: the video transport captures the canvas with
    // `new VideoFrame(canvas)`, which sees a cleared buffer otherwise.
    // （alphaType 不在 WebGLOptions 表里——CanvasKit 忽略未知键，无效果。）
    // 硬件 GL 下 preserve 迫使每帧 resolve/copy（flush ~2.4s/帧，实测）；
    // 采集改走 makeImageSnapshot+readPixels（自身强制 resolve，与 preserve
    // 无关）后，真 GPU 侧可以不 preserve——oracle 页以 __ocSurfaceNoPreserve
    // 开关。
    const surfaceOpts = (globalThis as unknown as Record<string, unknown>).__ocSurfaceNoPreserve
      ? undefined
      : { preserveDrawingBuffer: 1 };
    const surface = getReusableSurface(CK, this.canvas, undefined, surfaceOpts);
    if (!surface) throw new Error('surface creation failed');

    ocLog(`rs f${frame} ck-start`);
    renderEncodedDrawFrame(result.ir, surface.getCanvas(), CK, { surface });
    ocLog(`rs f${frame} ck-draw-done`);
    surface.flush();
    ocLog(`rs f${frame} ck-done`);
    assertFrameGates(frame, performance.now() - gateStart);
    return { comp, surface, CK, canvas: this.canvas };
  }

  /// 释放绑定在 canvas 上的可复用 surface（页面卸载 / 导出收尾）。
  destroy(): void {
    releaseReusableSurface(this.canvas);
    disposeGpuExecutors();
  }
}
