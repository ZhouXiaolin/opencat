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
  colorsArray: array<vec4f, 8>,
  positionsArray: array<vec4f, 2>,
  convertedColorsArray: array<vec3f, 8>,
  stopCount: f32,
  colorSpace: f32,
  grain: f32,
  contrast: f32,
  balance: f32,
  seed: f32,
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

fn noiseToneKColor(noise01: f32, contrast: f32, balance: f32) -> f32 {
  let k = clamp(((((noise01 - 0.5f) * (contrast + 1f)) + 0.5f) + balance), 0f, 1f);
  return (1f - k);
}

fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }

fn hash12(p: vec2f) -> f32 {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  return (f32(((u.x ^ u.y) * 3141592653u)) / 4294967296f);
}

fn blue12(p: vec2f) -> f32 {
  var v = 0f;
  for (var k = 0; (k < 9i); k++) {
    let fk = f32(k);
    let row = floor((fk / 3f));
    let col = (fk - (row * 3f));
    v = (v + hash12((p + vec2f((col - 1f), (row - 1f)))));
  }
  return ((0.9f * ((1.125f * hash12(p)) - (v / 8f))) + 0.5f);
}

fn pixelGridDomain(uv: vec2f, viewport: vec2f, grain: f32, seed: f32) -> vec2f {
  return (floor(((uv * viewport) / grain)) + seed);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, noiseToneKColor(blue12(pixelGridDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.grain, uniforms.n_n0.seed)), uniforms.n_n0.contrast, uniforms.n_n0.balance));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}