fn neonFlow(sdfUV: vec2f, time: f32, flowSpeed: f32, flowAmount: f32) -> f32 {
  let flowDx = (sdfUV.x - 0.5f);
  let flowDy = (sdfUV.y - 0.5f);
  let flowAngle = atan2(flowDy, flowDx);
  let flowPhase = ((flowAngle * 2f) + (time * flowSpeed));
  let flowWave = ((sin(flowPhase) * 0.5f) + 0.5f);
  return mix(1f, flowWave, flowAmount);
}

fn neonGlow(sdf: f32, glowRadius: f32, glowIntensity: f32) -> f32 {
  let glowR = max(glowRadius, 1e-3f);
  let sdfOutside = max(sdf, 0f);
  let innerGlow = (exp(((sdfOutside * -1f) * (12f / glowR))) * 0.7f);
  let outerGlow = (exp(((sdfOutside * -1f) * (4f / glowR))) * 0.35f);
  let totalGlow = ((innerGlow + outerGlow) * glowIntensity);
  let outsideFactor = smoothstep(-3e-3f, 3e-3f, sdf);
  return (totalGlow * outsideFactor);
}

fn neonFlicker(time: f32, flickerSpeed: f32, flickerAmount: f32) -> f32 {
  let flickerTime = (time * flickerSpeed);
  let f1 = sin((flickerTime * 17.13f));
  let f2 = sin((flickerTime * 31.71f));
  let f3 = sin((flickerTime * 47.29f));
  let combined = ((f1 + (f2 * 0.5f)) + (f3 * 0.3f));
  let threshold = mix(-2f, 0.8f, flickerAmount);
  return smoothstep((threshold - 0.1f), (threshold + 0.1f), combined);
}

fn neonComposite(sC: vec4f, sR: vec4f, sL: vec4f, sU: vec4f, sD: vec4f, smoothEPS: f32, sdfUV: vec2f, scale: f32, viewportSize: vec2f, time: f32, cornerSmoothing: f32, tubeThickness: f32, color: vec3f, secondaryColor: vec3f, secondaryBlend: f32, glowColor: vec3f, intensity: f32, hotCoreIntensity: f32, glowIntensity: f32, glowRadius: f32, lightAngle: f32, specularIntensity: f32, specularSize: f32, flickerSpeed: f32, flickerAmount: f32, flowSpeed: f32, flowAmount: f32, volumetric: f32) -> vec4f {
  let sdfCenter = sC.x;
  let sdfRight = sR.x;
  let sdfLeft = sL.x;
  let sdfUp = sU.x;
  let sdfDown = sD.x;
  let pxH = (1f / viewportSize.y);
  let glowR = max(glowRadius, 1e-3f);
  let softPx = max((pxH / scale), 4e-3f);
  let gradX = ((sdfRight - sdfLeft) / (smoothEPS * 2f));
  let gradY = ((sdfUp - sdfDown) / (smoothEPS * 2f));
  let smoothedSdf = (((((sdfCenter + sdfRight) + sdfLeft) + sdfUp) + sdfDown) / 5f);
  let sdf = mix(sdfCenter, smoothedSdf, cornerSmoothing);
  let gxRaw = select(gradX, (sR.w - sL.w), (volumetric > 0.5f));
  let gyRaw = select(gradY, (sU.w - sD.w), (volumetric > 0.5f));
  let gradLen = max(sqrt(((gxRaw * gxRaw) + (gyRaw * gyRaw))), 1e-4f);
  let gnx = (gxRaw / gradLen);
  let gny = (gyRaw / gradLen);
  let depthNorm2D = clamp(((sdf * -1f) / max(tubeThickness, 1e-3f)), 0f, 1f);
  let innerMask2D = smoothstep((tubeThickness + softPx), (tubeThickness - softPx), (sdf * -1f));
  let lap = (abs(((sR.w + sL.w) - (sC.w * 2f))) + abs(((sU.w + sD.w) - (sC.w * 2f))));
  let slopeMag = (abs(gxRaw) + abs(gyRaw));
  let crease = (lap / ((lap + (smoothEPS * 0.6f)) + (slopeMag * 0.35f)));
  let innerMaskVol = smoothstep(0.08f, 0.35f, crease);
  let depthNorm = select(depthNorm2D, crease, (volumetric > 0.5f));
  let innerMask = select(innerMask2D, innerMaskVol, (volumetric > 0.5f));
  let outerMask = smoothstep(softPx, (softPx * -1f), sdf);
  let tubeMask = (outerMask * innerMask);
  let t = depthNorm;
  let cylinderZ = sqrt(max((t * (2f - t)), 0f));
  let cylinderR = (1f - t);
  let nx = (gnx * cylinderR);
  let ny = (gny * cylinderR);
  let nz = cylinderZ;
  let lightRad = (lightAngle * 0.017453292519943295f);
  let lx = cos(lightRad);
  let ly = sin(lightRad);
  let lxn = (lx * 0.8192319205190405f);
  let lyn = (ly * 0.8192319205190405f);
  const lzn = 0.5734623443633283;
  let NdotL = (((nx * lxn) + (ny * lyn)) + (nz * lzn));
  let diffuse = clamp(NdotL, 0f, 1f);
  let NdotH = (((nx * (lxn * 0.5637114097207191f)) + (ny * (lyn * 0.5637114097207191f))) + (nz * 0.8869786762835193f));
  let shininess = exp2((8f - (specularSize * 7f)));
  let specular = (pow(clamp(NdotH, 0f, 1f), shininess) * specularIntensity);
  let flowMod = neonFlow(sdfUV, time, flowSpeed, flowAmount);
  let twoTone = mix(secondaryColor, color, diffuse);
  let baseColor = mix(color, twoTone, secondaryBlend);
  const ambient = 0.12;
  let selfEmission = (cylinderZ * 0.35f);
  let shadedRgb = (baseColor * ((ambient + (diffuse * 0.55f)) + selfEmission));
  let tubeRgb = vec3f((shadedRgb.x + specular), (shadedRgb.y + specular), (shadedRgb.z + specular));
  let coreProfile = smoothstep(0.55f, 1f, depthNorm);
  let hotCoreAmt = ((coreProfile * hotCoreIntensity) * flowMod);
  let hotCoreCol = mix(color, vec3f(1), 0.75f);
  let tubeWithCore = mix(tubeRgb, hotCoreCol, hotCoreAmt);
  let tubeFlowed = (tubeWithCore * mix(1f, flowMod, 0.4f));
  let glowMasked = neonGlow(sdf, glowRadius, glowIntensity);
  let flickerValue = neonFlicker(time, flickerSpeed, flickerAmount);
  let intensityMul = (intensity * flickerValue);
  let finalTubeRgb = (tubeFlowed * intensityMul);
  let finalGlowRgb = (glowColor * (glowMasked * intensityMul));
  let finalR = mix(finalGlowRgb.x, finalTubeRgb.x, tubeMask);
  let finalG = mix(finalGlowRgb.y, finalTubeRgb.y, tubeMask);
  let finalB = mix(finalGlowRgb.z, finalTubeRgb.z, tubeMask);
  let glowAlpha = clamp((glowMasked * 0.6f), 0f, 1f);
  let finalA = max(tubeMask, glowAlpha);
  let result = vec4f(finalR, finalG, finalB, finalA);
  let threshOut = (((glowR * 1.5f) + softPx) + smoothEPS);
  return select(result, vec4f(), (sdfCenter > threshOut));
}

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
  color: vec4f,
  secondaryColor: vec4f,
  secondaryBlend: f32,
  glowColor: vec4f,
  tubeThickness: f32,
  intensity: f32,
  hotCoreIntensity: f32,
  glowIntensity: f32,
  glowRadius: f32,
  lightAngle: f32,
  specularIntensity: f32,
  specularSize: f32,
  cornerSmoothing: f32,
  flickerSpeed: f32,
  flickerAmount: f32,
  flowSpeed: f32,
  flowAmount: f32,
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

@group(1) @binding(0) var compute_0: texture_2d<f32>;

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

fn neonSmoothEPS(tubeThickness: f32, cornerSmoothing: f32, volumetric: f32) -> f32 {
  let vol = max(tubeThickness, 6e-3f);
  let flat = (8e-3f + (cornerSmoothing * 0.03f));
  return select(flat, vol, (volumetric > 0.5f));
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
  var composed_0 = neonComposite(fieldSampleArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound), fieldSampleFastArg(offsetUV(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), neonSmoothEPS(uniforms.n_n0.tubeThickness, uniforms.n_n0.cornerSmoothing, 1.0), 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound), fieldSampleFastArg(offsetUV(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), (neonSmoothEPS(uniforms.n_n0.tubeThickness, uniforms.n_n0.cornerSmoothing, 1.0) * -1.0), 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound), fieldSampleFastArg(offsetUV(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), 0.0, neonSmoothEPS(uniforms.n_n0.tubeThickness, uniforms.n_n0.cornerSmoothing, 1.0)), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound), fieldSampleFastArg(offsetUV(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), 0.0, (neonSmoothEPS(uniforms.n_n0.tubeThickness, uniforms.n_n0.cornerSmoothing, 1.0) * -1.0)), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound), neonSmoothEPS(uniforms.n_n0.tubeThickness, uniforms.n_n0.cornerSmoothing, 1.0), sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), uniforms.n_n0.scale, uniforms._sys.viewportSize, uniforms._sys.time, uniforms.n_n0.cornerSmoothing, uniforms.n_n0.tubeThickness, uniforms.n_n0.color.rgb, uniforms.n_n0.secondaryColor.rgb, uniforms.n_n0.secondaryBlend, uniforms.n_n0.glowColor.rgb, uniforms.n_n0.intensity, uniforms.n_n0.hotCoreIntensity, uniforms.n_n0.glowIntensity, uniforms.n_n0.glowRadius, uniforms.n_n0.lightAngle, uniforms.n_n0.specularIntensity, uniforms.n_n0.specularSize, uniforms.n_n0.flickerSpeed, uniforms.n_n0.flickerAmount, uniforms.n_n0.flowSpeed, uniforms.n_n0.flowAmount, 1.0);
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}