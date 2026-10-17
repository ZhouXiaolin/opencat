fn sdfSpaceUV(center: vec2f, scale: f32, rotation: f32, uv: vec2f, aspect: f32) -> vec2f {
  let centerX = center.x;
  let centerY = (1f - center.y);
  let dxAc = ((uv.x - centerX) * aspect);
  let dy = (uv.y - centerY);
  let rotRad = (rotation * 0.017453292519943295f);
  let cosR = cos(rotRad);
  let sinR = sin(rotRad);
  let rdx = ((dxAc * cosR) + (dy * sinR));
  let rdy = ((dy * cosR) - (dxAc * sinR));
  return vec2f(((rdx / scale) + 0.5f), ((rdy / scale) + 0.5f));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  center: vec2f,
  scale: f32,
  rotation: f32,
  colorA: vec4f,
  colorB: vec4f,
  colorsArray: array<vec4f, 8>,
  positionsArray: array<vec4f, 2>,
  convertedColorsArray: array<vec3f, 8>,
  stopCount: f32,
  colorSpace: f32,
  innerGlow: f32,
  outerGlow: f32,
  contour: f32,
  angle: f32,
  speed: f32,
  _opacity: f32,
  _animTime: f32,
  _vfOriginX: f32,
  _vfOriginY: f32,
  _vfSpanX: f32,
  _vfSpanY: f32,
  _vfActiveRes: f32,
  _vfRBound: f32,
  _saRadius: f32,
  _saSides: f32,
  _saRounding: f32,
  _saInnerRatio: f32,
  _saRotation: f32,
  _saHeight: f32,
  _saOffset: f32,
  _saAperture: f32,
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

fn fieldSampleArg(uv: vec2f, field: texture_2d<f32>, originX: f32, originY: f32, spanX: f32, spanY: f32, activeResF: f32, rBound: f32) -> vec4f {
  let uxc = clamp(uv.x, originX, (originX + spanX));
  let uyc = clamp(uv.y, originY, (originY + spanY));
  let outsideDist = length(vec2f((uv.x - uxc), (uv.y - uyc)));
  let px = ((((uxc - originX) / spanX) * activeResF) - 0.5f);
  let py = ((((uyc - originY) / spanY) * activeResF) - 0.5f);
  let x0 = floor(px);
  let y0 = floor(py);
  let fx = (px - x0);
  let fy = (py - y0);
  let tx2 = (fx * fx);
  let tx3 = (tx2 * fx);
  let wx0 = (((tx3 * -0.5f) + tx2) - (fx * 0.5f));
  let wx1 = (((tx3 * 1.5f) - (tx2 * 2.5f)) + 1f);
  let wx2 = (((tx3 * -1.5f) + (tx2 * 2f)) + (fx * 0.5f));
  let wx3 = ((tx3 * 0.5f) - (tx2 * 0.5f));
  let ty2 = (fy * fy);
  let ty3 = (ty2 * fy);
  let wy0 = (((ty3 * -0.5f) + ty2) - (fy * 0.5f));
  let wy1 = (((ty3 * 1.5f) - (ty2 * 2.5f)) + 1f);
  let wy2 = (((ty3 * -1.5f) + (ty2 * 2f)) + (fy * 0.5f));
  let wy3 = ((ty3 * 0.5f) - (ty2 * 0.5f));
  let maxT = (activeResF - 1f);
  let cxm = u32(clamp((x0 - 1f), 0f, maxT));
  let cx0 = u32(clamp(x0, 0f, maxT));
  let cx1 = u32(clamp((x0 + 1f), 0f, maxT));
  let cx2 = u32(clamp((x0 + 2f), 0f, maxT));
  let cym = u32(clamp((y0 - 1f), 0f, maxT));
  let cy0 = u32(clamp(y0, 0f, maxT));
  let cy1 = u32(clamp((y0 + 1f), 0f, maxT));
  let cy2 = u32(clamp((y0 + 2f), 0f, maxT));
  let k00 = textureLoad(field, vec2u(cxm, cym), 0);
  let k01 = textureLoad(field, vec2u(cx0, cym), 0);
  let k02 = textureLoad(field, vec2u(cx1, cym), 0);
  let k03 = textureLoad(field, vec2u(cx2, cym), 0);
  let k10 = textureLoad(field, vec2u(cxm, cy0), 0);
  let k11 = textureLoad(field, vec2u(cx0, cy0), 0);
  let k12 = textureLoad(field, vec2u(cx1, cy0), 0);
  let k13 = textureLoad(field, vec2u(cx2, cy0), 0);
  let k20 = textureLoad(field, vec2u(cxm, cy1), 0);
  let k21 = textureLoad(field, vec2u(cx0, cy1), 0);
  let k22 = textureLoad(field, vec2u(cx1, cy1), 0);
  let k23 = textureLoad(field, vec2u(cx2, cy1), 0);
  let k30 = textureLoad(field, vec2u(cxm, cy2), 0);
  let k31 = textureLoad(field, vec2u(cx0, cy2), 0);
  let k32 = textureLoad(field, vec2u(cx1, cy2), 0);
  let k33 = textureLoad(field, vec2u(cx2, cy2), 0);
  let rAcc = (((((((((((((((((k00.r * wx0) * wy0) + ((k01.r * wx1) * wy0)) + ((k02.r * wx2) * wy0)) + ((k03.r * wx3) * wy0)) + ((k10.r * wx0) * wy1)) + ((k11.r * wx1) * wy1)) + ((k12.r * wx2) * wy1)) + ((k13.r * wx3) * wy1)) + ((k20.r * wx0) * wy2)) + ((k21.r * wx1) * wy2)) + ((k22.r * wx2) * wy2)) + ((k23.r * wx3) * wy2)) + ((k30.r * wx0) * wy3)) + ((k31.r * wx1) * wy3)) + ((k32.r * wx2) * wy3)) + ((k33.r * wx3) * wy3));
  let aAcc = (((((((((((((((((k00.a * wx0) * wy0) + ((k01.a * wx1) * wy0)) + ((k02.a * wx2) * wy0)) + ((k03.a * wx3) * wy0)) + ((k10.a * wx0) * wy1)) + ((k11.a * wx1) * wy1)) + ((k12.a * wx2) * wy1)) + ((k13.a * wx3) * wy1)) + ((k20.a * wx0) * wy2)) + ((k21.a * wx1) * wy2)) + ((k22.a * wx2) * wy2)) + ((k23.a * wx3) * wy2)) + ((k30.a * wx0) * wy3)) + ((k31.a * wx1) * wy3)) + ((k32.a * wx2) * wy3)) + ((k33.a * wx3) * wy3));
  let bl = mix(mix(k11, k12, fx), mix(k21, k22, fx), fy);
  let rMin = min(min(k11.r, k12.r), min(k21.r, k22.r));
  let rMax = max(max(k11.r, k12.r), max(k21.r, k22.r));
  let aMin = min(min(k11.a, k12.a), min(k21.a, k22.a));
  let aMax = max(max(k11.a, k12.a), max(k21.a, k22.a));
  let aOut = clamp(aAcc, aMin, aMax);
  let rNear = (clamp(rAcc, rMin, rMax) + outsideDist);
  let rCircle = (length(vec2f((uv.x - 0.5f), (uv.y - 0.5f))) - rBound);
  let rOut = mix(rNear, max(rCircle, rNear), smoothstep(0f, 0.25f, outsideDist));
  return vec4f(rOut, bl.g, bl.b, aOut);
}

@group(1) @binding(0) var compute_0: texture_2d<f32>;

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
  let sdfUV_8 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect);
  let f_7 = ((fieldSampleArg(sdfUV_8, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r / max(uniforms.n_n0.scale, 0.001));
  let suv_12 = sdfUV_8;
  let rel_11 = (suv_12.xy - vec2f(0.5, 0.5));
  let travelT_13 = vec2f(cos((uniforms.n_n0.angle * 0.017453292519943295)), sin((uniforms.n_n0.angle * 0.017453292519943295)));
  let a_10 = dot(rel_11, travelT_13);
  let tBase_14 = ((0.1 * uniforms.n_n0._animTime) - 0.3);
  let travelP_16 = vec2f((-(travelT_13.y)), travelT_13.x);
  let b_15 = (dot(rel_11, travelP_16) * -1.0);
  let aEff_9 = ((a_10 - mix(-0.6, 0.6, fract(tBase_14))) + ((value12(vec2f(((b_15 * 4.0) + (1.0 * 13.7)), ((fract(tBase_14) * 3.0) + (1.0 * 7.1)))) - 0.5) * 0.12));
  let inner1_6 = mix((0.8 + (0.8 * exp(((max((f_7 * -1.0), 0.0) * -1.0) / 0.12)))), 0.0, clamp((exp((((aEff_9 * aEff_9) * -1.0) / 0.0256)) * (smoothstep(0.0, 0.15, fract(tBase_14)) * (1.0 - smoothstep(0.85, 1.0, fract(tBase_14))))), 0.0, 1.0));
  let aEff_17 = ((a_10 - mix(-0.6, 0.6, fract((tBase_14 + 0.3333333333333333)))) + ((value12(vec2f(((b_15 * 4.0) + (2.0 * 13.7)), ((fract((tBase_14 + 0.3333333333333333)) * 3.0) + (2.0 * 7.1)))) - 0.5) * 0.12));
  let inner2_5 = mix(inner1_6, 0.0, clamp((exp((((aEff_17 * aEff_17) * -1.0) / 0.0256)) * (smoothstep(0.0, 0.15, fract((tBase_14 + 0.3333333333333333))) * (1.0 - smoothstep(0.85, 1.0, fract((tBase_14 + 0.3333333333333333)))))), 0.0, 1.0));
  let aEff_18 = ((a_10 - mix(-0.6, 0.6, fract((tBase_14 + 0.6666666666666666)))) + ((value12(vec2f(((b_15 * 4.0) + (3.0 * 13.7)), ((fract((tBase_14 + 0.6666666666666666)) * 3.0) + (3.0 * 7.1)))) - 0.5) * 0.12));
  let inner3_4 = mix(inner2_5, 0.0, clamp((exp((((aEff_18 * aEff_18) * -1.0) / 0.0256)) * (smoothstep(0.0, 0.15, fract((tBase_14 + 0.6666666666666666))) * (1.0 - smoothstep(0.85, 1.0, fract((tBase_14 + 0.6666666666666666)))))), 0.0, 1.0));
  let inside_19 = (1.0 - smoothstep(-0.003, 0.003, f_7));
  let inner_3 = pow(max((min(((inner3_4 * (2.0 * uniforms.n_n0.innerGlow)) + ((uniforms.n_n0.contour * 2.0) * exp(((abs(f_7) * -1.0) / 0.03)))), 1.0) * inside_19), 0.0), 1.2);
  let yy_21 = fract(((a_10 * 1.2) - fract(((tBase_14 * 3.0) - 0.1))));
  let outer_20 = ((((((0.9 * pow(exp(((max(f_7, 0.0) * -1.0) / (0.05 + (0.15 * uniforms.n_n0.outerGlow)))), 0.8)) * (0.5 + (smoothstep(0.3, 0.65, yy_21) * (1.0 - smoothstep(0.65, 1.0, yy_21))))) * 5.0) * uniforms.n_n0.outerGlow) * uniforms.n_n0.outerGlow) * (1.0 - inside_19));
  let frameFade_22 = (((smoothstep(-0.06, 0.1, suv_12.x) * (1.0 - smoothstep(0.9, 1.06, suv_12.x))) * smoothstep(-0.06, 0.1, suv_12.y)) * (1.0 - smoothstep(0.9, 1.06, suv_12.y)));
  let heat_2 = clamp(((clamp((inner_3 + outer_20), 0.0, 1.0) * frameFade_22) + (0.005 * (hash12(vec2f((suv_12.x * 991.3), (suv_12.y * 787.7))) - 0.5))), 0.0, 1.0);
  let stopMix_1 = gradientStopsInSpace(heat_2, uniforms.n_n0.colorsArray, uniforms.n_n0.positionsArray, uniforms.n_n0.convertedColorsArray, uniforms.n_n0.stopCount);
  let ramp_0 = vec4f(sRGBToP3(oklabToRgb(stopMix_1.conv)), stopMix_1.alpha);
  var composed_23 = vec4f(clamp(ramp_0.xyz, vec3f(0.0, 0.0, 0.0), vec3f(1.0, 1.0, 1.0)), (ramp_0.w * (clamp((inside_19 + outer_20), 0.0, 1.0) * frameFade_22)));
  return vec4f(linearToSrgb(tone_linear(composed_23.rgb)) * composed_23.a, composed_23.a);
}