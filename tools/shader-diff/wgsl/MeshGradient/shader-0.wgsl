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
  count: f32,
  smoothness: f32,
  variation: f32,
  swirl: f32,
  drift: f32,
  wrapping: f32,
  speed: f32,
  seed: f32,
  _opacity: f32,
  _animTime: f32,
  meshAnchors: array<vec4f, 4>,
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

fn warpDomain(pos: vec3f, t: f32, warpAmount: f32) -> vec3f {
  let qx = mxNoiseFloat3((pos + vec3f((t * 0.3f), (t * -0.2f), 0f)));
  let qy = mxNoiseFloat3((pos + vec3f(5.2f, 1.3f, (t * 0.25f))));
  let warped1 = (pos + (vec3f(qx, qy, 0f) * warpAmount));
  let rx = mxNoiseFloat3((warped1 + vec3f(1.7f, 9.2f, (t * 0.15f))));
  let ry = mxNoiseFloat3((warped1 + vec3f(8.3f, 2.8f, (t * 0.13f))));
  return (pos + (vec3f(rx, ry, 0f) * warpAmount));
}

fn awMix(a: vec3f, b: vec3f, wA: f32, wB: f32, sw: f32) -> vec3f {
  return (((a * wA) + (b * wB)) / sw);
}

struct MixStep {
  conv: vec3f,
  alpha: f32,
}

fn mixInSpace(convA: vec3f, alphaA: f32, convB: vec3f, alphaB: f32, t: f32) -> MixStep {
  let weightA = (alphaA * (1f - t));
  let weightB = (alphaB * t);
  let totalWeight = (weightA + weightB);
  let safeWeight = max(totalWeight, 1e-3f);
  return MixStep(awMix(convA, convB, weightA, weightB, safeWeight), totalWeight);
}

fn gradientStopsInSpace(t: f32, colors: array<vec4f, 8>, positions: array<vec4f, 2>, converted: array<vec3f, 8>, stopCount: f32) -> MixStep {
  let clampedT = clamp(t, 0f, 1f);
  let n = i32(stopCount);
  let c0 = converted[0i];
  var conv = vec3f(c0.x, c0.y, c0.z);
  var alpha = colors[0i].w;
  for (var i = 1; (i < n); i++) {
    let va = positions[((i - 1i) >> 2u)];
    let vb = positions[(i >> 2u)];
    let posA = va[((i - 1i) & 3i)];
    let posB = vb[(i & 3i)];
    let seg = clamp(((clampedT - posA) / max((posB - posA), 1e-6f)), 0f, 1f);
    let step_1 = mixInSpace(conv, alpha, converted[i], colors[i].w, seg);
    conv = vec3f(step_1.conv.x, step_1.conv.y, step_1.conv.z);
    alpha = step_1.alpha;
  }
  return MixStep(conv, alpha);
}

fn footprint1(x: f32, softness: f32) -> f32 {
  let baseW = max(abs(dpdx(x)), abs(dpdy(x)));
  return max((baseW + softness), 1e-5f);
}

fn rampWrap(t: f32, cycles: f32) -> f32 {
  let s = (t * cycles);
  let m = (fract((s * 0.5f)) * 2f);
  let band = (1f - abs((m - 1f)));
  let fp = (footprint1(t, 0f) * cycles);
  return mix(band, 0.5f, smoothstep(0.5f, 1.5f, fp));
}

fn floatBitsToUint1(x: f32) -> u32 { return bitcast<u32>(x); }

fn noise_hash11(p: f32) -> f32 {
  let u = floatBitsToUint1((p * 3141592653f));
  return (f32(((u * u) * 3141592653u)) / 4294967296f);
}

fn scatterField(q: vec2f, count: f32, seed: f32, anchors: array<vec4f, 4>, power: f32) -> f32 {
  var num = 0f;
  var den = 0f;
  let seedShuffle = noise_hash11(((seed * 0.531f) + 0.77f));
  for (var i = 0; (i < 8i); i++) {
    let fi = f32(i);
    let active_1 = select(0f, 1f, (fi < (count - 0.5f)));
    let pair = anchors[u32((f32(i) / 2f))];
    let anchor = select(pair.xy, pair.zw, ((i & 1i) == 1i));
    let dist = max(length((q - anchor)), 0.01f);
    let w = (active_1 / pow(dist, power));
    let ti = fract(((fi * 0.6180339887f) + seedShuffle));
    num = (num + (w * ti));
    den = (den + w);
  }
  return (num / max(den, 1e-6f));
}

fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }

fn hash12(p: vec2f) -> f32 {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  return (f32(((u.x ^ u.y) * 3141592653u)) / 4294967296f);
}

fn rgbDither(base: vec4f, uv: vec2f, viewport: vec2f, animTime: f32) -> vec4f {
  let n = hash12(((uv * viewport) + vec2f(animTime, (animTime * 1.618f))));
  let rgb = clamp((base.xyz + ((n - 0.5f) * 4e-3f)), vec3f(), vec3f(1));
  return vec4f(rgb, base.w);
}

fn sRGBToP3(srgb: vec3f) -> vec3f {
  let r = (((srgb.x * 0.8224621f) + (srgb.y * 0.177538f)) + (srgb.z * 0f));
  let g = (((srgb.x * 0.0331941f) + (srgb.y * 0.9668058f)) + (srgb.z * 0f));
  let b = (((srgb.x * 0.0170826f) + (srgb.y * 0.0723974f)) + (srgb.z * 0.9105199f));
  return vec3f(r, g, b);
}

fn oklabToRgb(lab: vec3f) -> vec3f {
  let L = lab.x;
  let a = lab.y;
  let b = lab.z;
  let l_ = ((L + (a * 0.3963377774f)) + (b * 0.2158037573f));
  let m_ = ((L - (a * 0.1055613458f)) - (b * 0.0638541728f));
  let s_ = ((L - (a * 0.0894841775f)) - (b * 1.291485548f));
  let l = pow(l_, 3f);
  let m = pow(m_, 3f);
  let s = pow(s_, 3f);
  return vec3f((((l * 4.0767416621f) - (m * 3.3077115913f)) + (s * 0.2309699292f)), (((l * -1.2684380046f) + (m * 2.6097574011f)) - (s * 0.3413193965f)), (((l * -0.0041960863f) - (m * 0.7034186147f)) + (s * 1.707614701f)));
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
  let aspect_6 = (uniforms._sys.viewportSize.x / max(uniforms._sys.viewportSize.y, 0.000001));
  let meshCentred_5 = vec2f(((uv.x * aspect_6) - (aspect_6 * 0.5)), (uv.y - 0.5));
  let swirlAngle_7 = ((-3.0 * uniforms.n_n0.swirl) * smoothstep(0.0, 1.0, length(meshCentred_5)));
  let meshRot_4 = vec2f(((meshCentred_5.x * cos(swirlAngle_7)) - (meshCentred_5.y * sin(swirlAngle_7))), ((meshCentred_5.x * sin(swirlAngle_7)) + (meshCentred_5.y * cos(swirlAngle_7))));
  let meshSwirled_3 = vec2f((meshRot_4.x + (aspect_6 * 0.5)), (meshRot_4.y + 0.5));
  let meshTime_8 = uniforms.n_n0._animTime;
  let meshWarp_2 = warpDomain(vec3f((meshSwirled_3.x * 1.6), (meshSwirled_3.y * 1.6), (uniforms.n_n0.seed * 7.31)), (meshTime_8 * 0.125), 0.48);
  let meshQ_1 = vec2f((meshWarp_2.x / 1.6), (meshWarp_2.y / 1.6));
  let stopMix_0 = gradientStopsInSpace(rampWrap(scatterField(meshQ_1, uniforms.n_n0.count, uniforms.n_n0.seed, uniforms.n_n0.meshAnchors, clamp(((8.0 / (uniforms.n_n0.smoothness + 0.5)) * exp(((mxNoiseFloat3(vec3f((meshQ_1.x * 1.9), (meshQ_1.y * 1.9), ((uniforms.n_n0.seed * 3.17) + (meshTime_8 * 0.045)))) * uniforms.n_n0.variation) * 1.4))), 0.6, 16.0)), (1.0 + (uniforms.n_n0.wrapping * 9.0))), uniforms.n_n0.colorsArray, uniforms.n_n0.positionsArray, uniforms.n_n0.convertedColorsArray, uniforms.n_n0.stopCount);
  var composed_9 = rgbDither(vec4f(sRGBToP3(oklabToRgb(stopMix_0.conv)), stopMix_0.alpha), uv, uniforms._sys.viewportSize, uniforms.n_n0._animTime);
  return vec4f(linearToSrgb(tone_linear(composed_9.rgb)) * composed_9.a, composed_9.a);
}