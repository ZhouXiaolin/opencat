/// 打开 design → 逐帧产 canvas 的半环唯一实现。
///
/// 两个消费方（dev 应用侧 `exportMp4` 的 ExportClip、oracle harness 页的
/// 采样/整段通道）共用：open-once 管线、comp 边界检查、可复用 CanvasKit
/// surface、native WebGPU 的 warmup/settle 泵全部在这里，消费方只剩各自的
/// 编码 adapter（WebAV / 裸 WebCodecs+MP4Box）。
///
/// 行为契约与拆并前的 oracle 页逐字一致——warmup/settle 的泵轮数与探针
/// 语义是 shaders 语料黑帧事故的修复本体，动它必须过 gate。

import type { CanvasKit, Surface } from 'canvaskit-wasm';
import { getRendererOrThrow, clearBlobs } from './wasm';
import { clearVideoCache } from './media/video-decoder';
import { injectVideoFramesForRender, prepareCatalogVideoSources } from './media/video-frame-injector';
import { renderEncodedDrawFrame } from './draw-ir';
import { getReusableSurface, releaseReusableSurface } from './media/surface';
import type { CompositionInfo } from './types';

type CanvasKitGlobal = typeof globalThis & { __canvasKit?: CanvasKit };

export interface RenderSessionOptions {
  /// 逐帧注入解码 VideoFrame 的参数分叉：oracle 页 headless SwiftShader
  /// 无法 GPU 纹理导入，需要 `'source'`（CPU RGBA 直出）；dev 应用侧不传。
  frameOutput?: 'source';
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
  async drawFrame(frame: number): Promise<{ comp: CompositionInfo; surface: Surface; CK: CanvasKit }> {
    const comp = this.comp;
    if (frame < 0 || frame >= this.frames) {
      throw new Error(`frame ${frame} out of range 0..${this.frames - 1}`);
    }

    this.canvas.width = comp.width;
    this.canvas.height = comp.height;
    // A previous frame at a different size invalidates the cached surface.
    if (this.surfaceSize.w !== comp.width || this.surfaceSize.h !== comp.height) {
      releaseReusableSurface(this.canvas);
      this.surfaceSize = { w: comp.width, h: comp.height };
    }

    const renderer = getRendererOrThrow();

    // Headless SwiftShader often cannot import VideoFrame textures via
    // makeImageFromTextureSource. Inject as CPU RGBA so draw-ir can
    // MakeImage() without a GPU texture path.
    const result = renderer.build_frame_ir(frame);
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
    const surface = getReusableSurface(CK, this.canvas, undefined, {
      preserveDrawingBuffer: 1,
    });
    if (!surface) throw new Error('surface creation failed');

    renderEncodedDrawFrame(result.ir, surface.getCanvas(), CK, { surface });
    surface.flush();
    return { comp, surface, CK };
  }

  /// Native WebGPU（shaders 语料）warmup：首帧的 build_frame_ir 发起作者
  /// initialize——真实 requestAdapter/requestDevice 的 promise 在浏览器
  /// 任务队列解析，realm 帧内微任务 drain 推不动；首绘（initialize().then
  /// 里的 renderFrame delta 0）因此落在帧 0 之后，单帧案就整段全白。
  /// 修法 = 先踢一脚（跑一次 drawFrame 触发 effect/initialize），泵事件
  /// 循环让异步链走完（getContext 注册 + 首绘都落定），再重绘同一
  /// 帧——重绘时 done 幂等守卫跳过重绘、帧末钩子读到已画好的交换链。
  async warmupNative(frame: number): Promise<void> {
    await this.drawFrame(frame);
    // 泵到所有已注册 native WebGPU canvas 拿到首绘（探针归零）；非
    // WebGPU 设计（无探针）至少泵 150 轮兜底（open 后的既有异步加载）。
    // 最短轮数是关键：drain()===0 在「零 canvas 注册」时也是真——
    // requestAdapter→getContext 的 IPC 是毫秒-百毫秒级浏览器任务，
    // 30 轮 setTimeout(0) 可能在注册发生前跑完，假归零提前 break，
    // 重绘时 inited=false 不画 → 帧末回读空交换链 → 单帧案整段黑帧
    // （blend/checkerboard/simplex-noise 黑帧根因，动画案表现=帧 0
    // 黑、后续帧自愈成 ~3.3%）。
    for (let i = 0; i < 600; i++) {
      await new Promise((r) => setTimeout(r, 0));
      const drain = (globalThis as any).__ocNativeWebGpuDrain;
      if (i >= 150 && (!drain || drain() === 0)) break;
    }
    await this.drawFrame(frame);
  }

  /// Native WebGPU 直读消费：copyTextureToBuffer 的 mapAsync 是任务级
  /// 异步，帧末同步钩子取不到字节。每帧 drawFrame 后泵宏任务等全部
  /// 在途直读落定，再重绘同一帧——done 幂等守卫跳过渲染，帧末钩子
  /// 消费当帧原始字节（发起帧本身不产出记录）。非 WebGPU 语料（探针
  /// 不存在 / 无在途拷贝）直通零开销。
  async settleNativeWebGpuReadback(frame: number): Promise<void> {
    const pend = (globalThis as any).__ocNativeWebGpuReadbackPending;
    if (!pend || pend() === 0) return;
    for (let i = 0; i < 600 && pend() > 0; i++) {
      await new Promise((r) => setTimeout(r, 0));
    }
    await this.drawFrame(frame);
  }

  /// 释放绑定在 canvas 上的可复用 surface（页面卸载 / 导出收尾）。
  destroy(): void {
    releaseReusableSurface(this.canvas);
  }
}
