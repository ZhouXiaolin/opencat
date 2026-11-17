// GPU stream executor: replays a recorded WebGL2 command stream on a real
// browser WebGL2 context.
//
// Authority: `crates/opencat-engine/src/gpu/stream_executor.rs` (glow replay).
// The recorded stream *is* a WebGL2 call sequence, so replay is direct
// dispatch — no translation. Structural differences from the engine executor,
// all of them simplifications:
//
// - One persistent OffscreenCanvas + WebGL2 context per `target_id` (a
//   browser canvas is its own context, exactly what the recorder saw). The
//   engine's per-canvas client-state bucketing / restore_default_state dance
//   exists because it multiplexes many canvases over one desktop GL context;
//   here the real state per context already equals the client's view, so
//   incremental commands always start from a trustworthy state.
// - `bindFramebuffer(null)` means the canvas drawing buffer — the default
//   framebuffer *is* the stream output, so readPixels reads it directly and
//   the engine's out-FBO + BACK→COLOR_ATTACHMENT0 remapping disappear.
// - UNPACK_FLIP_Y_WEBGL / UNPACK_ALIGNMENT are native pixelStorei state.
// - Unsized-base-format uploads (ALPHA/LUMINANCE/RGB) are legal WebGL2.
// - Shader sources run as-is: the web leg and the swgl reference render in
//   the same browser engine, so sin/cos/dot rounding matches without the
//   engine's SwiftShader math rewrite.
//
// What *is* kept from the engine executor, because the semantics demand it:
// persistent handle maps across streams (three only records resource creation
// on its first frame), lazy program compilation at USE with bindAttribLocation
// replay and active-resource-ordered attrib names (core's `index:N` reflection
// encoding), uniform declared-type dispatch for the N-component commands, and
// the client bind shadow for upload commands (three's state cache skips re-
// binds, so texImage/bufferData must resolve their target from the shadow).

import type { GpuStream } from '../draw-ir/gpu-stream';
import { generatedImageRgba } from '../draw-ir/decode';
import { GLOP, GLOP_PAYLOAD_LEN } from '../generated/ocir-schema.generated';
import { RealOffscreenCanvas } from '../real-offscreen';

const NULL_HANDLE = 0xffffffff;

// GL enums used by payload decoding (WebGL2 context constants).
const ELEMENT_ARRAY_BUFFER = 0x8893;
const ARRAY_BUFFER = 0x8892;
const TEXTURE_CUBE_MAP_POSITIVE_X = 0x8515;
const TEXTURE_CUBE_MAP_NEGATIVE_Z = 0x851a;
const TEXTURE_CUBE_MAP = 0x8513;
const RENDERBUFFER = 0x8d41;
const UNPACK_FLIP_Y_WEBGL = 0x9240;
const UNPACK_ALIGNMENT = 0x0cf5;
// uniform 类型枚举（0x8b50/0x8b51）——别与 shader 类型枚举（0x8b30/0x8b31，
// FRAMEBUFFER 下方 VERTEX/FRAGMENT_SHADER 用的是后者）混写：前者错值会让
// vec2/vec3 数组 uniform 全部掉进 default→uniform1fv→INVALID_OPERATION。
const FLOAT_VEC2 = 0x8b50;
const FLOAT_VEC3 = 0x8b51;
const FLOAT_VEC4 = 0x8b52;
const INT_VEC2 = 0x8b53;
const INT_VEC3 = 0x8b54;
const INT_VEC4 = 0x8b55;
const BOOL_VEC2 = 0x8b57;
const BOOL_VEC3 = 0x8b58;
const BOOL_VEC4 = 0x8b59;
const VERTEX_SHADER = 0x8b31 as const;
const FRAGMENT_SHADER = 0x8b30 as const;

type ProgramEntry = {
  program: WebGLProgram;
  /// Active attributes in driver enumeration order — core's `index:N`
  /// attrib encoding resolves through this table.
  attribNames: string[];
  attribLoc: Map<string, number | null>;
  uniformLoc: Map<string, WebGLUniformLocation | null>;
  uniformTypes: Map<string, number>;
};

/// Last-set client-visible bindings. GL state values (caps, blend, ...) are
/// NOT shadowed: the context is private to this executor, real state always
/// equals the client's incremental view. The shadow exists solely so upload
/// commands can resolve *which object* is bound when the stream omits the
/// bind (three's state cache only emits binds on change).
type ClientBinds = {
  activeUnit: number;
  texBinds: [number, number, number][]; // (unit, target, handle)
  rbBinds: [number, number][]; // (target, handle)
  arrayBuffer: number | null;
  curVao: number | null;
  curElement: number | null;
  curProgramKey: string | null;
  /// 全局值状态影子（WebGL2 上下文初值）。流是 delta 语义——three 的
  /// state 缓存只发变化，回放正确性依赖「真机状态 == 客户端影子」；
  /// 多 canvas 共享持久 executor 时上一条流的残留会把下一流的 draw
  /// 整片剔除（engine 同位注释：实测 vfx-canvas 全黑即此因）。流首按
  /// 这份影子 restore（restoreClientState），与 engine 的
  /// restore_default_state 同构。
  caps: [number, boolean][];
  frontFace: number;
  cullFaceMode: number;
  depthFunc: number;
  depthMask: boolean;
  colorMask: [boolean, boolean, boolean, boolean];
  blendEquation: [number, number];
  blendFunc: [number, number, number, number];
  blendColor: [number, number, number, number];
  stencilFunc: [number, number, number];
  stencilOp: [number, number, number];
  stencilMask: number;
  polygonOffset: [number, number];
  clearColor: [number, number, number, number];
  clearDepth: number;
  clearStencil: number;
  viewport: [number, number, number, number] | null;
  scissor: [number, number, number, number] | null;
  drawBuffers: number[] | null;
};

/// WebGL 上下文初值 cap 表（对齐 engine DEFAULT_CAPS）。多 canvas 共享
/// executor 时换 canvas 要靠这张表关掉上一 canvas 遗留的 cap。
const DEFAULT_CAPS: [number, boolean][] = [
  [0x0be2, false], // BLEND
  [0x0b44, false], // CULL_FACE
  [0x0b71, false], // DEPTH_TEST
  [0x0bd0, true],  // DITHER
  [0x8038, false], // POLYGON_OFFSET_FILL
  [0x0c11, false], // SCISSOR_TEST
  [0x0b90, false], // STENCIL_TEST
  [0x809e, false], // SAMPLE_ALPHA_TO_COVERAGE
  [0x80a0, false], // SAMPLE_COVERAGE
];

type GlTarget = {
  canvas: OffscreenCanvas;
  gl: WebGL2RenderingContext;
  handles: {
    buffers: Map<number, WebGLBuffer>;
    vaos: Map<number, WebGLVertexArrayObject>;
    textures: Map<number, WebGLTexture>;
    fbos: Map<number, WebGLFramebuffer>;
    renderbuffers: Map<number, WebGLRenderbuffer>;
    samplers: Map<number, WebGLSampler>;
  };
  shaderSrc: Map<number, Uint8Array>;
  shaderStage: Map<number, number>;
  programShaders: Map<number, { vs: number | null; fs: number | null }>;
  programAttribBinds: Map<number, [number, string][]>;
  programKeyByPid: Map<number, string>;
  programCache: Map<string, ProgramEntry>;
  /// (texHandle, target, level) slot → last uploaded content key.
  textureCache: Map<string, string>;
  vaoElement: Map<number, number>;
  client: ClientBinds;
  unpackFlipY: boolean;
  canvasW: number;
  canvasH: number;
};

/** cap 影子 upsert（engine ClientGlState::upsert_cap 同构）。 */
function upsertCap(caps: [number, boolean][], cap: number, on: boolean): void {
  const slot = caps.find(([c]) => c === cap);
  if (slot) slot[1] = on;
  else caps.push([cap, on]);
}

/**
 * 流首把真机状态拉回客户端影子（engine restore_default_state 同构）。
 *
 * 流是 delta 语义：three 的 state 缓存只在变化时重发，回放正确性依赖
 * 「真机状态 == 客户端影子」。多 canvas 的流跑在各自持久的 GL 上下文上，
 * 但同一 executor 的 client 影子按 target 分 context 独立——残留来源是
 * 同一 context 上一次流执行后的真机状态与影子的漂移（外部污染、上帧流
 * 尾状态）。流首无条件重挂绑定 + 按 caps 影子重置 cap，delta 的起点重新
 * 可信。FBO 解绑后预设输出面 = canvas back buffer（null）——engine 的
 * 对应动作是把 stream_out FBO 设为绘制目标。
 */
function restoreClientState(t: GlTarget, stream: GpuStream): void {
  const gl = t.gl;
  const c = t.client;
  // VAO 先绑回——element 绑定与 attrib 状态都属于 VAO（账本查真对象）。
  const vao = c.curVao != null ? (t.handles.vaos.get(c.curVao) ?? null) : null;
  gl.bindVertexArray(vao);
  if (!vao) {
    c.curElement = null;
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, null);
  } else {
    // element buffer 归 VAO 账本：绑回 VAO 后按账本重挂。
    const eh = c.curVao != null ? (t.vaoElement.get(c.curVao) ?? null) : null;
    const ebuf = eh != null ? (t.handles.buffers.get(eh) ?? null) : null;
    if (ebuf) gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, ebuf);
  }
  const program = c.curProgramKey ? (t.programCache.get(c.curProgramKey)?.program ?? null) : null;
  if (!program) c.curProgramKey = null;
  gl.useProgram(program);
  const ab = c.arrayBuffer != null ? (t.handles.buffers.get(c.arrayBuffer) ?? null) : null;
  if (ab) gl.bindBuffer(gl.ARRAY_BUFFER, ab);
  // 纹理：先全清 8 单元，再按影子恢复跨帧引用的绑定（three 的
  // bindTexture 带缓存，持久 renderer 的后续帧流里可能没有 bind）。
  c.texBinds = c.texBinds.filter(([, , h]) => t.handles.textures.has(h));
  for (let unit = 0; unit < 8; unit++) {
    gl.activeTexture(gl.TEXTURE0 + unit);
    gl.bindTexture(gl.TEXTURE_2D, null);
    gl.bindTexture(gl.TEXTURE_CUBE_MAP, null);
    gl.bindSampler(unit, null);
  }
  for (const [unit, target, h] of c.texBinds) {
    if (unit < 8) {
      const tex = t.handles.textures.get(h);
      if (tex) {
        gl.activeTexture(gl.TEXTURE0 + unit);
        gl.bindTexture(target, tex);
      }
    }
  }
  gl.activeTexture(gl.TEXTURE0 + c.activeUnit);
  // FBO/renderbuffer 不按影子恢复：协议规定每条流开画前由执行器自绑
  // 输出面（canvas back buffer = null），客户端（three）对流内 render
  // target 的绑定在流内自恢复。
  gl.bindRenderbuffer(gl.RENDERBUFFER, null);
  gl.bindFramebuffer(gl.FRAMEBUFFER, null);
  gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
  gl.bindFramebuffer(gl.DRAW_FRAMEBUFFER, null);
  // MRT 会把 draw buffer 组合改掉；残留会让输出落到不存在的 attachment。
  gl.drawBuffers([gl.BACK]);
  c.drawBuffers = null;
  // 全局值状态：按影子恢复。
  for (const [cap, on] of c.caps) {
    if (on) gl.enable(cap);
    else gl.disable(cap);
  }
  gl.frontFace(c.frontFace);
  gl.cullFace(c.cullFaceMode);
  gl.depthFunc(c.depthFunc);
  gl.depthMask(c.depthMask);
  gl.colorMask(c.colorMask[0], c.colorMask[1], c.colorMask[2], c.colorMask[3]);
  gl.blendEquationSeparate(c.blendEquation[0], c.blendEquation[1]);
  gl.blendFuncSeparate(c.blendFunc[0], c.blendFunc[1], c.blendFunc[2], c.blendFunc[3]);
  gl.blendColor(c.blendColor[0], c.blendColor[1], c.blendColor[2], c.blendColor[3]);
  gl.stencilFunc(c.stencilFunc[0], c.stencilFunc[1], c.stencilFunc[2]);
  gl.stencilOp(c.stencilOp[0], c.stencilOp[1], c.stencilOp[2]);
  gl.stencilMask(c.stencilMask);
  gl.polygonOffset(c.polygonOffset[0], c.polygonOffset[1]);
  gl.clearColor(c.clearColor[0], c.clearColor[1], c.clearColor[2], c.clearColor[3]);
  gl.clearDepth(c.clearDepth);
  gl.clearStencil(c.clearStencil);
  // FLIP_Y 不透传原生也不恢复（engine unpack_flip_y 是 executor 级 sticky
  // 影子，不在 restore 清单；three 每次上传前都会显式重设）。ALIGNMENT 恢复
  // 硬默认 4（engine restore 同款写死）。
  gl.pixelStorei(gl.UNPACK_ALIGNMENT, 4);
  // 视口/裁剪：影子缺省 = WebGL 初值（画布尺寸）。
  const vp = c.viewport ?? [0, 0, stream.width, stream.height];
  gl.viewport(vp[0], vp[1], vp[2], vp[3]);
  const sc = c.scissor ?? [0, 0, stream.width, stream.height];
  gl.scissor(sc[0], sc[1], sc[2], sc[3]);
}

export class GlStreamError extends Error {
  constructor(message: string) {
    super(message);
    this.name = 'GlStreamError';
  }
}

function fnv1a(bytes: Uint8Array): string {
  let h = 0xcbf29ce484222325n;
  const prime = 0x100000001b3n;
  const mask = 0xffffffffffffffffn;
  for (const b of bytes) {
    h ^= BigInt(b);
    h = (h * prime) & mask;
  }
  return `${bytes.length}:${h.toString(16)}`;
}

export class GlStreamExecutor {
  private targets = new Map<string, GlTarget>();

  /// Render one stream and land its pixels in `generatedImageRgba` under the
  /// stream's image id (top-down, RGBA8). Synchronous — WebGL readPixels on
  /// the context's own canvas needs no external settle.
  execute(stream: GpuStream): void {
    const isNewTarget = !this.targets.has(stream.targetId);
    const t = this.target(stream);
    // 流是 delta 语义：three 的 state 缓存吃掉大部分 rt 重绑，常规帧的
    // 流里没有「画到输出面」的开场 bind。engine 侧对应动作 = 流首把
    // stream_out FBO 设为绘制目标（stream_executor.rs：执行器把输出 FBO
    // 设为绘制目标，clear/draw 才有落点）；web 的输出面 = canvas back
    // buffer，等价动作是流首 bind null。没有这一步，持久 executor 会把
    // 上一帧流尾的 FBO 绑定带进来，draw 全落进残留 FBO → 回读全黑
    // （vfx f30 四流 mean=0.00、nullBinds=0/drawsAfterNull=0 实证）。
    restoreClientState(t, stream);
    this.executeStream(t, stream);
    const rgba = new Uint8Array(stream.width * stream.height * 4);
    // Read the stream output = the canvas drawing buffer. The stream may
    // leave an internal render-target FBO bound (three/composer multi-pass),
    // and readPixels reads the *currently bound* framebuffer — the engine
    // side redirects bindFramebuffer(null) to a dedicated stream-out FBO and
    // reads exactly that (stream_executor.rs readback_sync); the WebGL
    // equivalent of that redirect is binding null (the OffscreenCanvas back
    // buffer) before the readback.
    t.gl.bindFramebuffer(t.gl.FRAMEBUFFER, null);
    t.gl.readPixels(0, 0, stream.width, stream.height, t.gl.RGBA, t.gl.UNSIGNED_BYTE, rgba);
    // GL bottom-origin → composition top-origin: flip rows in place.
    flipRowsVertical(rgba, stream.height);
    generatedImageRgba.set(stream.imageId, { width: stream.width, height: stream.height, rgba });
    // oracle 实时探针：服务端墙钟裁决每流真实开销（页内时钟被 realm shim 接管）。
    try {
      fetch('/oracle-log', {
        method: 'POST',
        body: `glx ${stream.targetId.slice(-10)} new=${isNewTarget} ${stream.width}x${stream.height} opBytes=${stream.ops.ops.length} targets=${this.targets.size}`,
      }).catch(() => {});
    } catch { /* ignore */ }
  }

  /// Release every per-target context (canvas count is bounded but WebGL
  /// contexts are expensive; called on session teardown).
  dispose(): void {
    for (const t of this.targets.values()) {
      t.gl.getExtension('WEBGL_lose_context')?.loseContext();
    }
    this.targets.clear();
  }

  // ── target/context lifecycle ────────────────────────────────────────

  private target(stream: GpuStream): GlTarget {
    let t = this.targets.get(stream.targetId);
    if (!t) {
      // Realm shim overwrote globalThis.OffscreenCanvas (2d-only recording
      // face) — construct the stashed real browser class or webgl2 throws.
      const canvas = new RealOffscreenCanvas(stream.width, stream.height);
      const gl = canvas.getContext('webgl2', {
        alpha: stream.attrs.alpha,
        antialias: stream.attrs.antialias,
        premultipliedAlpha: stream.attrs.premultipliedAlpha,
        // 回放端的持久性要求（与 engine 的持久 out_fbo 纹理同构）：无绘制
        // delta 流（three 本帧未重绘该 canvas）依赖 back buffer 跨帧保持
        // 内容，非 preserve 模式的帧间清空会让这类流回读全黑。
        preserveDrawingBuffer: true,
        depth: stream.attrs.depth,
        stencil: stream.attrs.stencil,
      });
      if (!gl) {
        throw new GlStreamError(`webgl2 context failed for canvas ${stream.targetId}`);
      }
      // The recording face pins EXT_color_buffer_float / half_float as
      // available (webgl_api.js SUPPORTED_EXTENSIONS), so three commits to
      // HalfFloat render targets (EffectComposer's default). On the replay
      // side WebGL2 requires an explicit getExtension to make float
      // attachments renderable — without it every draw into the composer's
      // FBO hits "Framebuffer is incomplete: Attachment is not renderable"
      // and the whole stream lands black. (Native GL ES / mozangle on the
      // engine side enables float renderability by default; the WebGL spec
      // layer is web-only extra.)
      gl.getExtension('EXT_color_buffer_float');
      gl.getExtension('EXT_color_buffer_half_float');
      gl.getExtension('EXT_texture_filter_anisotropic');
      t = {
        canvas,
        gl,
        handles: {
          buffers: new Map(),
          vaos: new Map(),
          textures: new Map(),
          fbos: new Map(),
          renderbuffers: new Map(),
          samplers: new Map(),
        },
        shaderSrc: new Map(),
        shaderStage: new Map(),
        programShaders: new Map(),
        programAttribBinds: new Map(),
        programKeyByPid: new Map(),
        programCache: new Map(),
        textureCache: new Map(),
        vaoElement: new Map(),
        client: {
          activeUnit: 0,
          texBinds: [],
          rbBinds: [],
          arrayBuffer: null,
          curVao: null,
          curElement: null,
          curProgramKey: null,
          caps: DEFAULT_CAPS.map(([c, on]) => [c, on] as [number, boolean]),
          frontFace: 0x0901,             // CCW
          cullFaceMode: 0x0405,          // BACK
          depthFunc: 0x0201,             // LESS
          depthMask: true,
          colorMask: [true, true, true, true],
          blendEquation: [0x8006, 0x8006], // FUNC_ADD ×2
          blendFunc: [1, 0, 1, 0],         // ONE, ZERO
          blendColor: [0, 0, 0, 0],
          stencilFunc: [0x0207, 0, 0xffffffff], // ALWAYS, ref 0, mask ~0
          stencilOp: [0x1e00, 0x1e00, 0x1e00],  // KEEP ×3
          stencilMask: 0xffffffff,
          polygonOffset: [0, 0],
          clearColor: [0, 0, 0, 0],
          clearDepth: 1,
          clearStencil: 0,
          viewport: null,
          scissor: null,
          drawBuffers: null,
        },
        unpackFlipY: false,
        canvasW: stream.width,
        canvasH: stream.height,
      };
      this.targets.set(stream.targetId, t);
    }
    if (t.canvasW !== stream.width || t.canvasH !== stream.height) {
      // Resizing the drawing buffer keeps GL objects and state but clears
      // the buffer contents — viewport is re-asserted per stream below.
      t.canvas.width = stream.width;
      t.canvas.height = stream.height;
      t.canvasW = stream.width;
      t.canvasH = stream.height;
    }
    // Stream-start alignment: the recorder relies on the WebGL initial
    // viewport (= canvas size) when the page never calls viewport.
    t.gl.viewport(0, 0, stream.width, stream.height);
    return t;
  }

  // ── stream replay ───────────────────────────────────────────────────

  private executeStream(t: GlTarget, stream: GpuStream): void {
    const gl = t.gl;
    const { ops, bytes, byteRanges } = stream.ops;
    let p = 0;
    let program: WebGLProgram | null = null;
    let programKey: string | null = t.client.curProgramKey;
    if (programKey) {
      program = t.programCache.get(programKey)?.program ?? null;
    }

    while (p < ops.length) {
      const start = p;
      const op = ops[p]!;
      p += 1;
      switch (op) {
        case GLOP.SHADER_CREATE: {
          const stage = rdU16(ops, p); p += 2;
          const id = rdU32(ops, p); p += 4;
          t.shaderStage.set(id, stage);
          break;
        }
        case GLOP.SHADER_SOURCE: {
          const id = rdU32(ops, p); p += 4;
          const range = rdU32(ops, p); p += 4;
          const src = rangeBytes(bytes, byteRanges, range);
          t.shaderSrc.set(id, src);
          break;
        }
        case GLOP.COMPILE_SHADER:
        case GLOP.SHADER_DELETE: {
          p += 4; // lazy compile at USE; delete only detaches
          break;
        }
        case GLOP.PROGRAM_LINK: {
          const pid = rdU32(ops, p); p += 4; // lazy compile at USE
          break;
        }
        case GLOP.PROGRAM_CREATE: {
          // Handle sequences replay from zero every frame on the html path:
          // the same pid may be a different program next frame.
          const pid = rdU32(ops, p); p += 4;
          t.programShaders.delete(pid);
          t.programKeyByPid.delete(pid);
          t.programAttribBinds.delete(pid);
          break;
        }
        case GLOP.PROGRAM_ATTACH: {
          const pid = rdU32(ops, p); p += 4;
          const sid = rdU32(ops, p); p += 4;
          const slot = t.programShaders.get(pid) ?? { vs: null, fs: null };
          if (t.shaderStage.get(sid) === VERTEX_SHADER) slot.vs = sid;
          else slot.fs = sid;
          t.programShaders.set(pid, slot);
          break;
        }
        case GLOP.PROGRAM_USE: {
          const pid = rdU32(ops, p); p += 4;
          if (pid === NULL_HANDLE) {
            program = null;
            programKey = null;
            t.client.curProgramKey = null;
            gl.useProgram(null);
          } else {
            const key = this.programKeyFor(t, pid);
            const entry = t.programCache.get(key);
            if (!entry) throw new GlStreamError(`program ${pid} cache miss`);
            program = entry.program;
            programKey = key;
            t.client.curProgramKey = key;
            gl.useProgram(entry.program);
          }
          break;
        }
        case GLOP.PROGRAM_DELETE: {
          const pid = rdU32(ops, p); p += 4;
          t.programShaders.delete(pid);
          t.programKeyByPid.delete(pid);
          t.programAttribBinds.delete(pid);
          break;
        }
        case GLOP.BUFFER_CREATE: {
          const h = rdU32(ops, p); p += 4;
          if (!t.handles.buffers.has(h)) t.handles.buffers.set(h, gl.createBuffer());
          break;
        }
        case GLOP.BUFFER_BIND: {
          const target = rdU16(ops, p); p += 2;
          const h = rdU32(ops, p); p += 4;
          let buf: WebGLBuffer | null = null;
          if (h !== NULL_HANDLE) {
            if (target === ELEMENT_ARRAY_BUFFER) {
              t.client.curElement = h;
              const vao = t.client.curVao;
              if (vao !== null) t.vaoElement.set(vao, h);
            }
            buf = t.handles.buffers.get(h) ?? gl.createBuffer();
            if (!t.handles.buffers.has(h)) t.handles.buffers.set(h, buf);
            if (target === ARRAY_BUFFER) t.client.arrayBuffer = h;
          } else {
            if (target === ELEMENT_ARRAY_BUFFER) t.client.curElement = null;
            if (target === ARRAY_BUFFER) t.client.arrayBuffer = null;
          }
          gl.bindBuffer(target, buf);
          break;
        }
        case GLOP.BUFFER_DATA: {
          const target = rdU16(ops, p); p += 2;
          const usage = rdU16(ops, p); p += 2;
          const range = rdU32(ops, p); p += 4;
          const h = boundBuffer(t, target);
          if (h === null) throw new GlStreamError('bufferData with no client-bound buffer');
          const data = rangeBytes(bytes, byteRanges, range);
          gl.bindBuffer(target, t.handles.buffers.get(h)!);
          gl.bufferData(target, data, usage);
          break;
        }
        case GLOP.BUFFER_DATA_NULL: {
          const target = rdU16(ops, p); p += 2;
          const usage = rdU16(ops, p); p += 2;
          const size = rdU32(ops, p); p += 4;
          const h = boundBuffer(t, target);
          if (h === null) throw new GlStreamError('bufferData(null) with no client-bound buffer');
          gl.bindBuffer(target, t.handles.buffers.get(h)!);
          gl.bufferData(target, size, usage);
          break;
        }
        case GLOP.BUFFER_DELETE: {
          const h = rdU32(ops, p); p += 4;
          const buf = t.handles.buffers.get(h);
          if (buf) {
            gl.deleteBuffer(buf);
            t.handles.buffers.delete(h);
          }
          break;
        }
        case GLOP.VAO_CREATE: {
          const h = rdU32(ops, p); p += 4;
          if (!t.handles.vaos.has(h)) t.handles.vaos.set(h, gl.createVertexArray());
          break;
        }
        case GLOP.VAO_BIND: {
          const h = rdU32(ops, p); p += 4;
          let vao: WebGLVertexArrayObject | null = null;
          if (h !== NULL_HANDLE) {
            vao = t.handles.vaos.get(h) ?? gl.createVertexArray();
            if (!t.handles.vaos.has(h)) t.handles.vaos.set(h, vao);
            t.client.curVao = h;
          } else {
            t.client.curVao = null;
          }
          gl.bindVertexArray(vao);
          // Re-assert the ledger's element binding: the VAO owns element
          // state and three skips re-binds on cached state. 必须在
          // bindVertexArray 之后——bindBuffer(ELEMENT) 写进的是「当前
          // VAO」，写在切换前会把 element 挂到上一个 VAO 上（vfx 手机
          // 全 mesh INVALID_OPERATION 的直接根因；engine 侧
          // stream_executor.rs 同位置即先 bind 后挂）。
          if (h !== NULL_HANDLE) {
            const eh = t.vaoElement.get(h);
            if (eh !== undefined) gl.bindBuffer(ELEMENT_ARRAY_BUFFER, t.handles.buffers.get(eh) ?? null);
          }
          break;
        }
        case GLOP.VAO_DELETE: {
          const h = rdU32(ops, p); p += 4;
          const vao = t.handles.vaos.get(h);
          if (vao) {
            gl.deleteVertexArray(vao);
            t.handles.vaos.delete(h);
          }
          if (t.client.curVao === h) {
            t.client.curVao = null;
            t.client.curElement = null;
          }
          t.vaoElement.delete(h);
          break;
        }
        case GLOP.ATTRIB_ENABLE:
        case GLOP.ATTRIB_DISABLE: {
          const nameRange = rdU32(ops, p); p += 4;
          const loc = attribLoc(t, bytes, byteRanges, programKey, nameRange);
          if (loc !== null) {
            if (op === GLOP.ATTRIB_ENABLE) gl.enableVertexAttribArray(loc);
            else gl.disableVertexAttribArray(loc);
          }
          break;
        }
        case GLOP.ATTRIB_POINTER: {
          const nameRange = rdU32(ops, p); p += 4;
          const size = ops[p]!; p += 1;
          const ty = rdU16(ops, p); p += 2;
          const normalized = ops[p]!; p += 1;
          const stride = rdU32(ops, p); p += 4;
          const offset = rdU32(ops, p); p += 4;
          const loc = attribLoc(t, bytes, byteRanges, programKey, nameRange);
          if (loc !== null) gl.vertexAttribPointer(loc, size, ty, normalized !== 0, stride, offset);
          break;
        }
        case GLOP.ATTRIB_DIVISOR: {
          const nameRange = rdU32(ops, p); p += 4;
          const divisor = rdU32(ops, p); p += 4;
          const loc = attribLoc(t, bytes, byteRanges, programKey, nameRange);
          if (loc !== null) gl.vertexAttribDivisor(loc, divisor);
          break;
        }
        case GLOP.UNIFORM_1F: {
          const range = rdU32(ops, p); p += 4;
          const x = rdF32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform1f(loc, x);
          break;
        }
        case GLOP.UNIFORM_2F: {
          const range = rdU32(ops, p); p += 4;
          const x = rdF32(ops, p); p += 4;
          const y = rdF32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform2f(loc, x, y);
          break;
        }
        case GLOP.UNIFORM_3F: {
          const range = rdU32(ops, p); p += 4;
          const x = rdF32(ops, p); p += 4;
          const y = rdF32(ops, p); p += 4;
          const z = rdF32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform3f(loc, x, y, z);
          break;
        }
        case GLOP.UNIFORM_4F: {
          const range = rdU32(ops, p); p += 4;
          const x = rdF32(ops, p); p += 4;
          const y = rdF32(ops, p); p += 4;
          const z = rdF32(ops, p); p += 4;
          const w = rdF32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform4f(loc, x, y, z, w);
          break;
        }
        case GLOP.UNIFORM_1I: {
          const range = rdU32(ops, p); p += 4;
          const v = rdI32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform1i(loc, v);
          break;
        }
        case GLOP.UNIFORM_2I: {
          const range = rdU32(ops, p); p += 4;
          const x = rdI32(ops, p); p += 4;
          const y = rdI32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform2i(loc, x, y);
          break;
        }
        case GLOP.UNIFORM_3I: {
          const range = rdU32(ops, p); p += 4;
          const x = rdI32(ops, p); p += 4;
          const y = rdI32(ops, p); p += 4;
          const z = rdI32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform3i(loc, x, y, z);
          break;
        }
        case GLOP.UNIFORM_4I: {
          const range = rdU32(ops, p); p += 4;
          const x = rdI32(ops, p); p += 4;
          const y = rdI32(ops, p); p += 4;
          const z = rdI32(ops, p); p += 4;
          const w = rdI32(ops, p); p += 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) gl.uniform4i(loc, x, y, z, w);
          break;
        }
        case GLOP.UNIFORM_NF: {
          const range = rdU32(ops, p); p += 4;
          const count = rdU32(ops, p); p += 4;
          const data = tailF32(ops, p, count); p += count * 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) {
            const ut = uniformType(t, bytes, byteRanges, programKey, range);
            switch (ut) {
              case FLOAT_VEC2: gl.uniform2fv(loc, data); break;
              case FLOAT_VEC3: gl.uniform3fv(loc, data); break;
              case FLOAT_VEC4: gl.uniform4fv(loc, data); break;
              default: gl.uniform1fv(loc, data); break;
            }
          }
          break;
        }
        case GLOP.UNIFORM_NI: {
          const range = rdU32(ops, p); p += 4;
          const count = rdU32(ops, p); p += 4;
          const data = tailI32(ops, p, count); p += count * 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) {
            switch (uniformType(t, bytes, byteRanges, programKey, range)) {
              case INT_VEC2: case BOOL_VEC2: gl.uniform2iv(loc, data); break;
              case INT_VEC3: case BOOL_VEC3: gl.uniform3iv(loc, data); break;
              case INT_VEC4: case BOOL_VEC4: gl.uniform4iv(loc, data); break;
              default: gl.uniform1iv(loc, data); break;
            }
          }
          break;
        }
        case GLOP.UNIFORM_MATRIX_4F: {
          const range = rdU32(ops, p); p += 4;
          const count = rdU32(ops, p); p += 4;
          const order = ops[p]!; p += 1;
          const n = count * order * order;
          const data = tailF32(ops, p, n); p += n * 4;
          const loc = uniformLoc(t, bytes, byteRanges, programKey, range);
          if (loc) {
            if (order === 2) gl.uniformMatrix2fv(loc, false, data);
            else if (order === 3) gl.uniformMatrix3fv(loc, false, data);
            else gl.uniformMatrix4fv(loc, false, data);
          }
          break;
        }
        case GLOP.TEXTURE_CREATE: {
          const h = rdU32(ops, p); p += 4;
          if (!t.handles.textures.has(h)) t.handles.textures.set(h, gl.createTexture());
          break;
        }
        case GLOP.TEXTURE_DELETE: {
          const h = rdU32(ops, p); p += 4;
          const tex = t.handles.textures.get(h);
          if (tex) {
            gl.deleteTexture(tex);
            t.handles.textures.delete(h);
          }
          for (const key of t.textureCache.keys()) {
            if (key.startsWith(`${h}|`)) t.textureCache.delete(key);
          }
          t.client.texBinds = t.client.texBinds.filter(([, , th]) => th !== h);
          break;
        }
        case GLOP.TEXTURE_ACTIVE: {
          const unit = ops[p]!; p += 1;
          t.client.activeUnit = unit;
          gl.activeTexture(gl.TEXTURE0 + unit);
          break;
        }
        case GLOP.TEXTURE_BIND: {
          const target = rdU16(ops, p); p += 2;
          const h = rdU32(ops, p); p += 4;
          let tex: WebGLTexture | null = null;
          if (h !== NULL_HANDLE) {
            tex = t.handles.textures.get(h) ?? gl.createTexture();
            if (!t.handles.textures.has(h)) t.handles.textures.set(h, tex);
          }
          const bindTarget = textureBindTarget(target);
          t.client.texBinds = t.client.texBinds.filter(
            ([u, bt]) => !(u === t.client.activeUnit && bt === bindTarget),
          );
          if (h !== NULL_HANDLE) t.client.texBinds.push([t.client.activeUnit, bindTarget, h]);
          // WebGL rejects face targets on bindTexture (desktop GL allows
          // them; both bind the same cube object) — pass the normalized
          // target to keep the GL-ES-shaped stream replayable on WebGL.
          gl.bindTexture(bindTarget, tex);
          break;
        }
        case GLOP.TEX_PARAM_I: {
          const target = rdU16(ops, p); p += 2;
          const pname = rdU16(ops, p); p += 2;
          const value = rdI32(ops, p); p += 4;
          gl.texParameteri(target, pname, value);
          break;
        }
        case GLOP.TEX_PARAM_F: {
          const target = rdU16(ops, p); p += 2;
          const pname = rdU16(ops, p); p += 2;
          const value = rdF32(ops, p); p += 4;
          gl.texParameterf(target, pname, value);
          break;
        }
        case GLOP.TEX_IMAGE_2D: {
          const target = rdU16(ops, p); p += 2;
          const level = ops[p]!; p += 1;
          const internal = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          const format = rdU16(ops, p); p += 2;
          const ty = rdU16(ops, p); p += 2;
          const hasData = ops[p]!; p += 1;
          const range = rdU32(ops, p); p += 4;
          const texHandle = boundTexture(t, textureBindTarget(target));
          if (texHandle === null) throw new GlStreamError(`texImage2D with no client-bound texture (unit=${t.client.activeUnit} target=0x${target.toString(16)} binds=[${t.client.texBinds.slice(-8).map((b) => b.join(':')).join(' ')}])`);
          const tex = t.handles.textures.get(texHandle)!;
          const data = hasData !== 0 ? flipIfUnpackY(t, rangeBytes(bytes, byteRanges, range), h) : null;
          // Upload dedup by (texture, target, level) slot — the same texture
          // object with identical content skips the re-upload.
          const slotKey = `${texHandle}|${target}|${level}`;
          const contentKey = `${internal}|${w}x${h}|${format}|${ty}|${fnv1a(data ?? EMPTY)}`;
          if (t.textureCache.get(slotKey) !== contentKey) {
            gl.bindTexture(textureBindTarget(target), tex);
            gl.texImage2D(target, level, internal, w, h, 0, format, ty, data);
            t.textureCache.set(slotKey, contentKey);
          }
          break;
        }
        case GLOP.TEX_SUB_IMAGE_2D: {
          const target = rdU16(ops, p); p += 2;
          const level = ops[p]!; p += 1;
          const x = rdI32(ops, p); p += 4;
          const y = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          const format = rdU16(ops, p); p += 2;
          const ty = rdU16(ops, p); p += 2;
          const hasData = ops[p]!; p += 1;
          const range = rdU32(ops, p); p += 4;
          const texHandle = boundTexture(t, textureBindTarget(target));
          if (texHandle === null) throw new GlStreamError('texSubImage2D with no client-bound texture');
          const tex = t.handles.textures.get(texHandle)!;
          // A partial write invalidates the slot's content key.
          t.textureCache.delete(`${texHandle}|${target}|${level}`);
          const data = hasData !== 0 ? flipIfUnpackY(t, rangeBytes(bytes, byteRanges, range), h) : 0;
          gl.bindTexture(textureBindTarget(target), tex);
          if (hasData !== 0) gl.texSubImage2D(target, level, x, y, w, h, format, ty, data as Uint8Array);
          else gl.texSubImage2D(target, level, x, y, w, h, format, ty, 0);
          break;
        }
        case GLOP.TEX_IMAGE_3D: {
          const target = rdU16(ops, p); p += 2;
          const level = ops[p]!; p += 1;
          const internal = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          const d = rdU32(ops, p); p += 4;
          const format = rdU16(ops, p); p += 2;
          const ty = rdU16(ops, p); p += 2;
          const hasData = ops[p]!; p += 1;
          const range = rdU32(ops, p); p += 4;
          const texHandle = boundTexture(t, textureBindTarget(target));
          if (texHandle === null) throw new GlStreamError('texImage3D with no client-bound texture');
          gl.bindTexture(textureBindTarget(target), t.handles.textures.get(texHandle)!);
          gl.texImage3D(
            target, level, internal, w, h, d, 0, format, ty,
            hasData !== 0 ? rangeBytes(bytes, byteRanges, range) : null,
          );
          break;
        }
        case GLOP.TEX_SUB_IMAGE_3D: {
          const target = rdU16(ops, p); p += 2;
          const level = ops[p]!; p += 1;
          const x = rdI32(ops, p); p += 4;
          const y = rdI32(ops, p); p += 4;
          const z = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          const d = rdU32(ops, p); p += 4;
          const format = rdU16(ops, p); p += 2;
          const ty = rdU16(ops, p); p += 2;
          const hasData = ops[p]!; p += 1;
          const range = rdU32(ops, p); p += 4;
          const texHandle = boundTexture(t, textureBindTarget(target));
          if (texHandle === null) throw new GlStreamError('texSubImage3D with no client-bound texture');
          gl.bindTexture(textureBindTarget(target), t.handles.textures.get(texHandle)!);
          if (hasData !== 0) {
            gl.texSubImage3D(target, level, x, y, z, w, h, d, format, ty, rangeBytes(bytes, byteRanges, range));
          } else {
            gl.texSubImage3D(target, level, x, y, z, w, h, d, format, ty, 0);
          }
          break;
        }
        case GLOP.GENERATE_MIPMAP: {
          const target = rdU16(ops, p); p += 2;
          gl.generateMipmap(target);
          break;
        }
        case GLOP.PIXEL_STORE_I: {
          const pname = rdU16(ops, p); p += 2;
          const value = rdI32(ops, p); p += 4;
          // UNPACK_FLIP_Y_WEBGL: shadow-only. The browser's WebGL2 natively
          // honors this pname — passing it through would flip uploads once
          // plus the manual flip in the TEX_IMAGE arms = double flip (engine
          // 桌面 GL 不认此 pname，手动 flip 一次即正确；web 必须拦截).
          if (pname === UNPACK_FLIP_Y_WEBGL) t.unpackFlipY = value !== 0;
          else gl.pixelStorei(pname, value);
          break;
        }
        case GLOP.FRAMEBUFFER_CREATE: {
          const h = rdU32(ops, p); p += 4;
          if (!t.handles.fbos.has(h)) t.handles.fbos.set(h, gl.createFramebuffer());
          break;
        }
        case GLOP.FRAMEBUFFER_DELETE: {
          const h = rdU32(ops, p); p += 4;
          const fbo = t.handles.fbos.get(h);
          if (fbo) {
            gl.deleteFramebuffer(fbo);
            t.handles.fbos.delete(h);
          }
          break;
        }
        case GLOP.BIND_FRAMEBUFFER: {
          const target = rdU16(ops, p); p += 2;
          const h = rdU32(ops, p); p += 4;
          let fbo: WebGLFramebuffer | null = null;
          if (h !== NULL_HANDLE) {
            fbo = t.handles.fbos.get(h) ?? gl.createFramebuffer();
            if (!t.handles.fbos.has(h)) t.handles.fbos.set(h, fbo);
          }
          // null keeps its WebGL meaning: the canvas drawing buffer = the
          // stream output.
          gl.bindFramebuffer(target, fbo);
          break;
        }
        case GLOP.FRAMEBUFFER_TEXTURE_2D: {
          const target = rdU16(ops, p); p += 2;
          const attachment = rdU16(ops, p); p += 2;
          const textarget = rdU16(ops, p); p += 2;
          const h = rdU32(ops, p); p += 4;
          const level = rdI32(ops, p); p += 4;
          const tex = h !== NULL_HANDLE ? t.handles.textures.get(h) : null;
          if (h !== NULL_HANDLE && !tex) throw new GlStreamError(`FBO attach on unknown texture ${h}`);
          gl.framebufferTexture2D(target, attachment, textarget, tex ?? null, level);
          break;
        }
        case GLOP.VIEWPORT: {
          const x = rdI32(ops, p); p += 4;
          const y = rdI32(ops, p); p += 4;
          const w = rdI32(ops, p); p += 4;
          const h = rdI32(ops, p); p += 4;
          t.client.viewport = [x, y, w, h];
          gl.viewport(x, y, w, h);
          break;
        }
        case GLOP.SCISSOR: {
          const x = rdI32(ops, p); p += 4;
          const y = rdI32(ops, p); p += 4;
          const w = rdI32(ops, p); p += 4;
          const h = rdI32(ops, p); p += 4;
          t.client.scissor = [x, y, w, h];
          gl.scissor(x, y, w, h);
          break;
        }
        case GLOP.CLEAR_COLOR: {
          const r = rdF32(ops, p); p += 4;
          const g = rdF32(ops, p); p += 4;
          const b = rdF32(ops, p); p += 4;
          const a = rdF32(ops, p); p += 4;
          t.client.clearColor = [r, g, b, a];
          gl.clearColor(r, g, b, a);
          break;
        }
        case GLOP.CLEAR: {
          const bits = ops[p]!; p += 1;
          let mask = 0;
          if (bits & 1) mask |= gl.COLOR_BUFFER_BIT;
          if (bits & 2) mask |= gl.DEPTH_BUFFER_BIT;
          if (bits & 4) mask |= gl.STENCIL_BUFFER_BIT;
          gl.clear(mask);
          break;
        }
        case GLOP.ENABLE: {
          const cap = rdU16(ops, p); p += 2;
          upsertCap(t.client.caps, cap, true);
          gl.enable(cap);
          break;
        }
        case GLOP.DISABLE: {
          const cap = rdU16(ops, p); p += 2;
          upsertCap(t.client.caps, cap, false);
          gl.disable(cap);
          break;
        }
        case GLOP.DEPTH_FUNC: {
          const f = rdU16(ops, p); p += 2;
          t.client.depthFunc = f;
          gl.depthFunc(f);
          break;
        }
        case GLOP.DEPTH_MASK: {
          const on = ops[p]!; p += 1;
          t.client.depthMask = on !== 0;
          gl.depthMask(on !== 0);
          break;
        }
        case GLOP.BLEND_FUNC: {
          const src = rdU16(ops, p); p += 2;
          const dst = rdU16(ops, p); p += 2;
          t.client.blendFunc = [src, dst, src, dst];
          gl.blendFunc(src, dst);
          break;
        }
        case GLOP.BLEND_FUNC_SEPARATE: {
          const srcRgb = rdU16(ops, p); p += 2;
          const dstRgb = rdU16(ops, p); p += 2;
          const srcA = rdU16(ops, p); p += 2;
          const dstA = rdU16(ops, p); p += 2;
          t.client.blendFunc = [srcRgb, dstRgb, srcA, dstA];
          gl.blendFuncSeparate(srcRgb, dstRgb, srcA, dstA);
          break;
        }
        case GLOP.BLEND_EQUATION: {
          const eq = rdU16(ops, p); p += 2;
          t.client.blendEquation = [eq, eq];
          gl.blendEquation(eq);
          break;
        }
        case GLOP.BLEND_EQUATION_SEPARATE: {
          const rgb = rdU16(ops, p); p += 2;
          const a = rdU16(ops, p); p += 2;
          t.client.blendEquation = [rgb, a];
          gl.blendEquationSeparate(rgb, a);
          break;
        }
        case GLOP.BLEND_COLOR: {
          const r = rdF32(ops, p); p += 4;
          const g = rdF32(ops, p); p += 4;
          const b = rdF32(ops, p); p += 4;
          const a = rdF32(ops, p); p += 4;
          t.client.blendColor = [r, g, b, a];
          gl.blendColor(r, g, b, a);
          break;
        }
        case GLOP.CULL_FACE: {
          const mode = rdU16(ops, p); p += 2;
          t.client.cullFaceMode = mode;
          gl.cullFace(mode);
          break;
        }
        case GLOP.FRONT_FACE: {
          const mode = rdU16(ops, p); p += 2;
          t.client.frontFace = mode;
          gl.frontFace(mode);
          break;
        }
        case GLOP.CLEAR_DEPTH: {
          const d = rdF32(ops, p); p += 4;
          t.client.clearDepth = d;
          gl.clearDepth(d);
          break;
        }
        case GLOP.CLEAR_STENCIL: {
          const s = rdI32(ops, p); p += 4;
          t.client.clearStencil = s;
          gl.clearStencil(s);
          break;
        }
        case GLOP.POLYGON_OFFSET: {
          const factor = rdF32(ops, p); p += 4;
          const units = rdF32(ops, p); p += 4;
          t.client.polygonOffset = [factor, units];
          gl.polygonOffset(factor, units);
          break;
        }
        case GLOP.STENCIL_FUNC: {
          const f = rdU16(ops, p); p += 2;
          const ref_ = rdI32(ops, p); p += 4;
          const mask = rdU32(ops, p); p += 4;
          t.client.stencilFunc = [f, ref_, mask];
          gl.stencilFunc(f, ref_, mask);
          break;
        }
        case GLOP.STENCIL_FUNC_SEPARATE: {
          const face = rdU16(ops, p); p += 2;
          const f = rdU16(ops, p); p += 2;
          const ref_ = rdI32(ops, p); p += 4;
          const mask = rdU32(ops, p); p += 4;
          // 影子只记非 separate 面（引擎同构：stencil_func 三元组）。
          t.client.stencilFunc = [f, ref_, mask];
          gl.stencilFuncSeparate(face, f, ref_, mask);
          break;
        }
        case GLOP.STENCIL_OP: {
          const sfail = rdU16(ops, p); p += 2;
          const dpfail = rdU16(ops, p); p += 2;
          const dppass = rdU16(ops, p); p += 2;
          t.client.stencilOp = [sfail, dpfail, dppass];
          gl.stencilOp(sfail, dpfail, dppass);
          break;
        }
        case GLOP.STENCIL_OP_SEPARATE: {
          const face = rdU16(ops, p); p += 2;
          const sfail = rdU16(ops, p); p += 2;
          const dpfail = rdU16(ops, p); p += 2;
          const dppass = rdU16(ops, p); p += 2;
          t.client.stencilOp = [sfail, dpfail, dppass];
          gl.stencilOpSeparate(face, sfail, dpfail, dppass);
          break;
        }
        case GLOP.STENCIL_MASK: {
          const m = rdU32(ops, p); p += 4;
          t.client.stencilMask = m;
          gl.stencilMask(m);
          break;
        }
        case GLOP.COLOR_MASK: {
          const r = ops[p]!; p += 1;
          const g = ops[p]!; p += 1;
          const b = ops[p]!; p += 1;
          const a = ops[p]!; p += 1;
          t.client.colorMask = [r !== 0, g !== 0, b !== 0, a !== 0];
          gl.colorMask(r !== 0, g !== 0, b !== 0, a !== 0);
          break;
        }
        case GLOP.DRAW_ARRAYS: {
          const mode = rdU16(ops, p); p += 2;
          const first = rdI32(ops, p); p += 4;
          const count = rdI32(ops, p); p += 4;
          gl.drawArrays(mode, first, count);
          break;
        }
        case GLOP.DRAW_ELEMENTS: {
          const mode = rdU16(ops, p); p += 2;
          const count = rdI32(ops, p); p += 4;
          const ty = rdU16(ops, p); p += 2;
          const offset = rdI32(ops, p); p += 4;
          gl.drawElements(mode, count, ty, offset);
          break;
        }
        case GLOP.DRAW_ARRAYS_INSTANCED: {
          const mode = rdU16(ops, p); p += 2;
          const first = rdI32(ops, p); p += 4;
          const count = rdI32(ops, p); p += 4;
          const primcount = rdI32(ops, p); p += 4;
          gl.drawArraysInstanced(mode, first, count, primcount);
          break;
        }
        case GLOP.DRAW_ELEMENTS_INSTANCED: {
          const mode = rdU16(ops, p); p += 2;
          const count = rdI32(ops, p); p += 4;
          const ty = rdU16(ops, p); p += 2;
          const offset = rdI32(ops, p); p += 4;
          const primcount = rdI32(ops, p); p += 4;
          gl.drawElementsInstanced(mode, count, ty, offset, primcount);
          break;
        }
        case GLOP.VERTEX_ATTRIB_NFV: {
          const index = rdU32(ops, p); p += 4;
          const nfloats = rdU32(ops, p); p += 4;
          const range = rdU32(ops, p); p += 4;
          const data = rangeBytes(bytes, byteRanges, range);
          const f = [0, 0, 0, 0];
          for (let i = 0; i < 4 && i * 4 + 4 <= data.length; i++) {
            f[i] = rdF32(data, i * 4);
          }
          if (nfloats === 1) gl.vertexAttrib1f(index, f[0]!);
          else if (nfloats === 2) gl.vertexAttrib2f(index, f[0]!, f[1]!);
          else if (nfloats === 3) gl.vertexAttrib3f(index, f[0]!, f[1]!, f[2]!);
          else gl.vertexAttrib4f(index, f[0]!, f[1]!, f[2]!, f[3]!);
          break;
        }
        case GLOP.UNIFORM_BLOCK_BINDING: {
          const pid = rdU32(ops, p); p += 4;
          const blockIndex = rdU32(ops, p); p += 4;
          const binding = rdU32(ops, p); p += 4;
          const key = this.programKeyFor(t, pid);
          const entry = t.programCache.get(key);
          if (!entry) throw new GlStreamError(`uniformBlockBinding on uncompiled program ${pid}`);
          gl.uniformBlockBinding(entry.program, blockIndex, binding);
          break;
        }
        case GLOP.BIND_ATTRIB_LOCATION: {
          const pid = rdU32(ops, p); p += 4;
          const index = rdU32(ops, p); p += 4;
          const nameRange = rdU32(ops, p); p += 4;
          const name = new TextDecoder().decode(rangeBytes(bytes, byteRanges, nameRange));
          // Effective before link: the real program is compiled lazily at
          // USE, which replays this table.
          const binds = t.programAttribBinds.get(pid) ?? [];
          binds.push([index, name]);
          t.programAttribBinds.set(pid, binds);
          break;
        }
        case GLOP.RENDERBUFFER_CREATE: {
          const h = rdU32(ops, p); p += 4;
          if (!t.handles.renderbuffers.has(h)) t.handles.renderbuffers.set(h, gl.createRenderbuffer());
          break;
        }
        case GLOP.BIND_RENDERBUFFER: {
          const target = rdU16(ops, p); p += 2;
          const h = rdU32(ops, p); p += 4;
          let rb: WebGLRenderbuffer | null = null;
          t.client.rbBinds = t.client.rbBinds.filter(([bt]) => bt !== target);
          if (h !== NULL_HANDLE) {
            rb = t.handles.renderbuffers.get(h) ?? gl.createRenderbuffer();
            if (!t.handles.renderbuffers.has(h)) t.handles.renderbuffers.set(h, rb);
            t.client.rbBinds.push([target, h]);
          }
          gl.bindRenderbuffer(target, rb);
          break;
        }
        case GLOP.RENDERBUFFER_STORAGE: {
          p += 4; // handle echo; target resolved from the client bind shadow
          const internal = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          gl.renderbufferStorage(RENDERBUFFER, internal, w, h);
          break;
        }
        case GLOP.RENDERBUFFER_STORAGE_MS: {
          const samples = rdU32(ops, p); p += 4;
          const internal = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          gl.renderbufferStorageMultisample(RENDERBUFFER, samples, internal, w, h);
          break;
        }
        case GLOP.FRAMEBUFFER_RENDERBUFFER: {
          const target = rdU16(ops, p); p += 2;
          const attachment = rdU16(ops, p); p += 2;
          const h = rdU32(ops, p); p += 4;
          const rb = h !== NULL_HANDLE
            ? (t.handles.renderbuffers.get(h) ?? gl.createRenderbuffer())
            : null;
          if (h !== NULL_HANDLE && !t.handles.renderbuffers.has(h)) {
            t.handles.renderbuffers.set(h, rb!);
          }
          gl.framebufferRenderbuffer(target, attachment, RENDERBUFFER, rb);
          break;
        }
        case GLOP.TEX_STORAGE_2D: {
          const target = rdU16(ops, p); p += 2;
          const levels = ops[p]!; p += 1;
          const internal = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          gl.texStorage2D(target, levels, internal, w, h);
          break;
        }
        case GLOP.TEX_STORAGE_3D: {
          const target = rdU16(ops, p); p += 2;
          const levels = ops[p]!; p += 1;
          const internal = rdI32(ops, p); p += 4;
          const w = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          const d = rdU32(ops, p); p += 4;
          gl.texStorage3D(target, levels, internal, w, h, d);
          break;
        }
        case GLOP.BIND_BUFFER_BASE: {
          const target = rdU16(ops, p); p += 2;
          const index = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          let buf: WebGLBuffer | null = null;
          if (h !== NULL_HANDLE) {
            buf = t.handles.buffers.get(h) ?? gl.createBuffer();
            if (!t.handles.buffers.has(h)) t.handles.buffers.set(h, buf);
          }
          gl.bindBufferBase(target, index, buf);
          break;
        }
        case GLOP.BIND_BUFFER_RANGE: {
          const target = rdU16(ops, p); p += 2;
          const index = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          const offset = rdI32(ops, p); p += 4;
          const size = rdI32(ops, p); p += 4;
          let buf: WebGLBuffer | null = null;
          if (h !== NULL_HANDLE) {
            buf = t.handles.buffers.get(h) ?? gl.createBuffer();
            if (!t.handles.buffers.has(h)) t.handles.buffers.set(h, buf);
          }
          gl.bindBufferRange(target, index, buf, offset, size);
          break;
        }
        case GLOP.DRAW_BUFFERS: {
          const n = ops[p]!; p += 1;
          const modes: number[] = [];
          for (let i = 0; i < n; i++) {
            modes.push(rdU32(ops, p)); p += 4;
          }
          t.client.drawBuffers = modes;
          gl.drawBuffers(modes);
          break;
        }
        case GLOP.SAMPLER_CREATE: {
          const h = rdU32(ops, p); p += 4;
          if (!t.handles.samplers.has(h)) t.handles.samplers.set(h, gl.createSampler());
          break;
        }
        case GLOP.SAMPLER_DELETE: {
          const h = rdU32(ops, p); p += 4;
          const smp = t.handles.samplers.get(h);
          if (smp) {
            gl.deleteSampler(smp);
            t.handles.samplers.delete(h);
          }
          break;
        }
        case GLOP.SAMPLER_BIND: {
          const unit = rdU32(ops, p); p += 4;
          const h = rdU32(ops, p); p += 4;
          let smp: WebGLSampler | null = null;
          if (h !== NULL_HANDLE) {
            smp = t.handles.samplers.get(h) ?? gl.createSampler();
            if (!t.handles.samplers.has(h)) t.handles.samplers.set(h, smp);
          }
          gl.bindSampler(unit, smp);
          break;
        }
        case GLOP.SAMPLER_PARAM_I: {
          const h = rdU32(ops, p); p += 4;
          const pname = rdU16(ops, p); p += 2;
          const value = rdI32(ops, p); p += 4;
          const smp = t.handles.samplers.get(h);
          if (smp) gl.samplerParameteri(smp, pname, value);
          break;
        }
        case GLOP.SAMPLER_PARAM_F: {
          const h = rdU32(ops, p); p += 4;
          const pname = rdU16(ops, p); p += 2;
          const value = rdF32(ops, p); p += 4;
          const smp = t.handles.samplers.get(h);
          if (smp) gl.samplerParameterf(smp, pname, value);
          break;
        }
        case GLOP.CLEAR_BUFFER_FV: {
          const target = rdU16(ops, p); p += 2;
          const drawbuffer = rdI32(ops, p); p += 4;
          const v = [rdF32(ops, p), rdF32(ops, p + 4), rdF32(ops, p + 8), rdF32(ops, p + 12)]; p += 16;
          gl.clearBufferfv(target, drawbuffer, v);
          break;
        }
        case GLOP.CLEAR_BUFFER_IV: {
          const target = rdU16(ops, p); p += 2;
          const drawbuffer = rdI32(ops, p); p += 4;
          const v = [rdI32(ops, p), rdI32(ops, p + 4), rdI32(ops, p + 8), rdI32(ops, p + 12)]; p += 16;
          gl.clearBufferiv(target, drawbuffer, v);
          break;
        }
        case GLOP.CLEAR_BUFFER_FI: {
          const target = rdU16(ops, p); p += 2;
          const drawbuffer = rdI32(ops, p); p += 4;
          const depth = rdF32(ops, p); p += 4;
          const stencil = rdI32(ops, p); p += 4;
          gl.clearBufferfi(target, drawbuffer, depth, stencil);
          break;
        }
        case GLOP.BLIT_FRAMEBUFFER: {
          const sx0 = rdI32(ops, p); p += 4;
          const sy0 = rdI32(ops, p); p += 4;
          const sx1 = rdI32(ops, p); p += 4;
          const sy1 = rdI32(ops, p); p += 4;
          const dx0 = rdI32(ops, p); p += 4;
          const dy0 = rdI32(ops, p); p += 4;
          const dx1 = rdI32(ops, p); p += 4;
          const dy1 = rdI32(ops, p); p += 4;
          const mask = rdU16(ops, p); p += 2;
          const filter = rdU16(ops, p); p += 2;
          let glMask = 0;
          if (mask & 1) glMask |= gl.COLOR_BUFFER_BIT;
          if (mask & 2) glMask |= gl.DEPTH_BUFFER_BIT;
          if (mask & 4) glMask |= gl.STENCIL_BUFFER_BIT;
          gl.blitFramebuffer(sx0, sy0, sx1, sy1, dx0, dy0, dx1, dy1, glMask, filter);
          break;
        }
        default:
          throw new GlStreamError(`unknown gl opcode ${op} @ ${start}`);
      }
      // Fixed-length command stride check (variable commands self-account).
      const declared = GLOP_PAYLOAD_LEN[op];
      if (declared !== undefined && declared >= 0) {
        const consumed = p - start - 1;
        if (consumed !== declared) {
          throw new GlStreamError(
            `gl opcode ${op} payload drift (table ${declared}, consumed ${consumed}) @ ${start}`,
          );
        }
      }
    }
  }

  // ── program / location resolution ───────────────────────────────────

  /// Resolve (or lazily compile) the program `pid` refers to; returns the
  /// content cache key.
  private programKeyFor(t: GlTarget, pid: number): string {
    const hit = t.programKeyByPid.get(pid);
    if (hit) return hit;
    const shaders = t.programShaders.get(pid);
    if (!shaders?.vs || !shaders?.fs) {
      throw new GlStreamError(`program ${pid} missing shaders`);
    }
    const vs = t.shaderSrc.get(shaders.vs);
    const fs = t.shaderSrc.get(shaders.fs);
    if (!vs || !fs) throw new GlStreamError(`program ${pid} missing shader sources`);
    const binds = t.programAttribBinds.get(pid) ?? [];
    // The attrib bind table participates in the key: same sources with
    // different binds must compile separately, or the first program's
    // location layout gets reused by the second.
    let key = `${fnv1a(vs)}|${fnv1a(fs)}`;
    if (binds.length > 0) {
      key += `|ab${fnv1a(new TextEncoder().encode(binds.map(([i, n]) => `${i}\u{1}${n}\u{2}`).join('')))}`;
    }
    if (!t.programCache.has(key)) {
      t.programCache.set(key, compileProgram(t.gl, vs, fs, binds));
    }
    t.programKeyByPid.set(pid, key);
    return key;
  }
}

// ── location resolution (per-stream byte pool passed explicitly) ──────

/// Scan-index (`index:N`) or bare-name attrib location, cached per program.
function attribLoc(
  t: GlTarget,
  bytes: Uint8Array,
  byteRanges: { start: number; len: number }[],
  programKey: string | null,
  nameRange: number,
): number | null {
  const name = new TextDecoder().decode(rangeBytes(bytes, byteRanges, nameRange));
  // `index:N` is the recorder's getAttribLocation return value — the
  // reflection session's driver-assigned location (`l`). vertexAttrib* is
  // VAO state, valid before any useProgram; use it directly (engine
  // attrib_loc 同语义：不查 active-attrib 枚举序，驱动序 ≠ 枚举序).
  if (name.startsWith('index:')) {
    return Number(name.slice(6));
  }
  const entry = requireEntry(t, programKey);
  return resolveAttrib(t, entry, name);
}

function uniformLoc(
  t: GlTarget,
  bytes: Uint8Array,
  byteRanges: { start: number; len: number }[],
  programKey: string | null,
  nameRange: number,
): WebGLUniformLocation | null {
  const name = new TextDecoder().decode(rangeBytes(bytes, byteRanges, nameRange));
  const entry = requireEntry(t, programKey);
  let loc = entry.uniformLoc.get(name);
  if (loc === undefined) {
    loc = t.gl.getUniformLocation(entry.program, name);
    entry.uniformLoc.set(name, loc);
  }
  return loc ?? null;
}

/// Declared type of a uniform; `arr[3]`-style names resolve via bare name.
function uniformType(
  t: GlTarget,
  bytes: Uint8Array,
  byteRanges: { start: number; len: number }[],
  programKey: string | null,
  nameRange: number,
): number | undefined {
  const name = new TextDecoder().decode(rangeBytes(bytes, byteRanges, nameRange));
  const entry = requireEntry(t, programKey);
  const direct = entry.uniformTypes.get(name);
  if (direct !== undefined) return direct;
  const m = name.match(/^(.*)\[\d+\]$/);
  return m ? entry.uniformTypes.get(m[1]!) : undefined;
}

// ── helpers ────────────────────────────────────────────────────────────

const EMPTY = new Uint8Array(0);

function requireEntry(t: GlTarget, programKey: string | null): ProgramEntry {
  if (!programKey) throw new GlStreamError('uniform/attrib command before USE_PROGRAM');
  const entry = t.programCache.get(programKey);
  if (!entry) throw new GlStreamError('program cache miss');
  return entry;
}

function resolveAttrib(t: GlTarget, entry: ProgramEntry, name: string): number | null {
  let loc = entry.attribLoc.get(name);
  if (loc === undefined) {
    loc = t.gl.getAttribLocation(entry.program, name);
    entry.attribLoc.set(name, loc);
  }
  return loc === null ? null : loc;
}

function boundTexture(t: GlTarget, target: number): number | null {
  const { texBinds, activeUnit } = t.client;
  for (let i = texBinds.length - 1; i >= 0; i--) {
    const [unit, bt, h] = texBinds[i]!;
    if (unit === activeUnit && bt === target) return h;
  }
  return null;
}

function boundBuffer(t: GlTarget, target: number): number | null {
  return target === ARRAY_BUFFER ? t.client.arrayBuffer : t.client.curElement;
}

/// Cube faces upload under their face target but bind/parametrize on
/// TEXTURE_CUBE_MAP (passing a face target to bind/texParameter is
/// INVALID_ENUM).
function textureBindTarget(target: number): number {
  if (target >= TEXTURE_CUBE_MAP_POSITIVE_X && target <= TEXTURE_CUBE_MAP_NEGATIVE_Z) {
    return TEXTURE_CUBE_MAP;
  }
  return target;
}

function flipIfUnpackY(t: GlTarget, data: Uint8Array, height: number): Uint8Array {
  if (t.unpackFlipY) flipRowsVertical(data, height);
  return data;
}

function flipRowsVertical(bytes: Uint8Array, height: number): void {
  const h = height;
  if (h <= 1 || bytes.length === 0) return;
  const row = Math.floor(bytes.length / h);
  if (row === 0) return;
  const tmp = new Uint8Array(row);
  for (let i = 0; i < Math.floor(h / 2); i++) {
    const top = i * row;
    const bottom = (h - 1 - i) * row;
    tmp.set(bytes.subarray(top, top + row));
    bytes.copyWithin(top, bottom, bottom + row);
    bytes.set(tmp, bottom);
  }
}

function rangeBytes(bytes: Uint8Array, ranges: { start: number; len: number }[], range: number): Uint8Array {
  const r = ranges[range];
  if (!r) throw new GlStreamError(`bad byte range ${range}`);
  return bytes.subarray(r.start, r.start + r.len);
}

function tailF32(ops: Uint8Array, at: number, count: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < count; i++) out.push(rdF32(ops, at + i * 4));
  return out;
}

function tailI32(ops: Uint8Array, at: number, count: number): number[] {
  const out: number[] = [];
  for (let i = 0; i < count; i++) out.push(rdI32(ops, at + i * 4));
  return out;
}

function rdU16(ops: Uint8Array, at: number): number {
  return ops[at]! | (ops[at + 1]! << 8);
}

function rdU32(ops: Uint8Array, at: number): number {
  return (ops[at]! | (ops[at + 1]! << 8) | (ops[at + 2]! << 16) | (ops[at + 3]! << 24)) >>> 0;
}

function rdI32(ops: Uint8Array, at: number): number {
  return ops[at]! | (ops[at + 1]! << 8) | (ops[at + 2]! << 16) | (ops[at + 3]! << 24);
}

// Typed-array views would demand 4-byte alignment the in-stream offset does
// not guarantee (odd-length commands shift every later field) — assemble
// bytes through a shared scratch DataView instead.
const SCRATCH = new ArrayBuffer(4);
const SCRATCH_VIEW = new DataView(SCRATCH);

function rdF32(ops: Uint8Array, at: number): number {
  SCRATCH_VIEW.setUint8(0, ops[at]!);
  SCRATCH_VIEW.setUint8(1, ops[at + 1]!);
  SCRATCH_VIEW.setUint8(2, ops[at + 2]!);
  SCRATCH_VIEW.setUint8(3, ops[at + 3]!);
  return SCRATCH_VIEW.getFloat32(0, true);
}

function compileProgram(
  gl: WebGL2RenderingContext,
  vsSrc: Uint8Array,
  fsSrc: Uint8Array,
  attribBinds: [number, string][],
): ProgramEntry {
  const decode = (src: Uint8Array) => new TextDecoder().decode(src);
  const compile = (ty: number, src: string): WebGLShader => {
    const sh = gl.createShader(ty)!;
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(sh);
      throw new GlStreamError(`shader compile failed: ${log}`);
    }
    return sh;
  };
  const vs = compile(gl.VERTEX_SHADER, decode(vsSrc));
  const fs = compile(gl.FRAGMENT_SHADER, decode(fsSrc));
  const program = gl.createProgram()!;
  gl.attachShader(program, vs);
  gl.attachShader(program, fs);
  gl.deleteShader(vs);
  gl.deleteShader(fs);
  // Replay link-pending bindAttribLocation requests in call order — the
  // caller issues vertexAttribPointer with these fixed slots.
  for (const [index, name] of attribBinds) {
    gl.bindAttribLocation(program, index, name);
  }
  gl.linkProgram(program);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program);
    throw new GlStreamError(`program link failed: ${log}`);
  }
  // Attrib names in driver enumeration order — the same order core's
  // getAttribLocation reflection (`index:N`) was built from.
  const attribNames: string[] = [];
  const nAttribs = gl.getProgramParameter(program, gl.ACTIVE_ATTRIBUTES) as number;
  for (let i = 0; i < nAttribs; i++) {
    const info = gl.getActiveAttrib(program, i);
    if (info) attribNames.push(info.name);
  }
  const uniformTypes = new Map<string, number>();
  const nUniforms = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS) as number;
  for (let i = 0; i < nUniforms; i++) {
    const info = gl.getActiveUniform(program, i);
    if (!info) continue;
    if (info.name.endsWith('[0]')) {
      uniformTypes.set(info.name.slice(0, -3), info.type);
    }
    uniformTypes.set(info.name, info.type);
  }
  return {
    program,
    attribNames,
    attribLoc: new Map(),
    uniformLoc: new Map(),
    uniformTypes,
  };
}
