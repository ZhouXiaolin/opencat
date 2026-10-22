fn thinFilmCompose(baseColor: vec3f, rim: f32, intensity: f32, saturation: f32) -> vec4f {
  let white = vec3f(1);
  let col = mix(white, baseColor, saturation);
  let rgb = ((col * rim) * intensity);
  let alpha = clamp(rim, 0f, 1f);
  return vec4f(rgb.x, rgb.y, rgb.z, alpha);
}

fn cosinePalette(t: f32, phaseR: f32, phaseG: f32, phaseB: f32) -> vec3f {
  return vec3f(((cos(((t + phaseR) * 6.283185307179586f)) * 0.5f) + 0.5f), ((cos(((t + phaseG) * 6.283185307179586f)) * 0.5f) + 0.5f), ((cos(((t + phaseB) * 6.283185307179586f)) * 0.5f) + 0.5f));
}

fn thinFilmRainbow(phase: f32) -> vec3f {
  return cosinePalette(phase, 0f, 0.3333f, 0.6667f);
}

fn thinFilmShade(sdfRaw: f32, scale: f32, gradX: f32, gradY: f32, edgeSoftness: f32, rimWidthUniform: f32, thickness: f32, dispersion: f32, hueShift: f32, lightAngle: f32, animTime: f32, volumetric: f32) -> vec2f {
  let sdf = (sdfRaw / scale);
  let sharp = max((edgeSoftness * 0.5f), 1e-3f);
  let gLen = max(sqrt(((gradX * gradX) + (gradY * gradY))), 1e-4f);
  let nx = (gradX / gLen);
  let ny = (gradY / gLen);
  let rb1 = clamp((((sdf * -1f) / sharp) * 32f), 0f, 1f);
  let rimWidth = max((rimWidthUniform * 0.1f), 1e-3f);
  let depth = clamp(((sdf * -1f) / rimWidth), 0f, 1f);
  let cosView = (1f / sqrt(((gLen * gLen) + 1f)));
  let exponent = mix(7f, 1.5f, clamp(rimWidthUniform, 0f, 1f));
  let volRim = (pow((1f - cosView), exponent) * rb1);
  let flatRim = (((1f - depth) * (1f - depth)) * rb1);
  let rim = select(flatRim, volRim, (volumetric > 0.5f));
  let lx = cos((lightAngle * 0.017453292519943295f));
  let ly = sin((lightAngle * 0.017453292519943295f));
  let baseCos = ((nx * lx) + (ny * ly));
  let baseSin = ((ny * lx) - (nx * ly));
  let theta = (animTime * 6.283185307179586f);
  let rotatedDot = ((baseCos * cos(theta)) + (baseSin * sin(theta)));
  let thickVol = ((thickness * max((sdf * -1f), 0f)) * 10f);
  let thickFlat = ((thickness * depth) * 2f);
  let thicknessTerm = select(thickFlat, thickVol, (volumetric > 0.5f));
  let phase = ((hueShift + (dispersion * rotatedDot)) + thicknessTerm);
  return vec2f(phase, rim);
}

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
  intensity: f32,
  rimWidth: f32,
  edgeSoftness: f32,
  thickness: f32,
  dispersion: f32,
  saturation: f32,
  hueShift: f32,
  lightAngle: f32,
  colorA: vec4f,
  colorB: vec4f,
  colorC: vec4f,
  colorSpace: f32,
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

@group(1) @binding(0) var compute_0: texture_2d<f32>;

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
  var composed_0 = thinFilmCompose(thinFilmRainbow((thinFilmShade((fieldSampleGradArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, uniforms.n_n0.scale, (fieldSampleGradArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).g, (fieldSampleGradArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).b, uniforms.n_n0.edgeSoftness, uniforms.n_n0.rimWidth, uniforms.n_n0.thickness, uniforms.n_n0.dispersion, uniforms.n_n0.hueShift, uniforms.n_n0.lightAngle, uniforms.n_n0._animTime, 1.0)).x), (thinFilmShade((fieldSampleGradArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, uniforms.n_n0.scale, (fieldSampleGradArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).g, (fieldSampleGradArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).b, uniforms.n_n0.edgeSoftness, uniforms.n_n0.rimWidth, uniforms.n_n0.thickness, uniforms.n_n0.dispersion, uniforms.n_n0.hueShift, uniforms.n_n0.lightAngle, uniforms.n_n0._animTime, 1.0)).y, uniforms.n_n0.intensity, uniforms.n_n0.saturation);
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}