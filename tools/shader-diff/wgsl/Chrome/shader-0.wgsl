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
  tint: vec4f,
  warmColor: vec4f,
  coolColor: vec4f,
  bevelWidth: f32,
  bevelShape: f32,
  curvature: f32,
  waviness: f32,
  environment: f32,
  envRotation: f32,
  softness: f32,
  spectral: f32,
  dispersion: f32,
  shadows: f32,
  speed: f32,
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

fn nudgeNormal(n: vec3f, dx: f32, dy: f32) -> vec3f {
  return normalize(vec3f((n.x + dx), (n.y + dy), n.z));
}

fn tonemapNeutral(color: vec3f) -> vec3f {
  let x = min(color.x, min(color.y, color.z));
  let offset = select(0.04f, (x - ((6.25f * x) * x)), (x < 0.08f));
  var c = (color - offset);
  let peak = max(c.x, max(c.y, c.z));
  if ((peak < 0.76f)) {
    return c;
  }
  const dd = 0.24;
  let newPeak = (1f - ((dd * dd) / ((peak + dd) - 0.76f)));
  c = (c * (newPeak / peak));
  let g = (1f - (1f / ((0.15f * (peak - newPeak)) + 1f)));
  return mix(c, vec3f(newPeak), g);
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
  var chrome_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let coolC_6 = uniforms.n_n0.coolColor.rgb;
  let shadows_7 = uniforms.n_n0.shadows;
  let viewI_13 = perspectiveViewRay(uv, (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y), 0.55);
  let sX_16 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sY_17 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let nGeo_15 = volumetricNormal(s0_2, sX_16, sY_17, 0.01, 5.0);
  let pat_19 = patternCoords(sdfUV_3, s0_2, 1.0);
  let wg_18 = clampedPerlinGrad(vec2f(((pat_19.x * 1.05) + (uniforms.n_n0._animTime * 0.08)), (pat_19.y * 1.05)), 1.0);
  let wAmt_20 = (uniforms.n_n0.waviness * 0.2);
  let n_14 = nudgeNormal(nGeo_15, (wg_18.x * wAmt_20), (wg_18.y * wAmt_20));
  let R_12 = reflect(viewI_13, n_14);
  let rotA_22 = ((uniforms.n_n0.envRotation * 0.017453292519943295) + (sin((uniforms.n_n0._animTime * 0.23)) * 0.05));
  let envS_21 = sin(rotA_22);
  let upY_23 = (R_12.y * -1.0);
  let envC_24 = cos(rotA_22);
  let ry0_11 = ((R_12.x * envS_21) + (upY_23 * envC_24));
  let nod_26 = (sin(((uniforms.n_n0._animTime * 0.31) + 0.9)) * 0.12);
  let nc_25 = cos(nod_26);
  let orbit_29 = ((sin((uniforms.n_n0._animTime * 0.5)) * 0.4) + (sin(((uniforms.n_n0._animTime * 0.19) + 1.7)) * 0.2));
  let oc_28 = cos(orbit_29);
  let rx0_30 = ((R_12.x * envC_24) - (upY_23 * envS_21));
  let os_31 = sin(orbit_29);
  let rz0_27 = ((R_12.z * oc_28) - (rx0_30 * os_31));
  let ns_32 = sin(nod_26);
  let ry_10 = ((ry0_11 * nc_25) - (rz0_27 * ns_32));
  let grazing_34 = (1.0 - clamp((-(dot(n_14, viewI_13))), 0.0, 1.0));
  let delta_33 = (uniforms.n_n0.dispersion * (0.004 + ((grazing_34 * grazing_34) * 0.06)));
  let ey_9 = (ry_10 + delta_33);
  let up_8 = smoothstep(-0.65, 1.0, ey_9);
  let cd_35 = ((ey_9 + 0.45) / 0.2);
  let spectral_36 = uniforms.n_n0.spectral;
  let rx_40 = ((rx0_30 * oc_28) + (R_12.z * os_31));
  let ex_39 = rx_40;
  let horiz_38 = (ex_39 / (length(vec2f(ex_39, ey_9)) + 0.0001));
  let coolSide_37 = (0.45 + (0.55 * smoothstep(-0.7, 0.7, (horiz_38 * -1.0))));
  let soft_43 = uniforms.n_n0.softness;
  let softEff_42 = (soft_43 + (smoothstep(0.75, 1.0, (n_14.z * -1.0)) * 0.2));
  let boxSoft_41 = (0.05 + (softEff_42 * 0.4));
  let rz_49 = ((rz0_27 * nc_25) + (ry0_11 * ns_32));
  let ez_48 = rz_49;
  let pz_47 = max((0.32 - ez_48), 0.06);
  let px_46 = (ex_39 / pz_47);
  let bx_45 = (abs(px_46) - 0.62);
  let py_51 = (ey_9 / pz_47);
  let by_50 = (abs((py_51 - 0.3)) - 0.28);
  let dbox_44 = (length(max(vec2f(bx_45, by_50), vec2f(0.0, 0.0))) + min(max(bx_45, by_50), 0.0));
  let facing_52 = smoothstep(0.18, -0.1, ez_48);
  let warmC_53 = uniforms.n_n0.warmColor.rgb;
  let dOut_54 = max(dbox_44, 0.0);
  let gw_55 = (0.065 + (soft_43 * 0.05));
  let warmSide_56 = (0.35 + (0.65 * smoothstep(-0.7, 0.7, horiz_38)));
  let fb_57 = ((ey_9 + 0.12) / 0.05);
  let cbx_59 = (abs(px_46) - 0.7);
  let cby_60 = (abs((py_51 + 0.78)) - 0.05);
  let dcard_58 = (length(max(vec2f(cbx_59, cby_60), vec2f(0.0, 0.0))) + min(max(cbx_59, cby_60), 0.0));
  let envRt_5 = (((((((mix(mix(vec3f(0.17, 0.175, 0.19), ((coolC_6 * 0.03) + vec3f(0.003, 0.003, 0.004)), vec3f(shadows_7)), vec3f(0.56, 0.57, 0.6), vec3f((up_8 * up_8))) + (vec3f(0.55, 0.55, 0.56) * smoothstep(0.3, 0.9, ey_9))) + (coolC_6 * (((exp(((cd_35 * cd_35) * -1.0)) * spectral_36) * 0.2) * coolSide_37))) + (vec3f(2.9, 2.87, 2.82) * (smoothstep(boxSoft_41, (boxSoft_41 * -0.6), dbox_44) * facing_52))) * mix(1.0, mix(0.55, 0.07, shadows_7), smoothstep(0.5, 0.92, abs(ex_39)))) + (warmC_53 * (((((exp((((dOut_54 / gw_55) * (dOut_54 / gw_55)) * -1.0)) * smoothstep(-0.06, 0.1, dbox_44)) * spectral_36) * 1.3) * warmSide_56) * facing_52))) + (warmC_53 * ((((exp(((fb_57 * fb_57) * -1.0)) * spectral_36) * 0.5) * warmSide_56) * (1.0 - facing_52)))) + (vec3f(0.5, 0.5, 0.52) * (smoothstep(boxSoft_41, (boxSoft_41 * -0.6), dcard_58) * facing_52)));
  let ey_63 = ry_10;
  let up_62 = smoothstep(-0.65, 1.0, ey_63);
  let cd_64 = ((ey_63 + 0.45) / 0.2);
  let ex_67 = rx_40;
  let horiz_66 = (ex_67 / (length(vec2f(ex_67, ey_63)) + 0.0001));
  let coolSide_65 = (0.45 + (0.55 * smoothstep(-0.7, 0.7, (horiz_66 * -1.0))));
  let boxSoft_68 = (0.05 + (softEff_42 * 0.4));
  let ez_73 = rz_49;
  let pz_72 = max((0.32 - ez_73), 0.06);
  let px_71 = (ex_67 / pz_72);
  let bx_70 = (abs(px_71) - 0.62);
  let py_75 = (ey_63 / pz_72);
  let by_74 = (abs((py_75 - 0.3)) - 0.28);
  let dbox_69 = (length(max(vec2f(bx_70, by_74), vec2f(0.0, 0.0))) + min(max(bx_70, by_74), 0.0));
  let facing_76 = smoothstep(0.18, -0.1, ez_73);
  let dOut_77 = max(dbox_69, 0.0);
  let gw_78 = (0.065 + (soft_43 * 0.05));
  let warmSide_79 = (0.35 + (0.65 * smoothstep(-0.7, 0.7, horiz_66)));
  let fb_80 = ((ey_63 + 0.12) / 0.05);
  let cbx_82 = (abs(px_71) - 0.7);
  let cby_83 = (abs((py_75 + 0.78)) - 0.05);
  let dcard_81 = (length(max(vec2f(cbx_82, cby_83), vec2f(0.0, 0.0))) + min(max(cbx_82, cby_83), 0.0));
  let envG_61 = (((((((mix(mix(vec3f(0.17, 0.175, 0.19), ((coolC_6 * 0.03) + vec3f(0.003, 0.003, 0.004)), vec3f(shadows_7)), vec3f(0.56, 0.57, 0.6), vec3f((up_62 * up_62))) + (vec3f(0.55, 0.55, 0.56) * smoothstep(0.3, 0.9, ey_63))) + (coolC_6 * (((exp(((cd_64 * cd_64) * -1.0)) * spectral_36) * 0.2) * coolSide_65))) + (vec3f(2.9, 2.87, 2.82) * (smoothstep(boxSoft_68, (boxSoft_68 * -0.6), dbox_69) * facing_76))) * mix(1.0, mix(0.55, 0.07, shadows_7), smoothstep(0.5, 0.92, abs(ex_67)))) + (warmC_53 * (((((exp((((dOut_77 / gw_78) * (dOut_77 / gw_78)) * -1.0)) * smoothstep(-0.06, 0.1, dbox_69)) * spectral_36) * 1.3) * warmSide_79) * facing_76))) + (warmC_53 * ((((exp(((fb_80 * fb_80) * -1.0)) * spectral_36) * 0.5) * warmSide_79) * (1.0 - facing_76)))) + (vec3f(0.5, 0.5, 0.52) * (smoothstep(boxSoft_68, (boxSoft_68 * -0.6), dcard_81) * facing_76)));
  let ey_86 = (ry_10 - delta_33);
  let up_85 = smoothstep(-0.65, 1.0, ey_86);
  let cd_87 = ((ey_86 + 0.45) / 0.2);
  let ex_90 = rx_40;
  let horiz_89 = (ex_90 / (length(vec2f(ex_90, ey_86)) + 0.0001));
  let coolSide_88 = (0.45 + (0.55 * smoothstep(-0.7, 0.7, (horiz_89 * -1.0))));
  let boxSoft_91 = (0.05 + (softEff_42 * 0.4));
  let ez_96 = rz_49;
  let pz_95 = max((0.32 - ez_96), 0.06);
  let px_94 = (ex_90 / pz_95);
  let bx_93 = (abs(px_94) - 0.62);
  let py_98 = (ey_86 / pz_95);
  let by_97 = (abs((py_98 - 0.3)) - 0.28);
  let dbox_92 = (length(max(vec2f(bx_93, by_97), vec2f(0.0, 0.0))) + min(max(bx_93, by_97), 0.0));
  let facing_99 = smoothstep(0.18, -0.1, ez_96);
  let dOut_100 = max(dbox_92, 0.0);
  let gw_101 = (0.065 + (soft_43 * 0.05));
  let warmSide_102 = (0.35 + (0.65 * smoothstep(-0.7, 0.7, horiz_89)));
  let fb_103 = ((ey_86 + 0.12) / 0.05);
  let cbx_105 = (abs(px_94) - 0.7);
  let cby_106 = (abs((py_98 + 0.78)) - 0.05);
  let dcard_104 = (length(max(vec2f(cbx_105, cby_106), vec2f(0.0, 0.0))) + min(max(cbx_105, cby_106), 0.0));
  let envBt_84 = (((((((mix(mix(vec3f(0.17, 0.175, 0.19), ((coolC_6 * 0.03) + vec3f(0.003, 0.003, 0.004)), vec3f(shadows_7)), vec3f(0.56, 0.57, 0.6), vec3f((up_85 * up_85))) + (vec3f(0.55, 0.55, 0.56) * smoothstep(0.3, 0.9, ey_86))) + (coolC_6 * (((exp(((cd_87 * cd_87) * -1.0)) * spectral_36) * 0.2) * coolSide_88))) + (vec3f(2.9, 2.87, 2.82) * (smoothstep(boxSoft_91, (boxSoft_91 * -0.6), dbox_92) * facing_99))) * mix(1.0, mix(0.55, 0.07, shadows_7), smoothstep(0.5, 0.92, abs(ex_90)))) + (warmC_53 * (((((exp((((dOut_100 / gw_101) * (dOut_100 / gw_101)) * -1.0)) * smoothstep(-0.06, 0.1, dbox_92)) * spectral_36) * 1.3) * warmSide_102) * facing_99))) + (warmC_53 * ((((exp(((fb_103 * fb_103) * -1.0)) * spectral_36) * 0.5) * warmSide_102) * (1.0 - facing_99)))) + (vec3f(0.5, 0.5, 0.52) * (smoothstep(boxSoft_91, (boxSoft_91 * -0.6), dcard_104) * facing_99)));
  chrome_0 = vec4f(tonemapNeutral(((vec3f(envRt_5.x, envG_61.y, envBt_84.z) * uniforms.n_n0.environment) * uniforms.n_n0.tint.rgb)), silhouetteAlpha(sdf_1, uniforms.n_n0.edgeSoftness, pxH_4));
}
  var composed_107 = chrome_0;
  return vec4f(linearToSrgb(tone_linear(composed_107.rgb)) * composed_107.a, composed_107.a);
}