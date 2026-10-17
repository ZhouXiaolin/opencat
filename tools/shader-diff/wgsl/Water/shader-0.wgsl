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
  waterColor: vec4f,
  clarity: f32,
  depth: f32,
  shallows: f32,
  caustics: f32,
  causticScale: f32,
  choppiness: f32,
  waveScale: f32,
  swirl: f32,
  speed: f32,
  reflection: f32,
  envRotation: f32,
  sharpness: f32,
  lightAngle: f32,
  foam: f32,
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

fn volumetricNormal(surf0: vec4f, surfX: vec4f, surfY: vec4f, eps: f32, gradClamp: f32) -> vec3f {
  let ddx = clamp(((surfX.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  let ddy = clamp(((surfY.w - surf0.w) / eps), (gradClamp * -1f), gradClamp);
  return normalize(vec3f(ddx, ddy, -1f));
}

fn patternCoords(sdfUV: vec2f, surf0: vec4f, volumetric: f32) -> vec2f {
  return vec2f(select((sdfUV.x - 0.5f), surf0.y, (volumetric > 0.5f)), select((sdfUV.y - 0.5f), surf0.z, (volumetric > 0.5f)));
}

fn flowWarpOffset(warpInput: vec2f, flowT: f32) -> vec2f {
  let wx = (sin((((warpInput.x * 1.7f) + (flowT * 0.7f)) + 1.7f)) * cos((((warpInput.y * 1.3f) + (flowT * 0.5f)) + 2.3f)));
  let wy = (cos((((warpInput.x * 1.4f) + (flowT * 0.9f)) + 8.7f)) * sin((((warpInput.y * 1.6f) + (flowT * 0.6f)) + 5.1f)));
  return vec2f(wx, wy);
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
  var water_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let wc_7 = uniforms.n_n0.waterColor.rgb;
  let thickness_9 = (opticalThickness(sdf_1, 1.0) * uniforms.n_n0.depth);
  let depthT_8 = clamp((thickness_9 * 1.3), 0.0, 1.0);
  let transmit_10 = beerLambert(thickness_9, vec3f(((1.0 - wc_7.x) * 2.2), ((1.0 - wc_7.y) * 2.2), ((1.0 - wc_7.z) * 2.2)));
  let bodyTint_6 = ((mix(mix(wc_7, vec3f(1.0, 1.0, 1.0), vec3f(clamp(uniforms.n_n0.shallows, 0.0, 1.0))), wc_7, vec3f(depthT_8)) * transmit_10) + (wc_7 * (depthT_8 * 0.18)));
  let sX_16 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.01, 0.0), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sY_17 = fieldSampleFastArg(offsetUV(sdfUV_3, 0.0, 0.01), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let nGeo_15 = volumetricNormal(s0_2, sX_16, sY_17, 0.01, 5.0);
  let pat_20 = patternCoords(sdfUV_3, s0_2, 1.0);
  let freq_21 = (uniforms.n_n0.waveScale * 2.4);
  let q_19 = vec2f(((pat_20.x * freq_21) * 0.8), ((pat_20.y * freq_21) * 1.25));
  let flowT_22 = (uniforms.n_n0._animTime * 0.2);
  let qw_18 = ((q_19 + (flowWarpOffset((q_19 * 0.6), flowT_22) * (uniforms.n_n0.swirl * 0.9))) + vec2f(0.0, (flowT_22 * -1.0)));
  let h0n1_23 = (mxNoiseFloat2(qw_18) * 0.52);
  let relief_24 = (uniforms.n_n0.choppiness * 0.5);
  let qf_25 = (((qw_18 * 3.3) + vec2f((flowT_22 * 1.7), (flowT_22 * -1.15))) + vec2f(11.3, 4.7));
  let f0n1_26 = (mxNoiseFloat2(qf_25) * 0.52);
  let reliefFine_27 = (uniforms.n_n0.choppiness * 0.22);
  let n_14 = normalize(vec3f(((nGeo_15.x - (clamp((((mxNoiseFloat2((qw_18 + vec2f(0.06, 0.0))) * 0.52) - h0n1_23) / 0.06), -4.0, 4.0) * relief_24)) - (clamp((((mxNoiseFloat2((qf_25 + vec2f(0.025, 0.0))) * 0.52) - f0n1_26) / 0.025), -4.0, 4.0) * reliefFine_27)), ((nGeo_15.y - (clamp((((mxNoiseFloat2((qw_18 + vec2f(0.0, 0.06))) * 0.52) - h0n1_23) / 0.06), -4.0, 4.0) * relief_24)) - (clamp((((mxNoiseFloat2((qf_25 + vec2f(0.0, 0.025))) * 0.52) - f0n1_26) / 0.025), -4.0, 4.0) * reliefFine_27)), nGeo_15.z));
  let tDir_13 = refract(vec3f(0.0, 0.0, 1.0), n_14, 0.7501875468867217);
  let env_28 = vec2f(cos((uniforms.n_n0.envRotation * 0.017453292519943295)), sin((uniforms.n_n0.envRotation * 0.017453292519943295)));
  let transEy_12 = ((tDir_13.x * env_28.y) + (tDir_13.y * env_28.x));
  let transUp_11 = clamp(((transEy_12 * 0.5) + 0.5), 0.0, 1.0);
  let sharp_30 = clamp(uniforms.n_n0.sharpness, 0.0, 1.0);
  let transSunRad_29 = mix(0.5, 0.13, sharp_30);
  let transEx_31 = ((tDir_13.x * env_28.x) - (tDir_13.y * env_28.y));
  let light_32 = vec2f(cos((uniforms.n_n0.lightAngle * 0.017453292519943295)), sin((uniforms.n_n0.lightAngle * 0.017453292519943295)));
  let kl_34 = keyLight(light_32.x, light_32.y, -0.7);
  let diffuse_33 = ((pow(clamp(((dot(n_14, kl_34.L) * 0.5) + 0.5), 0.0, 1.0), 0.8) * 0.5) + 0.5);
  let causT_38 = (uniforms.n_n0._animTime * 3.0);
  let causP_37 = ((vec2f(pat_20.x, pat_20.y) * uniforms.n_n0.causticScale) + vec2f((sin((causT_38 * 0.15)) * 0.4), ((causT_38 * 0.3) * -1.0)));
  let fbmC1_36 = causP_37;
  let fbmC2_39 = ((causP_37 * 1.9) + vec2f(3.7, ((causT_38 * 0.18) * -1.0)));
  let causV_35 = pow(clamp((((1.0 - abs((((mxNoiseFloat2(fbmC1_36) + (mxNoiseFloat2(((fbmC1_36 * 2.07) + vec2f(4.3, 9.1))) * 0.5)) + (mxNoiseFloat2(((fbmC1_36 * 4.13) + vec2f(1.7, 8.2))) * 0.25)) * 0.52))) * 0.6) + ((1.0 - abs((((mxNoiseFloat2(fbmC2_39) + (mxNoiseFloat2(((fbmC2_39 * 2.07) + vec2f(4.3, 9.1))) * 0.5)) + (mxNoiseFloat2(((fbmC2_39 * 4.13) + vec2f(1.7, 8.2))) * 0.25)) * 0.52))) * 0.4)), 0.0, 1.0), 3.0);
  let causMask_40 = (((depthT_8 * 0.8) + 0.2) * uniforms.n_n0.caustics);
  let R_44 = reflect(vec3f(0.0, 0.0, 1.0), n_14);
  let reflEy_43 = ((R_44.x * env_28.y) + (R_44.y * env_28.x));
  let reflUp_42 = clamp(((reflEy_43 * 0.5) + 0.5), 0.0, 1.0);
  let reflSunRad_45 = mix(0.5, 0.13, sharp_30);
  let reflEx_46 = ((R_44.x * env_28.x) - (R_44.y * env_28.y));
  let reflCol_41 = ((mix(vec3f(0.04, 0.1, 0.16), mix(vec3f(0.9, 0.96, 1.0), vec3f(0.2, 0.46, 0.85), vec3f(smoothstep(0.5, 1.0, reflUp_42))), vec3f(smoothstep(0.4, 0.55, reflUp_42))) + (vec3f(1.0, 0.97, 0.88) * (smoothstep(reflSunRad_45, (reflSunRad_45 * 0.15), length((vec2f(reflEx_46, reflEy_43) - vec2f((light_32.x * 0.8), ((light_32.y * 0.8) + 0.12))))) * 1.5))) * uniforms.n_n0.reflection);
  let grazing_47 = grazingView(n_14);
  let fbmH0_50 = qw_18;
  let h0_49 = (((mxNoiseFloat2(fbmH0_50) + (mxNoiseFloat2(((fbmH0_50 * 2.07) + vec2f(4.3, 9.1))) * 0.5)) + (mxNoiseFloat2(((fbmH0_50 * 4.13) + vec2f(1.7, 8.2))) * 0.25)) * 0.52);
  let foamMask_48 = (clamp((smoothstep(0.62, 0.92, clamp(((h0_49 * 0.5) + 0.5), 0.0, 1.0)) + ((1.0 - smoothstep(0.0, 0.045, (sdf_1 * -1.0))) * 0.7)), 0.0, 1.0) * uniforms.n_n0.foam);
  let waterRgb_5 = mix((mix((((mix(bodyTint_6, ((((mix(vec3f(0.04, 0.1, 0.16), mix(vec3f(0.9, 0.96, 1.0), vec3f(0.2, 0.46, 0.85), vec3f(smoothstep(0.5, 1.0, transUp_11))), vec3f(smoothstep(0.4, 0.55, transUp_11))) + (vec3f(1.0, 0.97, 0.88) * (smoothstep(transSunRad_29, (transSunRad_29 * 0.15), length((vec2f(transEx_31, transEy_12) - vec2f((light_32.x * 0.8), ((light_32.y * 0.8) + 0.12))))) * 1.5))) * transmit_10) * 0.6) + (bodyTint_6 * 0.4)), vec3f((clamp(uniforms.n_n0.clarity, 0.0, 1.0) * (1.0 - (depthT_8 * 0.8))))) * diffuse_33) * (1.0 + ((causV_35 * causMask_40) * 0.7))) + (vec3f(0.55, 0.82, 1.0) * ((causV_35 * causMask_40) * 0.45))), reflCol_41, vec3f(clamp((0.04 + (pow(grazing_47, 5.0) * 0.96)), 0.0, 1.0))) + (vec3f(1.0, 0.98, 0.92) * sharpGlint(clamp(dot(n_14, kl_34.H), 0.0, 1.0), sharp_30, vec4f(40.0, 600.0, 0.3, 1.3)))), vec3f(1.0, 1.0, 1.0), vec3f((foamMask_48 * 0.85)));
  water_0 = vec4f(waterRgb_5, silhouetteAlpha(sdf_1, uniforms.n_n0.edgeSoftness, pxH_4));
}
  var composed_51 = water_0;
  return vec4f(linearToSrgb(tone_linear(composed_51.rgb)) * composed_51.a, composed_51.a);
}