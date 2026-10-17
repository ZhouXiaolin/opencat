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
  bodyColor: vec4f,
  colorA: vec4f,
  colorB: vec4f,
  colorsArray: array<vec4f, 8>,
  positionsArray: array<vec4f, 2>,
  convertedColorsArray: array<vec3f, 8>,
  stopCount: f32,
  colorSpace: f32,
  iridescence: f32,
  rimWidth: f32,
  rimSoftness: f32,
  flowAngle: f32,
  flowScale: f32,
  speed: f32,
  gloss: f32,
  envRotation: f32,
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

fn patternCoords(sdfUV: vec2f, surf0: vec4f, volumetric: f32) -> vec2f {
  return vec2f(select((sdfUV.x - 0.5f), surf0.y, (volumetric > 0.5f)), select((sdfUV.y - 0.5f), surf0.z, (volumetric > 0.5f)));
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

fn perspectiveViewRay(uv: vec2f, aspect: f32, fov: f32) -> vec3f {
  return normalize(vec3f((((uv.x - 0.5f) * aspect) * fov), ((uv.y - 0.5f) * fov), 1f));
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

fn gradientEdgeMirror(t: f32) -> f32 {
  let m = (abs(t) % 2f);
  return select(m, (2f - m), (m > 1f));
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
  var obsidian_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let pat_8 = patternCoords(sdfUV_3, s0_2, 1.0);
  let flowT_9 = vec2f(cos((uniforms.n_n0.flowAngle * 0.017453292519943295)), sin((uniforms.n_n0.flowAngle * 0.017453292519943295)));
  let along_7 = dot(pat_8, flowT_9);
  let sX_12 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sY_13 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let nGeo_11 = volumetricNormal(s0_2, sX_12, sY_13, 0.01, 5.0);
  let viewI_14 = perspectiveViewRay(uv, (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y), 0.6);
  let grazing_10 = (1.0 - clamp((-(dot(nGeo_11, viewI_14))), 0.0, 1.0));
  let stopMix_6 = gradientStopsInSpace(gradientEdgeMirror((((along_7 * (uniforms.n_n0.flowScale * 1.8181818181818181)) + (grazing_10 * 0.9)) + (uniforms.n_n0._animTime * 0.2))), uniforms.n_n0.colorsArray, uniforms.n_n0.positionsArray, uniforms.n_n0.convertedColorsArray, uniforms.n_n0.stopCount);
  let iri_5 = (vec4f(sRGBToP3(oklabToRgb(stopMix_6.conv)), stopMix_6.alpha)).rgb;
  let rimEdge_16 = (1.0 - uniforms.n_n0.rimWidth);
  let rim_15 = smoothstep((rimEdge_16 - uniforms.n_n0.rimSoftness), (rimEdge_16 + uniforms.n_n0.rimSoftness), grazing_10);
  let R_18 = reflect(viewI_14, nGeo_11);
  let env_19 = vec2f(cos((uniforms.n_n0.envRotation * 0.017453292519943295)), sin((uniforms.n_n0.envRotation * 0.017453292519943295)));
  let shine_17 = (studio5Softbox(R_18.x, R_18.y, env_19.x, env_19.y, 0.3, 0.0, uniforms.n_n0.gloss, 0.25) * (0.04 + (pow(grazing_10, 4.0) * 0.96)));
  obsidian_0 = vec4f((mix(uniforms.n_n0.bodyColor.rgb, iri_5, vec3f((rim_15 * uniforms.n_n0.iridescence))) + (mix(iri_5, vec3f(1.0), vec3f(0.6)) * vec3f((shine_17 * 0.5)))), silhouetteAlpha(sdf_1, uniforms.n_n0.edgeSoftness, pxH_4));
}
  var composed_20 = obsidian_0;
  return vec4f(linearToSrgb(tone_linear(composed_20.rgb)) * composed_20.a, composed_20.a);
}