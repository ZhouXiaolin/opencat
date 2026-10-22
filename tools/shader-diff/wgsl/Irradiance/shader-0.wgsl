@group(1) @binding(0) var compute_1: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

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
  lights_position: array<vec4f, 6>,
  lights_color: array<vec4f, 6>,
  lights_intensity: array<vec4f, 6>,
  lightsCount: f32,
  lightHeight: f32,
  lightRange: f32,
  reach: f32,
  core: f32,
  wrap: f32,
  shadows: f32,
  shadowSoftness: f32,
  bodyColor: vec4f,
  bodyLight: f32,
  bevelWidth: f32,
  edgeSoftness: f32,
  _opacity: f32,
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

@group(1) @binding(1) var compute_0: texture_2d<f32>;

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

fn fieldGradient(surf0: vec4f, surfX: vec4f, surfY: vec4f, eps: f32) -> vec2f {
  return vec2f(((surfX.x - surf0.x) / eps), ((surfY.x - surf0.x) / eps));
}

fn outsideShape(sdf: f32, pxH: f32) -> bool {
  return (sdf > (pxH * 2f));
}

fn volumetricNormal(surf0: vec4f, surfX: vec4f, surfY: vec4f, eps: f32, gradClamp: f32) -> vec3f {
  let ddx = clamp(((surfX.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  let ddy = clamp(((surfY.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  return normalize(vec3f(ddx, ddy, -1f));
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
  let irrTex_1 = textureSampleLevel(compute_1, linearClamp, uv, 0.0);
  let pxH_5 = (1.0 / uniforms._sys.viewportSize.y);
  let pxUV_4 = (pxH_5 / max(uniforms.n_n0.scale, 0.001));
  let sdfUV_8 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect);
  let s0_7 = fieldSampleFastArg(sdfUV_8, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sdf_6 = s0_7.x;
  let rim_3 = (1.0 - smoothstep(0.0, (pxUV_4 * 2.5), sdf_6));
  var emitter_2 = vec3f(0.0);
  if ((rim_3 > 0.0)) {
  let rimS0_12 = fieldSampleArg(sdfUV_8, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let rimX_13 = fieldSampleFastArg((sdfUV_8 + vec2f(0.01, 0.0)), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let rimY_14 = fieldSampleFastArg((sdfUV_8 + vec2f(0.0, 0.01)), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let grad_11 = fieldGradient(rimS0_12, rimX_13, rimY_14, 0.01);
  let edgeN_10 = ((grad_11 + vec2f(0.0, 0.0)) / max(length((grad_11 + vec2f(0.0, 0.0))), 0.0001));
  let edgeP_9 = (sdfUV_8 - (edgeN_10 * max(sdf_6, 0.0)));
  var rimSum_15 = vec3f(0.0);
  for (var item_16 = 0u; item_16 < u32(uniforms.n_n0.lightsCount); item_16++) {
  let lightUV_19 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, vec2f(((uniforms.n_n0.lights_position[item_16]).xy).x, (1.0 - ((uniforms.n_n0.lights_position[item_16]).xy).y)), uniforms._sys.aspect);
  let rimLightD_18 = (lightUV_19 - edgeP_9);
  let rimLightDist_20 = length(rimLightD_18);
  let rimLightDir_17 = (rimLightD_18 / max(rimLightDist_20, 0.0001));
  let shFrom_22 = (edgeP_9 + (edgeN_10 * (pxUV_4 * 2.0)));
  let shSeg_23 = lightUV_19 - shFrom_22;
  let shLen_24 = length(shSeg_23);
  let shDir_25 = shSeg_23 / max(shLen_24, 0.0001);
  let shEps_26 = pxUV_4;
  let shSoft_27 = uniforms.n_n0.shadowSoftness;
  var rimVisibility_21 = 1.0;
  {
  var shT_28 = shEps_26 * 2.0;
  for (var shStep_31 = 0u; shStep_31 < 12u; shStep_31++) {
    if (shT_28 >= shLen_24) { break; }
    let shPos_29 = shFrom_22 + shDir_25 * shT_28;
    let shD_30 = (fieldSampleFastArg(shPos_29, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).x;
    rimVisibility_21 = min(rimVisibility_21, clamp(shD_30 / max(shT_28 * shSoft_27, shEps_26), 0.0, 1.0));
    if (rimVisibility_21 <= 0.0) { break; }
    shT_28 += max(shD_30, shEps_26);
  }
}
  rimSum_15 += (((uniforms.n_n0.lights_color[item_16]).rgb * (uniforms.n_n0.lights_intensity[item_16]).x) * ((clamp(((dot(edgeN_10, rimLightDir_17) + uniforms.n_n0.wrap) / (1.0 + uniforms.n_n0.wrap)), 0.0, 1.0) * (1.0 / (1.0 + ((rimLightDist_20 / uniforms.n_n0.lightRange) * (rimLightDist_20 / uniforms.n_n0.lightRange))))) * rimVisibility_21));
}
  emitter_2 = (rimSum_15 * (rim_3 * uniforms.n_n0.core));
}
  let glow_0 = (1.0 - exp((-(((irrTex_1.rgb / max(irrTex_1.a, 0.001)) + emitter_2)))));
  let glowA_32 = clamp(max(max(glow_0.x, glow_0.y), glow_0.z), 0.0, 1.0);
  var body_33 = vec3f(0.0);
  if ((!(outsideShape(sdf_6, pxUV_4)))) {
  let bodyS0_35 = fieldSampleArg(sdfUV_8, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let bodyX_36 = fieldSampleFastArg((sdfUV_8 + vec2f(0.01, 0.0)), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let bodyY_37 = fieldSampleFastArg((sdfUV_8 + vec2f(0.0, 0.01)), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let nGeo_34 = volumetricNormal(bodyS0_35, bodyX_36, bodyY_37, 0.01, 5.0);
  var bodySum_38 = vec3f(0.0);
  for (var item_39 = 0u; item_39 < u32(uniforms.n_n0.lightsCount); item_39++) {
  let lightUV_42 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, vec2f(((uniforms.n_n0.lights_position[item_39]).xy).x, (1.0 - ((uniforms.n_n0.lights_position[item_39]).xy).y)), uniforms._sys.aspect);
  let bodyLightD_41 = (lightUV_42 - sdfUV_8);
  let bodyLightL_40 = normalize(vec3f(bodyLightD_41.x, bodyLightD_41.y, (-(uniforms.n_n0.lightHeight))));
  let bodyLightDist_44 = length(bodyLightD_41);
  let bodyLit_43 = ((1.0 / (1.0 + ((bodyLightDist_44 / uniforms.n_n0.lightRange) * (bodyLightDist_44 / uniforms.n_n0.lightRange)))) * uniforms.n_n0.bodyLight);
  bodySum_38 += ((uniforms.n_n0.bodyColor.rgb * (max(dot(nGeo_34, bodyLightL_40), 0.0) * bodyLit_43)) + (((uniforms.n_n0.lights_color[item_39]).rgb * (uniforms.n_n0.lights_intensity[item_39]).x) * ((pow(max(dot(nGeo_34, normalize((bodyLightL_40 + vec3f(0.0, 0.0, -1.0)))), 0.0), 60.0) * 0.6) * bodyLit_43)));
}
  body_33 = (1.0 - exp((-(((uniforms.n_n0.bodyColor.rgb * 0.3) + bodySum_38)))));
}
  let cov_45 = silhouetteAlpha(sdf_6, uniforms.n_n0.edgeSoftness, pxUV_4);
  var composed_46 = vec4f(mix((vec4f((glow_0 / max(glowA_32, 0.0001)), glowA_32)).rgb, body_33, cov_45), mix((vec4f((glow_0 / max(glowA_32, 0.0001)), glowA_32)).a, 1.0, cov_45));
  return vec4f(linearToSrgb(tone_linear(composed_46.rgb)) * composed_46.a, composed_46.a);
}