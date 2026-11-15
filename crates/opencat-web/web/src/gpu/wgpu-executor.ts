// OCIR section 13 WebGPU stream executor — browser-native replay of the
// WebGPU op stream produced by the core recorder.
//
// Authority: `crates/opencat-engine/src/gpu/wgpu_executor.rs` (the native
// engine's executor over the wgpu crate). Semantics preserved here:
//   - handle tables persist across streams (recording only emits resource
//     creation on the first frame);
//   - BUFFER_CREATE gets COPY_DST added and `size.max(4)`; TEXTURE_CREATE and
//     the configure target get RENDER_ATTACHMENT|COPY_SRC|COPY_DST added;
//   - CANVAS_CONFIGURE reuses the out texture when format+size are unchanged;
//   - CANVAS_GET_CURRENT_TEXTURE registers the handle as an alias of the out
//     texture (the recorder never sends TEXTURE_CREATE for it);
//   - VertexState gap fill (WGSL @location referenced but not declared →
//     arrayStride=0 fallback buffer) + draw validation net (declared-but-
//     unbound vertex buffer slots get a dummy buffer before draw);
//   - mesh coverage patch (fragment input missing from vertex output →
//     `opacity_t = smoothstep(...)` line pinned to 1.0);
//   - derived bind group layouts ({ph,pi} → pipeline.getBindGroupLayout).
//
// Browser-native simplifications vs the engine executor:
//   - no recorded-pass replay: GPURenderPassEncoder borrows nothing, so
//     commands replay strictly in stream order (the engine buffers pass
//     segments only because wgpu's RenderPass borrows the encoder);
//   - no Dawn default-layout pre-scan/reject: a browser validation error
//     already discards exactly the one finished command buffer and lets the
//     rest of the submit through — same observable semantics;
//   - enum descriptors go through as the spec-name strings they already are
//     (only 'mutable-storage' maps to the spec's 'storage');
//   - no BGRA byte swap on readback when the target is rgba8unorm.
//
// Readback is asynchronous (copyTextureToBuffer → mapAsync); execute() kicks
// it off without awaiting and settle() joins all pending readbacks — the
// frame driver awaits settle() before replaying the draw stream so injected
// pixels are visible to `resolveImage('generated')`.

import {
  WGPU,
  WGPU_PAYLOAD_LEN,
} from '../generated/ocir-schema.generated';
import { generatedImageRgba } from '../draw-ir/decode';
import type { GpuStream } from '../draw-ir/gpu-stream';

// ---------------------------------------------------------------------------
// Payload readers (little-endian; DataView avoids TypedArray alignment).
// ---------------------------------------------------------------------------

function u32(p: Uint8Array, at: number): number {
  const b = p.buffer, o = p.byteOffset + at;
  return new DataView(b, o, 4).getUint32(0, true);
}
function i32(p: Uint8Array, at: number): number {
  const b = p.buffer, o = p.byteOffset + at;
  return new DataView(b, o, 4).getInt32(0, true);
}
function u64(p: Uint8Array, at: number): number {
  const b = p.buffer, o = p.byteOffset + at;
  return Number(new DataView(b, o, 8).getBigUint64(0, true));
}
function f32(p: Uint8Array, at: number): number {
  const b = p.buffer, o = p.byteOffset + at;
  return new DataView(b, o, 4).getFloat32(0, true);
}

function rangeBytes(pool: Uint8Array, ranges: { start: number; len: number }[], id: number): Uint8Array {
  const r = ranges[id];
  if (!r) throw new Error(`wgpu stream: byte range ${id} out of bounds`);
  return pool.subarray(r.start, r.start + r.len);
}
function jsonAt(pool: Uint8Array, ranges: { start: number; len: number }[], id: number): Record<string, unknown> {
  return JSON.parse(new TextDecoder().decode(rangeBytes(pool, ranges, id))) as Record<string, unknown>;
}
function utf8At(pool: Uint8Array, ranges: { start: number; len: number }[], id: number): string {
  return new TextDecoder().decode(rangeBytes(pool, ranges, id));
}

type Desc = Record<string, unknown>;
function d(v: Desc | undefined, key: string): Desc {
  const x = v?.[key];
  if (!x || typeof x !== 'object') return {};
  return x as Desc;
}
function num(v: Desc | undefined, key: string, fallback: number): number {
  const x = v?.[key];
  return typeof x === 'number' ? x : fallback;
}
function str(v: Desc | undefined, key: string): string | undefined {
  const x = v?.[key];
  return typeof x === 'string' ? x : undefined;
}
/// Handle wrapper {"h": n}.
function hOf(v: unknown): number {
  if (v && typeof v === 'object' && typeof (v as Desc).h === 'number') return (v as Desc).h as number;
  throw new Error(`wgpu stream: expected handle wrapper {"h":..}, got ${JSON.stringify(v)}`);
}
/// GPUExtent3D dual form: [w,h?,d?] sequence or {width,height,depthOrArrayLayers}.
function extentOf(v: unknown): GPUExtent3D {
  if (Array.isArray(v)) {
    return { width: (v[0] as number) ?? 1, height: (v[1] as number) ?? 1, depthOrArrayLayers: (v[2] as number) ?? 1 };
  }
  const x = (v ?? {}) as Desc;
  return {
    width: num(x, 'width', 1),
    height: num(x, 'height', 1),
    depthOrArrayLayers: num(x, 'depthOrArrayLayers', 1),
  };
}
function originOf(v: unknown): GPUOrigin3D {
  if (Array.isArray(v)) return { x: (v[0] as number) ?? 0, y: (v[1] as number) ?? 0, z: (v[2] as number) ?? 0 };
  const x = (v ?? {}) as Desc;
  return { x: num(x, 'x', 0), y: num(x, 'y', 0), z: num(x, 'z', 0) };
}

// ---------------------------------------------------------------------------
// Resource tables
// ---------------------------------------------------------------------------

const enum Kind {
  Buffer, Texture, TextureView, Sampler, BindGroupLayout, BindGroup,
  PipelineLayout, ShaderModule, RenderPipeline, ComputePipeline, QuerySet,
}

class StreamResources {
  readonly kindOf = new Map<number, Kind>();
  readonly buffers = new Map<number, GPUBuffer>();
  readonly textures = new Map<number, GPUTexture>();
  readonly views = new Map<number, GPUTextureView>();
  readonly samplers = new Map<number, GPUSampler>();
  readonly bindGroupLayouts = new Map<number, GPUBindGroupLayout>();
  readonly bindGroups = new Map<number, GPUBindGroup>();
  readonly pipelineLayouts = new Map<number, GPUPipelineLayout>();
  readonly shaders = new Map<number, GPUShaderModule>();
  readonly renderPipelines = new Map<number, GPURenderPipeline>();
  readonly computePipelines = new Map<number, GPUComputePipeline>();
  readonly querySets = new Map<number, GPUQuerySet>();
  /// VertexState slot count per render pipeline (incl. gap-fill fallback
  /// buffers) — drives the draw validation net.
  readonly pipelineVertexSlots = new Map<number, number>();
  /// Shader source text per module — drives the coverage patch and the
  /// vertex gap-fill scan.
  readonly codes = new Map<number, string>();

  buffer(h: number): GPUBuffer {
    const x = this.buffers.get(h);
    if (!x) throw new Error(`wgpu stream: unknown buffer handle ${h}`);
    return x;
  }
  texture(h: number): GPUTexture {
    const x = this.textures.get(h);
    if (!x) throw new Error(`wgpu stream: unknown texture handle ${h}`);
    return x;
  }
  view(h: number): GPUTextureView {
    const x = this.views.get(h);
    if (!x) throw new Error(`wgpu stream: unknown textureView handle ${h}`);
    return x;
  }
  destroy(h: number): void {
    const k = this.kindOf.get(h);
    if (k === undefined) return;
    this.kindOf.delete(h);
    switch (k) {
      case Kind.Buffer: this.buffers.delete(h); break;
      case Kind.Texture: this.textures.delete(h); break;
      case Kind.TextureView: this.views.delete(h); break;
      case Kind.Sampler: this.samplers.delete(h); break;
      case Kind.BindGroupLayout: this.bindGroupLayouts.delete(h); break;
      case Kind.BindGroup: this.bindGroups.delete(h); break;
      case Kind.PipelineLayout: this.pipelineLayouts.delete(h); break;
      case Kind.ShaderModule: this.shaders.delete(h); break;
      case Kind.RenderPipeline: this.renderPipelines.delete(h); break;
      case Kind.ComputePipeline: this.computePipelines.delete(h); break;
      case Kind.QuerySet: this.querySets.delete(h); break;
    }
  }
}

// ---------------------------------------------------------------------------
// WGSL inspection for the G2 compat patches (verbatim semantics from the
// engine executor): fragment inputs missing vertex outputs → coverage line
// pin; vertex params referenced but not declared in VertexState → fallback
// buffer declaration.
// ---------------------------------------------------------------------------

function fragLocations(fs: string): number[] {
  const mainAt = fs.indexOf('fn main(');
  if (mainAt < 0) return [];
  const tail = fs.slice(mainAt);
  const end = tail.indexOf('->') >= 0 ? tail.indexOf('->') : tail.length;
  const head = tail.slice(0, end);
  const locs: number[] = [];
  for (let n = 0; n < 16; n++) if (head.includes(`@location(${n})`)) locs.push(n);
  return locs;
}

function vertOutLocations(vs: string): number[] {
  const outAt = vs.indexOf('struct VertexOutput');
  if (outAt < 0) return [];
  const tail = vs.slice(outAt);
  const closeRel = tail.indexOf('}');
  const head = closeRel >= 0 ? tail.slice(0, closeRel + 1) : tail;
  const locs: number[] = [];
  for (let n = 0; n < 16; n++) if (head.includes(`@location(${n})`)) locs.push(n);
  return locs;
}

function fragmentInputsMissingVertexOutputs(fs: string, vs: string): boolean {
  const fsLocs = fragLocations(fs);
  const vsLocs = vertOutLocations(vs);
  return fsLocs.length > 0 && vsLocs.length > 0 && fsLocs.some((l) => !vsLocs.includes(l));
}

function patchMeshCoverage(fs: string): string {
  return fs
    .split('\n')
    .map((line) => {
      const t = line.trimStart();
      return t.startsWith('opacity_t = smoothstep(') ? '    opacity_t = 1f;' : line;
    })
    .join('\n');
}

/// Parameter region text of `@vertex fn NAME(...)` entries — the only legal
/// scan domain for @location references (whole-file scans would pick up
/// fragment outputs / final_Input inputs).
function vertexParamRegion(code: string, entry: string | undefined): string {
  const regions: string[] = [];
  let rest = code;
  for (;;) {
    const pos = rest.indexOf('@vertex');
    if (pos < 0) break;
    const tail = rest.slice(pos);
    const fnPos = tail.indexOf('fn ');
    if (fnPos < 0) break;
    const afterFn = tail.slice(fnPos + 3);
    const nameMatch = /^[\w]+/.exec(afterFn.trimStart());
    const name = nameMatch ? nameMatch[0] : '';
    const paren = afterFn.indexOf('(');
    if (paren < 0) break;
    let depth = 0;
    let close = -1;
    for (let i = paren; i < afterFn.length; i++) {
      const c = afterFn[i];
      if (c === '(') depth++;
      else if (c === ')') {
        depth--;
        if (depth === 0) {
          close = i;
          break;
        }
      }
    }
    if (close < 0) break;
    if (entry === undefined || entry === name) {
      regions.push(afterFn.slice(paren + 1, close));
      if (entry !== undefined) break;
    }
    rest = tail.slice(fnPos + close);
  }
  return regions.join('\n');
}

function vertexLocationsReferenced(code: string, entry: string | undefined): number[] {
  const params = vertexParamRegion(code, entry);
  const locs: number[] = [];
  let rest = params;
  for (;;) {
    const pos = rest.indexOf('@location(');
    if (pos < 0) break;
    const tail = rest.slice(pos + '@location('.length);
    const digits = /^\d+/.exec(tail);
    if (digits) {
      const n = Number(digits[0]);
      if (!locs.includes(n)) locs.push(n);
    }
    rest = tail;
  }
  return locs;
}

// ---------------------------------------------------------------------------
// Executor
// ---------------------------------------------------------------------------

export class WgpuStreamExecutor {
  private device: GPUDevice | null = null;
  private queue: GPUQueue | null = null;
  private res = new StreamResources();
  private pending: Promise<void>[] = [];
  private dummyVertexBuffer: GPUBuffer | null = null;

  constructor() {}

  /// Request the adapter/device up front (render-session warmup). Returns
  /// false when WebGPU is unavailable — the driver then must not enqueue
  /// webgpu streams.
  async ensureDevice(): Promise<boolean> {
    if (this.device) return true;
    if (navigator.gpu === undefined) return false;
    const adapter = await navigator.gpu.requestAdapter();
    if (!adapter) return false;
    this.device = await adapter.requestDevice();
    this.queue = this.device.queue;
    return true;
  }

  /// Replay one stream. All GPU work is enqueued synchronously; the readback
  /// promise lands in `pending` and its pixels are written into
  /// `generatedImageRgba` once it resolves (joined by `settle()`).
  execute(stream: GpuStream): void {
    if (!this.device || !this.queue) {
      throw new Error('wgpu stream executor: device not initialized (call ensureDevice)');
    }
    const device = this.device;
    const queue = this.queue;
    const res = this.res;
    const ops = stream.ops.ops;
    const pool = stream.ops.bytes;
    const ranges = stream.ops.byteRanges;

    // Per-stream frame state (resources persist across streams).
    let outTexture: GPUTexture | null = null;
    let outFormat = '';
    let outHandle: number | null = null;
    const encoders = new Map<number, GPUCommandEncoder>();
    /// buf_handle → encoder_handle (engine's frame.cmd_buffers mapping).
    const cmdBuffers = new Map<number, number>();
    const passes = new Map<number, GPURenderPassEncoder>();
    const computePasses = new Map<number, GPUComputePassEncoder>();
    let curPipeline: number | null = null;
    const boundVbs: number[] = [];

    const dummy = () => {
      if (!this.dummyVertexBuffer) {
        this.dummyVertexBuffer = device.createBuffer({
          size: 16,
          usage: GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST,
        });
      }
      return this.dummyVertexBuffer;
    };
    /// Draw validation net: declared-but-unbound vertex slots get the dummy
    /// buffer (stride=0 slots read zero bytes — the empty slice satisfies).
    const bindMissingVertexBuffers = (pass: GPURenderPassEncoder) => {
      if (curPipeline === null) return;
      const slots = res.pipelineVertexSlots.get(curPipeline);
      if (slots === undefined) return;
      for (let s = 0; s < slots; s++) {
        if (!boundVbs.includes(s)) {
          pass.setVertexBuffer(s, dummy());
          boundVbs.push(s);
        }
      }
    };

    let i = 0;
    while (i < ops.length) {
      const opcode = ops[i];
      i += 1;
      let payload: Uint8Array;
      if (opcode === WGPU.QUEUE_SUBMIT) {
        const count = u32(ops, i);
        payload = ops.subarray(i, i + 4 + 4 * count);
        i += payload.length;
      } else {
        const plen = WGPU_PAYLOAD_LEN[opcode];
        if (plen === undefined || plen < 0) throw new Error(`wgpu stream: unknown opcode ${opcode}`);
        payload = ops.subarray(i, i + plen);
        i += plen;
      }

      switch (opcode) {
        // ── resource creation ────────────────────────────────────────────
        case WGPU.BUFFER_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const size = typeof v.size === 'number' ? v.size : (() => { throw new Error('BUFFER_CREATE: missing size'); })();
          // COPY_DST added: upload/readback stand-ins go through writeBuffer /
          // copyTextureToBuffer, which the declared usage may not cover.
          const usage = (num(v, 'usage', 0) | GPUBufferUsage.COPY_DST) as GPUBufferUsageFlags;
          const buf = device.createBuffer({ size: Math.max(size, 4), usage });
          res.buffers.set(handle, buf);
          res.kindOf.set(handle, Kind.Buffer);
          break;
        }
        case WGPU.TEXTURE_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          if (v.size === undefined || v.size === null) throw new Error('TEXTURE_CREATE: missing size');
          const size = extentOf(v.size);
          // Same usage freedom as the configure target: extra RENDER_
          // ATTACHMENT|COPY_SRC|COPY_DST bits are legal and keep readback
          // and pass attachment semantics intact.
          const usage = (num(v, 'usage', 0) |
            GPUTextureUsage.RENDER_ATTACHMENT |
            GPUTextureUsage.COPY_SRC |
            GPUTextureUsage.COPY_DST) as GPUTextureUsageFlags;
          const tex = device.createTexture({
            size,
            mipLevelCount: num(v, 'mipLevelCount', 1),
            sampleCount: num(v, 'sampleCount', 1),
            dimension: (str(v, 'dimension') ?? '2d') as GPUTextureDimension,
            format: (str(v, 'format') ?? 'rgba8unorm') as GPUTextureFormat,
            usage,
          });
          res.textures.set(handle, tex);
          res.kindOf.set(handle, Kind.Texture);
          break;
        }
        case WGPU.TEXTURE_VIEW_CREATE: {
          const handle = u32(payload, 0);
          const srcHandle = u32(payload, 4);
          const v = jsonAt(pool, ranges, u32(payload, 8));
          const src = srcHandle === outHandle && outTexture ? outTexture : res.texture(srcHandle);
          const view = src.createView({
            format: str(v, 'format') as GPUTextureFormat | undefined,
            dimension: str(v, 'dimension') as GPUTextureViewDimension | undefined,
            aspect: (str(v, 'aspect') ?? 'all') as GPUTextureAspect,
            baseMipLevel: num(v, 'baseMipLevel', 0),
            mipLevelCount: optNum(v, 'mipLevelCount'),
            baseArrayLayer: num(v, 'baseArrayLayer', 0),
            arrayLayerCount: optNum(v, 'arrayLayerCount'),
          });
          res.views.set(handle, view);
          res.kindOf.set(handle, Kind.TextureView);
          break;
        }
        case WGPU.SAMPLER_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const sampler = device.createSampler({
            addressModeU: (str(v, 'addressModeU') ?? 'clamp-to-edge') as GPUAddressMode,
            addressModeV: (str(v, 'addressModeV') ?? 'clamp-to-edge') as GPUAddressMode,
            addressModeW: (str(v, 'addressModeW') ?? 'clamp-to-edge') as GPUAddressMode,
            magFilter: (str(v, 'magFilter') ?? 'nearest') as GPUFilterMode,
            minFilter: (str(v, 'minFilter') ?? 'nearest') as GPUFilterMode,
            mipmapFilter: (str(v, 'mipmapFilter') ?? 'nearest') as GPUMipmapFilterMode,
            lodMinClamp: num(v, 'lodMinClamp', 0),
            lodMaxClamp: num(v, 'lodMaxClamp', 32),
            compare: str(v, 'compare') as GPUCompareFunction | undefined,
            maxAnisotropy: num(v, 'maxAnisotropy', 1),
          });
          res.samplers.set(handle, sampler);
          res.kindOf.set(handle, Kind.Sampler);
          break;
        }
        case WGPU.BIND_GROUP_LAYOUT_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const entries = (v.entries as Desc[] | undefined)?.map((e) => {
            const out: GPUBindGroupLayoutEntry = {
              binding: num(e, 'binding', 0),
              visibility: num(e, 'visibility', 0) as GPUShaderStageFlags,
            };
            const b = d(e, 'buffer');
            if (b && typeof e.buffer === 'object') {
              // 'mutable-storage' is the g-device-api dialect name; the spec
              // spell is 'storage'. Everything else reads as 'uniform'.
              const ty = str(b, 'type') ?? 'uniform';
              out.buffer = {
                type: (ty === 'storage' || ty === 'mutable-storage' ? 'storage' : ty === 'read-only-storage' ? 'read-only-storage' : 'uniform') as GPUBufferBindingType,
                hasDynamicOffset: !!b.hasDynamicOffset,
                minBindingSize: optNum(b, 'minBindingSize'),
              };
            } else if (typeof e.sampler === 'object' && e.sampler) {
              out.sampler = { type: (str(d(e, 'sampler'), 'type') ?? 'filtering') as GPUSamplerBindingType };
            } else if (typeof e.texture === 'object' && e.texture) {
              const t = d(e, 'texture');
              out.texture = {
                sampleType: (str(t, 'sampleType') ?? 'float') as GPUTextureSampleType,
                viewDimension: (str(t, 'viewDimension') ?? '2d') as GPUTextureViewDimension,
                multisampled: !!t.multisampled,
              };
            } else if (typeof e.storageTexture === 'object' && e.storageTexture) {
              const s = d(e, 'storageTexture');
              out.storageTexture = {
                access: (str(s, 'access') ?? 'write-only') as GPUStorageTextureAccess,
                format: (str(s, 'format') ?? 'rgba8unorm') as GPUTextureFormat,
                viewDimension: (str(s, 'viewDimension') ?? '2d') as GPUTextureViewDimension,
              };
            }
            return out;
          }) ?? [];
          const layout = device.createBindGroupLayout({ entries });
          res.bindGroupLayouts.set(handle, layout);
          res.kindOf.set(handle, Kind.BindGroupLayout);
          break;
        }
        case WGPU.BIND_GROUP_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const bg = this.createBindGroup(device, v);
          res.bindGroups.set(handle, bg);
          res.kindOf.set(handle, Kind.BindGroup);
          break;
        }
        case WGPU.PIPELINE_LAYOUT_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const layouts = (v.bindGroupLayouts as unknown[] | undefined)?.map((l) =>
            l ? res.bindGroupLayouts.get(hOf(l)) ?? (() => { throw new Error(`unknown bindGroupLayout ${hOf(l)}`); })() : null,
          ) ?? [];
          const layout = device.createPipelineLayout({
            bindGroupLayouts: layouts as GPUBindGroupLayout[],
          });
          res.pipelineLayouts.set(handle, layout);
          res.kindOf.set(handle, Kind.PipelineLayout);
          break;
        }
        case WGPU.SHADER_MODULE_CREATE: {
          const handle = u32(payload, 0);
          const code = utf8At(pool, ranges, u32(payload, 4));
          const module = device.createShaderModule({ code });
          res.shaders.set(handle, module);
          res.codes.set(handle, code);
          res.kindOf.set(handle, Kind.ShaderModule);
          break;
        }
        case WGPU.RENDER_PIPELINE_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const pipe = this.createRenderPipeline(device, handle, v);
          res.renderPipelines.set(handle, pipe);
          res.kindOf.set(handle, Kind.RenderPipeline);
          break;
        }
        case WGPU.COMPUTE_PIPELINE_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const compute = d(v, 'compute');
          const module = res.shaders.get(hOf(compute.module));
          if (!module) throw new Error(`unknown shaderModule ${hOf(compute.module)}`);
          const pipe = device.createComputePipeline({
            layout: this.pipelineLayoutOf(v),
            compute: { module, entryPoint: str(compute, 'entryPoint') },
          });
          res.computePipelines.set(handle, pipe);
          res.kindOf.set(handle, Kind.ComputePipeline);
          break;
        }
        case WGPU.QUERY_SET_CREATE: {
          const handle = u32(payload, 0);
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const qs = device.createQuerySet({
            type: (str(v, 'type') ?? 'occlusion') as GPUQueryType,
            count: num(v, 'count', 0),
          });
          res.querySets.set(handle, qs);
          res.kindOf.set(handle, Kind.QuerySet);
          break;
        }
        case WGPU.RESOURCE_DESTROY: {
          res.destroy(u32(payload, 0));
          break;
        }

        // ── canvas target ────────────────────────────────────────────────
        case WGPU.CANVAS_CONFIGURE: {
          const fmtName = utf8At(pool, ranges, u32(payload, 0));
          const usage = u32(payload, 5);
          outFormat = fmtName;
          const fullUsage = (usage |
            GPUTextureUsage.RENDER_ATTACHMENT |
            GPUTextureUsage.COPY_SRC |
            GPUTextureUsage.COPY_DST) as GPUTextureUsageFlags;
          // Reuse when format+size unchanged (configure re-fires every canvas
          // tick; rebuilding would drop the previous tick's resolved content).
          const same = outTexture !== null &&
            outTexture.width === stream.width &&
            outTexture.height === stream.height &&
            outTexture.format === fmtName;
          if (!same) {
            // Old texture left alive (the engine keeps its Arc too) — stale
            // views may still be referenced by in-flight commands.
            outTexture = device.createTexture({
              size: { width: stream.width, height: stream.height, depthOrArrayLayers: 1 },
              format: fmtName as GPUTextureFormat,
              usage: fullUsage,
            });
          }
          break;
        }
        case WGPU.CANVAS_GET_CURRENT_TEXTURE: {
          const h = u32(payload, 0);
          outHandle = h;
          if (!outTexture) throw new Error('getCurrentTexture before configure');
          res.textures.set(h, outTexture);
          res.kindOf.set(h, Kind.Texture);
          break;
        }

        // ── queue ops ────────────────────────────────────────────────────
        case WGPU.QUEUE_WRITE_BUFFER: {
          const buffer = res.buffer(u32(payload, 0));
          const offset = u64(payload, 4);
          const data = rangeBytes(pool, ranges, u32(payload, 12));
          queue.writeBuffer(buffer, offset, data);
          break;
        }
        case WGPU.QUEUE_WRITE_TEXTURE: {
          const dest = jsonAt(pool, ranges, u32(payload, 0));
          const size = jsonAt(pool, ranges, u32(payload, 8));
          const layout = jsonAt(pool, ranges, u32(payload, 12));
          const data = rangeBytes(pool, ranges, u32(payload, 4));
          queue.writeTexture(
            { texture: res.texture(hOf(dest.texture)), mipLevel: num(dest, 'mipLevel', 0), origin: originOf(dest.origin) },
            data,
            {
              offset: num(layout, 'offset', 0),
              bytesPerRow: optNum(layout, 'bytesPerRow'),
              rowsPerImage: optNum(layout, 'rowsPerImage'),
            },
            extentOf(size),
          );
          break;
        }
        case WGPU.QUEUE_SUBMIT: {
          const count = u32(payload, 0);
          const cbs: GPUCommandBuffer[] = [];
          for (let k = 0; k < count; k++) {
            const bufH = u32(payload, 4 + 4 * k);
            const encH = cmdBuffers.get(bufH);
            if (encH === undefined) continue;
            const enc = encoders.get(encH);
            if (!enc) continue;
            // Browser-native validation semantics: a failed finish/submit
            // discards exactly this command buffer; the rest go through —
            // no pre-scan needed.
            cbs.push(enc.finish());
            encoders.delete(encH);
            cmdBuffers.delete(bufH);
          }
          queue.submit(cbs);
          break;
        }

        // ── encoder commands (strictly in-order; no segment replay) ──────
        case WGPU.ENC_BEGIN_RENDER_PASS: {
          const encH = u32(payload, 0);
          const passH = u32(payload, 4);
          const v = jsonAt(pool, ranges, u32(payload, 8));
          const enc = this.encoderOf(encoders, encH);
          const pass = enc.beginRenderPass(this.renderPassDesc(v));
          passes.set(passH, pass);
          curPipeline = null;
          boundVbs.length = 0;
          break;
        }
        case WGPU.ENC_BEGIN_COMPUTE_PASS: {
          const encH = u32(payload, 0);
          const passH = u32(payload, 4);
          const enc = this.encoderOf(encoders, encH);
          const pass = enc.beginComputePass();
          computePasses.set(passH, pass);
          break;
        }
        case WGPU.ENC_COPY_BUFFER_TO_BUFFER: {
          const enc = this.encoderOf(encoders, u32(payload, 0));
          enc.copyBufferToBuffer(
            res.buffer(u32(payload, 4)), u64(payload, 8),
            res.buffer(u32(payload, 16)), u64(payload, 20),
            u64(payload, 28),
          );
          break;
        }
        case WGPU.ENC_COPY_TEXTURE_TO_TEXTURE:
        case WGPU.ENC_COPY_TEXTURE_TO_BUFFER:
        case WGPU.ENC_COPY_BUFFER_TO_TEXTURE: {
          const enc = this.encoderOf(encoders, u32(payload, 0));
          const v = jsonAt(pool, ranges, u32(payload, 4));
          const src = d(v, 'source');
          const dst = d(v, 'destination');
          const size = extentOf(v.size);
          const texSide = (side: Desc) => ({
            texture: res.texture(hOf(side.texture)),
            mipLevel: num(side, 'mipLevel', 0),
            origin: originOf(side.origin),
          });
          const bufSide = (side: Desc) => ({
            buffer: res.buffer(hOf(side.buffer)),
            offset: num(side, 'offset', 0),
            bytesPerRow: optNum(side, 'bytesPerRow'),
            rowsPerImage: optNum(side, 'rowsPerImage'),
          });
          if (opcode === WGPU.ENC_COPY_TEXTURE_TO_TEXTURE) {
            enc.copyTextureToTexture(texSide(src), texSide(dst), size);
          } else if (opcode === WGPU.ENC_COPY_TEXTURE_TO_BUFFER) {
            enc.copyTextureToBuffer(texSide(src), bufSide(dst), size);
          } else {
            enc.copyBufferToTexture(bufSide(src), texSide(dst), size);
          }
          break;
        }
        case WGPU.ENC_FINISH: {
          cmdBuffers.set(u32(payload, 4), u32(payload, 0));
          break;
        }

        // ── pass commands ────────────────────────────────────────────────
        case WGPU.PASS_SET_PIPELINE: {
          const ph = u32(payload, 4);
          const passH = u32(payload, 0);
          // Shared opcode: dispatch by which pass flavor holds the handle
          // (a pass id is exactly one flavor — mirrors the engine's
          // per-segment dispatch).
          const cp = computePasses.get(passH);
          if (cp) {
            const pipe = res.computePipelines.get(ph);
            if (!pipe) throw new Error(`unknown computePipeline ${ph}`);
            cp.setPipeline(pipe);
            break;
          }
          const rp = passes.get(passH);
          if (!rp) throw new Error('set_pipeline outside pass');
          const pipe = res.renderPipelines.get(ph);
          if (!pipe) throw new Error(`unknown renderPipeline ${ph}`);
          rp.setPipeline(pipe);
          curPipeline = ph;
          break;
        }
        case WGPU.PASS_SET_BIND_GROUP: {
          const idx = u32(payload, 4);
          const bg = res.bindGroups.get(u32(payload, 8));
          if (!bg) throw new Error(`unknown bindGroup ${u32(payload, 8)}`);
          const rp = passes.get(u32(payload, 0));
          if (rp) rp.setBindGroup(idx, bg);
          else computePasses.get(u32(payload, 0))?.setBindGroup(idx, bg);
          break;
        }
        case WGPU.PASS_SET_VERTEX_BUFFER: {
          const slot = u32(payload, 4);
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('set_vertex_buffer outside render pass');
          rp.setVertexBuffer(slot, res.buffer(u32(payload, 8)), u64(payload, 12));
          if (!boundVbs.includes(slot)) boundVbs.push(slot);
          break;
        }
        case WGPU.PASS_SET_INDEX_BUFFER: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('set_index_buffer outside render pass');
          rp.setIndexBuffer(
            res.buffer(u32(payload, 4)),
            payload[8] === 1 ? 'uint32' : 'uint16',
            u64(payload, 9),
          );
          break;
        }
        case WGPU.PASS_SET_VIEWPORT: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('set_viewport outside render pass');
          rp.setViewport(f32(payload, 4), f32(payload, 8), f32(payload, 12), f32(payload, 16), f32(payload, 20), f32(payload, 24));
          break;
        }
        case WGPU.PASS_SET_SCISSOR_RECT: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('set_scissor_rect outside render pass');
          rp.setScissorRect(u32(payload, 4), u32(payload, 8), u32(payload, 12), u32(payload, 16));
          break;
        }
        case WGPU.PASS_SET_BLEND_CONSTANT: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('set_blend_constant outside render pass');
          rp.setBlendConstant({ r: f32(payload, 4), g: f32(payload, 8), b: f32(payload, 12), a: f32(payload, 16) });
          break;
        }
        case WGPU.PASS_SET_STENCIL_REFERENCE: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('set_stencil_reference outside render pass');
          rp.setStencilReference(u32(payload, 4));
          break;
        }
        case WGPU.PASS_DRAW: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('draw outside render pass');
          bindMissingVertexBuffers(rp);
          rp.draw(u32(payload, 4), u32(payload, 8), u32(payload, 12), u32(payload, 16));
          break;
        }
        case WGPU.PASS_DRAW_INDEXED: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('draw_indexed outside render pass');
          bindMissingVertexBuffers(rp);
          rp.drawIndexed(u32(payload, 4), u32(payload, 8), u32(payload, 12), i32(payload, 16), u32(payload, 20));
          break;
        }
        case WGPU.PASS_DRAW_INDIRECT: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('draw_indirect outside render pass');
          rp.drawIndirect(res.buffer(u32(payload, 4)), u64(payload, 8));
          break;
        }
        case WGPU.PASS_DRAW_INDEXED_INDIRECT: {
          const rp = passes.get(u32(payload, 0));
          if (!rp) throw new Error('draw_indexed_indirect outside render pass');
          rp.drawIndexedIndirect(res.buffer(u32(payload, 4)), u64(payload, 8));
          break;
        }
        case WGPU.PASS_DISPATCH_WORKGROUPS: {
          const cp = computePasses.get(u32(payload, 0));
          if (!cp) throw new Error('dispatch_workgroups outside compute pass');
          cp.dispatchWorkgroups(u32(payload, 4), u32(payload, 8), u32(payload, 12));
          break;
        }
        case WGPU.PASS_DISPATCH_WORKGROUPS_INDIRECT: {
          const cp = computePasses.get(u32(payload, 0));
          if (!cp) throw new Error('dispatch_workgroups_indirect outside compute pass');
          cp.dispatchWorkgroupsIndirect(res.buffer(u32(payload, 4)), u64(payload, 8));
          break;
        }
        case WGPU.PASS_END: {
          const passH = u32(payload, 0);
          passes.get(passH)?.end();
          passes.delete(passH);
          computePasses.get(passH)?.end();
          computePasses.delete(passH);
          break;
        }
        default:
          throw new Error(`wgpu stream: opcode ${opcode} not implemented`);
      }
    }

    // Tail flush: encoders the stream never submitted still go through
    // (abnormal frames must not lose earlier work).
    for (const [encH, enc] of encoders) {
      queue.submit([enc.finish()]);
      encoders.delete(encH);
    }

    // Readback → generatedImageRgba (async; joined by settle()). Streams
    // whose placeholder is not in the visible plan still execute fully —
    // resource creation only fires on the first frame — but their output is
    // dropped by the integration layer (same as the engine executor).
    if (!outTexture) {
      throw new Error('wgpu stream: no output texture (missing configure/getCurrentTexture)');
    }
    const tex = outTexture;
    const fmt = outFormat;
    const w = stream.width;
    const h = stream.height;
    const imageId = stream.imageId;
    this.pending.push(
      (async () => {
        const rgba = await readbackRgba(device, queue, tex, w, h, fmt);
        generatedImageRgba.set(imageId, { width: w, height: h, rgba });
      })(),
    );
  }

  /// Await all pending readbacks from prior execute() calls. The frame
  /// driver calls this before draw-stream replay so injected pixels are
  /// visible to `resolveImage('generated')`.
  async settle(): Promise<void> {
    const all = this.pending;
    this.pending = [];
    await Promise.all(all);
  }

  dispose(): void {
    this.pending = [];
    this.dummyVertexBuffer?.destroy();
    this.dummyVertexBuffer = null;
    this.device?.destroy();
    this.device = null;
    this.queue = null;
    this.res = new StreamResources();
  }

  // ── descriptor builders ───────────────────────────────────────────────

  private encoderOf(encoders: Map<number, GPUCommandEncoder>, h: number): GPUCommandEncoder {
    let enc = encoders.get(h);
    if (!enc) {
      enc = this.device!.createCommandEncoder();
      encoders.set(h, enc);
    }
    return enc;
  }

  private pipelineLayoutOf(v: Desc): GPUPipelineLayout | 'auto' {
    const l = v.layout;
    if (l === undefined || l === null || l === 'auto') return 'auto';
    const h = hOf(l);
    const layout = this.res.pipelineLayouts.get(h);
    if (!layout) throw new Error(`unknown pipelineLayout ${h}`);
    return layout;
  }

  private bindGroupEntry(e: Desc): GPUBindGroupEntry | null {
    const resource = e.resource;
    if (!resource || typeof resource !== 'object') return null; // empty slot
    const rv = resource as Desc;
    if (rv.buffer && typeof rv.buffer === 'object') {
      const b = rv.buffer as Desc;
      return {
        binding: num(e, 'binding', 0),
        resource: { buffer: this.res.buffer(hOf(b)), offset: num(rv, 'offset', 0), size: optNum(rv, 'size') },
      };
    }
    if (rv.sampler !== undefined) {
      const h = hOf(rv.sampler);
      const s = this.res.samplers.get(h);
      if (!s) throw new Error(`unknown sampler ${h}`);
      return { binding: num(e, 'binding', 0), resource: s };
    }
    if (rv.textureView !== undefined) {
      return { binding: num(e, 'binding', 0), resource: this.res.view(hOf(rv.textureView)) };
    }
    // Bare handle: kind decides sampler vs view.
    const h = hOf(rv);
    if (this.res.kindOf.get(h) === Kind.Sampler) {
      const s = this.res.samplers.get(h);
      if (!s) throw new Error(`unknown sampler ${h}`);
      return { binding: num(e, 'binding', 0), resource: s };
    }
    return { binding: num(e, 'binding', 0), resource: this.res.view(h) };
  }

  private createBindGroup(device: GPUDevice, v: Desc): GPUBindGroup {
    const layoutV = v.layout;
    if (!layoutV) throw new Error('bindGroup: missing layout');
    let layout: GPUBindGroupLayout;
    const lv = layoutV as Desc;
    if (lv.ph !== undefined) {
      // Derived layout: {"ph": pipelineHandle, "pi": groupIndex}.
      const ph = Number(lv.ph);
      const pi = Number(lv.pi ?? 0);
      const rp = this.res.renderPipelines.get(ph);
      const cp = rp ? undefined : this.res.computePipelines.get(ph);
      const src = rp ?? cp;
      if (!src) throw new Error(`derived layout: handle ${ph} is not a pipeline`);
      layout = (rp ? (src as GPURenderPipeline) : (src as GPUComputePipeline)).getBindGroupLayout(pi);
    } else {
      const h = hOf(layoutV);
      const l = this.res.bindGroupLayouts.get(h);
      if (!l) throw new Error(`unknown bindGroupLayout ${h}`);
      layout = l;
    }
    const entries: GPUBindGroupEntry[] = [];
    for (const e of (v.entries as Desc[] | undefined) ?? []) {
      const entry = this.bindGroupEntry(e);
      if (entry) entries.push(entry);
    }
    return device.createBindGroup({ layout, entries });
  }

  private createRenderPipeline(device: GPUDevice, handle: number, v: Desc): GPURenderPipeline {
    const vertexV = d(v, 'vertex');
    const vsHandle = hOf(vertexV.module);
    const vsModule = this.res.shaders.get(vsHandle);
    if (!vsModule) throw new Error(`vertex: unknown shaderModule ${vsHandle}`);
    const vsEntry = str(vertexV, 'entryPoint');
    const vsCode = this.res.codes.get(vsHandle);

    const buffers: GPUVertexBufferLayout[] = [];
    for (const b of (vertexV.buffers as (Desc | null)[] | undefined) ?? []) {
      if (!b) continue;
      buffers.push({
        arrayStride: num(b, 'arrayStride', 0),
        stepMode: (str(b, 'stepMode') ?? 'vertex') as GPUVertexStepMode,
        attributes: ((b.attributes as Desc[] | undefined) ?? []).map((a) => ({
          shaderLocation: num(a, 'shaderLocation', 0),
          format: (str(a, 'format') ?? 'float32x4') as GPUVertexFormat,
          offset: num(a, 'offset', 0),
        })),
      });
    }
    // G2 gap fill: WGSL vertex params referenced but not declared →
    // arrayStride=0 fallback buffer (float32x4 accepts the vecN<f32> family).
    // Scan domain is the vertex entry's parameter list only.
    const declared = new Set<number>();
    for (const b of buffers) for (const a of b.attributes ?? []) declared.add(a.shaderLocation);
    const missing: number[] = [];
    if (vsCode) {
      for (const loc of vertexLocationsReferenced(vsCode, vsEntry)) {
        if (!declared.has(loc) && !missing.includes(loc)) missing.push(loc);
      }
    }
    if (missing.length > 0) {
      buffers.push({
        arrayStride: 0,
        attributes: missing.map((n) => ({ shaderLocation: n, format: 'float32x4' as GPUVertexFormat, offset: 0 })),
      });
    }

    const fragmentV = v.fragment && typeof v.fragment === 'object' ? d(v, 'fragment') : null;
    let fragment: GPUFragmentState | null = null;
    if (fragmentV) {
      const fsHandle = hOf(fragmentV.module);
      const fsModuleOrig = this.res.shaders.get(fsHandle);
      if (!fsModuleOrig) throw new Error(`fragment: unknown shaderModule ${fsHandle}`);
      let fsModule = fsModuleOrig;
      // Mesh coverage patch: fragment input missing from vertex output →
      // pin the coverage line to a definite value (matches the reference's
      // observed float path; the register residue otherwise diverges by
      // backend). Patched module replaces the original for this pipeline.
      if (this.res.codes.has(fsHandle) && this.res.codes.has(vsHandle)) {
        const fsCode = this.res.codes.get(fsHandle)!;
        const vsCode = this.res.codes.get(vsHandle)!;
        if (fragmentInputsMissingVertexOutputs(fsCode, vsCode)) {
          fsModule = device.createShaderModule({ code: patchMeshCoverage(fsCode) });
        }
      }
      const targets: (GPUColorTargetState | null)[] = ((fragmentV.targets as (Desc | null)[] | undefined) ?? [])
        .map((t) => {
          if (!t) return null;
          const out: GPUColorTargetState = { format: (str(t, 'format') ?? 'rgba8unorm') as GPUTextureFormat };
          const blendV = t.blend as Desc | undefined;
          if (blendV) {
            const comp = (c: Desc): GPUBlendComponent => ({
              srcFactor: (str(c, 'srcFactor') ?? 'one') as GPUBlendFactor,
              dstFactor: (str(c, 'dstFactor') ?? 'zero') as GPUBlendFactor,
              operation: (str(c, 'operation') ?? 'add') as GPUBlendOperation,
            });
            out.blend = { color: comp(d(blendV, 'color')), alpha: comp(d(blendV, 'alpha')) };
          }
          out.writeMask = num(t, 'writeMask', 0xF) as GPUColorWriteFlags;
          return out;
        });
      fragment = {
        module: fsModule,
        entryPoint: str(fragmentV, 'entryPoint'),
        constants: (fragmentV.constants as Record<string, number> | undefined) ?? undefined,
        targets,
      };
    }

    const primitiveV = d(v, 'primitive');    const primitive: GPUPrimitiveState = {
      topology: (str(primitiveV, 'topology') ?? 'triangle-list') as GPUPrimitiveTopology,
      frontFace: (str(primitiveV, 'frontFace') ?? 'ccw') as GPUFrontFace,
      cullMode: (str(primitiveV, 'cullMode') ?? 'none') as GPUCullMode,
      stripIndexFormat: str(primitiveV, 'stripIndexFormat') as GPUIndexFormat | undefined,
      unclippedDepth: !!primitiveV.unclippedDepth,
    };

    let depthStencil: GPUDepthStencilState | undefined;
    const dsV = v.depthStencil && typeof v.depthStencil === 'object' ? d(v, 'depthStencil') : null;
    if (dsV) {
      const face = (fv: Desc): GPUStencilFaceState => ({
        compare: (str(fv, 'compare') ?? 'always') as GPUCompareFunction,
        failOp: (str(fv, 'failOp') ?? 'keep') as GPUStencilOperation,
        depthFailOp: (str(fv, 'depthFailOp') ?? 'keep') as GPUStencilOperation,
        passOp: (str(fv, 'passOp') ?? 'keep') as GPUStencilOperation,
      });
      const stencilV = d(dsV, 'stencil');
      // Spec-flattened faces (stencilFront/stencilBack) — the recorder's
      // nested `stencil:{front,back,readMask,writeMask}` is wgpu dialect.
      // depthBias* has no WebGPU equivalent and is dropped (the browser
      // reference never applied it either).
      depthStencil = {
        format: (str(dsV, 'format') ?? 'depth24plus') as GPUTextureFormat,
        depthWriteEnabled: !!dsV.depthWriteEnabled,
        depthCompare: (str(dsV, 'depthCompare') ?? 'always') as GPUCompareFunction,
        stencilFront: face(d(stencilV, 'front')),
        stencilBack: face(d(stencilV, 'back')),
        stencilReadMask: num(stencilV, 'readMask', 0xFFFFFFFF),
        stencilWriteMask: num(stencilV, 'writeMask', 0xFFFFFFFF),
      };
    }

    const msV = d(v, 'multisample');

    const pipe = device.createRenderPipeline({
      layout: this.pipelineLayoutOf(v),
      vertex: {
        module: vsModule,
        entryPoint: vsEntry,
        constants: (vertexV.constants as Record<string, number> | undefined) ?? undefined,
        buffers,
      },
      primitive: primitive as GPUPrimitiveState,
      depthStencil,
      multisample: {
        count: num(msV, 'count', 1),
        mask: num(msV, 'mask', 0xFFFFFFFF),
        alphaToCoverageEnabled: !!msV.alphaToCoverageEnabled,
      },
      fragment: fragment ?? undefined,
    });
    // Slot count includes the gap-fill fallback buffer — the draw validation
    // net covers its declared slots too.
    this.res.pipelineVertexSlots.set(handle, buffers.length);
    return pipe;
  }

  private renderPassDesc(v: Desc): GPURenderPassDescriptor {
    const colorAttachments: (GPURenderPassColorAttachment | null)[] = [];
    for (const ca of (v.colorAttachments as (Desc | null)[] | undefined) ?? []) {
      if (!ca) {
        colorAttachments.push(null); // sparse slot: keep the index position
        continue;
      }
      const loadOp = (str(ca, 'loadOp') ?? 'load') as GPULoadOp;
      const clearValueV = ca.clearValue;
      colorAttachments.push({
        view: this.res.view(hOf(ca.view)),
        resolveTarget: ca.resolveTarget ? this.res.view(hOf(ca.resolveTarget)) : undefined,
        clearValue: clearValueV !== undefined && clearValueV !== null
          ? (Array.isArray(clearValueV)
            ? { r: clearValueV[0] ?? 0, g: clearValueV[1] ?? 0, b: clearValueV[2] ?? 0, a: clearValueV[3] ?? 0 }
            : clearValueV as GPUColor)
          : undefined,
        loadOp,
        storeOp: (str(ca, 'storeOp') ?? 'store') as GPUStoreOp,
      });
    }
    let depthStencilAttachment: GPURenderPassDepthStencilAttachment | undefined;
    const dsV = v.depthStencilAttachment && typeof v.depthStencilAttachment === 'object' ? d(v, 'depthStencilAttachment') : null;
    if (dsV) {
      depthStencilAttachment = {
        view: this.res.view(hOf(dsV.view)),
        depthLoadOp: (str(dsV, 'depthLoadOp') ?? undefined) as GPULoadOp | undefined,
        depthClearValue: optNum(dsV, 'depthClearValue') ?? (str(dsV, 'depthLoadOp') === 'clear' ? 1 : undefined),
        depthStoreOp: (str(dsV, 'depthStoreOp') ?? undefined) as GPUStoreOp | undefined,
        stencilLoadOp: (str(dsV, 'stencilLoadOp') ?? undefined) as GPULoadOp | undefined,
        stencilClearValue: optNum(dsV, 'stencilClearValue') ?? (str(dsV, 'stencilLoadOp') === 'clear' ? 0 : undefined),
        stencilStoreOp: (str(dsV, 'stencilStoreOp') ?? undefined) as GPUStoreOp | undefined,
      };
    }
    const occ = v.occlusionQuerySet;
    return {
      colorAttachments,
      depthStencilAttachment,
      occlusionQuerySet: occ && occ !== null ? this.res.querySets.get(hOf(occ)) : undefined,
    };
  }
}

// ---------------------------------------------------------------------------
// Readback: copyTextureToBuffer (256-aligned bytesPerRow) → mapAsync → row
// repack (+ BGRA→RGBA when the target format is bgra*).
// ---------------------------------------------------------------------------

async function readbackRgba(
  device: GPUDevice,
  queue: GPUQueue,
  tex: GPUTexture,
  w: number,
  h: number,
  format: string,
): Promise<Uint8Array> {
  const bpr = w * 4;
  const aligned = Math.ceil(bpr / 256) * 256;
  const buf = device.createBuffer({
    size: aligned * h,
    usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST,
  });
  const enc = device.createCommandEncoder();
  enc.copyTextureToBuffer(
    { texture: tex },
    { buffer: buf, bytesPerRow: aligned, rowsPerImage: h },
    { width: w, height: h, depthOrArrayLayers: 1 },
  );
  queue.submit([enc.finish()]);
  await buf.mapAsync(GPUMapMode.READ);
  const raw = new Uint8Array(buf.getMappedRange());
  const out = new Uint8Array(bpr * h);
  const bgra = format.startsWith('bgra');
  for (let row = 0; row < h; row++) {
    const srcRow = raw.subarray(row * aligned, row * aligned + bpr);
    const dstRow = out.subarray(row * bpr, (row + 1) * bpr);
    if (bgra) {
      for (let px = 0; px < bpr; px += 4) {
        dstRow[px] = srcRow[px + 2];
        dstRow[px + 1] = srcRow[px + 1];
        dstRow[px + 2] = srcRow[px];
        dstRow[px + 3] = srcRow[px + 3];
      }
    } else {
      dstRow.set(srcRow);
    }
  }
  buf.unmap();
  buf.destroy();
  return out;
}

/// Optional-number helper (absent vs 0 must stay distinguishable).
function optNum(v: Desc, key: string): number | undefined {
  const x = v?.[key];
  return typeof x === 'number' ? x : undefined;
}
