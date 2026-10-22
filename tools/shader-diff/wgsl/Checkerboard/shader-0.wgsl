fn awMix(a: vec3f, b: vec3f, wA: f32, wB: f32, sw: f32) -> vec3f {
  return (((a * wA) + (b * wB)) / sw);
}

fn mixColors(colorA: vec4f, colorB: vec4f, t: f32) -> vec4f {
  let weightA = (colorA.w * (1f - t));
  let weightB = (colorB.w * t);
  let totalWeight = (weightA + weightB);
  let safeWeight = max(totalWeight, 1e-3f);
  let rgb = awMix(colorA.xyz, colorB.xyz, weightA, weightB, safeWeight);
  return vec4f(rgb, totalWeight);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  colorA: vec4f,
  colorB: vec4f,
  cells: f32,
  softness: f32,
  colorSpace: f32,
  _opacity: f32,
}

struct struct_2 {
  time: f32,
  viewportSize: vec2f,
  logicalViewportSize: vec2f,
  aspect: f32,
  pointer: vec2f,
  pointerActive: f32,
}

struct combined {
  n_root: padded,
  n_n0: struct_1,
  _sys: struct_2,
}

@group(0) @binding(0) var<uniform> uniforms: combined;

fn aspectCorrectedUVFlipY(uv: vec2f, viewport: vec2f) -> vec2f {
  let aspect = (viewport.x / max(viewport.y, 1e-6f));
  return vec2f((uv.x * aspect), (1f - uv.y));
}

fn footprint2(p: vec2f, softness: f32) -> vec2f {
  let dpx = dpdx(p);
  let dpy = dpdy(p);
  let baseW = vec2f(max(abs(dpx.x), abs(dpy.x)), max(abs(dpx.y), abs(dpy.y)));
  return vec2f(max((baseW.x + softness), 1e-5f), max((baseW.y + softness), 1e-5f));
}

fn quilezCheckerFilter(p: vec2f, w: vec2f) -> f32 {
  let aHalf = vec2f((w.x * 0.5f), (w.y * 0.5f));
  let minusSideX = abs((fract(((p.x - aHalf.x) * 0.5f)) - 0.5f));
  let minusSideY = abs((fract(((p.y - aHalf.y) * 0.5f)) - 0.5f));
  let plusSideX = abs((fract(((p.x + aHalf.x) * 0.5f)) - 0.5f));
  let plusSideY = abs((fract(((p.y + aHalf.y) * 0.5f)) - 0.5f));
  let iX = (((minusSideX - plusSideX) * 2f) / w.x);
  let iY = (((minusSideY - plusSideY) * 2f) / w.y);
  return clamp((0.5f - ((iX * iY) * 0.5f)), 0f, 1f);
}

fn checkerBlend(uv: vec2f, viewport: vec2f, cells: f32, softness: f32) -> f32 {
  let p = (aspectCorrectedUVFlipY(uv, viewport) * cells);
  return quilezCheckerFilter(p, footprint2(p, softness));
}

fn linearToSrgb(color: vec3f) -> vec3f {
  let a = ((pow(color, vec3f(0.41666001081466675)) * 1.055f) - 0.055f);
  let b = (color * 12.92f);
  let factor = (color <= vec3f(0.0031308000907301903));
  return select(a, b, factor);
}

fn tone_linear(color: vec3f) -> vec3f {
  return vec3f(color.x, color.y, color.z);
}

struct final_Input {
  @location(0) uv: vec2f,
}

@fragment fn final_1(in: final_Input) -> @location(0)  vec4f {
  var uv = in.uv;
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, checkerBlend(uv, uniforms._sys.viewportSize, uniforms.n_n0.cells, uniforms.n_n0.softness));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}