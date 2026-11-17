/// 宿主侧 GLSL 反射桥：`globalThis.__opencatGlslReflect(vs, fs)`。
///
/// 录制垫片（core webgl_api.js）在 linkProgram 时查询 program 的
/// active uniforms/attributes——three 拿这份反射决定每帧上传哪些
/// uniform、设置哪些 attrib。engine 用 ANGLE 离屏会话反射（真 GL）；
/// 浏览器宿主同样有真 GL（WebGL2），这里是它的等价通道。没有它，
/// core 的 store 兜底返回空表 → three 认为程序没有 uniform/attrib →
/// 录制流里永不出现 UNIFORM_*/ATTRIB_* op → 回放的 program 没有
/// uniform 值 → 整个 GL 流渲染黑（vfx 实证）。
///
/// 输出结构与 engine 的 reflect_program_gl 一致：
/// `{uniforms:[{n,t,s}], attributes:[{n,t,s,l}], blocks:[{n,i}]}`。
/// `l` 是驱动 location（与 engine 同样：反射会话不做 bindAttribLocation，
/// 返回驱动分配值，与 three 的 bind 值按枚举序对齐）。
///
/// 反射结果按 (vs, fs) 缓存——three 每 pass/帧都可能 link 同一 program。

// canvas_api.js shim 会把 globalThis.OffscreenCanvas 换成 2d-only 录制面——
// 真 GL 上下文必须建在真类上（与 gl-executor 的 target 同款别名）。
import { RealOffscreenCanvas } from '../real-offscreen';

let reflectGl: WebGL2RenderingContext | null = null;
const reflectCache = new Map<string, object>();

function reflectContext(): WebGL2RenderingContext {
  reflectGl ??= (() => {
    const canvas = new RealOffscreenCanvas(4, 4);
    const gl = canvas.getContext('webgl2');
    if (!gl) throw new Error('glsl reflect: webgl2 context failed');
    return gl;
  })();
  return reflectGl;
}

function reflect(vs: string, fs: string): object {
  const gl = reflectContext();
  const cached = reflectCache.get(`${vs}\u{0}${fs}`);
  if (cached) return cached;

  const compile = (ty: number, src: string): WebGLShader => {
    const sh = gl.createShader(ty)!;
    gl.shaderSource(sh, src);
    gl.compileShader(sh);
    if (!gl.getShaderParameter(sh, gl.COMPILE_STATUS)) {
      const log = gl.getShaderInfoLog(sh);
      gl.deleteShader(sh);
      throw new Error(`glsl reflect compile failed: ${log}`);
    }
    return sh;
  };
  const v = compile(gl.VERTEX_SHADER, vs);
  const f = compile(gl.FRAGMENT_SHADER, fs);
  const program = gl.createProgram()!;
  gl.attachShader(program, v);
  gl.attachShader(program, f);
  gl.linkProgram(program);
  gl.deleteShader(v);
  gl.deleteShader(f);
  if (!gl.getProgramParameter(program, gl.LINK_STATUS)) {
    const log = gl.getProgramInfoLog(program);
    gl.deleteProgram(program);
    throw new Error(`glsl reflect link failed: ${log}`);
  }

  const uniforms: object[] = [];
  const nUniforms = gl.getProgramParameter(program, gl.ACTIVE_UNIFORMS) as number;
  for (let i = 0; i < nUniforms; i++) {
    const u = gl.getActiveUniform(program, i);
    if (!u) continue;
    // 驱动名原样透传（数组 uniform 是 `name[0]`）——engine 的 ANGLE 反射器
    // 同语义。绝不能剥 `[0]`：three 的 parseUniform 靠「`[0]` 后缀 + 长度
    // 校验」识别 PureArrayUniform，裸名会让它退化成 SingleUniform，vec3 数组
    // 被当成单个 vec3 直传（Vector3 数组 → NaN 进流 → bloom composite 全黑）。
    uniforms.push({ n: u.name, t: u.type, s: Math.max(1, u.size) });
  }
  const attributes: object[] = [];
  const nAttribs = gl.getProgramParameter(program, gl.ACTIVE_ATTRIBUTES) as number;
  for (let i = 0; i < nAttribs; i++) {
    const a = gl.getActiveAttrib(program, i);
    if (!a) continue;
    const loc = gl.getAttribLocation(program, a.name);
    attributes.push({ n: a.name, t: a.type, s: Math.max(1, a.size), l: loc });
  }
  // uniform 块按序号探名（glow 无块计数 API，engine 同款循环）。
  const blocks: object[] = [];
  for (let i = 0; i < 16; i++) {
    const name = gl.getActiveUniformBlockName(program, i);
    if (name === null || name === undefined || name === '') break;
    blocks.push({ n: name, i });
  }
  gl.deleteProgram(program);
  while (gl.getError() !== gl.NO_ERROR) {}

  const result = { uniforms, attributes, blocks };
  reflectCache.set(`${vs}\u{0}${fs}`, result);
  return result;
}

export function installGlslReflector(): void {
  const g = globalThis as unknown as Record<string, unknown>;
  if (typeof g.__opencatGlslReflect === 'function') return;
  g.__opencatGlslReflect = reflect;
}
