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
  scale: f32,
  balance: f32,
  contrast: f32,
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

fn noiseToneSigned(v: f32, contrast: f32, balance: f32) -> f32 {
  let adjusted = ((v * (contrast + 1f)) + balance);
  let k = clamp(((adjusted * 0.5f) + 0.5f), 0f, 1f);
  return (1f - k);
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

fn mxHashInt2(x: i32, y: i32, z: i32) -> u32 {
  const len = 3u;
  let seed = ((3735928559u + (len << 2u)) + 13u);
  let a = (seed + u32(x));
  let b = (seed + u32(y));
  let c = (seed + u32(z));
  return mxBjfinal(a, b, c);
}

fn mxGradientFloat3(hash: u32, x: f32, y: f32, z: f32) -> f32 {
  let h = (hash & 15u);
  let u = select(y, x, (h < 8u));
  let inner = select(z, x, ((h == 12u) || (h == 14u)));
  let v = select(inner, y, (h < 4u));
  let uu = select(u, -(u), ((h & 1u) != 0u));
  let vv = select(v, -(v), ((h & 2u) != 0u));
  return (uu + vv);
}

fn mxTrilerp(v0: f32, v1: f32, v2: f32, v3: f32, v4: f32, v5: f32, v6: f32, v7: f32, s: f32, t: f32, r: f32) -> f32 {
  let s1 = (1f - s);
  let t1 = (1f - t);
  let r1 = (1f - r);
  return ((r1 * ((t1 * ((v0 * s1) + (v1 * s))) + (t * ((v2 * s1) + (v3 * s))))) + (r * ((t1 * ((v4 * s1) + (v5 * s))) + (t * ((v6 * s1) + (v7 * s))))));
}

fn mxGradientScale3d(v: f32) -> f32 {
  return (0.982f * v);
}

fn mxPerlinNoiseFloat3(p: vec3f) -> f32 {
  let X = i32(floor(p.x));
  let Y = i32(floor(p.y));
  let Z = i32(floor(p.z));
  let fx = (p.x - f32(X));
  let fy = (p.y - f32(Y));
  let fz = (p.z - f32(Z));
  let u = mxFade(fx);
  let v = mxFade(fy);
  let w = mxFade(fz);
  let result = mxTrilerp(mxGradientFloat3(mxHashInt2(X, Y, Z), fx, fy, fz), mxGradientFloat3(mxHashInt2((X + 1i), Y, Z), (fx - 1f), fy, fz), mxGradientFloat3(mxHashInt2(X, (Y + 1i), Z), fx, (fy - 1f), fz), mxGradientFloat3(mxHashInt2((X + 1i), (Y + 1i), Z), (fx - 1f), (fy - 1f), fz), mxGradientFloat3(mxHashInt2(X, Y, (Z + 1i)), fx, fy, (fz - 1f)), mxGradientFloat3(mxHashInt2((X + 1i), Y, (Z + 1i)), (fx - 1f), fy, (fz - 1f)), mxGradientFloat3(mxHashInt2(X, (Y + 1i), (Z + 1i)), fx, (fy - 1f), (fz - 1f)), mxGradientFloat3(mxHashInt2((X + 1i), (Y + 1i), (Z + 1i)), (fx - 1f), (fy - 1f), (fz - 1f)), u, v, w);
  return mxGradientScale3d(result);
}

fn mxNoiseFloat3(p: vec3f) -> f32 {
  return mxPerlinNoiseFloat3(p);
}

fn timeAxisDomain(pos: vec2f, t: f32, rate: f32) -> vec3f {
  return vec3f(pos.x, pos.y, (t * rate));
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn aspectScaledDomain(uv: vec2f, viewport: vec2f, scale: f32, seed: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, noiseToneSigned(mxNoiseFloat3(timeAxisDomain(aspectScaledDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.seed), uniforms.n_n0._animTime, 0.5)), uniforms.n_n0.contrast, uniforms.n_n0.balance));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}