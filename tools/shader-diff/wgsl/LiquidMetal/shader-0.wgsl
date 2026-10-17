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
  turbulence: f32,
  ripple: f32,
  warp: f32,
  speed: f32,
  environment: f32,
  envRotation: f32,
  sharpness: f32,
  dispersion: f32,
  lightAngle: f32,
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

fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }

fn hash22(p: vec2f) -> vec2f {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  let h = (u.x ^ u.y);
  let prod = (vec2u(h, h) * vec2u(3141592653, 1618033988));
  return (vec2f(f32(prod.x), f32(prod.y)) / 4294967296f);
}

fn perlin12d(p: vec2f) -> vec3f {
  let i = floor(p);
  let f = fract(p);
  let u = (((f * f) * f) * ((f * ((f * 6f) - 15f)) + 10f));
  let du = (((f * f) * ((f * (f - 2f)) + 1f)) * 30f);
  let ga = ((hash22((i + vec2f())) * 2f) - 1f);
  let gb = ((hash22((i + vec2f(1, 0))) * 2f) - 1f);
  let gc = ((hash22((i + vec2f(0, 1))) * 2f) - 1f);
  let gd = ((hash22((i + vec2f(1))) * 2f) - 1f);
  let va = dot(ga, (f - vec2f()));
  let vb = dot(gb, (f - vec2f(1, 0)));
  let vc = dot(gc, (f - vec2f(0, 1)));
  let vd = dot(gd, (f - vec2f(1)));
  let k = (((va - vb) - vc) + vd);
  let value = (((va + (u.x * (vb - va))) + (u.y * (vc - va))) + ((u.x * u.y) * k));
  let deriv = ((((ga + ((gb - ga) * u.x)) + ((gc - ga) * u.y)) + ((((ga - gb) - gc) + gd) * (u.x * u.y))) + (du * (((vec2f(u.y, u.x) * k) + vec2f(vb, vc)) - va)));
  return vec3f(value, deriv.x, deriv.y);
}

fn clampedPerlinGrad(q: vec2f, gain: f32) -> vec2f {
  const gMax = 4f;
  let nd = perlin12d(q);
  return vec2f(clamp((nd.y * gain), (gMax * -1f), gMax), clamp((nd.z * gain), (gMax * -1f), gMax));
}

fn flowWarpOffset(warpInput: vec2f, flowT: f32) -> vec2f {
  let wx = (sin((((warpInput.x * 1.7f) + (flowT * 0.7f)) + 1.7f)) * cos((((warpInput.y * 1.3f) + (flowT * 0.5f)) + 2.3f)));
  let wy = (cos((((warpInput.x * 1.4f) + (flowT * 0.9f)) + 8.7f)) * sin((((warpInput.y * 1.6f) + (flowT * 0.6f)) + 5.1f)));
  return vec2f(wx, wy);
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

fn sharpGlint(ndh: f32, sharp: f32, cfg: vec4f) -> f32 {
  return (pow(ndh, mix(cfg.x, cfg.y, sharp)) * mix(cfg.z, cfg.w, sharp));
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
  var liquidMetal_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let dark_5 = uniforms.n_n0.darkColor.rgb;
  let light_6 = uniforms.n_n0.lightColor.rgb;
  let viewI_11 = perspectiveViewRay(uv, (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y), 0.6);
  let sX_14 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sY_15 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let nGeo_13 = volumetricNormal(s0_2, sX_14, sY_15, 0.01, 5.0);
  let pat_18 = patternCoords(sdfUV_3, s0_2, 1.0);
  let freq_19 = (uniforms.n_n0.ripple * 2.6);
  let q_17 = vec2f((pat_18.x * freq_19), (pat_18.y * freq_19));
  let flowT_20 = (uniforms.n_n0._animTime * 0.12);
  let hg_16 = clampedPerlinGrad((q_17 + (flowWarpOffset((q_17 * 0.6), flowT_20) * (uniforms.n_n0.warp * 0.9))), 0.62);
  let relief_21 = (uniforms.n_n0.turbulence * 0.5);
  let n_12 = nudgeNormal(nGeo_13, ((hg_16.x * relief_21) * -1.0), ((hg_16.y * relief_21) * -1.0));
  let R_10 = reflect(viewI_11, n_12);
  let env_22 = vec2f(cos((uniforms.n_n0.envRotation * 0.017453292519943295)), sin((uniforms.n_n0.envRotation * 0.017453292519943295)));
  let ey_9 = ((R_10.x * env_22.y) + (R_10.y * env_22.x));
  let sky_8 = smoothstep(-0.7, 0.85, ey_9);
  let sharp_25 = clamp(uniforms.n_n0.sharpness, 0.0, 1.0);
  let keyRad_24 = mix(0.5, 0.18, sharp_25);
  let r0_23 = (keyRad_24 * 1.05);
  let ex_27 = ((R_10.x * env_22.x) - (R_10.y * env_22.y));
  let p_26 = vec2f(ex_27, ey_9);
  let drift_28 = (sin((uniforms.n_n0._animTime * 0.07)) * 0.05);
  let envStr_29 = uniforms.n_n0.environment;
  let r1_30 = (keyRad_24 * 1.25);
  let r2_31 = (keyRad_24 * 1.15);
  let light_35 = vec2f(cos((uniforms.n_n0.lightAngle * 0.017453292519943295)), sin((uniforms.n_n0.lightAngle * 0.017453292519943295)));
  let kl_34 = keyLight(light_35.x, light_35.y, -0.7);
  let Hh_33 = kl_34.H;
  let spec_32 = sharpGlint(clamp((((n_12.x * Hh_33.x) + (n_12.y * Hh_33.y)) + (n_12.z * Hh_33.z)), 0.0, 1.0), sharp_25, vec4f(60.0, 900.0, 0.4, 1.4));
  let lumG_7 = ((((((sky_8 * sky_8) * 0.4) + ((smoothstep(r0_23, (r0_23 * 0.2), length((p_26 - vec2f((0.0 + drift_28), 0.55)))) * 1.75) * envStr_29)) + ((smoothstep(r1_30, (r1_30 * 0.2), length((p_26 - vec2f((-0.45 + drift_28), -0.4)))) * 1.15) * envStr_29)) + ((smoothstep(r2_31, (r2_31 * 0.2), length((p_26 - vec2f((0.55 + drift_28), 0.1)))) * 1.1) * envStr_29)) + spec_32);
  let dth_39 = ((uniforms.n_n0.dispersion * 0.09) * (0.4 + ((1.0 - clamp((n_12.z * -1.0), 0.0, 1.0)) * 1.3)));
  let ey_38 = (((R_10.x + (R_10.y * dth_39)) * env_22.y) + ((R_10.y - (R_10.x * dth_39)) * env_22.x));
  let sky_37 = smoothstep(-0.7, 0.85, ey_38);
  let r0_40 = (keyRad_24 * 1.05);
  let ex_42 = (((R_10.x + (R_10.y * dth_39)) * env_22.x) - ((R_10.y - (R_10.x * dth_39)) * env_22.y));
  let p_41 = vec2f(ex_42, ey_38);
  let r1_43 = (keyRad_24 * 1.25);
  let r2_44 = (keyRad_24 * 1.15);
  let lumBoffset_36 = ((((((sky_37 * sky_37) * 0.4) + ((smoothstep(r0_40, (r0_40 * 0.2), length((p_41 - vec2f((0.0 + drift_28), 0.55)))) * 1.75) * envStr_29)) + ((smoothstep(r1_43, (r1_43 * 0.2), length((p_41 - vec2f((-0.45 + drift_28), -0.4)))) * 1.15) * envStr_29)) + ((smoothstep(r2_44, (r2_44 * 0.2), length((p_41 - vec2f((0.55 + drift_28), 0.1)))) * 1.1) * envStr_29)) + spec_32);
  liquidMetal_0 = vec4f(mix(dark_5.x, light_6.x, clamp(((lumG_7 * 2.0) - lumBoffset_36), 0.0, 1.8)), mix(dark_5.y, light_6.y, clamp(lumG_7, 0.0, 1.8)), mix(dark_5.z, light_6.z, clamp(lumBoffset_36, 0.0, 1.8)), silhouetteAlpha(sdf_1, uniforms.n_n0.edgeSoftness, pxH_4));
}
  var composed_45 = liquidMetal_0;
  return vec4f(linearToSrgb(tone_linear(composed_45.rgb)) * composed_45.a, composed_45.a);
}