struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  colorA: vec4f,
  colorB: vec4f,
  colorC: vec4f,
  colorSpace: f32,
  balance: f32,
  intensity: f32,
  curtainCount: f32,
  speed: f32,
  waviness: f32,
  rayDensity: f32,
  height: f32,
  center: vec2f,
  seed: f32,
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
  let aspect_4 = (uniforms._sys.viewportSize.x / max(uniforms._sys.viewportSize.y, 0.000001));
  let wx_3 = (((uv.x - uniforms.n_n0.center.x) * aspect_4) * 2.0);
  let auroraTime_6 = uniforms.n_n0._animTime;
  let tSlow_5 = ((auroraTime_6 * 0.06) + 0.0);
  let lSeed_7 = (uniforms.n_n0.seed + 0.0);
  let curtainNoise_8 = mxNoiseFloat3(vec3f((wx_3 * 0.5), (tSlow_5 * 0.25), (lSeed_7 * 0.1)));
  let distFromPath_2 = (uv.y - (((1.0 - uniforms.n_n0.center.y) + ((((sin((((wx_3 * 0.7) + (tSlow_5 * 0.4)) + (fract((sin((lSeed_7 * 12.9)) * 4758.5)) * 6.283185307179586))) * 0.15) + (sin((((wx_3 * 1.6) + (tSlow_5 * 0.65)) + (fract((sin((lSeed_7 * 78.2)) * 2847.1)) * 6.283185307179586))) * 0.08)) + (sin((((wx_3 * 2.9) + (tSlow_5 * 0.3)) + (fract((sin((lSeed_7 * 41.6)) * 1593.7)) * 6.283185307179586))) * 0.04)) * (uniforms.n_n0.waviness * 0.01))) + ((curtainNoise_8 * 0.14) * (uniforms.n_n0.waviness * 0.01))));
  let curtainRays_10 = pow(((sin(((((wx_3 + (curtainNoise_8 * 2.5)) * (mix(6.0, 20.0, (uniforms.n_n0.rayDensity * 0.01)) * 0.15)) + (tSlow_5 * 0.1)) + (fract((sin((lSeed_7 * 53.7)) * 3847.2)) * 6.283185307179586))) * 0.5) + 0.5), 1.5);
  let topExtent_9 = (((uniforms.n_n0.height * 0.01) * mix(0.4, 1.0, curtainRays_10)) * ((curtainRays_10 * 0.4) + 0.6));
  let colorT_1 = clamp((distFromPath_2 / (topExtent_9 + 0.001)), 0.0, 1.0);
  let curtainAlpha_11 = ((((smoothstep(-0.04, 0.015, distFromPath_2) * (1.0 - smoothstep(0.0, topExtent_9, distFromPath_2))) * curtainRays_10) * mix(0.4, 1.0, ((curtainNoise_8 * 0.5) + 0.5))) * ((exp((((colorT_1 - 0.2) * (colorT_1 - 0.2)) * -6.0)) * 0.3) + 1.0));
  let wx_14 = (((uv.x - uniforms.n_n0.center.x) * aspect_4) * 2.0);
  let tSlow_15 = ((auroraTime_6 * 0.06) + 3.1);
  let lSeed_16 = (uniforms.n_n0.seed + 31.700000762939453);
  let curtainNoise_17 = mxNoiseFloat3(vec3f((wx_14 * 0.5), (tSlow_15 * 0.25), (lSeed_16 * 0.1)));
  let distFromPath_13 = (uv.y - (((1.0 - uniforms.n_n0.center.y) + ((((sin((((wx_14 * 0.7) + (tSlow_15 * 0.4)) + (fract((sin((lSeed_16 * 12.9)) * 4758.5)) * 6.283185307179586))) * 0.15) + (sin((((wx_14 * 1.6) + (tSlow_15 * 0.65)) + (fract((sin((lSeed_16 * 78.2)) * 2847.1)) * 6.283185307179586))) * 0.08)) + (sin((((wx_14 * 2.9) + (tSlow_15 * 0.3)) + (fract((sin((lSeed_16 * 41.6)) * 1593.7)) * 6.283185307179586))) * 0.04)) * (uniforms.n_n0.waviness * 0.01))) + ((curtainNoise_17 * 0.14) * (uniforms.n_n0.waviness * 0.01))));
  let curtainRays_19 = pow(((sin(((((wx_14 + (curtainNoise_17 * 2.5)) * (mix(6.0, 20.0, (uniforms.n_n0.rayDensity * 0.01)) * 0.15)) + (tSlow_15 * 0.1)) + (fract((sin((lSeed_16 * 53.7)) * 3847.2)) * 6.283185307179586))) * 0.5) + 0.5), 1.5);
  let topExtent_18 = (((uniforms.n_n0.height * 0.01) * mix(0.4, 1.0, curtainRays_19)) * ((curtainRays_19 * 0.4) + 0.6));
  let colorT_12 = clamp((distFromPath_13 / (topExtent_18 + 0.001)), 0.0, 1.0);
  let curtainAlpha_20 = ((((smoothstep(-0.04, 0.015, distFromPath_13) * (1.0 - smoothstep(0.0, topExtent_18, distFromPath_13))) * curtainRays_19) * mix(0.4, 1.0, ((curtainNoise_17 * 0.5) + 0.5))) * ((exp((((colorT_12 - 0.2) * (colorT_12 - 0.2)) * -6.0)) * 0.3) + 1.0));
  let wx_23 = (((uv.x - uniforms.n_n0.center.x) * aspect_4) * 2.0);
  let tSlow_24 = ((auroraTime_6 * 0.06) + 6.7);
  let lSeed_25 = (uniforms.n_n0.seed + 63.400001525878906);
  let curtainNoise_26 = mxNoiseFloat3(vec3f((wx_23 * 0.5), (tSlow_24 * 0.25), (lSeed_25 * 0.1)));
  let distFromPath_22 = (uv.y - (((1.0 - uniforms.n_n0.center.y) + ((((sin((((wx_23 * 0.7) + (tSlow_24 * 0.4)) + (fract((sin((lSeed_25 * 12.9)) * 4758.5)) * 6.283185307179586))) * 0.15) + (sin((((wx_23 * 1.6) + (tSlow_24 * 0.65)) + (fract((sin((lSeed_25 * 78.2)) * 2847.1)) * 6.283185307179586))) * 0.08)) + (sin((((wx_23 * 2.9) + (tSlow_24 * 0.3)) + (fract((sin((lSeed_25 * 41.6)) * 1593.7)) * 6.283185307179586))) * 0.04)) * (uniforms.n_n0.waviness * 0.01))) + ((curtainNoise_26 * 0.14) * (uniforms.n_n0.waviness * 0.01))));
  let curtainRays_28 = pow(((sin(((((wx_23 + (curtainNoise_26 * 2.5)) * (mix(6.0, 20.0, (uniforms.n_n0.rayDensity * 0.01)) * 0.15)) + (tSlow_24 * 0.1)) + (fract((sin((lSeed_25 * 53.7)) * 3847.2)) * 6.283185307179586))) * 0.5) + 0.5), 1.5);
  let topExtent_27 = (((uniforms.n_n0.height * 0.01) * mix(0.4, 1.0, curtainRays_28)) * ((curtainRays_28 * 0.4) + 0.6));
  let colorT_21 = clamp((distFromPath_22 / (topExtent_27 + 0.001)), 0.0, 1.0);
  let curtainAlpha_29 = ((((smoothstep(-0.04, 0.015, distFromPath_22) * (1.0 - smoothstep(0.0, topExtent_27, distFromPath_22))) * curtainRays_28) * mix(0.4, 1.0, ((curtainNoise_26 * 0.5) + 0.5))) * ((exp((((colorT_21 - 0.2) * (colorT_21 - 0.2)) * -6.0)) * 0.3) + 1.0));
  let wx_32 = (((uv.x - uniforms.n_n0.center.x) * aspect_4) * 2.0);
  let tSlow_33 = ((auroraTime_6 * 0.06) + 9.4);
  let lSeed_34 = (uniforms.n_n0.seed + 95.10000610351562);
  let curtainNoise_35 = mxNoiseFloat3(vec3f((wx_32 * 0.5), (tSlow_33 * 0.25), (lSeed_34 * 0.1)));
  let distFromPath_31 = (uv.y - (((1.0 - uniforms.n_n0.center.y) + ((((sin((((wx_32 * 0.7) + (tSlow_33 * 0.4)) + (fract((sin((lSeed_34 * 12.9)) * 4758.5)) * 6.283185307179586))) * 0.15) + (sin((((wx_32 * 1.6) + (tSlow_33 * 0.65)) + (fract((sin((lSeed_34 * 78.2)) * 2847.1)) * 6.283185307179586))) * 0.08)) + (sin((((wx_32 * 2.9) + (tSlow_33 * 0.3)) + (fract((sin((lSeed_34 * 41.6)) * 1593.7)) * 6.283185307179586))) * 0.04)) * (uniforms.n_n0.waviness * 0.01))) + ((curtainNoise_35 * 0.14) * (uniforms.n_n0.waviness * 0.01))));
  let curtainRays_37 = pow(((sin(((((wx_32 + (curtainNoise_35 * 2.5)) * (mix(6.0, 20.0, (uniforms.n_n0.rayDensity * 0.01)) * 0.15)) + (tSlow_33 * 0.1)) + (fract((sin((lSeed_34 * 53.7)) * 3847.2)) * 6.283185307179586))) * 0.5) + 0.5), 1.5);
  let topExtent_36 = (((uniforms.n_n0.height * 0.01) * mix(0.4, 1.0, curtainRays_37)) * ((curtainRays_37 * 0.4) + 0.6));
  let colorT_30 = clamp((distFromPath_31 / (topExtent_36 + 0.001)), 0.0, 1.0);
  let curtainAlpha_38 = ((((smoothstep(-0.04, 0.015, distFromPath_31) * (1.0 - smoothstep(0.0, topExtent_36, distFromPath_31))) * curtainRays_37) * mix(0.4, 1.0, ((curtainNoise_35 * 0.5) + 0.5))) * ((exp((((colorT_30 - 0.2) * (colorT_30 - 0.2)) * -6.0)) * 0.3) + 1.0));
  let auroraAlpha_39 = (((curtainAlpha_11 + (curtainAlpha_20 * 0.65)) + (curtainAlpha_29 * 0.45)) + (curtainAlpha_38 * 0.3));
  let auroraAvgT_0 = pow((((((colorT_1 * curtainAlpha_11) + (colorT_12 * (curtainAlpha_20 * 0.65))) + (colorT_21 * (curtainAlpha_29 * 0.45))) + (colorT_30 * (curtainAlpha_38 * 0.3))) / (auroraAlpha_39 + 0.001)), mix(2.5, 0.4, (uniforms.n_n0.balance * 0.01)));
  let auroraGlow_40 = (auroraAlpha_39 * (uniforms.n_n0.intensity * 0.018));
  var composed_41 = vec4f(clamp(((mixColors(mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, smoothstep(0.0, 0.2, auroraAvgT_0)), uniforms.n_n0.colorC, smoothstep(0.4, 0.85, auroraAvgT_0))).rgb * auroraGlow_40), vec3f(0.0), vec3f(1.0)), clamp((auroraGlow_40 * 0.5), 0.0, 1.0));
  return vec4f(linearToSrgb(tone_linear(composed_41.rgb)) * composed_41.a, composed_41.a);
}