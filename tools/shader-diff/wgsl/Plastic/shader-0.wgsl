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
  gradientAngle: f32,
  colorSpace: f32,
  thickness: f32,
  roughness: f32,
  reflectivity: f32,
  crumple: f32,
  crumpleScale: f32,
  crumpleCoverage: f32,
  seed: f32,
  edgeSoftness: f32,
  lightAngle: f32,
  speed: f32,
  shading: f32,
  rim: f32,
  rimColor: vec4f,
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

fn outsideShape(sdf: f32, pxH: f32) -> bool {
  return (sdf > (pxH * 2f));
}

fn fieldSampleFastArg(uv: vec2f, field: texture_2d<f32>, originX: f32, originY: f32, spanX: f32, spanY: f32, activeResF: f32, rBound: f32) -> vec4f {
  let uxc = clamp(uv.x, originX, (originX + spanX));
  let uyc = clamp(uv.y, originY, (originY + spanY));
  let outsideDist = length(vec2f((uv.x - uxc), (uv.y - uyc)));
  let px = ((((uxc - originX) / spanX) * activeResF) - 0.5f);
  let py = ((((uyc - originY) / spanY) * activeResF) - 0.5f);
  let x0 = floor(px);
  let y0 = floor(py);
  let fx = (px - x0);
  let fy = (py - y0);
  let maxT = (activeResF - 1f);
  let cx0 = u32(clamp(x0, 0f, maxT));
  let cx1 = u32(clamp((x0 + 1f), 0f, maxT));
  let cy0 = u32(clamp(y0, 0f, maxT));
  let cy1 = u32(clamp((y0 + 1f), 0f, maxT));
  let t00 = textureLoad(field, vec2u(cx0, cy0), 0);
  let t10 = textureLoad(field, vec2u(cx1, cy0), 0);
  let t01 = textureLoad(field, vec2u(cx0, cy1), 0);
  let t11 = textureLoad(field, vec2u(cx1, cy1), 0);
  let bl = mix(mix(t00, t10, fx), mix(t01, t11, fx), fy);
  let rNear = (bl.r + outsideDist);
  let rCircle = (length(vec2f((uv.x - 0.5f), (uv.y - 0.5f))) - rBound);
  let rOut = mix(rNear, max(rCircle, rNear), smoothstep(0f, 0.25f, outsideDist));
  return vec4f(rOut, bl.g, bl.b, bl.a);
}

fn offsetUV(uv: vec2f, ox: f32, oy: f32) -> vec2f {
  return vec2f((uv.x + ox), (uv.y + oy));
}

fn volumetricNormal(surf0: vec4f, surfX: vec4f, surfY: vec4f, eps: f32, gradClamp: f32) -> vec3f {
  let ddx = clamp(((surfX.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  let ddy = clamp(((surfY.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  return normalize(vec3f(ddx, ddy, -1f));
}

fn patternCoords(sdfUV: vec2f, surf0: vec4f, volumetric: f32) -> vec2f {
  return vec2f(select((sdfUV.x - 0.5f), surf0.y, (volumetric > 0.5f)), select((sdfUV.y - 0.5f), surf0.z, (volumetric > 0.5f)));
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

fn nudgeNormal(n: vec3f, dx: f32, dy: f32) -> vec3f {
  return normalize(vec3f((n.x + dx), (n.y + dy), n.z));
}

struct KeyLight {
  L: vec3f,
  H: vec3f,
}

fn keyLight(lx: f32, ly: f32, lz: f32) -> KeyLight {
  let L = normalize(vec3f(lx, ly, lz));
  let H = normalize((L + vec3f(0, 0, -1)));
  return KeyLight(L, H);
}

fn grazingView(n: vec3f) -> f32 {
  return (1f - clamp((n.z * -1f), 0f, 1f));
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

fn insideMask(sdf: f32, sharpEdge: f32, pxH: f32, minPixels: f32) -> f32 {
  let w = max((sharpEdge * 0.03125f), (pxH * minPixels));
  return clamp(((sdf * -1f) / w), 0f, 1f);
}

fn silhouetteAlpha(sdf: f32, edgeSoftness: f32, pxH: f32) -> f32 {
  let sharpEdge = max((edgeSoftness * 0.5f), 1e-3f);
  return smoothstep(0f, 1f, insideMask(sdf, sharpEdge, pxH, 1.5f));
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
  let sdfUV_3 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect);
  let s0_2 = fieldSampleArg(sdfUV_3, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sdf_1 = (s0_2.x / uniforms.n_n0.scale);
  let pxH_4 = (1.0 / uniforms._sys.viewportSize.y);
  var plastic_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let sdfUV_6 = sdfUV_3;
  let gDir_7 = vec2f(cos((uniforms.n_n0.gradientAngle * 0.017453292519943295)), sin((uniforms.n_n0.gradientAngle * 0.017453292519943295)));
  let sX_11 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sY_12 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let nBase_10 = volumetricNormal(s0_2, sX_11, sY_12, 0.01, 4.0);
  let pat_13 = patternCoords(sdfUV_6, s0_2, 1.0);
  let cScale_14 = (uniforms.n_n0.crumpleScale * 4.0);
  let seedOff_15 = (uniforms.n_n0.seed * 7.31);
  let cr0_16 = (abs(mxNoiseFloat2(vec2f(((pat_13.x * cScale_14) + seedOff_15), ((pat_13.y * cScale_14) + (seedOff_15 * 0.617))))) + (abs(mxNoiseFloat2(vec2f(((((pat_13.x * cScale_14) * 2.3) + 13.1) + seedOff_15), ((((pat_13.y * cScale_14) * 2.3) + 7.7) + (seedOff_15 * 0.617))))) * 0.45));
  let covThreshold_18 = (1.25 - (clamp(uniforms.n_n0.crumpleCoverage, 0.0, 1.0) * 1.75));
  let depthNorm_19 = clamp(((sdf_1 * -1.0) / max((uniforms.n_n0.thickness * 0.3), 0.01)), 0.0, 1.0);
  let crumpleW_17 = (((uniforms.n_n0.crumple * 0.9) * smoothstep((covThreshold_18 - 0.25), (covThreshold_18 + 0.25), ((mxNoiseFloat2(vec2f((((pat_13.x * 2.6) + seedOff_15) + 31.7), (((pat_13.y * 2.6) + (seedOff_15 * 0.617)) + 17.3))) * 0.5) + 0.5))) * ((smoothstep(0.0, 0.35, depthNorm_19) * 0.85) + 0.15));
  let n_9 = nudgeNormal(nBase_10, (((((abs(mxNoiseFloat2(vec2f((((pat_13.x + 0.012) * cScale_14) + seedOff_15), ((pat_13.y * cScale_14) + (seedOff_15 * 0.617))))) + (abs(mxNoiseFloat2(vec2f((((((pat_13.x + 0.012) * cScale_14) * 2.3) + 13.1) + seedOff_15), ((((pat_13.y * cScale_14) * 2.3) + 7.7) + (seedOff_15 * 0.617))))) * 0.45)) - cr0_16) / 0.012) * crumpleW_17) * 0.25), (((((abs(mxNoiseFloat2(vec2f(((pat_13.x * cScale_14) + seedOff_15), (((pat_13.y + 0.012) * cScale_14) + (seedOff_15 * 0.617))))) + (abs(mxNoiseFloat2(vec2f(((((pat_13.x * cScale_14) * 2.3) + 13.1) + seedOff_15), (((((pat_13.y + 0.012) * cScale_14) * 2.3) + 7.7) + (seedOff_15 * 0.617))))) * 0.45)) - cr0_16) / 0.012) * crumpleW_17) * 0.25));
  let light_21 = vec2f(cos(((uniforms.n_n0.lightAngle + (uniforms.n_n0._animTime * 6.0)) * 0.017453292519943295)), sin(((uniforms.n_n0.lightAngle + (uniforms.n_n0._animTime * 6.0)) * 0.017453292519943295)));
  let kl_20 = keyLight(light_21.x, light_21.y, -0.8);
  let lit_8 = ((clamp(dot(n_9, kl_20.L), -1.0, 1.0) * 0.5) + 0.5);
  let rough_25 = clamp(uniforms.n_n0.roughness, 0.0, 1.0);
  let soft_24 = mix(0.02, 0.45, rough_25);
  let win1S_23 = soft_24;
  let R_28 = reflect(vec3f(0.0, 0.0, 1.0), n_9);
  let rJit_29 = ((rough_25 * mxNoiseFloat2(vec2f((pat_13.x * 140.0), (pat_13.y * 140.0)))) * 0.18);
  let eu_27 = (((R_28.x * light_21.x) + (R_28.y * light_21.y)) + rJit_29);
  let win1X_26 = eu_27;
  let win2S_30 = soft_24;
  let ev_32 = (((R_28.x * (light_21.y * -1.0)) + (R_28.y * light_21.x)) + (rJit_29 * 0.6));
  let win2X_31 = ev_32;
  let mullS_33 = (soft_24 * 0.6);
  let mullX_34 = ev_32;
  let strip1S_35 = (soft_24 * 0.7);
  let strip1X_36 = eu_27;
  let strip2S_37 = soft_24;
  let strip2X_38 = ev_32;
  let fill1S_39 = (soft_24 * 1.6);
  let fill1X_40 = eu_27;
  let fill2S_41 = (soft_24 * 1.6);
  let fill2X_42 = ev_32;
  let grazing_43 = grazingView(n_9);
  let specular_22 = (((((((((smoothstep((0.18 - win1S_23), (0.18 + win1S_23), win1X_26) * (1.0 - smoothstep((0.95 - win1S_23), (0.95 + win1S_23), win1X_26))) * (smoothstep((-0.42 - win2S_30), (-0.42 + win2S_30), win2X_31) * (1.0 - smoothstep((0.42 - win2S_30), (0.42 + win2S_30), win2X_31)))) * (1.0 - (((smoothstep((-0.045 - mullS_33), (-0.045 + mullS_33), mullX_34) * (1.0 - smoothstep((0.045 - mullS_33), (0.045 + mullS_33), mullX_34))) * 0.8) * (1.0 - rough_25)))) + (((smoothstep((-0.16 - strip1S_35), (-0.16 + strip1S_35), strip1X_36) * (1.0 - smoothstep((-0.05 - strip1S_35), (-0.05 + strip1S_35), strip1X_36))) * (smoothstep((-0.85 - strip2S_37), (-0.85 + strip2S_37), strip2X_38) * (1.0 - smoothstep((0.85 - strip2S_37), (0.85 + strip2S_37), strip2X_38)))) * 0.85)) + (((smoothstep((-0.95 - fill1S_39), (-0.95 + fill1S_39), fill1X_40) * (1.0 - smoothstep((-0.45 - fill1S_39), (-0.45 + fill1S_39), fill1X_40))) * (smoothstep((-0.6 - fill2S_41), (-0.6 + fill2S_41), fill2X_42) * (1.0 - smoothstep((0.6 - fill2S_41), (0.6 + fill2S_41), fill2X_42)))) * 0.3)) + (clamp(((eu_27 * 0.5) + 0.5), 0.0, 1.0) * 0.12)) * (1.0 - (rough_25 * 0.55))) + ((pow(clamp(dot(n_9, kl_20.H), 0.0, 1.0), exp2(mix(9.0, 4.0, rough_25))) * 1.6) * (1.0 - (rough_25 * 0.7)))) * (uniforms.n_n0.reflectivity * (0.3 + (pow(grazing_43, 3.0) * 0.7))));
  let plasticRgb_5 = (((((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, smoothstep(0.0, 1.0, clamp(((dot((sdfUV_6.xy - vec2f(0.5, 0.5)), gDir_7) / 0.7) + 0.5), 0.0, 1.0)))).rgb * mix((1.0 - (uniforms.n_n0.shading * 0.75)), (1.0 + (uniforms.n_n0.shading * 0.45)), lit_8)) * mix(0.6, 1.0, smoothstep(0.0, 1.0, clamp(((sdf_1 * -1.0) / 0.025), 0.0, 1.0)))) + vec3f(specular_22)) + (uniforms.n_n0.rimColor.rgb * (((pow(grazing_43, 3.5) * uniforms.n_n0.rim) * ((lit_8 * 0.6) + 0.4)) * 1.6)));
  plastic_0 = vec4f(plasticRgb_5, silhouetteAlpha(sdf_1, uniforms.n_n0.edgeSoftness, pxH_4));
}
  var composed_44 = plastic_0;
  return vec4f(linearToSrgb(tone_linear(composed_44.rgb)) * composed_44.a, composed_44.a);
}