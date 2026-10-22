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
  dispersion: f32,
  facets: f32,
  fresnel: f32,
  fresnelSoftness: f32,
  fresnelColor: vec4f,
  edgeSoftness: f32,
  innerZoom: f32,
  lightAngle: f32,
  highlights: f32,
  shadows: f32,
  brightness: f32,
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

fn fieldGradient(surf0: vec4f, surfX: vec4f, surfY: vec4f, eps: f32) -> vec2f {
  return vec2f(((surfX.x - surf0.x) / eps), ((surfY.x - surf0.x) / eps));
}

fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(1) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn volumetricNormal(surf0: vec4f, surfX: vec4f, surfY: vec4f, eps: f32, gradClamp: f32) -> vec3f {
  let ddx = clamp(((surfX.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  let ddy = clamp(((surfY.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  return normalize(vec3f(ddx, ddy, -1f));
}

fn patternCoords(sdfUV: vec2f, surf0: vec4f, volumetric: f32) -> vec2f {
  return vec2f(select((sdfUV.x - 0.5f), surf0.y, (volumetric > 0.5f)), select((sdfUV.y - 0.5f), surf0.z, (volumetric > 0.5f)));
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

fn mxHashInt2(x: i32, y: i32, z: i32) -> u32 {
  const len = 3u;
  let seed = ((3735928559u + (len << 2u)) + 13u);
  let a = (seed + u32(x));
  let b = (seed + u32(y));
  let c = (seed + u32(z));
  return mxBjfinal(a, b, c);
}

fn mxBitsTo01(bits: u32) -> f32 {
  return (f32(bits) / 4294967296f);
}

fn mxCellNoiseVec3_2d(p: vec2f) -> vec3f {
  let ix = i32(floor(p.x));
  let iy = i32(floor(p.y));
  return vec3f(mxBitsTo01(mxHashInt2(ix, iy, 0i)), mxBitsTo01(mxHashInt2(ix, iy, 1i)), mxBitsTo01(mxHashInt2(ix, iy, 2i)));
}

fn mxWorleyDistance2d(p: vec2f, x: i32, y: i32, xoff: i32, yoff: i32, jitter: f32, metric: i32) -> f32 {
  let tmp = mxCellNoiseVec3_2d(vec2f(f32((x + xoff)), f32((y + yoff))));
  var off = vec2f(tmp.x, tmp.y);
  off = (((off - 0.5f) * jitter) + 0.5f);
  let cellpos = (vec2f(f32(x), f32(y)) + off);
  let diff = (cellpos - p);
  if ((metric == 2i)) {
    return (abs(diff.x) + abs(diff.y));
  }
  if ((metric == 3i)) {
    return max(abs(diff.x), abs(diff.y));
  }
  return dot(diff, diff);
}

fn mxWorleyNoiseFloat2(p: vec2f, jitter: f32, metric: i32) -> f32 {
  let X = i32(floor(p.x));
  let Y = i32(floor(p.y));
  let localpos = vec2f((p.x - f32(X)), (p.y - f32(Y)));
  var sqdist = 1e+6f;
  for (var x = -1; (x <= 1i); x++) {
    for (var y = -1; (y <= 1i); y++) {
      let dist = mxWorleyDistance2d(localpos, x, y, X, Y, jitter, metric);
      sqdist = min(sqdist, dist);
    }
  }
  if ((metric == 0i)) {
    sqdist = sqrt(sqdist);
  }
  return sqdist;
}

fn mxWorleyNoiseFloat2Pub(p: vec2f, jitter: f32) -> f32 {
  return mxWorleyNoiseFloat2(p, jitter, 1i);
}

fn insideMask(sdf: f32, sharpEdge: f32, pxH: f32, minPixels: f32) -> f32 {
  let w = max((sharpEdge * 0.03125f), (pxH * minPixels));
  return clamp(((sdf * -1f) / w), 0f, 1f);
}

fn grazingView(n: vec3f) -> f32 {
  return (1f - clamp((n.z * -1f), 0f, 1f));
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
  let sdfUV_5 = sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect);
  let s0_4 = fieldSampleArg(sdfUV_5, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sX_6 = fieldSampleFastArg(offsetUV(sdfUV_5, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sY_7 = fieldSampleFastArg(offsetUV(sdfUV_5, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let grad_3 = fieldGradient(s0_4, sX_6, sY_7, 0.01);
  let sdf_9 = (s0_4.x / uniforms.n_n0.scale);
  let outerWarp_8 = ((1.0 - smoothstep(0.0, 0.03, sdf_9)) * 0.4);
  let aspect_10 = (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y);
  let haloOffX_2 = ((((grad_3.x * -1.0) * (uniforms.n_n0.refraction * 0.15)) * outerWarp_8) / aspect_10);
  let haloOffY_11 = (((grad_3.y * -1.0) * (uniforms.n_n0.refraction * 0.15)) * outerWarp_8);
  let baseColor_1 = unpremultiplyAlpha(textureSample(rtt_0, linearClamp, vec2f((uv.x + haloOffX_2), (uv.y + haloOffY_11))));
  let centerPosX_16 = uniforms.n_n0.center.x;
  let nSurf_20 = volumetricNormal(s0_4, sX_6, sY_7, 0.01, 4.0);
  let suv_28 = sdfUV_5;
  let pat_27 = patternCoords(suv_28, s0_4, 1.0);
  let kfDx_26 = ((pat_27.x + 0.5) - 0.5);
  let kfDy_29 = ((pat_27.y + 0.5) - 0.5);
  let kfR_25 = sqrt(((kfDx_26 * kfDx_26) + (kfDy_29 * kfDy_29)));
  let sAngle_32 = (6.283185307179586 / uniforms.n_n0.facets);
  let kfInSector_31 = (fract(((atan2(kfDy_29, kfDx_26) + 3.141592653589793) / sAngle_32)) * sAngle_32);
  let kfFolded_30 = min(kfInSector_31, (sAngle_32 - kfInSector_31));
  let fUv_24 = vec2f((kfR_25 * cos(kfFolded_30)), (kfR_25 * sin(kfFolded_30)));
  let crystalRNorm_34 = (sqrt(((pat_27.x * pat_27.x) + (pat_27.y * pat_27.y))) / 0.35);
  let vScale1_33 = ((uniforms.n_n0.facets * 1.2) * (1.0 + (clamp(crystalRNorm_34, 0.0, 1.0) * 0.6)));
  let fUvS1_23 = (fUv_24 * vScale1_33);
  let facet10_35 = mxWorleyNoiseFloat2Pub(fUvS1_23, 1.0);
  let facet1X_22 = ((mxWorleyNoiseFloat2Pub((fUvS1_23 + vec2f(0.02, 0.0)), 1.0) - facet10_35) / 0.02);
  let facet1Y_37 = ((mxWorleyNoiseFloat2Pub((fUvS1_23 + vec2f(0.0, 0.02)), 1.0) - facet10_35) / 0.02);
  let gLen1_36 = max(sqrt(((facet1X_22 * facet1X_22) + (facet1Y_37 * facet1Y_37))), 0.001);
  let nx1_21 = (facet1X_22 / gLen1_36);
  let facetRefrAmp_38 = clamp((uniforms.n_n0.refraction * 0.8), 0.0, 1.6);
  let tableMask_40 = (1.0 - smoothstep(0.22, 0.32, crystalRNorm_34));
  let tableSuppress_39 = (1.0 - (tableMask_40 * 0.9));
  let ny1_41 = (facet1Y_37 / gLen1_36);
  let fUvS2_44 = ((fUv_24 * (vScale1_33 * 1.7)) + vec2f(3.7, 1.2));
  let facet20_45 = mxWorleyNoiseFloat2Pub(fUvS2_44, 1.0);
  let facet2X_43 = ((mxWorleyNoiseFloat2Pub((fUvS2_44 + vec2f(0.02, 0.0)), 1.0) - facet20_45) / 0.02);
  let facet2Y_47 = ((mxWorleyNoiseFloat2Pub((fUvS2_44 + vec2f(0.0, 0.02)), 1.0) - facet20_45) / 0.02);
  let gLen2_46 = max(sqrt(((facet2X_43 * facet2X_43) + (facet2Y_47 * facet2Y_47))), 0.001);
  let nx2_42 = (facet2X_43 / gLen2_46);
  let ny2_48 = (facet2Y_47 / gLen2_46);
  let nRefr_19 = normalize(((nSurf_20 + vec3f((((nx1_21 * 0.75) * facetRefrAmp_38) * tableSuppress_39), (((ny1_41 * 0.75) * facetRefrAmp_38) * tableSuppress_39), 0.0)) + vec3f(((nx2_42 * 0.55) * facetRefrAmp_38), ((ny2_48 * 0.55) * facetRefrAmp_38), 0.0)));
  let refr_18 = refract(vec3f(0.0, 0.0, 1.0), nRefr_19, (1.0 / (1.0 + (uniforms.n_n0.refraction * 0.55))));
  let thickEff_50 = clamp((s0_4.x * -1.0), 0.0, 0.6);
  let cellId1_52 = floor(fUvS1_23);
  let cellRand_51 = fract((sin(((cellId1_52.x * 12.9898) + (cellId1_52.y * 78.233))) * 43758.5453));
  let sharp_54 = max((uniforms.n_n0.edgeSoftness * 0.5), 0.001);
  let pxH_55 = (1.0 / uniforms._sys.viewportSize.y);
  let rb1_53 = insideMask(sdf_9, sharp_54, pxH_55, 1.5);
  let offMag_49 = (((thickEff_50 * (0.55 + (cellRand_51 * 0.9))) * uniforms.n_n0.scale) * rb1_53);
  let totalOffX_17 = ((((refr_18.x / max(refr_18.z, 0.25)) * offMag_49) / aspect_10) + haloOffX_2);
  let lensX_15 = ((centerPosX_16 + ((uv.x - centerPosX_16) / uniforms.n_n0.innerZoom)) + totalOffX_17);
  let chrOffX_56 = (totalOffX_17 * (uniforms.n_n0.dispersion * 0.12));
  let centerPosY_58 = (1.0 - uniforms.n_n0.center.y);
  let totalOffY_59 = (((refr_18.y / max(refr_18.z, 0.25)) * offMag_49) + haloOffY_11);
  let lensY_57 = ((centerPosY_58 + ((uv.y - centerPosY_58) / uniforms.n_n0.innerZoom)) + totalOffY_59);
  let chrOffY_60 = (totalOffY_59 * (uniforms.n_n0.dispersion * 0.12));
  let sampledR_14 = unpremultiplyAlpha(textureSample(rtt_0, linearClamp, vec2f((lensX_15 + chrOffX_56), (lensY_57 + chrOffY_60))));
  let sampledG_61 = unpremultiplyAlpha(textureSample(rtt_0, linearClamp, vec2f(lensX_15, lensY_57)));
  let sampledB_62 = unpremultiplyAlpha(textureSample(rtt_0, linearClamp, vec2f((lensX_15 - chrOffX_56), (lensY_57 - chrOffY_60))));
  let nBack_65 = normalize(vec3f((nx2_42 * 0.7), (ny2_48 * 0.7), -1.0));
  let bounce_64 = reflect(refr_18, nBack_65);
  let bounceMag_66 = (((thickEff_50 * uniforms.n_n0.scale) * rb1_53) * 0.8);
  let innerSample_63 = unpremultiplyAlpha(textureSample(rtt_0, linearClamp, vec2f((lensX_15 + (((bounce_64.x / max(abs(bounce_64.z), 0.3)) * bounceMag_66) / aspect_10)), (lensY_57 + ((bounce_64.y / max(abs(bounce_64.z), 0.3)) * bounceMag_66)))));
  let grazing_68 = grazingView(nSurf_20);
  let innerW_67 = ((((grazing_68 * 0.5) + 0.18) * ((cellRand_51 * 0.7) + 0.3)) * rb1_53);
  let nF1_70 = normalize((nSurf_20 + vec3f((nx1_21 * 0.55), (ny1_41 * 0.55), 0.0)));
  let lx_72 = (cos((uniforms.n_n0.lightAngle * 0.017453292519943295)) + ((centerPosX_16 - 0.5) * 3.0));
  let ly_73 = (sin((uniforms.n_n0.lightAngle * 0.017453292519943295)) + ((centerPosY_58 - 0.5) * 3.0));
  let kl1_71 = keyLight(lx_72, ly_73, -0.75);
  let ndlS_74 = dot(nSurf_20, kl1_71.L);
  let ndl1_69 = ((dot(nF1_70, kl1_71.L) - ndlS_74) * 2.2);
  let nF2_76 = normalize((nSurf_20 + vec3f((nx2_42 * 0.4), (ny2_48 * 0.4), 0.0)));
  let kl2_77 = keyLight((ly_73 * -1.0), lx_72, -0.4);
  let ndl2_75 = ((dot(nF2_76, kl2_77.L) - dot(nSurf_20, kl2_77.L)) * 2.2);
  let kfDx_82 = (((pat_27.x + (((refr_18.x / max(refr_18.z, 0.25)) * thickEff_50) * 1.2)) + 0.5) - 0.5);
  let kfDy_83 = (((pat_27.y + (((refr_18.y / max(refr_18.z, 0.25)) * thickEff_50) * 1.2)) + 0.5) - 0.5);
  let kfR_81 = sqrt(((kfDx_82 * kfDx_82) + (kfDy_83 * kfDy_83)));
  let kfInSector_85 = (fract(((atan2(kfDy_83, kfDx_82) + 3.141592653589793) / sAngle_32)) * sAngle_32);
  let kfFolded_84 = min(kfInSector_85, (sAngle_32 - kfInSector_85));
  let fUvS3_80 = ((vec2f((kfR_81 * cos(kfFolded_84)), (kfR_81 * sin(kfFolded_84))) * (vScale1_33 * 1.4)) + vec2f(9.1, 4.7));
  let cellId3_79 = floor(fUvS3_80);
  let cellRand3_78 = fract((sin(((cellId3_79.x * 31.727) + (cellId3_79.y * 57.131))) * 43758.5453));
  let facet30_89 = mxWorleyNoiseFloat2Pub(fUvS3_80, 1.0);
  let facet3X_88 = ((mxWorleyNoiseFloat2Pub((fUvS3_80 + vec2f(0.02, 0.0)), 1.0) - facet30_89) / 0.02);
  let facet3Y_91 = ((mxWorleyNoiseFloat2Pub((fUvS3_80 + vec2f(0.0, 0.02)), 1.0) - facet30_89) / 0.02);
  let gLen3_90 = max(sqrt(((facet3X_88 * facet3X_88) + (facet3Y_91 * facet3Y_91))), 0.001);
  let nx3_87 = (facet3X_88 / gLen3_90);
  let ny3_92 = (facet3Y_91 / gLen3_90);
  let wedgeLit_86 = ((((nx3_87 * lx_72) + (ny3_92 * ly_73)) * 0.5) + 0.5);
  let w3_93 = ((clamp((thickEff_50 * 2.5), 0.0, 1.0) * clamp((uniforms.n_n0.refraction * 1.2), 0.0, 1.0)) * rb1_53);
  let litRgb_13 = (mix(vec3f(sampledR_14.x, sampledG_61.y, sampledB_62.z), vec3f(innerSample_63.x, innerSample_63.y, innerSample_63.z), vec3f(innerW_67)) * ((((((max(((1.0 + (clamp(ndl1_69, 0.0, 1.0) * uniforms.n_n0.highlights)) - (clamp((ndl1_69 * -1.0), 0.0, 1.0) * uniforms.n_n0.shadows)), 0.02) * ((1.0 + (clamp(ndl2_75, 0.0, 1.0) * (uniforms.n_n0.highlights * 0.5))) - (clamp((ndl2_75 * -1.0), 0.0, 1.0) * (uniforms.n_n0.shadows * 0.5)))) * ((1.0 + ((clamp(ndlS_74, 0.0, 1.0) * uniforms.n_n0.highlights) * 0.4)) - ((clamp((ndlS_74 * -1.0), 0.0, 1.0) * uniforms.n_n0.shadows) * 0.6))) + (tableMask_40 * 0.25)) * ((mix(smoothstep(0.2, 0.8, gLen1_36), 1.0, tableMask_40) * 0.4) + 0.6)) * mix(1.0, ((((cellRand3_78 * 0.9) + 0.55) * ((wedgeLit_86 * 0.7) + 0.65)) * ((smoothstep(0.15, 0.7, gLen3_90) * 0.45) + 0.55)), w3_93)) * uniforms.n_n0.brightness));
  let tinted_12 = mix(litRgb_13, uniforms.n_n0.tintColor.rgb, vec3f(uniforms.n_n0.tintIntensity));
  let ndh1_95 = clamp(dot(nF1_70, kl1_71.H), 0.0, 1.0);
  let flashContrib_94 = ((((((pow(ndh1_95, 110.0) * 2.2) + (pow(clamp(dot(nF2_76, kl2_77.H), 0.0, 1.0), 56.0) * 0.9)) + (smoothstep(0.965, 0.995, ndh1_95) * 3.0)) + ((smoothstep(0.75, 1.0, (wedgeLit_86 * (cellRand3_78 + 0.4))) * w3_93) * 0.8)) * uniforms.n_n0.highlights) * uniforms.n_n0.brightness);
  let rb2_96 = (((clamp(((((sdf_9 - pxH_55) * -1.0) / sharp_54) * 16.0), 0.0, 1.0) - clamp((((sdf_9 * -1.0) / sharp_54) * 16.0), 0.0, 1.0)) * ((((grad_3.x * lx_72) + (grad_3.y * ly_73)) * 0.5) + 0.5)) * 0.35);
  let transition_97 = smoothstep(0.0, 1.0, rb1_53);
  let composited_0 = mix(baseColor_1, vec4f((((select(tinted_12, (tinted_12 * (dot(litRgb_13, vec3f(0.299, 0.587, 0.114)) / max(dot(tinted_12, vec3f(0.299, 0.587, 0.114)), 0.0001))), (uniforms.n_n0.tintPreserveLuminosity > 0.5)) + vec3f(flashContrib_94)) + (uniforms.n_n0.fresnelColor.rgb * (((pow(grazing_68, mix(6.0, 1.5, clamp((uniforms.n_n0.fresnelSoftness * 0.5), 0.0, 1.0))) * uniforms.n_n0.fresnel) * 2.0) * rb1_53))) + vec3f(rb2_96)), sampledG_61.w), vec4f(transition_97));
  var composed_98 = vec4f(composited_0.xyz, (composited_0.w * 1.0));
  return vec4f(linearToSrgb(tone_linear(composed_98.rgb)) * composed_98.a, composed_98.a);
}