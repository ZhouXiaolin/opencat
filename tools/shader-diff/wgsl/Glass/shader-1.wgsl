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
  cutout: f32,
  refraction: f32,
  edgeSoftness: f32,
  blur: f32,
  thickness: f32,
  aberration: f32,
  innerZoom: f32,
  lightAngle: f32,
  highlight: f32,
  highlightColor: vec4f,
  highlightSoftness: f32,
  fresnel: f32,
  fresnelSoftness: f32,
  fresnelColor: vec4f,
  tintColor: vec4f,
  tintIntensity: f32,
  tintPreserveLuminosity: f32,
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

fn fieldSampleGradArg(uv: vec2f, field: texture_2d<f32>, originX: f32, originY: f32, spanX: f32, spanY: f32, activeResF: f32, rBound: f32) -> vec4f {
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
  let oxf = (1f - fx);
  let bx0 = (((oxf * oxf) * oxf) * 0.16666667f);
  let bx1 = (((tx3 * 0.5f) - tx2) + 0.66666667f);
  let bx2 = ((((tx3 * -0.5f) + (tx2 * 0.5f)) + (fx * 0.5f)) + 0.16666667f);
  let bx3 = (tx3 * 0.16666667f);
  let dbx0 = ((oxf * oxf) * -0.5f);
  let dbx1 = ((tx2 * 1.5f) - (fx * 2f));
  let dbx2 = (((tx2 * -1.5f) + fx) + 0.5f);
  let dbx3 = (tx2 * 0.5f);
  let oyf = (1f - fy);
  let by0 = (((oyf * oyf) * oyf) * 0.16666667f);
  let by1 = (((ty3 * 0.5f) - ty2) + 0.66666667f);
  let by2 = ((((ty3 * -0.5f) + (ty2 * 0.5f)) + (fy * 0.5f)) + 0.16666667f);
  let by3 = (ty3 * 0.16666667f);
  let dby0 = ((oyf * oyf) * -0.5f);
  let dby1 = ((ty2 * 1.5f) - (fy * 2f));
  let dby2 = (((ty2 * -1.5f) + fy) + 0.5f);
  let dby3 = (ty2 * 0.5f);
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
  let drdpx = (((((((((((((((((k00.r * dbx0) * by0) + ((k01.r * dbx1) * by0)) + ((k02.r * dbx2) * by0)) + ((k03.r * dbx3) * by0)) + ((k10.r * dbx0) * by1)) + ((k11.r * dbx1) * by1)) + ((k12.r * dbx2) * by1)) + ((k13.r * dbx3) * by1)) + ((k20.r * dbx0) * by2)) + ((k21.r * dbx1) * by2)) + ((k22.r * dbx2) * by2)) + ((k23.r * dbx3) * by2)) + ((k30.r * dbx0) * by3)) + ((k31.r * dbx1) * by3)) + ((k32.r * dbx2) * by3)) + ((k33.r * dbx3) * by3));
  let drdpy = (((((((((((((((((k00.r * bx0) * dby0) + ((k01.r * bx1) * dby0)) + ((k02.r * bx2) * dby0)) + ((k03.r * bx3) * dby0)) + ((k10.r * bx0) * dby1)) + ((k11.r * bx1) * dby1)) + ((k12.r * bx2) * dby1)) + ((k13.r * bx3) * dby1)) + ((k20.r * bx0) * dby2)) + ((k21.r * bx1) * dby2)) + ((k22.r * bx2) * dby2)) + ((k23.r * bx3) * dby2)) + ((k30.r * bx0) * dby3)) + ((k31.r * bx1) * dby3)) + ((k32.r * bx2) * dby3)) + ((k33.r * bx3) * dby3));
  let rMin = min(min(k11.r, k12.r), min(k21.r, k22.r));
  let rMax = max(max(k11.r, k12.r), max(k21.r, k22.r));
  let aMin = min(min(k11.a, k12.a), min(k21.a, k22.a));
  let aMax = max(max(k11.a, k12.a), max(k21.a, k22.a));
  let aOut = clamp(aAcc, aMin, aMax);
  let kSoft = (((rMax - rMin) * 0.3f) + 1e-6f);
  let hCap = clamp(((((rMax - rAcc) / kSoft) * 0.5f) + 0.5f), 0f, 1f);
  let rCap = (mix(rMax, rAcc, hCap) - ((kSoft * hCap) * (1f - hCap)));
  let hFlr = clamp(((((rCap - rMin) / kSoft) * 0.5f) + 0.5f), 0f, 1f);
  let rSoft = (mix(rMin, rCap, hFlr) + ((kSoft * hFlr) * (1f - hFlr)));
  let rNear = (rSoft + outsideDist);
  let rCircle = (length(vec2f((uv.x - 0.5f), (uv.y - 0.5f))) - rBound);
  let rOut = mix(rNear, max(rCircle, rNear), smoothstep(0f, 0.25f, outsideDist));
  let insideX = select(0f, 1f, (abs((uv.x - uxc)) < 1e-7f));
  let insideY = select(0f, 1f, (abs((uv.y - uyc)) < 1e-7f));
  let invOut = (1f / max(outsideDist, 1e-5f));
  let gU = (((drdpx * (activeResF / spanX)) * insideX) + ((uv.x - uxc) * invOut));
  let gV = (((drdpy * (activeResF / spanY)) * insideY) + ((uv.y - uyc) * invOut));
  return vec4f(rOut, gU, gV, aOut);
}

@group(1) @binding(0) var compute_0: texture_2d<f32>;

fn glassRefrStrength(sdf: f32, thickness: f32, volumetric: f32) -> f32 {
  let thicknessRange = max((thickness * 0.3f), 5e-3f);
  let depthNorm = clamp(((sdf * -1f) / thicknessRange), 0f, 1f);
  let vol = smoothstep(0f, 1f, depthNorm);
  let flat = ((1f - depthNorm) * (1f - depthNorm));
  return select(flat, vol, (volumetric > 0.5f));
}

struct GlassLensUVs {
  lensUV: vec2f,
  rUV: vec2f,
  bUV: vec2f,
}

fn glassLensUVs(uv: vec2f, center: vec2f, sdfRaw: f32, scale: f32, gradX: f32, gradY: f32, refraction: f32, innerZoom: f32, thickness: f32, aberration: f32, aspect: f32, volumetric: f32) -> GlassLensUVs {
  let sdf = (sdfRaw / scale);
  let refrStrength = glassRefrStrength(sdf, thickness, volumetric);
  let refrScale = (refraction * 0.15f);
  let slopeLen = max(sqrt(((gradX * gradX) + (gradY * gradY))), 1e-4f);
  let slopeScale = min(1f, (12f / slopeLen));
  let offsetX = (((((gradX * slopeScale) * -1f) * refrScale) * refrStrength) / aspect);
  let offsetY = ((((gradY * slopeScale) * -1f) * refrScale) * refrStrength);
  let centerX = center.x;
  let centerY = (1f - center.y);
  let zoomedX = (centerX + ((uv.x - centerX) / innerZoom));
  let zoomedY = (centerY + ((uv.y - centerY) / innerZoom));
  let lensX = (zoomedX + offsetX);
  let lensY = (zoomedY + offsetY);
  let chrScale = (aberration * 0.06f);
  let chrOffX = (offsetX * chrScale);
  let chrOffY = (offsetY * chrScale);
  return GlassLensUVs(vec2f(lensX, lensY), vec2f((lensX + chrOffX), (lensY + chrOffY)), vec2f((lensX - chrOffX), (lensY - chrOffY)));
}

fn glassBorderHighlight(sdf: f32, edgeSoftness: f32, pxH: f32, gradX: f32, gradY: f32, lightAngle: f32, highlight: f32) -> f32 {
  let sharp = max((edgeSoftness * 0.5f), 1e-3f);
  let w2 = max((sharp * 0.0625f), (pxH * 1.5f));
  let sdfOuter = (sdf - pxH);
  let rb2base = (clamp(((sdfOuter * -1f) / w2), 0f, 1f) - clamp(((sdf * -1f) / w2), 0f, 1f));
  let lx = cos((lightAngle * 0.017453292519943295f));
  let ly = sin((lightAngle * 0.017453292519943295f));
  let normalDotLight = ((gradX * lx) + (gradY * ly));
  let lightFacing = clamp(((normalDotLight * 0.5f) + 0.5f), 0f, 1f);
  return ((rb2base * lightFacing) * highlight);
}

fn glassTint(rgb: vec3f, tintColor: vec3f, tintIntensity: f32, preserveLum: f32) -> vec3f {
  let lumWeights = vec3f(0.29899999499320984, 0.5870000123977661, 0.11400000005960464);
  let origLum = dot(rgb, lumWeights);
  let tinted = mix(rgb, tintColor, tintIntensity);
  let tintedLum = dot(tinted, lumWeights);
  let lumPreserved = (tinted * (origLum / max(tintedLum, 1e-4f)));
  let preserveFactor = select(0f, 1f, (preserveLum > 0.5f));
  return mix(tinted, lumPreserved, preserveFactor);
}

fn glassSpecular(gradX: f32, gradY: f32, lightAngle: f32, highlight: f32, highlightSoftness: f32, refrStrength: f32) -> f32 {
  let lx = cos((lightAngle * 0.017453292519943295f));
  let ly = sin((lightAngle * 0.017453292519943295f));
  const nz = 2f;
  let nLen = sqrt((((gradX * gradX) + (gradY * gradY)) + (nz * nz)));
  let nx = (gradX / nLen);
  let ny = (gradY / nLen);
  let nnz = (nz / nLen);
  let nDotH = (((nx * (lx * 0.4472135954999579f)) + (ny * (ly * 0.4472135954999579f))) + (nnz * 0.8944271909999159f));
  let shininess = exp2((8f - (highlightSoftness * 7f)));
  return ((pow(clamp(nDotH, 0f, 1f), shininess) * highlight) * refrStrength);
}

fn glassFresnelRim(sdf: f32, gradX: f32, gradY: f32, fresnel: f32, fresnelSoftness: f32, rb1: f32, volumetric: f32) -> f32 {
  let slopeSq = ((gradX * gradX) + (gradY * gradY));
  let cosView = (1f / sqrt((slopeSq + 1f)));
  let exponent = mix(6f, 1.5f, clamp(fresnelSoftness, 0f, 1f));
  let volRim = (((pow((1f - cosView), exponent) * fresnel) * 2f) * rb1);
  let fresnelWidth = max((fresnelSoftness * 0.06f), 1e-3f);
  let fresnelDepth = clamp(((sdf * -1f) / fresnelWidth), 0f, 1f);
  let flatRim = ((((1f - fresnelDepth) * (1f - fresnelDepth)) * fresnel) * rb1);
  return select(flatRim, volRim, (volumetric > 0.5f));
}

fn glassComposite(baseColor: vec4f, blurred: vec4f, sdfRaw: f32, scale: f32, gradX: f32, gradY: f32, viewportSize: vec2f, edgeSoftness: f32, lightAngle: f32, highlight: f32, highlightColor: vec3f, highlightSoftness: f32, fresnel: f32, fresnelColor: vec3f, fresnelSoftness: f32, thickness: f32, tintColor: vec3f, tintIntensity: f32, tintPreserveLuminosity: f32, cutout: f32, volumetric: f32) -> vec4f {
  let sharp = max((edgeSoftness * 0.5f), 1e-3f);
  let pxH = (1f / viewportSize.y);
  let sdf = (sdfRaw / scale);
  let w1 = max((sharp * 0.03125f), (pxH * 1.5f));
  let rb1 = clamp(((sdf * -1f) / w1), 0f, 1f);
  let refrStrength = glassRefrStrength(sdf, thickness, volumetric);
  let rb2 = glassBorderHighlight(sdf, edgeSoftness, pxH, gradX, gradY, lightAngle, highlight);
  let blurredRgb = vec3f(blurred.x, blurred.y, blurred.z);
  let tintedRgb = glassTint(blurredRgb, tintColor, tintIntensity, tintPreserveLuminosity);
  let specGlint = glassSpecular(gradX, gradY, lightAngle, highlight, highlightSoftness, refrStrength);
  let fresnelRim = glassFresnelRim(sdf, gradX, gradY, fresnel, fresnelSoftness, rb1, volumetric);
  let litR = (((tintedRgb.x + (highlightColor.x * rb2)) + (highlightColor.x * specGlint)) + (fresnelColor.x * fresnelRim));
  let litG = (((tintedRgb.y + (highlightColor.y * rb2)) + (highlightColor.y * specGlint)) + (fresnelColor.y * fresnelRim));
  let litB = (((tintedRgb.z + (highlightColor.z * rb2)) + (highlightColor.z * specGlint)) + (fresnelColor.z * fresnelRim));
  let litA = (blurred.w + rb2);
  let lighting = vec4f(litR, litG, litB, litA);
  let transition = smoothstep(0f, 1f, rb1);
  let composited = mix(baseColor, lighting, transition);
  let cutoutAlpha = select(1f, transition, (cutout > 0.5f));
  return vec4f(composited.x, composited.y, composited.z, (composited.w * cutoutAlpha));
}

fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(1) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

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
  let glassSdfUV_2 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect);
  let glassSurf_1 = fieldSampleGradArg(glassSdfUV_2, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let glassLens_0 = glassLensUVs(uv, uniforms.n_n0.center, glassSurf_1.r, uniforms.n_n0.scale, glassSurf_1.g, glassSurf_1.b, uniforms.n_n0.refraction, uniforms.n_n0.innerZoom, uniforms.n_n0.thickness, uniforms.n_n0.aberration, uniforms._sys.aspect, 1.0);
  var composed_3 = glassComposite(unpremultiplyAlpha(textureSample(rtt_0, linearClamp, uv)), unpremultiplyAlpha(vec4f((textureSample(rtt_0, linearClamp, glassLens_0.rUV)).r, (textureSample(rtt_0, linearClamp, glassLens_0.lensUV)).g, (textureSample(rtt_0, linearClamp, glassLens_0.bUV)).b, (textureSample(rtt_0, linearClamp, glassLens_0.lensUV)).a)), glassSurf_1.r, uniforms.n_n0.scale, glassSurf_1.g, glassSurf_1.b, uniforms._sys.viewportSize, uniforms.n_n0.edgeSoftness, uniforms.n_n0.lightAngle, uniforms.n_n0.highlight, uniforms.n_n0.highlightColor.rgb, uniforms.n_n0.highlightSoftness, uniforms.n_n0.fresnel, uniforms.n_n0.fresnelColor.rgb, uniforms.n_n0.fresnelSoftness, uniforms.n_n0.thickness, uniforms.n_n0.tintColor.rgb, uniforms.n_n0.tintIntensity, uniforms.n_n0.tintPreserveLuminosity, uniforms.n_n0.cutout, 1.0);
  return vec4f(linearToSrgb(tone_linear(composed_3.rgb)) * composed_3.a, composed_3.a);
}