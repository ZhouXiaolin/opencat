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
  scale: f32,
  thickness: f32,
  seed: f32,
  speed: f32,
  _opacity: f32,
  _animTime: f32,
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

fn scratchOne(uv: vec2f, f: f32, time: f32, thickness: f32) -> f32 {
  let seed0 = floor(uv);
  let lo = (uv - seed0);
  let sx = floor((sin((seed0.x * 51024f)) * 3104f));
  let sy = floor((sin((seed0.y * 1324f)) * 554f));
  let ss = (sx + sy);
  var u2 = ((lo * 2f) - 1f);
  u2 = ((u2 * cos(ss)) + (vec2f(-(u2.y), u2.x) * sin(ss)));
  u2 = (u2 + sin((sx - sy)));
  u2 = ((u2 * 0.5f) + 0.5f);
  const WAVYNESS = 0.2;
  let sVal = ((sin((sx + (u2.y * 3.141592653589793f))) + sin((sy + (u2.y * 3.141592653589793f)))) * WAVYNESS);
  var x = abs(((u2.x - 0.5f) + sVal));
  x = (0.5f - (x * (f / thickness)));
  x = (smoothstep(-2f, ((fwidth(x) * 1.5f) + 16f), x) * 12f);
  let pulse = ((sin((time + (hash12(seed0) * 6.2831853f))) * 0.5f) + 0.5f);
  return ((x * u2.y) * pulse);
}

fn scratches12(uv: vec2f, time: f32, thickness: f32) -> f32 {
  var scratches = 0f;
  var p = vec2f(uv.x, uv.y);
  let f = (1f / length(fwidth(uv)));
  for (var i = 0; (i < 8i); i++) {
    scratches = max(scratches, scratchOne(p, f, time, thickness));
    p = (vec2f((p.x + (p.y * 0.7f)), ((p.x * -0.7f) + p.y)) - 12.31f);
  }
  return scratches;
}

fn rawAspectScaledDomain(uv: vec2f, viewport: vec2f, scale: f32, seed: f32) -> vec2f {
  let aspect = (viewport.x / viewport.y);
  let aspectUV = vec2f((uv.x * aspect), uv.y);
  return ((aspectUV * exp(scale)) + seed);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, noiseToneKColor(scratches12(rawAspectScaledDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.seed), uniforms.n_n0._animTime, uniforms.n_n0.thickness), 0.0, 0.0));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}