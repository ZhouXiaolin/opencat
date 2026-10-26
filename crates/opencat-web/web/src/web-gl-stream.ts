// WebGL2 stream replay executor: decodes the OCIR GPU_STREAMS section (13)
// and replays each stream's commands one-for-one onto a real WebGL2 context
// (OffscreenCanvas per target canvas). The browser IS the WebGL semantics
// standard — replay is a direct mapping, no translation.
//
// Output contract (same as the Scene3 era): each stream renders into its own
// framebuffer; `readPixels` hands straight-alpha RGBA to
// `provideGeneratedImageRgba(imageId, w, h, rgba)` so the 2D CanvasKit replay
// composites it at the placeholder's z-position.
//
// Shader programs are compiled lazily at USE_PROGRAM from the stream's
// SHADER_SOURCE records (three streams the final GLSL — no template system to
// replay). Cross-frame content caches skip re-compiles and re-uploads of
// unchanged sources / buffers / textures.

import {
  GLOP,
  GLOP_PAYLOAD_LEN,
  SECTION,
  IR_MAGIC,
  IR_VERSION,
} from './generated/ocir-schema.generated';
import { provideGeneratedImageRgba } from './draw-ir';

export type GpuStreamDecoded = {
  imageId: bigint;
  targetId: string;
  width: number;
  height: number;
  backend: number; // 0 = WebGL2, 1 = WebGPU (reserved)
  attrs: {
    alpha: boolean;
    antialias: boolean;
    premultipliedAlpha: boolean;
    preserveDrawingBuffer: boolean;
    depth: boolean;
    stencil: boolean;
  };
  ops: Uint8Array;
  bytes: Uint8Array;
  byteRanges: { start: number; len: number }[];
};

const NULL_HANDLE = 0xffffffff;

// ── section 13 decode ───────────────────────────────────────────────

export function decodeGpuStreams(bytes: Uint8Array): GpuStreamDecoded[] {
  if (bytes.byteLength < 4) throw new Error('GPU_STREAMS section truncated');
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const count = view.getUint32(0, true);
  let p = 4;
  const streams: GpuStreamDecoded[] = [];
  const decoder = new TextDecoder();
  for (let i = 0; i < count; i++) {
    const imageId = view.getBigUint64(p, true);
    p += 8;
    const targetLen = view.getUint32(p, true);
    p += 4;
    const targetId = decoder.decode(bytes.subarray(p, p + targetLen));
    p += targetLen;
    const width = view.getUint32(p, true);
    p += 4;
    const height = view.getUint32(p, true);
    p += 4;
    const backend = bytes[p];
    p += 1;
    const attrBits = bytes[p];
    p += 1;
    const opsLen = view.getUint32(p, true);
    p += 4;
    const ops = bytes.subarray(p, p + opsLen);
    p += opsLen;
    const bytesLen = view.getUint32(p, true);
    p += 4;
    const streamBytes = bytes.subarray(p, p + bytesLen);
    p += bytesLen;
    const rangeCount = view.getUint32(p, true);
    p += 4;
    const byteRanges: { start: number; len: number }[] = [];
    for (let r = 0; r < rangeCount; r++) {
      byteRanges.push({ start: view.getUint32(p, true), len: view.getUint32(p + 4, true) });
      p += 8;
    }
    streams.push({
      imageId,
      targetId,
      width,
      height,
      backend,
      attrs: {
        alpha: (attrBits & 1) !== 0,
        antialias: (attrBits & 2) !== 0,
        premultipliedAlpha: (attrBits & 4) !== 0,
        preserveDrawingBuffer: (attrBits & 8) !== 0,
        depth: (attrBits & 16) !== 0,
        stencil: (attrBits & 32) !== 0,
      },
      ops,
      bytes: streamBytes,
      byteRanges,
    });
  }
  return streams;
}

/** Detect + decode the GPU_STREAMS section from a raw OCIR envelope (or null). */
export function decodeGpuStreamsFromEnvelope(envelope: Uint8Array): GpuStreamDecoded[] | null {
  if (envelope.byteLength < 12) return null;
  const view = new DataView(envelope.buffer, envelope.byteOffset, envelope.byteLength);
  if (
    envelope[0] !== IR_MAGIC[0] ||
    envelope[1] !== IR_MAGIC[1] ||
    envelope[2] !== IR_MAGIC[2] ||
    envelope[3] !== IR_MAGIC[3]
  ) {
    return null;
  }
  if (view.getUint32(4, true) !== IR_VERSION) return null;
  const sectionCount = view.getUint32(8, true);
  for (let i = 0; i < sectionCount; i++) {
    const base = 12 + i * 12;
    if (view.getUint32(base, true) !== SECTION.GPU_STREAMS) continue;
    const offset = view.getUint32(base + 4, true);
    const len = view.getUint32(base + 8, true);
    return decodeGpuStreams(envelope.subarray(offset, offset + len));
  }
  return null;
}

// ── per-target live contexts ────────────────────────────────────────

type LiveContext = { canvas: OffscreenCanvas; gl: WebGL2RenderingContext };

const liveContexts = new Map<string, LiveContext>();

// core wasm 领域初始化时用 2d-only 的 shim 覆盖 globalThis.OffscreenCanvas 与
// document（dom_api.js / canvas_api.js）。模块加载先于 initWasm，这里捕获
// 原始引用，replay 永远用真 DOM。
const PRISTINE = {
  OffscreenCanvas: typeof OffscreenCanvas !== 'undefined' ? OffscreenCanvas : undefined,
  document: typeof document !== 'undefined' ? document : undefined,
};

function acquireContext(
  targetId: string,
  w: number,
  h: number,
  attrs: GpuStreamDecoded['attrs'],
): LiveContext {
  let live = liveContexts.get(targetId);
  if (!live) {
    // OffscreenCanvas 在部分宿主（oracle 页的桩、老内核）只开放 2d：
    // 回退到不入 DOM 的 HTMLCanvasElement，replay 只做 draw + readPixels，
    // 两者行为一致。
    const attrsGL = {
      alpha: attrs.alpha,
      antialias: attrs.antialias,
      premultipliedAlpha: attrs.premultipliedAlpha,
      preserveDrawingBuffer: true,
      depth: attrs.depth,
      stencil: attrs.stencil,
    } as WebGLContextAttributes;
    let canvas: OffscreenCanvas | HTMLCanvasElement;
    let gl: WebGL2RenderingContext | null;
    if (PRISTINE.OffscreenCanvas) {
      canvas = new PRISTINE.OffscreenCanvas(w, h);
      gl = canvas.getContext('webgl2', attrsGL) as WebGL2RenderingContext | null;
    } else {
      if (!PRISTINE.document) throw new Error(`web-gl-stream: no DOM for '${targetId}'`);
      canvas = PRISTINE.document.createElement('canvas');
      canvas.width = w;
      canvas.height = h;
      gl = canvas.getContext('webgl2', attrsGL) as WebGL2RenderingContext | null;
    }
    if (!gl) throw new Error(`web-gl-stream: WebGL2 unavailable for '${targetId}'`);
    live = { canvas, gl };
    liveContexts.set(targetId, live);
  }
  if (live.canvas.width !== w || live.canvas.height !== h) {
    live.canvas.width = w;
    live.canvas.height = h;
  }
  return live;
}

// ── cross-frame content caches ──────────────────────────────────────

function hashBytes(bytes: Uint8Array): string {
  // FNV-1a — content-addressed reuse, not security.
  let h = 0x811c9dc5;
  for (let i = 0; i < bytes.length; i++) {
    h ^= bytes[i];
    h = Math.imul(h, 0x01000193);
  }
  return `${bytes.length}:${(h >>> 0).toString(16)}`;
}

const programCache = new Map<string, WebGLProgram>();
const textureCache = new Map<string, WebGLTexture>();

// ── replay ──────────────────────────────────────────────────────────

export function executeGpuStreams(streams: GpuStreamDecoded[]): void {
  for (const stream of streams) {
    if (stream.backend !== 0) {
      throw new Error(`web-gl-stream: unsupported backend ${stream.backend} (WebGPU is reserved)`);
    }
    executeStream(stream);
  }
}

function executeStream(stream: GpuStreamDecoded): void {
  const { gl } = acquireContext(stream.targetId, stream.width, stream.height, stream.attrs);

  // Per-stream replay state.
  const shaderSource = new Map<number, Uint8Array>();
  const shaderStage = new Map<number, number>();
  const programShaders = new Map<number, { v?: number; f?: number }>();
  const programByHandle = new Map<number, WebGLProgram>();
  let currentProgram: WebGLProgram | null = null;
  const attribLoc = new Map<number, number>();
  const bufferByHandle = new Map<number, WebGLBuffer>();
  const vaoByHandle = new Map<number, WebGLVertexArrayObject>();
  const textureByHandle = new Map<number, WebGLTexture>();
  const fboByHandle = new Map<number, WebGLFramebuffer>();
  const renderbufferByHandle = new Map<number, WebGLRenderbuffer>();
  let lastBoundBuffer: number | null = null;
  let lastBoundTexture: number | null = null;
  let lastBoundTextureTarget = 0x0de1;
  let lastBoundRenderbuffer: number | null = null;

  const bufferEnsureTs = (handle: number): WebGLBuffer => {
    let buf = bufferByHandle.get(handle);
    if (!buf) {
      buf = gl.createBuffer()!;
      bufferByHandle.set(handle, buf);
    }
    return buf;
  };
  const rbEnsure = (handle: number): WebGLRenderbuffer => {
    let rb = renderbufferByHandle.get(handle);
    if (!rb) {
      rb = gl.createRenderbuffer()!;
      renderbufferByHandle.set(handle, rb);
    }
    return rb;
  };

  const dv = new DataView(stream.ops.buffer, stream.ops.byteOffset, stream.ops.byteLength);
  let p = 0;
  let opStart = 0;

  const u8 = () => dv.getUint8((opStart = p++));
  const u16 = () => {
    const v = dv.getUint16(p, true);
    p += 2;
    return v;
  };
  const i32 = () => {
    const v = dv.getInt32(p, true);
    p += 4;
    return v;
  };
  const u32 = () => {
    const v = dv.getUint32(p, true);
    p += 4;
    return v;
  };
  const f32 = () => {
    const v = dv.getFloat32(p, true);
    p += 4;
    return v;
  };

  const rangeBytes = (rangeId: number): Uint8Array => {
    const range = stream.byteRanges[rangeId];
    if (!range) throw new Error(`web-gl-stream: bad byte range ${rangeId}`);
    return stream.bytes.subarray(range.start, range.start + range.len);
  };
  const rangeString = (rangeId: number): string => new TextDecoder().decode(rangeBytes(rangeId));

  // Attribute addressing: the recording face's getAttribLocation returns the
  // scan-order index of the program's vertex attributes; three caches that
  // number and streams it back. The replay resolves index → name with THE
  // SAME scan rule as core (declaration order in the vertex source) and then
  // to the driver location. Scan-order drift would silently mis-bind — the
  // regexes below must mirror webgl_api.js collectDecls verbatim.
  const programAttribNames = new Map<WebGLProgram, string[]>();
  const attribIndex = (rangeId: number): number => {
    if (!currentProgram) throw new Error('web-gl-stream: attrib command before USE_PROGRAM');
    const name = rangeString(rangeId);
    if (!name.startsWith('index:')) {
      // Named form (direct); resolve against the driver once.
      let loc = attribLocByName.get(name);
      if (loc === undefined) {
        loc = gl.getAttribLocation(currentProgram, name);
        attribLocByName.set(name, loc);
      }
      return loc;
    }
    const index = Number(name.slice(6));
    let names = programAttribNames.get(currentProgram);
    if (!names) {
      names = scanAttribNames(programSourceByKey.get(programKey(currentProgram)) ?? '');
      programAttribNames.set(currentProgram, names);
    }
    if (index >= names.length) {
      throw new Error(`web-gl-stream: attrib index ${index} out of scan range ${names.length}`);
    }
    const real = names[index];
    let loc = attribLocByName.get(real);
    if (loc === undefined) {
      loc = gl.getAttribLocation(currentProgram, real);
      attribLocByName.set(real, loc);
    }
    return loc;
  };
  const attribLocByName = new Map<string, number>();

  const uniformLoc = (rangeId: number): WebGLUniformLocation | null => {
    if (!currentProgram) throw new Error('web-gl-stream: uniform command before USE_PROGRAM');
    return gl.getUniformLocation(currentProgram, rangeString(rangeId));
  };

  const programSourceByKey = new Map<string, string>();
  const compileProgram = (pid: number): WebGLProgram => {
    const ids = programShaders.get(pid);
    if (!ids || ids.v === undefined || ids.f === undefined) {
      throw new Error(`web-gl-stream: program ${pid} missing shaders at USE`);
    }
    const vsBytes = shaderSource.get(ids.v);
    const fsBytes = shaderSource.get(ids.f);
    if (!vsBytes || !fsBytes) throw new Error(`web-gl-stream: program ${pid} missing sources`);
    const key = hashBytes(vsBytes) + '|' + hashBytes(fsBytes);
    programSourceByKey.set(key, new TextDecoder().decode(vsBytes));
    let program = programCache.get(key);
    if (!program) {
      program = gl.createProgram()!;
      const compile = (type: number, src: string) => {
        const sh = gl.createShader(type)!;
        gl.shaderSource(sh, src);
        gl.compileShader(sh);
        if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
          const log = gl.getShaderInfoLog(sh) ?? '';
          throw new Error(`web-gl-stream: shader compile failed: ${log}`);
        }
        return sh;
      };
      const vs = compile(gl.VERTEX_SHADER, new TextDecoder().decode(vsBytes));
      const fs = compile(gl.FRAGMENT_SHADER, new TextDecoder().decode(fsBytes));
      gl.attachShader(program, vs);
      gl.attachShader(program, fs);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
      gl.linkProgram(program);
      if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
        const log = gl.getProgramInfoLog(program) ?? '';
        throw new Error(`web-gl-stream: program link failed: ${log}`);
      }
      programCache.set(key, program);
    }
    return program;
  };

  while (p < stream.ops.byteLength) {
    const opcode = u8();
    opStart = p - 1;
    const start = p;
    switch (opcode) {
      case GLOP.SHADER_CREATE: {
        const stage = u16();
        const handle = u32();
        shaderStage.set(handle, stage);
        break;
      }
      case GLOP.SHADER_SOURCE: {
        const handle = u32();
        shaderSource.set(handle, rangeBytes(u32()).slice());
        break;
      }
      case GLOP.COMPILE_SHADER:
      case GLOP.PROGRAM_CREATE: {
        u32();
        break;
      }
      case GLOP.PROGRAM_ATTACH: {
        const pid = u32();
        const sid = u32();
        const entry = programShaders.get(pid) ?? {};
        const stage = shaderStage.get(sid);
        if (stage === 0x8b31) entry.v = sid;
        else if (stage === 0x8b30) entry.f = sid;
        programShaders.set(pid, entry);
        break;
      }
      case GLOP.PROGRAM_LINK:
      case GLOP.PROGRAM_DELETE:
      case GLOP.SHADER_DELETE: {
        u32();
        break;
      }
      case GLOP.PROGRAM_USE: {
        const pid = u32();
        currentProgram = pid === NULL_HANDLE ? null : compileProgram(pid);
        if (currentProgram) programByHandle.set(pid, currentProgram);
        // 光记句柄不绑定的话 draw 会落到固定管线（兼容模式白/黑帧）。
        gl.useProgram(currentProgram);
        break;
      }
      case GLOP.BUFFER_CREATE: {
        const handle = u32();
        if (!bufferByHandle.has(handle)) bufferByHandle.set(handle, gl.createBuffer()!);
        break;
      }
      case GLOP.BUFFER_BIND: {
        const target = u16();
        const handle = u32();
        lastBoundBuffer = handle;
        gl.bindBuffer(target, handle === NULL_HANDLE ? null : bufferByHandle.get(handle)!);
        break;
      }
      case GLOP.BUFFER_DATA: {
        const target = u16();
        const usage = u16();
        const bytes = rangeBytes(u32());
        const buf = bufferByHandle.get(lastBoundBuffer ?? NULL_HANDLE);
        if (!buf) throw new Error('web-gl-stream: BUFFER_DATA without a bound buffer');
        gl.bindBuffer(target, buf);
        gl.bufferData(target, bytes, usage);
        break;
      }
      case GLOP.BUFFER_DATA_NULL: {
        const target = u16();
        const usage = u16();
        const size = u32();
        const buf = bufferByHandle.get(lastBoundBuffer ?? NULL_HANDLE);
        if (!buf) throw new Error('web-gl-stream: BUFFER_DATA_NULL without a bound buffer');
        gl.bindBuffer(target, buf);
        gl.bufferData(target, size, usage);
        break;
      }
      case GLOP.BUFFER_DELETE: {
        const handle = u32();
        const buf = bufferByHandle.get(handle);
        if (buf) gl.deleteBuffer(buf);
        break;
      }
      case GLOP.VAO_CREATE: {
        const handle = u32();
        if (!vaoByHandle.has(handle)) vaoByHandle.set(handle, gl.createVertexArray()!);
        break;
      }
      case GLOP.VAO_BIND: {
        const handle = u32();
        gl.bindVertexArray(handle === NULL_HANDLE ? null : vaoByHandle.get(handle)!);
        break;
      }
      case GLOP.VAO_DELETE: {
        const handle = u32();
        const vao = vaoByHandle.get(handle);
        if (vao) gl.deleteVertexArray(vao);
        break;
      }
      case GLOP.ATTRIB_ENABLE: {
        gl.enableVertexAttribArray(attribIndex(u32()));
        break;
      }
      case GLOP.ATTRIB_DISABLE: {
        gl.disableVertexAttribArray(attribIndex(u32()));
        break;
      }
      case GLOP.ATTRIB_POINTER: {
        const range = u32();
        const size = u8();
        const type = u16();
        const normalized = u8();
        const stride = u32();
        const offset = u32();
        gl.vertexAttribPointer(attribIndex(range), size, type, normalized !== 0, stride, offset);
        break;
      }
      case GLOP.ATTRIB_DIVISOR: {
        const range = u32();
        const divisor = u32();
        gl.vertexAttribDivisor(attribIndex(range), divisor);
        break;
      }
      case GLOP.UNIFORM_1F: {
        const range = u32();
        gl.uniform1f(uniformLoc(range), f32());
        break;
      }
      case GLOP.UNIFORM_2F: {
        const range = u32();
        const x = f32();
        gl.uniform2f(uniformLoc(range), x, f32());
        break;
      }
      case GLOP.UNIFORM_3F: {
        const range = u32();
        const x = f32();
        const y = f32();
        gl.uniform3f(uniformLoc(range), x, y, f32());
        break;
      }
      case GLOP.UNIFORM_4F: {
        const range = u32();
        const x = f32();
        const y = f32();
        const z = f32();
        gl.uniform4f(uniformLoc(range), x, y, z, f32());
        break;
      }
      case GLOP.UNIFORM_1I: {
        const range = u32();
        gl.uniform1i(uniformLoc(range), i32());
        break;
      }
      case GLOP.UNIFORM_NF: {
        const range = u32();
        const count = u32();
        gl.uniform1fv(uniformLoc(range), new Float32Array(stream.ops.buffer, stream.ops.byteOffset + p, count));
        p += count * 4;
        break;
      }
      case GLOP.UNIFORM_NI: {
        const range = u32();
        const count = u32();
        gl.uniform1iv(uniformLoc(range), new Int32Array(stream.ops.buffer, stream.ops.byteOffset + p, count));
        p += count * 4;
        break;
      }
      case GLOP.UNIFORM_MATRIX_4F: {
        const range = u32();
        const count = u32();
        const order = u8();
        const elems = count * order * order;
        const tail = new Float32Array(stream.ops.buffer, stream.ops.byteOffset + p, elems);
        if (order === 2) gl.uniformMatrix2fv(uniformLoc(range), false, tail);
        else if (order === 3) gl.uniformMatrix3fv(uniformLoc(range), false, tail);
        else gl.uniformMatrix4fv(uniformLoc(range), false, tail);
        p += elems * 4;
        break;
      }
      case GLOP.TEXTURE_CREATE: {
        const handle = u32();
        if (!textureByHandle.has(handle)) textureByHandle.set(handle, gl.createTexture()!);
        break;
      }
      case GLOP.TEXTURE_DELETE: {
        const handle = u32();
        const tex = textureByHandle.get(handle);
        if (tex) gl.deleteTexture(tex);
        break;
      }
      case GLOP.TEXTURE_ACTIVE: {
        gl.activeTexture(gl.TEXTURE0 + u8());
        break;
      }
      case GLOP.TEXTURE_BIND: {
        const target = u16();
        const handle = u32();
        lastBoundTexture = handle;
        lastBoundTextureTarget = target;
        gl.bindTexture(
          target,
          handle === NULL_HANDLE ? null : textureByHandle.get(handle)!,
        );
        break;
      }
      case GLOP.TEX_PARAM_I: {
        const target = u16();
        const pname = u16();
        const value = i32();
        gl.texParameteri(target, pname, value);
        break;
      }
      case GLOP.TEX_PARAM_F: {
        const target = u16();
        const pname = u16();
        const value = f32();
        gl.texParameterf(target, pname, value);
        break;
      }
      case GLOP.TEX_IMAGE_2D: {
        const target = u16();
        const level = u8();
        const internal = i32();
        const w = u32();
        const h = u32();
        const format = u16();
        const type = u16();
        const hasData = u8();
        const bytes = hasData ? rangeBytes(u32()) : null;
        const tex = textureByHandle.get(lastBoundTexture ?? NULL_HANDLE);
        if (!tex) throw new Error('web-gl-stream: TEX_IMAGE_2D without a bound texture');
        // Content-addressed skip: unchanged uploads are a no-op (the range
        // interning in core already dedups transmission).
        const contentKey = `${hashBytes(bytes ?? new Uint8Array(0))}|${internal}|${w}x${h}|${format}|${type}`;
        if (!textureCache.has(contentKey)) {
          gl.bindTexture(lastBoundTextureTarget, tex);
          gl.texImage2D(target, level, internal, w, h, 0, format, type, bytes);
          textureCache.set(contentKey, tex);
        }
        break;
      }
      case GLOP.GENERATE_MIPMAP: {
        gl.generateMipmap(u16());
        break;
      }
      case GLOP.PIXEL_STORE_I: {
        const pname = u16();
        const value = i32();
        gl.pixelStorei(pname, value);
        break;
      }
      case GLOP.FRAMEBUFFER_CREATE: {
        const handle = u32();
        if (!fboByHandle.has(handle)) fboByHandle.set(handle, gl.createFramebuffer()!);
        break;
      }
      case GLOP.FRAMEBUFFER_DELETE: {
        const handle = u32();
        const fbo = fboByHandle.get(handle);
        if (fbo) gl.deleteFramebuffer(fbo);
        break;
      }
      case GLOP.BIND_FRAMEBUFFER: {
        const target = u16();
        const handle = u32();
        gl.bindFramebuffer(target, handle === NULL_HANDLE ? null : fboByHandle.get(handle)!);
        break;
      }
      case GLOP.FRAMEBUFFER_TEXTURE_2D: {
        const target = u16();
        const attachment = u16();
        const textarget = u16();
        const handle = u32();
        const level = i32();
        const tex = handle === NULL_HANDLE ? null : textureByHandle.get(handle);
        if (handle !== NULL_HANDLE && !tex) {
          throw new Error('web-gl-stream: FBO attach of unknown texture handle');
        }
        gl.framebufferTexture2D(target, attachment, textarget, tex ?? null, level);
        break;
      }
      case GLOP.VIEWPORT: {
        const x = i32();
        const y = i32();
        gl.viewport(x, y, i32(), i32());
        break;
      }
      case GLOP.CLEAR_COLOR: {
        const r = f32();
        const g = f32();
        const b = f32();
        gl.clearColor(r, g, b, f32());
        break;
      }
      case GLOP.CLEAR: {
        const bits = u8();
        gl.clear(
          (bits & 1 ? gl.COLOR_BUFFER_BIT : 0) |
            (bits & 2 ? gl.DEPTH_BUFFER_BIT : 0) |
            (bits & 4 ? gl.STENCIL_BUFFER_BIT : 0),
        );
        break;
      }
      case GLOP.ENABLE: {
        gl.enable(u16());
        break;
      }
      case GLOP.DISABLE: {
        gl.disable(u16());
        break;
      }
      case GLOP.DEPTH_FUNC: {
        gl.depthFunc(u16());
        break;
      }
      case GLOP.DEPTH_MASK: {
        gl.depthMask(u8() !== 0);
        break;
      }
      case GLOP.BLEND_FUNC: {
        const src = u16();
        gl.blendFunc(src, u16());
        break;
      }
      case GLOP.BLEND_EQUATION: {
        gl.blendEquation(u16());
        break;
      }
      case GLOP.BLEND_FUNC_SEPARATE: {
        const srcRgb = u16();
        const dstRgb = u16();
        const srcA = u16();
        gl.blendFuncSeparate(srcRgb, dstRgb, srcA, u16());
        break;
      }
      case GLOP.BLEND_EQUATION_SEPARATE: {
        const rgb = u16();
        gl.blendEquationSeparate(rgb, u16());
        break;
      }
      case GLOP.TEX_IMAGE_3D: {
        const target = u16();
        const level = u8();
        const internal = i32();
        const w = u32();
        const h = u32();
        const d = u32();
        const format = u16();
        const type = u16();
        const hasData = u8();
        const bytes = hasData ? rangeBytes(u32()) : null;
        const tex = textureByHandle.get(lastBoundTexture ?? NULL_HANDLE);
        if (!tex) throw new Error('web-gl-stream: TEX_IMAGE_3D without a bound texture');
        gl.bindTexture(lastBoundTextureTarget, tex);
        gl.texImage3D(target, level, internal, w, h, d, 0, format, type, bytes);
        break;
      }
      case GLOP.TEX_SUB_IMAGE_2D: {
        const target = u16();
        const level = u8();
        const x = i32();
        const y = i32();
        const w = u32();
        const h = u32();
        const format = u16();
        const type = u16();
        const hasData = u8();
        const bytes = hasData ? rangeBytes(u32()) : null;
        const tex = textureByHandle.get(lastBoundTexture ?? NULL_HANDLE);
        if (!tex) throw new Error('web-gl-stream: TEX_SUB_IMAGE_2D without a bound texture');
        gl.bindTexture(target, tex);
        gl.texSubImage2D(target, level, x, y, w, h, format, type, bytes);
        break;
      }
      case GLOP.RENDERBUFFER_CREATE: {
        const handle = u32();
        if (!renderbufferByHandle.has(handle)) {
          renderbufferByHandle.set(handle, gl.createRenderbuffer()!);
        }
        break;
      }
      case GLOP.BIND_RENDERBUFFER: {
        const target = u16();
        const handle = u32();
        const rb = handle === NULL_HANDLE ? null : rbEnsure(handle);
        if (rb) lastBoundRenderbuffer = handle;
        gl.bindRenderbuffer(target, rb);
        break;
      }
      case GLOP.RENDERBUFFER_STORAGE: {
        u32(); // handle 保留位：语义走最近绑定（GL 状态机）
        const internal = i32();
        const w = u32();
        gl.renderbufferStorage(gl.RENDERBUFFER, internal, w, u32());
        break;
      }
      case GLOP.FRAMEBUFFER_RENDERBUFFER: {
        const target = u16();
        const attachment = u16();
        const rbHandle = u32();
        const rb = rbHandle === NULL_HANDLE ? null : rbEnsure(rbHandle);
        gl.framebufferRenderbuffer(target, attachment, target, rb);
        break;
      }
      case GLOP.COLOR_MASK: {
        const r = u8();
        const g = u8();
        const b = u8();
        gl.colorMask(r !== 0, g !== 0, b !== 0, u8() !== 0);
        break;
      }
      case GLOP.UNIFORM_2I: {
        const loc = uniformLoc(u32());
        const x = i32();
        gl.uniform2i(loc, x, i32());
        break;
      }
      case GLOP.UNIFORM_3I: {
        const loc = uniformLoc(u32());
        const x = i32();
        const y = i32();
        gl.uniform3i(loc, x, y, i32());
        break;
      }
      case GLOP.UNIFORM_4I: {
        const loc = uniformLoc(u32());
        const x = i32();
        const y = i32();
        const z = i32();
        gl.uniform4i(loc, x, y, z, i32());
        break;
      }
      case GLOP.TEX_STORAGE_2D: {
        const target = u16();
        const levels = u8();
        const internal = i32();
        const w = u32();
        gl.texStorage2D(target, levels, internal, w, u32());
        break;
      }
      case GLOP.TEX_STORAGE_3D: {
        const target = u16();
        const levels = u8();
        const internal = i32();
        const w = u32();
        const h = u32();
        gl.texStorage3D(target, levels, internal, w, h, u32());
        break;
      }
      case GLOP.TEX_SUB_IMAGE_3D: {
        const target = u16();
        const level = u8();
        const x = i32();
        const y = i32();
        const z = i32();
        const w = u32();
        const h = u32();
        const d = u32();
        const format = u16();
        const type = u16();
        const hasData = u8();
        const bytes = hasData ? rangeBytes(u32()) : null;
        const tex = textureByHandle.get(lastBoundTexture ?? NULL_HANDLE);
        if (!tex) throw new Error('web-gl-stream: TEX_SUB_IMAGE_3D without a bound texture');
        gl.bindTexture(target, tex);
        gl.texSubImage3D(target, level, x, y, z, w, h, d, format, type, bytes);
        break;
      }
      case GLOP.BIND_BUFFER_BASE: {
        const target = u16();
        const index = u32();
        const handle = u32();
        const buf = handle === NULL_HANDLE ? null : bufferByHandle.get(handle) ?? bufferEnsureTs(handle);
        gl.bindBufferBase(target, index, buf);
        break;
      }
      case GLOP.DRAW_ARRAYS_INSTANCED: {
        const mode = u16();
        const first = i32();
        const count = i32();
        gl.drawArraysInstanced(mode, first, count, i32());
        break;
      }
      case GLOP.DRAW_ELEMENTS_INSTANCED: {
        const mode = u16();
        const count = i32();
        const type = u16();
        const offset = i32();
        gl.drawElementsInstanced(mode, count, type, offset, i32());
        break;
      }
      case GLOP.VERTEX_ATTRIB_NFV: {
        const index = u32();
        const nfloats = u32();
        const data = rangeBytes(u32());
        const dvv = new DataView(data.buffer, data.byteOffset, data.byteLength);
        if (nfloats === 1) gl.vertexAttrib1f(index, dvv.getFloat32(0, true));
        else if (nfloats === 2) gl.vertexAttrib2f(index, dvv.getFloat32(0, true), dvv.getFloat32(4, true));
        else if (nfloats === 3) gl.vertexAttrib3f(index, dvv.getFloat32(0, true), dvv.getFloat32(4, true), dvv.getFloat32(8, true));
        else gl.vertexAttrib4f(index, dvv.getFloat32(0, true), dvv.getFloat32(4, true), dvv.getFloat32(8, true), dvv.getFloat32(12, true));
        break;
      }
      case GLOP.UNIFORM_BLOCK_BINDING: {
        p += 12;
        break;
      }
      case GLOP.BIND_ATTRIB_LOCATION: {
        p += 12;
        break;
      }
      case GLOP.BLEND_COLOR: {
        const r = f32();
        const g = f32();
        const b = f32();
        gl.blendColor(r, g, b, f32());
        break;
      }
      case GLOP.CULL_FACE: {
        gl.cullFace(u16());
        break;
      }
      case GLOP.FRONT_FACE: {
        gl.frontFace(u16());
        break;
      }
      case GLOP.CLEAR_DEPTH: {
        gl.clearDepth(f32());
        break;
      }
      case GLOP.CLEAR_STENCIL: {
        gl.clearStencil(i32());
        break;
      }
      case GLOP.POLYGON_OFFSET: {
        const factor = f32();
        gl.polygonOffset(factor, f32());
        break;
      }
      case GLOP.SCISSOR: {
        const x = i32();
        const y = i32();
        const w = i32();
        gl.scissor(x, y, w, i32());
        break;
      }
      case GLOP.STENCIL_FUNC: {
        const func = u16();
        const reference = i32();
        gl.stencilFunc(func, reference, u32());
        break;
      }
      case GLOP.STENCIL_OP: {
        const sfail = u16();
        const dpfail = u16();
        gl.stencilOp(sfail, dpfail, u16());
        break;
      }
      case GLOP.STENCIL_MASK: {
        gl.stencilMask(u32());
        break;
      }
      case GLOP.DRAW_BUFFERS: {
        // WebGL2 MRT：count 首部 + count×u32（变长，与 UNIFORM_MATRIX_4F 同型）。
        const n = u8();
        const modes: number[] = [];
        for (let i = 0; i < n; i++) modes.push(u32());
        gl.drawBuffers(modes);
        break;
      }
      case GLOP.DRAW_ARRAYS: {
        const mode = u16();
        const first = i32();
        gl.drawArrays(mode, first, i32());
        break;
      }
      case GLOP.DRAW_ELEMENTS: {
        const mode = u16();
        const count = i32();
        const type = u16();
        gl.drawElements(mode, count, type, i32());
        break;
      }
      default:
        throw new Error(`web-gl-stream: unknown opcode ${opcode} at ${opStart}`);
    }
    // Payload-size sanity for the fixed-length commands: a drift here means
    // the recorder and this table disagree — fail loudly, never silently
    // misparse the rest of the stream.
    const fixed = GLOP_PAYLOAD_LEN[opcode];
    if (fixed >= 0 && p - start !== fixed) {
      throw new Error(
        `web-gl-stream: opcode ${opcode} payload drift (expected ${fixed}, got ${p - start})`,
      );
    }
  }

  // Straight-alpha readback, flipped (GL origin bottom-left, composite top-down).
  gl.bindFramebuffer(gl.READ_FRAMEBUFFER, null);
  const rgba = new Uint8Array(stream.width * stream.height * 4);
  gl.readPixels(0, 0, stream.width, stream.height, gl.RGBA, gl.UNSIGNED_BYTE, rgba);
  provideGeneratedImageRgba(
    stream.imageId,
    stream.width,
    stream.height,
    flipRows(rgba, stream.width, stream.height),
  );
}

function flipRows(rgba: Uint8Array, width: number, height: number): Uint8Array {
  const out = new Uint8Array(rgba.length);
  const row = width * 4;
  for (let y = 0; y < height; y++) {
    const src = (height - 1 - y) * row;
    out.set(rgba.subarray(src, src + row), y * row);
  }
  return out;
}

// Scan a vertex source for attribute declarations — MUST mirror core
// webgl_api.js collectDecls (declaration order, `[0]` array convention is
// irrelevant for attributes but the dedup + regex are not).
function scanAttribNames(src: string): string[] {
  const text = src.replace(/\/\*[\s\S]*?\*\//g, ' ').replace(/\/\/[^\n]*/g, ' ');
  const out: string[] = [];
  const seen = new Set<string>();
  const re = /(uniform|attribute|in)\s+(?:highp\s+|mediump\s+|lowp\s+)?[A-Za-z_]\w*\s+([A-Za-z_]\w*(?:\s*\[[^\]]*\])?(?:\s*,\s*[A-Za-z_]\w*(?:\s*\[[^\]]*\])?)*)\s*;/g;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    const kw = m[1];
    if (kw !== 'attribute' && kw !== 'in') continue;
    for (const decl of m[2].split(',')) {
      const base = decl.trim().replace(/\s*\[[^\]]*\]$/, '');
      if (base && !seen.has(base)) {
        seen.add(base);
        out.push(base);
      }
    }
  }
  return out;
}

function programKey(program: WebGLProgram): string {
  for (const [k, v] of programCache) {
    if (v === program) return k;
  }
  return String(program);
}
