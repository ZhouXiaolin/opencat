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
  octaves: f32,
  detail: f32,
  contrast: f32,
  speed: f32,
  angle: f32,
  seed: f32,
  colorSpace: f32,
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

fn fbmNormalize(sum: vec2f) -> f32 {
  return clamp((((sum.x / sum.y) * 0.5f) + 0.5f), 0f, 1f);
}

fn mxFade(t: f32) -> f32 {
  return (((t * t) * t) * ((t * ((t * 6f) - 15f)) + 10f));
}

fn mxRotl32(x: u32, k: i32) -> u32 {
  return ((x << u32(k)) | (x >> u32((32i - k))));
}

fn mxBjfinal(a0: u32, b0: u32, c0: u32) -> u32 {
  var a = a0;
  var b = b0;
  var c = c0;
  c = (c ^ b);
  c = (c - mxRotl32(b, 14i));
  a = (a ^ c);
  a = (a - mxRotl32(c, 11i));
  b = (b ^ a);
  b = (b - mxRotl32(a, 25i));
  c = (c ^ b);
  c = (c - mxRotl32(b, 16i));
  a = (a ^ c);
  a = (a - mxRotl32(c, 4i));
  b = (b ^ a);
  b = (b - mxRotl32(a, 14i));
  c = (c ^ b);
  c = (c - mxRotl32(b, 24i));
  return c;
}

fn mxHashInt1(x: i32, y: i32) -> u32 {
  const len = 2u;
  let seed = ((3735928559u + (len << 2u)) + 13u);
  let a = (seed + u32(x));
  let b = (seed + u32(y));
  return mxBjfinal(a, b, seed);
}

fn mxGradientFloat2(hash: u32, x: f32, y: f32) -> f32 {
  let h = (hash & 7u);
  let u = select(y, x, (h < 4u));
  let v = (2f * select(x, y, (h < 4u)));
  let uu = select(u, -(u), ((h & 1u) != 0u));
  let vv = select(v, -(v), ((h & 2u) != 0u));
  return (uu + vv);
}

fn mxBilerp(v0: f32, v1: f32, v2: f32, v3: f32, s: f32, t: f32) -> f32 {
  let s1 = (1f - s);
  return (((1f - t) * ((v0 * s1) + (v1 * s))) + (t * ((v2 * s1) + (v3 * s))));
}

fn mxGradientScale2d(v: f32) -> f32 {
  return (0.6616f * v);
}

fn mxPerlinNoiseFloat2(p: vec2f) -> f32 {
  let X = i32(floor(p.x));
  let Y = i32(floor(p.y));
  let fx = (p.x - f32(X));
  let fy = (p.y - f32(Y));
  let u = mxFade(fx);
  let v = mxFade(fy);
  let result = mxBilerp(mxGradientFloat2(mxHashInt1(X, Y), fx, fy), mxGradientFloat2(mxHashInt1((X + 1i), Y), (fx - 1f), fy), mxGradientFloat2(mxHashInt1(X, (Y + 1i)), fx, (fy - 1f)), mxGradientFloat2(mxHashInt1((X + 1i), (Y + 1i)), (fx - 1f), (fy - 1f)), u, v);
  return mxGradientScale2d(result);
}

fn mxNoiseFloat2(p: vec2f) -> f32 {
  return mxPerlinNoiseFloat2(p);
}

fn fractalOctaveSumGolden8(base: vec2f, baseFreq: f32, lacunarity: f32, persistence: f32, animTime: f32, seedOffset: vec2f, octaveCount: f32) -> vec2f {
  let n = i32(octaveCount);
  var freq = baseFreq;
  var weight = 1f;
  var accumulated = 0f;
  var totalWeight = 0f;
  for (var i = 0; (i < 8i); i++) {
    let fi = f32(i);
    let dirX = cos((fi * 2.3999632297286535f));
    let dirY = sin((fi * 2.3999632297286535f));
    let coord = vec2f((((base.x * freq) + (animTime * dirX)) + seedOffset.x), (((base.y * freq) + (animTime * dirY)) + seedOffset.y));
    let active_1 = select(0f, 1f, (i < n));
    accumulated = (accumulated + ((mxNoiseFloat2(coord) * weight) * active_1));
    totalWeight = (totalWeight + (weight * active_1));
    freq = (freq * lacunarity);
    weight = (weight * persistence);
  }
  return vec2f(accumulated, totalWeight);
}

fn fractalSum(pos: vec2f, detail: f32, contrast: f32, animTime: f32, seed: f32, octaveCount: f32) -> vec2f {
  return fractalOctaveSumGolden8(pos, 2f, detail, contrast, animTime, vec2f(seed, (seed * 0.7f)), octaveCount);
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn rotatedCenteredDomain(uv: vec2f, viewport: vec2f, angle: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let angleRad = (angle * 0.017453292519943295f);
  let cosA = cos(angleRad);
  let sinA = sin(angleRad);
  let au = ((uv.x * aspect) - (aspect * 0.5f));
  let av = (uv.y - 0.5f);
  let ru = ((au * cosA) + (av * sinA));
  let rv = ((au * -(sinA)) + (av * cosA));
  return vec2f(ru, rv);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, fbmNormalize(fractalSum(rotatedCenteredDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle), uniforms.n_n0.detail, uniforms.n_n0.contrast, uniforms.n_n0._animTime, uniforms.n_n0.seed, uniforms.n_n0.octaves)));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}