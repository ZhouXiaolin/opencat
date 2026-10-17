fn beamFrame(uv: vec2f, aspect: f32, anchorRaw: vec2f) -> vec2f {
  let pA = vec2f((uv.x * aspect), uv.y);
  let anchor = vec2f((anchorRaw.x * aspect), (1f - anchorRaw.y));
  let toCenter = (vec2f((aspect * 0.5f), 0.5f) - anchor);
  let cLen = length(toCenter);
  let dirX = select((toCenter.x / max(cLen, 1e-3f)), 1f, (cLen < 0.02f));
  let dirY = select((toCenter.y / max(cLen, 1e-3f)), 0f, (cLen < 0.02f));
  let relX = (pA.x - anchor.x);
  let relY = (pA.y - anchor.y);
  return vec2f(((relX * dirX) + (relY * dirY)), ((relY * dirX) - (relX * dirY)));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  position: vec2f,
  spread: f32,
  intensity: f32,
  streaks: f32,
  colorHot: vec4f,
  colorMid: vec4f,
  colorFringe: vec4f,
  flicker: f32,
  speed: f32,
  seed: f32,
  _opacity: f32,
  _animTime: f32,
}

struct struct_2 {
  colorA: vec4f,
  colorB: vec4f,
  colorsArray: array<vec4f, 8>,
  positionsArray: array<vec4f, 2>,
  convertedColorsArray: array<vec3f, 8>,
  stopCount: f32,
  start: vec2f,
  end: vec2f,
  angle: f32,
  edges: f32,
  colorSpace: f32,
  _opacity: f32,
}

struct struct_3 {
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
  n_n0c0: struct_2,
  _sys: struct_3,
}

@group(0) @binding(0) var<uniform> uniforms: combined;

fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }

fn hash12(p: vec2f) -> f32 {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  return (f32(((u.x ^ u.y) * 3141592653u)) / 4294967296f);
}

fn value12(p: vec2f) -> f32 {
  let i = floor(p);
  let f0 = (p - i);
  let f = ((f0 * f0) * (3f - (f0 * 2f)));
  return mix(mix(hash12(i), hash12((i + vec2f(1, 0))), f.x), mix(hash12((i + vec2f(0, 1))), hash12((i + vec2f(1))), f.x), f.y);
}

fn leakDrift(t: f32, seed: f32, flicker: f32) -> vec4f {
  let nA = value12(vec2f((t * 0.9f), ((seed * 0.53f) + 11.7f)));
  let nB = value12(vec2f(((t * 2.7f) + 41.3f), ((seed * 0.29f) + 5.1f)));
  let breathe = ((nA * 0.65f) + (nB * 0.35f));
  let flickerMod = mix(1f, (0.25f + (1.5f * breathe)), flicker);
  let evo1 = value12(vec2f(((t * 0.13f) + 3.1f), ((seed * 0.41f) + 27f)));
  let evo2 = value12(vec2f(((t * 0.09f) + 17.7f), ((seed * 0.61f) + 8.4f)));
  let evo3 = value12(vec2f(((t * 0.17f) + 51.2f), ((seed * 0.23f) + 2.9f)));
  return vec4f(flickerMod, evo1, evo2, evo3);
}

fn lightfields_anisotropicGaussianSpot(du: f32, dv: f32, sharpness: f32) -> f32 {
  return exp((((du * du) + (dv * dv)) * -(sharpness)));
}

fn leakBloom(u: f32, v: f32, spread: f32, drift: vec4f) -> vec2f {
  let evo2 = drift.z;
  let sp = max(spread, 0.05f);
  let elong = (sp * (1.5f + (0.7f * evo2)));
  let du = (u / sp);
  let dv = (v / elong);
  let main = lightfields_anisotropicGaussianSpot(du, dv, 1.9f);
  let shoulder = (lightfields_anisotropicGaussianSpot(du, dv, 0.45f) * 0.22f);
  return vec2f(main, shoulder);
}

fn tone_luma601Dot(rgb: vec3f) -> f32 {
  return dot(rgb, vec3f(0.29899999499320984, 0.5870000123977661, 0.11400000005960464));
}

fn screenGlowComposite(color: vec4f, leakRgb: vec3f) -> vec4f {
  let base = color.xyz;
  let screened = (vec3f(1) - ((vec3f(1) - clamp(base, vec3f(), vec3f(1))) * (vec3f(1) - clamp(leakRgb, vec3f(), vec3f(1)))));
  let lifted = (screened + (leakRgb * 0.16f));
  let outRgb = clamp(lifted, vec3f(), vec3f(1));
  let leakLuma = clamp(tone_luma601Dot(leakRgb), 0f, 1f);
  let outA = clamp((color.w + (leakLuma * 0.85f)), 0f, 1f);
  return vec4f(outRgb, outA);
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

fn gradientClamp01(t: f32) -> f32 {
  return clamp(t, 0f, 1f);
}

fn unflipPosition(p: vec2f) -> vec2f {
  return vec2f(p.x, (1f - p.y));
}

fn geom_rotate2(p: vec2f, cosA: f32, sinA: f32) -> vec2f {
  return vec2f(((p.x * cosA) - (p.y * sinA)), ((p.x * sinA) + (p.y * cosA)));
}

fn gradientRawT(start: vec2f, end: vec2f, angle: f32, uv: vec2f, viewport: vec2f) -> f32 {
  let startPos = unflipPosition(start);
  let endPos = unflipPosition(end);
  let gradientVector = (endPos - startPos);
  let gradientLength = length(gradientVector);
  let gradientDir = normalize(gradientVector);
  let safeViewportY = max(viewport.y, 1e-6f);
  let aspect = (viewport.x / safeViewportY);
  let angleRad = ((angle * 0.017453292519943295f) * -1f);
  let cosAngle = cos(angleRad);
  let sinAngle = sin(angleRad);
  let midpoint = ((startPos + endPos) * 0.5f);
  let centeredUV = (uv - midpoint);
  let rotated = geom_rotate2(vec2f((centeredUV.x * aspect), centeredUV.y), cosAngle, sinAngle);
  let rotatedUV = (vec2f((rotated.x / aspect), rotated.y) + midpoint);
  let relativePos = (rotatedUV - startPos);
  let projection = dot(relativePos, gradientDir);
  return (projection / max(gradientLength, 1e-6f));
}

fn heatRamp3(heat: f32, hot: vec3f, mid: vec3f, fringe: vec3f) -> vec3f {
  let toMid = smoothstep(0f, 0.5f, heat);
  let toHot = smoothstep(0.55f, 1.15f, heat);
  let rgb = mix(mix(fringe, mid, vec3f(toMid)), hot, vec3f(toHot));
  return (rgb * clamp(heat, 0f, 1.6f));
}

fn chromaticHeatTaps(heat: f32, hot: vec3f, mid: vec3f, fringe: vec3f) -> vec3f {
  let rampR = heatRamp3((heat * 1.08f), hot, mid, fringe);
  let rampG = heatRamp3(heat, hot, mid, fringe);
  let rampB = heatRamp3((heat * 0.88f), hot, mid, fringe);
  return vec3f(rampR.x, rampG.y, rampB.z);
}

fn heatDither(heat: f32, uv: vec2f, t: f32) -> f32 {
  let dithered = (heat + ((hash12(((uv * 1024f) + vec2f(t, (t * 1.7f)))) - 0.5f) * 0.012f));
  return max(dithered, 0f);
}

fn lightfields_streakBand(u: f32, center: f32, width: f32) -> f32 {
  return exp(((((u - center) * (u - center)) * -1f) / (width * width)));
}

fn leakStreaks(u: f32, v: f32, spread: f32, streaks: f32, drift: vec4f) -> f32 {
  let evo1 = drift.y;
  let evo2 = drift.z;
  let evo3 = drift.w;
  let sp = max(spread, 0.05f);
  let elong = (sp * (1.5f + (0.7f * evo2)));
  let dv = (v / elong);
  let alongFade = exp(((dv * dv) * -0.55f));
  let s1c = (sp * (0.42f + (0.22f * evo1)));
  let s1w = max((sp * (0.07f + (0.07f * evo3))), 8e-3f);
  let streak1 = (lightfields_streakBand(u, s1c, s1w) * alongFade);
  let s2c = (sp * (0.8f + (0.3f * evo1)));
  let s2w = max((sp * (0.16f + (0.12f * evo3))), 0.015f);
  let streak2 = ((lightfields_streakBand(u, s2c, s2w) * alongFade) * 0.45f);
  return ((streak1 + streak2) * streaks);
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
  let leakFrame_1 = beamFrame(uv, uniforms._sys.aspect, uniforms.n_n0.position);
  let leakClock_3 = ((uniforms.n_n0._animTime * 0.6) + (uniforms.n_n0.seed * 7.9));
  let leakDrift_2 = leakDrift(leakClock_3, uniforms.n_n0.seed, uniforms.n_n0.flicker);
  let leakBloom_0 = leakBloom(leakFrame_1.x, leakFrame_1.y, uniforms.n_n0.spread, leakDrift_2);
  var composed_4 = screenGlowComposite(mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, uv, uniforms._sys.viewportSize)))), chromaticHeatTaps(heatDither(((((((leakBloom_0.x * 1.25) + leakBloom_0.y) + (leakStreaks(leakFrame_1.x, leakFrame_1.y, uniforms.n_n0.spread, uniforms.n_n0.streaks, leakDrift_2) * 0.85)) * uniforms.n_n0.intensity) * 2.2) * leakDrift_2.x), uv, leakClock_3), uniforms.n_n0.colorHot.rgb, uniforms.n_n0.colorMid.rgb, uniforms.n_n0.colorFringe.rgb));
  return vec4f(linearToSrgb(tone_linear(composed_4.rgb)) * composed_4.a, composed_4.a);
}