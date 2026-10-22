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
  iceColor: vec4f,
  ambient: f32,
  edgeSoftness: f32,
  density: f32,
  absorption: f32,
  scatter: f32,
  frostAmount: f32,
  frostDepth: f32,
  frostScale: f32,
  frostRoughness: f32,
  sparkle: f32,
  gloss: f32,
  fresnel: f32,
  lightAngle: f32,
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

fn opticalThickness(sdf: f32, volumetric: f32) -> f32 {
  let thickVol = clamp(((sdf * -1f) * 2f), 0f, 1.2f);
  let thickFlat = (clamp((sdf * -1f), 0f, 0.5f) * 2f);
  return select(thickFlat, thickVol, (volumetric > 0.5f));
}

fn beerLambert(thickness: f32, absorb: vec3f) -> vec3f {
  return vec3f(exp(((thickness * absorb.x) * -1f)), exp(((thickness * absorb.y) * -1f)), exp(((thickness * absorb.z) * -1f)));
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

fn grazingView(n: vec3f) -> f32 {
  return (1f - clamp((n.z * -1f), 0f, 1f));
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
  var frost_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let thickness_8 = (opticalThickness(sdf_1, 1.0) * uniforms.n_n0.density);
  let depthT_7 = clamp((thickness_8 * 1.2), 0.0, 1.0);
  let transmit_9 = beerLambert(thickness_8, (vec3f(1.6, 0.85, 0.45) * uniforms.n_n0.absorption));
  let scatteredRgb_6 = (mix(vec3f(1.0, 1.0, 1.0), uniforms.n_n0.iceColor.rgb, vec3f(depthT_7)) * transmit_9);
  let sX_14 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let ddx_13 = clamp(((sX_14.w - s0_2.w) / 0.01), -4.0, 4.0);
  let sY_16 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let ddy_15 = clamp(((sY_16.w - s0_2.w) / 0.01), -4.0, 4.0);
  let nBase_12 = normalize(vec3f(ddx_13, ddy_15, -1.0));
  let frostField_19 = ((1.0 - smoothstep(0.0, max((uniforms.n_n0.frostDepth * 0.3), 0.01), (sdf_1 * -1.0))) * uniforms.n_n0.frostAmount);
  let pat_20 = patternCoords(sdfUV_3, s0_2, 1.0);
  let frostScl_21 = (uniforms.n_n0.frostScale * 60.0);
  let steepAtten_22 = (1.0 / (1.0 + (clamp(((ddx_13 * ddx_13) + (ddy_15 * ddy_15)), 0.0, 2.0) * 2.0)));
  var frostFx_18 = vec3f(0.0, 0.0, 0.0);
  if ((frostField_19 > 0.0)) {
  let fn0_23 = mxNoiseFloat2(vec2f((pat_20.x * frostScl_21), (pat_20.y * frostScl_21)));
  let frostNormalAmp_24 = ((frostField_19 * uniforms.n_n0.frostRoughness) * steepAtten_22);
  frostFx_18 = vec3f(((((mxNoiseFloat2(vec2f(((pat_20.x + 0.004) * frostScl_21), (pat_20.y * frostScl_21))) - fn0_23) / 0.004) * frostNormalAmp_24) * 0.03), ((((mxNoiseFloat2(vec2f((pat_20.x * frostScl_21), ((pat_20.y + 0.004) * frostScl_21))) - fn0_23) / 0.004) * frostNormalAmp_24) * 0.03), (smoothstep(0.25, 0.0, mxWorleyNoiseFloat2Pub(vec2f((pat_20.x * (uniforms.n_n0.frostScale * 40.0)), (pat_20.y * (uniforms.n_n0.frostScale * 40.0))), 1.0)) * frostField_19));
}
  let fx3_17 = frostFx_18;
  let n_11 = nudgeNormal(nBase_12, fx3_17.x, fx3_17.y);
  let light_26 = vec2f(cos((uniforms.n_n0.lightAngle * 0.017453292519943295)), sin((uniforms.n_n0.lightAngle * 0.017453292519943295)));
  let kl_25 = keyLight(light_26.x, light_26.y, -0.6);
  let ndl_10 = dot(n_11, kl_25.L);
  let ndh_28 = clamp(dot(n_11, kl_25.H), 0.0, 1.0);
  let broadSpec_27 = ((pow(ndh_28, 16.0) * 0.4) * uniforms.n_n0.gloss);
  let grazing_29 = grazingView(n_11);
  var sparkleGlint_30 = 0.0;
  if ((frostField_19 > 0.0)) {
  let glintField_31 = ((mxNoiseFloat2(vec2f(((pat_20.x * 190.0) + 13.7), ((pat_20.y * 190.0) - 4.3))) * 0.5) + 0.5);
  sparkleGlint_30 = (((smoothstep(0.72, 0.96, (glintField_31 * ((sin(((uniforms.n_n0._animTime * 3.0) + (glintField_31 * 40.0))) * 0.5) + 0.5))) * ((pow(ndh_28, 6.0) * 0.6) + 0.4)) * frostField_19) * uniforms.n_n0.sparkle);
}
  let iceRgb_5 = ((((((scatteredRgb_6 * ((pow(((ndl_10 * 0.5) + 0.5), 0.7) * 0.6) + 0.5)) + ((((transmit_9 * clamp(((dot(kl_25.L, vec3f(0.0, 0.0, 1.0)) * 0.5) + 0.5), 0.0, 1.0)) * uniforms.n_n0.scatter) * (1.0 - (depthT_7 * 0.4))) * 0.5)) + (vec3f(0.4, 0.46, 0.55) * uniforms.n_n0.ambient)) + vec3f(broadSpec_27)) + (vec3f(0.85, 0.93, 1.0) * ((pow(grazing_29, 3.0) * uniforms.n_n0.fresnel) * 1.5))) + (vec3f(0.95, 0.98, 1.0) * ((fx3_17.z * 0.4) + (sparkleGlint_30 * 4.0))));
  frost_0 = vec4f(iceRgb_5, silhouetteAlpha(sdf_1, uniforms.n_n0.edgeSoftness, pxH_4));
}
  var composed_32 = frost_0;
  return vec4f(linearToSrgb(tone_linear(composed_32.rgb)) * composed_32.a, composed_32.a);
}