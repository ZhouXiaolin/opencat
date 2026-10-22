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
  thickness: f32,
  softness: f32,
  intensity: f32,
  bloom: f32,
  spots: f32,
  spotSize: f32,
  pulse: f32,
  smoke: f32,
  smokeSize: f32,
  speed: f32,
  seed: f32,
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

fn heartbeatPulse(animTime: f32) -> f32 {
  let time = (0.18f * animTime);
  let first = pow(abs(sin((time * 6.283185307179586f))), 10f);
  let second = pow(abs(sin(((time - 0.15f) * 6.283185307179586f))), 10f);
  return clamp((first + (0.6f * second)), 0f, 1f);
}

struct EdgeGlowAccum {
  rgb: vec3f,
  w: f32,
  ba: f32,
  ac: vec3f,
  aa: f32,
}

fn edgeGlowCompose(st: EdgeGlowAccum, bloom: f32) -> vec4f {
  let k = (bloom * 4f);
  let blendPremult = (st.rgb * st.ba);
  let accumC = mix(blendPremult, st.ac, vec3f(k));
  let accumA = clamp(mix(st.ba, st.aa, k), 0f, 1f);
  let straight = clamp((accumC / max(accumA, 1e-4f)), vec3f(), vec3f(1));
  return vec4f(straight, accumA);
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

fn edgeGlowAccumColor_0(st: EdgeGlowAccum, color: vec4f, sectors: vec2f) -> EdgeGlowAccum {
  let wNew = (color.w * sectors.x);
  let t = (wNew / max((st.w + wNew), 1e-5f));
  let mixed = mixColors(vec4f(st.rgb, 1f), vec4f(color.xyz, 1f), t);
  let ba = (st.ba + ((1f - st.ba) * wNew));
  let ac = (st.ac + (color.xyz * (color.w * sectors.y)));
  let aa = (st.aa + (color.w * sectors.y));
  return EdgeGlowAccum(mixed.xyz, (st.w + wNew), ba, ac, aa);
}

fn edgeGlowAccumZero() -> EdgeGlowAccum {
  return EdgeGlowAccum(vec3f(), 0f, 0f, vec3f(), 0f);
}

fn colorAtIndex(colors: array<vec4f, 8>, idx: f32) -> vec4f {
  let c = colors[i32(idx)];
  return vec4f(c.x, c.y, c.z, c.w);
}

fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }

fn hash22(p: vec2f) -> vec2f {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  let h = (u.x ^ u.y);
  let prod = (vec2u(h, h) * vec2u(3141592653, 1618033988));
  return (vec2f(f32(prod.x), f32(prod.y)) / 4294967296f);
}

fn orbitSpotsAccum(angle01: f32, border: f32, animTime: f32, beat: f32, spotsF: f32, spotSize: f32, pulse: f32, intensity: f32, softness: f32, seed: f32, colorIdxF: f32) -> vec2f {
  let t = ((animTime * 1.2f) + 131f);
  let intensityEff = (1f + ((1f + (4f * softness)) * intensity));
  var overAcc = 0f;
  var addAcc = 0f;
  for (var s = 0; (s < 5i); s++) {
    let sF = f32(s);
    let gate = select(0f, 1f, (sF < spotsF));
    let randVal = hash22(vec2f((((sF * 10f) + 2f) + (seed * 7.31f)), ((40f + colorIdxF) + (seed * 3.17f))));
    let speedFactor = (0.1f + (0.15f * abs((sin((sF * (2f + colorIdxF))) * cos((sF * (2f + (2.5f * colorIdxF))))))));
    let dir = select(1f, -1f, (randVal.y < 0.5f));
    let sTime = (((speedFactor * t) + (randVal.x * 3f)) * dir);
    let evenCond = (abs((colorIdxF - (2f * floor((colorIdxF * 0.5f))))) < 0.5f);
    let sinVal = sin((t + (sF * (5f - (1.5f * colorIdxF)))));
    let cosVal = cos((t + (sF * (3f + (1.3f * colorIdxF)))));
    var mask = (0.5f + (0.5f * select(cosVal, sinVal, evenCond)));
    let pWeight = clamp(((2f * pulse) - randVal.x), 0f, 1f);
    mask = mix(mask, beat, pWeight);
    let atg = fract((angle01 + sTime));
    var ss = ((0.05f + ((0.6f * spotSize) * spotSize)) + (0.05f * randVal.x));
    ss = mix(ss, 0.1f, pWeight);
    var sector = (smoothstep((0.5f - ss), 0.5f, atg) * (1f - smoothstep(0.5f, (0.5f + ss), atg)));
    sector = (clamp((((sector * mask) * border) * intensityEff), 0f, 1f) * gate);
    overAcc = (overAcc + ((1f - overAcc) * sector));
    addAcc = (addAcc + sector);
  }
  return vec2f(overAcc, addAcc);
}

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

fn edgeGlowBand(fieldRaw: f32, sdfUV: vec2f, viewport: vec2f, scale: f32, thickness: f32, softness: f32, smokeSize: f32, smokeAmt: f32, animTime: f32, pulse: f32, beat: f32) -> vec2f {
  let t = ((animTime * 1.2f) + 131f);
  let th = max(((0.5f * thickness) * 0.35f), 2e-3f);
  let f = ((fieldRaw / max(scale, 1e-3f)) + mix(th, 0f, softness));
  let bd = abs(f);
  let borderTh = mix(th, (3f * th), softness);
  let aa = (2f / (viewport.y * max(scale, 1e-3f)));
  let e0 = min(mix(borderTh, (borderTh * -1f), softness), (borderTh + aa));
  let e1 = max(mix(borderTh, (borderTh * -1f), softness), (borderTh + aa));
  var border = (1f - smoothstep(e0, e1, bd));
  border = pow(clamp(border, 0f, 1f), (1f + softness));
  let suv = (vec2f((sdfUV.x - 0.5f), (sdfUV.y - 0.5f)) * (0.5f + (2.5f * smokeSize)));
  let drift = (0.5f * t);
  let n1 = value12(vec2f(((suv.x * 2.7f) + drift), ((suv.y * 2.7f) + drift)));
  let n2 = value12(vec2f(((suv.x * 3.4f) - drift), ((suv.y * 3.4f) - drift)));
  var smoke = (clamp((3f * n1), 0f, 1f) - n2);
  let smokeTh = clamp((th + 0.2f), 0.1f, 0.4f);
  let smokeBand = pow(clamp((1f - smoothstep((smokeTh * -1f), (smokeTh + aa), bd)), 0f, 1f), 2f);
  smoke = (smoke * smokeBand);
  smoke = (((30f * smoke) * smoke) * ((0.5f * smokeAmt) * smokeAmt));
  smoke = (smoke * mix(1f, beat, pulse));
  border = clamp((border + clamp(smoke, 0f, 1f)), 0f, 1f);
  let angle01 = (atan2((sdfUV.y - 0.5f), (sdfUV.x - 0.5f)) / 6.283185307179586f);
  return vec2f(border, angle01);
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
  let sdfUV_0 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect);
  let beat_1 = heartbeatPulse(uniforms.n_n0._animTime);
  var composed_2 = edgeGlowCompose(edgeGlowAccumColor_0(edgeGlowAccumColor_0(edgeGlowAccumColor_0(edgeGlowAccumZero(), colorAtIndex(uniforms.n_n0.colorsArray, 0.0), orbitSpotsAccum((edgeGlowBand((fieldSampleArg(sdfUV_0, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, sdfUV_0, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.smokeSize, uniforms.n_n0.smoke, uniforms.n_n0._animTime, uniforms.n_n0.pulse, beat_1)).y, (edgeGlowBand((fieldSampleArg(sdfUV_0, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, sdfUV_0, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.smokeSize, uniforms.n_n0.smoke, uniforms.n_n0._animTime, uniforms.n_n0.pulse, beat_1)).x, uniforms.n_n0._animTime, beat_1, uniforms.n_n0.spots, uniforms.n_n0.spotSize, uniforms.n_n0.pulse, uniforms.n_n0.intensity, uniforms.n_n0.softness, uniforms.n_n0.seed, 0.0)), colorAtIndex(uniforms.n_n0.colorsArray, 1.0), orbitSpotsAccum((edgeGlowBand((fieldSampleArg(sdfUV_0, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, sdfUV_0, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.smokeSize, uniforms.n_n0.smoke, uniforms.n_n0._animTime, uniforms.n_n0.pulse, beat_1)).y, (edgeGlowBand((fieldSampleArg(sdfUV_0, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, sdfUV_0, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.smokeSize, uniforms.n_n0.smoke, uniforms.n_n0._animTime, uniforms.n_n0.pulse, beat_1)).x, uniforms.n_n0._animTime, beat_1, uniforms.n_n0.spots, uniforms.n_n0.spotSize, uniforms.n_n0.pulse, uniforms.n_n0.intensity, uniforms.n_n0.softness, uniforms.n_n0.seed, 1.0)), colorAtIndex(uniforms.n_n0.colorsArray, 2.0), orbitSpotsAccum((edgeGlowBand((fieldSampleArg(sdfUV_0, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, sdfUV_0, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.smokeSize, uniforms.n_n0.smoke, uniforms.n_n0._animTime, uniforms.n_n0.pulse, beat_1)).y, (edgeGlowBand((fieldSampleArg(sdfUV_0, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).r, sdfUV_0, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.smokeSize, uniforms.n_n0.smoke, uniforms.n_n0._animTime, uniforms.n_n0.pulse, beat_1)).x, uniforms.n_n0._animTime, beat_1, uniforms.n_n0.spots, uniforms.n_n0.spotSize, uniforms.n_n0.pulse, uniforms.n_n0.intensity, uniforms.n_n0.softness, uniforms.n_n0.seed, 2.0)), uniforms.n_n0.bloom);
  return vec4f(linearToSrgb(tone_linear(composed_2.rgb)) * composed_2.a, composed_2.a);
}