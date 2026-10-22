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
  lightColor: vec4f,
  darkColor: vec4f,
  brushAngle: f32,
  anisotropy: f32,
  grain: f32,
  grainScale: f32,
  roughness: f32,
  environment: f32,
  envRotation: f32,
  lightAngle: f32,
  speed: f32,
  bevelWidth: f32,
  bevelShape: f32,
  edgeSoftness: f32,
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

fn perspectiveViewRay(uv: vec2f, aspect: f32, fov: f32) -> vec3f {
  return normalize(vec3f((((uv.x - 0.5f) * aspect) * fov), ((uv.y - 0.5f) * fov), 1f));
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

fn studio5Softbox(rx: f32, ry: f32, envC: f32, envS: f32, keyRad: f32, drift: f32, envStr: f32, skyGain: f32) -> f32 {
  let ex = ((rx * envC) - (ry * envS));
  let ey = ((rx * envS) + (ry * envC));
  let p = vec2f(ex, ey);
  let sky = smoothstep(-0.7f, 0.85f, ey);
  var lum = ((sky * sky) * skyGain);
  let r0 = (keyRad * 1f);
  lum = (lum + ((smoothstep(r0, (r0 * 0.2f), length((p - vec2f((0f + drift), 0.55f)))) * 1.7f) * envStr));
  let r1 = (keyRad * 1.15f);
  lum = (lum + ((smoothstep(r1, (r1 * 0.2f), length((p - vec2f((-0.55f + drift), 0.18f)))) * 1.05f) * envStr));
  let r2 = (keyRad * 1f);
  lum = (lum + ((smoothstep(r2, (r2 * 0.2f), length((p - vec2f((0.55f + drift), 0.1f)))) * 1f) * envStr));
  let r3 = (keyRad * 1.3f);
  lum = (lum + ((smoothstep(r3, (r3 * 0.2f), length((p - vec2f((0.2f + drift), -0.5f)))) * 0.7f) * envStr));
  let r4 = (keyRad * 1.1f);
  lum = (lum + ((smoothstep(r4, (r4 * 0.2f), length((p - vec2f((-0.35f + drift), -0.25f)))) * 0.85f) * envStr));
  return lum;
}

struct TangentFrame {
  tangent: vec3f,
  bitangent: vec3f,
}

fn orthonormalTangentFrame(normal: vec3f, dir: vec3f) -> TangentFrame {
  let T3 = normalize((dir - (normal * dot(normal, dir))));
  let B3 = vec3f(((normal.y * T3.z) - (normal.z * T3.y)), ((normal.z * T3.x) - (normal.x * T3.z)), ((normal.x * T3.y) - (normal.y * T3.x)));
  return TangentFrame(T3, B3);
}

struct WardSpecularInput {
  normal: vec3f,
  tangent: vec3f,
  light: vec3f,
  view: vec3f,
  alphaAlong: f32,
  alphaAcross: f32,
}

fn wardAnisotropicSpecular(p: WardSpecularInput) -> f32 {
  let frame = orthonormalTangentFrame(p.normal, p.tangent);
  let H = normalize((p.light - p.view));
  let hT = dot(H, frame.tangent);
  let hB = dot(H, frame.bitangent);
  let hN = clamp(abs(dot(H, p.normal)), 1e-4f, 1f);
  let aT = max(p.alphaAlong, 1e-4f);
  let aB = max(p.alphaAcross, 1e-4f);
  let expo = ((((hT / aT) * (hT / aT)) + ((hB / aB) * (hB / aB))) / ((hN + 1f) * 0.5f));
  return exp((expo * -1f));
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
  var brushedMetal_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let viewI_8 = perspectiveViewRay(uv, (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y), 0.6);
  let sX_11 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sY_12 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let nGeo_10 = volumetricNormal(s0_2, sX_11, sY_12, 0.01, 5.0);
  let brushT_14 = vec2f(cos((uniforms.n_n0.brushAngle * 0.017453292519943295)), sin((uniforms.n_n0.brushAngle * 0.017453292519943295)));
  let brushP_13 = vec2f((-(brushT_14.y)), brushT_14.x);
  let pat_18 = patternCoords(sdfUV_3, s0_2, 1.0);
  let along_17 = dot(pat_18, brushT_14);
  let driftA_19 = (sin((uniforms.n_n0._animTime * 0.05)) * 0.35);
  let across_20 = dot(pat_18, brushP_13);
  let sheen_16 = mxNoiseFloat2(vec2f(((along_17 * 1.3) + driftA_19), (across_20 * 6.5)));
  let aniso_22 = uniforms.n_n0.anisotropy;
  let sheenAmt_21 = ((aniso_22 * 0.5) + 0.08);
  let dens_24 = (uniforms.n_n0.grainScale * 150.0);
  let lines_23 = (mxNoiseFloat2(vec2f((along_17 * 4.5), (across_20 * dens_24))) + (mxNoiseFloat2(vec2f(((along_17 * 8.0) + 3.1), ((across_20 * (dens_24 * 2.3)) + 7.7))) * 0.5));
  let rAcross_15 = ((sheen_16 * sheenAmt_21) + (lines_23 * (uniforms.n_n0.grain * 0.16)));
  let rAlong_25 = (mxNoiseFloat2(vec2f((((along_17 * 1.3) + 15.21) + driftA_19), (across_20 * 6.5))) * (sheenAmt_21 * 0.12));
  let n_9 = normalize(vec3f(((nGeo_10.x + (brushP_13.x * rAcross_15)) + (brushT_14.x * rAlong_25)), ((nGeo_10.y + (brushP_13.y * rAcross_15)) + (brushT_14.y * rAlong_25)), nGeo_10.z));
  let R_7 = reflect(viewI_8, n_9);
  let rough_28 = clamp(uniforms.n_n0.roughness, 0.0, 1.0);
  let step_27 = (((aniso_22 * ((rough_28 * 0.6) + 0.25)) * 0.42) / 4.0);
  let off_26 = (step_27 * -4.0);
  let env_29 = vec2f(cos((uniforms.n_n0.envRotation * 0.017453292519943295)), sin((uniforms.n_n0.envRotation * 0.017453292519943295)));
  let keyRad_30 = mix(0.42, 0.16, (1.0 - rough_28));
  let drift_31 = (sin((uniforms.n_n0._animTime * 0.07)) * 0.05);
  let envStr_32 = uniforms.n_n0.environment;
  let off_33 = (step_27 * -3.0);
  let off_34 = (step_27 * -2.0);
  let off_35 = (step_27 * -1.0);
  let off_36 = (step_27 * 0.0);
  let off_37 = (step_27 * 1.0);
  let off_38 = (step_27 * 2.0);
  let off_39 = (step_27 * 3.0);
  let off_40 = (step_27 * 4.0);
  let envLum_6 = (((((((((0.0 + (studio5Softbox((R_7.x + (brushT_14.x * off_26)), (R_7.y + (brushT_14.y * off_26)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.03193849024655744)) + (studio5Softbox((R_7.x + (brushT_14.x * off_33)), (R_7.y + (brushT_14.y * off_33)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.07063054268601367)) + (studio5Softbox((R_7.x + (brushT_14.x * off_34)), (R_7.y + (brushT_14.y * off_34)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.12450627184903387)) + (studio5Softbox((R_7.x + (brushT_14.x * off_35)), (R_7.y + (brushT_14.y * off_35)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.17494858755412865)) + (studio5Softbox((R_7.x + (brushT_14.x * off_36)), (R_7.y + (brushT_14.y * off_36)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.19595221532853271)) + (studio5Softbox((R_7.x + (brushT_14.x * off_37)), (R_7.y + (brushT_14.y * off_37)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.17494858755412865)) + (studio5Softbox((R_7.x + (brushT_14.x * off_38)), (R_7.y + (brushT_14.y * off_38)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.12450627184903387)) + (studio5Softbox((R_7.x + (brushT_14.x * off_39)), (R_7.y + (brushT_14.y * off_39)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.07063054268601367)) + (studio5Softbox((R_7.x + (brushT_14.x * off_40)), (R_7.y + (brushT_14.y * off_40)), env_29.x, env_29.y, keyRad_30, drift_31, envStr_32, 0.38) * 0.03193849024655744));
  let light_42 = vec2f(cos((uniforms.n_n0.lightAngle * 0.017453292519943295)), sin((uniforms.n_n0.lightAngle * 0.017453292519943295)));
  let L_41 = normalize(vec3f(light_42.x, light_42.y, -0.7));
  let baseA_43 = mix(0.05, 0.45, rough_28);
  let grazing_44 = (1.0 - clamp((-(dot(n_9, viewI_8))), 0.0, 1.0));
  let lit_5 = clamp(((envLum_6 + (wardAnisotropicSpecular(WardSpecularInput(n_9, vec3f(brushT_14.x, brushT_14.y, 0.0), L_41, viewI_8, (baseA_43 * (1.0 + (aniso_22 * 7.0))), max((baseA_43 * (1.0 - (aniso_22 * 0.55))), 0.02))) * mix(0.45, 1.1, (1.0 - rough_28)))) * (1.0 + (pow(grazing_44, 3.0) * 0.6))), 0.0, 1.7);
  brushedMetal_0 = vec4f(mix(uniforms.n_n0.darkColor.rgb, uniforms.n_n0.lightColor.rgb, vec3f(lit_5)), silhouetteAlpha(sdf_1, uniforms.n_n0.edgeSoftness, pxH_4));
}
  var composed_45 = brushedMetal_0;
  return vec4f(linearToSrgb(tone_linear(composed_45.rgb)) * composed_45.a, composed_45.a);
}