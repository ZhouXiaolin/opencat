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
  voxelSize: f32,
  voxelScale: f32,
  fill: f32,
  bevel: f32,
  depth: f32,
  colorA: vec4f,
  colorB: vec4f,
  colorVariation: f32,
  colorSpace: f32,
  lightAngle: f32,
  lightElevation: f32,
  lightColor: vec4f,
  lightIntensity: f32,
  ambientColor: vec4f,
  ambient: f32,
  shadows: f32,
  shadowSoftness: f32,
  ao: f32,
  glossiness: f32,
  specular: f32,
  seams: f32,
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
  _vxMx: vec3f,
  _vxMy: vec3f,
  _vxMz: vec3f,
  _vxVoxel: f32,
  _vxGridOrigin: f32,
  _vxL: vec3f,
  _vxT1: vec3f,
  _vxT2: vec3f,
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

@group(1) @binding(0) var compute_0: texture_2d<f32>;

fn outsideShape(sdf: f32, pxH: f32) -> bool {
  return (sdf > (pxH * 2f));
}

fn fieldSampleTexelArg(uv: vec2f, field: texture_2d<f32>, originX: f32, originY: f32, spanX: f32, spanY: f32, activeResF: f32, rBound: f32) -> vec4f {
  let uxc = clamp(uv.x, originX, (originX + spanX));
  let uyc = clamp(uv.y, originY, (originY + spanY));
  let outsideDist = length(vec2f((uv.x - uxc), (uv.y - uyc)));
  let maxT = (activeResF - 1f);
  let px = clamp(floor((((uxc - originX) / spanX) * activeResF)), 0f, maxT);
  let py = clamp(floor((((uyc - originY) / spanY) * activeResF)), 0f, maxT);
  let t = textureLoad(field, vec2u(u32(px), u32(py)), 0);
  let rNear = (t.r + outsideDist);
  let rCircle = (length(vec2f((uv.x - 0.5f), (uv.y - 0.5f))) - rBound);
  let rOut = mix(rNear, max(rCircle, rNear), smoothstep(0f, 0.25f, outsideDist));
  return vec4f(rOut, t.g, t.b, t.a);
}

fn floatBitsToUint3(v: vec3f) -> vec3u { return bitcast<vec3u>(v); }

fn hash13(p: vec3f) -> f32 {
  let u = floatBitsToUint3((p * vec3f(141421360, 2718281728, 1618034048)));
  return (f32((((u.x ^ u.y) ^ u.z) * 3141592653u)) / 4294967296f);
}

fn p3ToSRGB(p3: vec3f) -> vec3f {
  let r = (((p3.x * 1.2249401f) - (p3.y * 0.2249404f)) - (p3.z * 0f));
  let g = (((p3.x * -0.0420569f) + (p3.y * 1.0420571f)) + (p3.z * 0f));
  let b = (((p3.x * -0.0196376f) - (p3.y * 0.0786361f)) + (p3.z * 1.0982735f));
  return vec3f(r, g, b);
}

fn rgbToOklab(rgb: vec3f) -> vec3f {
  let r = rgb.x;
  let g = rgb.y;
  let b = rgb.z;
  let l = (((r * 0.4122214708f) + (g * 0.5363325363f)) + (b * 0.0514459929f));
  let m = ((r * 0.2119034982f) + ((g * 0.6806995451f) + (b * 0.1073969566f)));
  let s = (((r * 0.0883024619f) + (g * 0.2817188376f)) + (b * 0.6299787005f));
  let l_ = (sign(l) * pow(abs(l), 0.3333333333333333f));
  let m_ = (sign(m) * pow(abs(m), 0.3333333333333333f));
  let s_ = (sign(s) * pow(abs(s), 0.3333333333333333f));
  return vec3f((((l_ * 0.2104542553f) + (m_ * 0.793617785f)) - (s_ * 0.0040720468f)), (((l_ * 1.9779984951f) - (m_ * 2.428592205f)) + (s_ * 0.4505937099f)), (((l_ * 0.0259040371f) + (m_ * 0.7827717662f)) - (s_ * 0.808675766f)));
}

fn awMix(a: vec3f, b: vec3f, wA: f32, wB: f32, sw: f32) -> vec3f {
  return (((a * wA) + (b * wB)) / sw);
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

fn sRGBToP3(srgb: vec3f) -> vec3f {
  let r = (((srgb.x * 0.8224621f) + (srgb.y * 0.177538f)) + (srgb.z * 0f));
  let g = (((srgb.x * 0.0331941f) + (srgb.y * 0.9668058f)) + (srgb.z * 0f));
  let b = (((srgb.x * 0.0170826f) + (srgb.y * 0.0723974f)) + (srgb.z * 0.9105199f));
  return vec3f(r, g, b);
}

fn mixColors(colorA: vec4f, colorB: vec4f, t: f32) -> vec4f {
  let weightA = (colorA.w * (1f - t));
  let weightB = (colorB.w * t);
  let totalWeight = (weightA + weightB);
  let safeWeight = max(totalWeight, 1e-3f);
  let oklabA = rgbToOklab(p3ToSRGB(colorA.xyz));
  let oklabB = rgbToOklab(p3ToSRGB(colorB.xyz));
  let mixed = awMix(oklabA, oklabB, weightA, weightB, safeWeight);
  let rgb = sRGBToP3(oklabToRgb(mixed));
  return vec4f(rgb, totalWeight);
}

fn voxelRayBox(ro: vec3f, rd: vec3f, c: vec3f, h: f32) -> f32 {
  let sx = select(-1f, 1f, (rd.x >= 0f));
  let sy = select(-1f, 1f, (rd.y >= 0f));
  let sz = select(-1f, 1f, (rd.z >= 0f));
  let ix = (sx / max(abs(rd.x), 1e-6f));
  let iy = (sy / max(abs(rd.y), 1e-6f));
  let iz = (sz / max(abs(rd.z), 1e-6f));
  let t1x = (((c.x - h) - ro.x) * ix);
  let t2x = (((c.x + h) - ro.x) * ix);
  let t1y = (((c.y - h) - ro.y) * iy);
  let t2y = (((c.y + h) - ro.y) * iy);
  let t1z = (((c.z - h) - ro.z) * iz);
  let t2z = (((c.z + h) - ro.z) * iz);
  let tIn = max(min(t1x, t2x), max(min(t1y, t2y), min(t1z, t2z)));
  let tOut = min(max(t1x, t2x), min(max(t1y, t2y), max(t1z, t2z)));
  let ok = (select(0f, 1f, (tIn <= tOut)) * select(0f, 1f, (tOut >= 0f)));
  return select(-1f, max(tIn, 0f), (ok > 0.5f));
}

fn voxelShadowCompare(tex: texture_2d<f32>, mx: f32, my: f32, ref_1: f32) -> f32 {
  const maxT = 1023f;
  let x0 = floor(mx);
  let y0 = floor(my);
  let fx = (mx - x0);
  let fy = (my - y0);
  let cx0 = u32(clamp(x0, 0f, maxT));
  let cx1 = u32(clamp((x0 + 1f), 0f, maxT));
  let cy0 = u32(clamp(y0, 0f, maxT));
  let cy1 = u32(clamp((y0 + 1f), 0f, maxT));
  let s00 = select(0f, 1f, (textureLoad(tex, vec2u(cx0, cy0), 0).r < ref_1));
  let s10 = select(0f, 1f, (textureLoad(tex, vec2u(cx1, cy0), 0).r < ref_1));
  let s01 = select(0f, 1f, (textureLoad(tex, vec2u(cx0, cy1), 0).r < ref_1));
  let s11 = select(0f, 1f, (textureLoad(tex, vec2u(cx1, cy1), 0).r < ref_1));
  return mix(mix(s00, s10, fx), mix(s01, s11, fx), fy);
}

fn voxelShadowLookup(tex: texture_2d<f32>, p: vec3f, L: vec3f, T1: vec3f, T2: vec3f, rBound: f32, softness: f32, voxel: f32) -> f32 {
  const RES = 1024f;
  let texelWorld = ((rBound * 2f) / RES);
  let u = dot(p, T1);
  let w = dot(p, T2);
  let dL = (rBound - dot(p, L));
  let mx = (((((u / rBound) * 0.5f) + 0.5f) * RES) - 0.5f);
  let my = (((((w / rBound) * 0.5f) + 0.5f) * RES) - 0.5f);
  let ref_1 = (dL - ((voxel * 0.05f) + (texelWorld * 2f)));
  var shadow = 0f;
  if ((softness < 1e-3f)) {
    shadow = voxelShadowCompare(tex, mx, my, ref_1);
  }
  else {
    let k = (softness * 0.27f);
    let rSearch = max((min((rBound * k), (rBound * 0.25f)) / texelWorld), 1f);
    var sum = 0f;
    var count = 0f;
    for (var s = 0; (s < 8i); s++) {
      let r = (sqrt(((f32(s) + 0.5f) / 8f)) * rSearch);
      let th = (f32(s) * 2.39996323f);
      let sx = clamp((mx + (cos(th) * r)), 0f, (RES - 1f));
      let sy = clamp((my + (sin(th) * r)), 0f, (RES - 1f));
      let dep = textureLoad(tex, vec2u(u32(sx), u32(sy)), 0).r;
      if ((dep < ref_1)) {
        sum = (sum + dep);
        count = (count + 1f);
      }
    }
    if ((count > 0.5f)) {
      let avgBlocker = (sum / count);
      let penumbra = clamp((((dL - avgBlocker) * k) / texelWorld), 1f, rSearch);
      var acc = 0f;
      for (var s = 0; (s < 12i); s++) {
        let r = (sqrt(((f32(s) + 0.5f) / 12f)) * penumbra);
        let th = ((f32(s) * 2.39996323f) + 0.7f);
        acc = (acc + voxelShadowCompare(tex, (mx + (cos(th) * r)), (my + (sin(th) * r)), ref_1));
      }
      shadow = (acc / 12f);
    }
  }
  return shadow;
}

@group(1) @binding(1) var compute_1: texture_2d<f32>;

fn perspectiveViewRay(uv: vec2f, aspect: f32, fov: f32) -> vec3f {
  return normalize(vec3f((((uv.x - 0.5f) * aspect) * fov), ((uv.y - 0.5f) * fov), 1f));
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
  let s0_2 = fieldSampleFastArg(sdfUV_3, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let sdf_1 = (s0_2.x / uniforms.n_n0.scale);
  let pxH_4 = (1.0 / uniforms._sys.viewportSize.y);
  var voxels_0 = vec4f(0.0, 0.0, 0.0, 0.0);
  if ((!(outsideShape(sdf_1, pxH_4)))) {
  let vxRes_13 = uniforms.n_n0._vfActiveRes;
  let vxX0_12 = clamp(floor(((((sdfUV_3.x - uniforms.n_n0._vfOriginX) / uniforms.n_n0._vfSpanX) * vxRes_13) - 0.5)), 0.0, (vxRes_13 - 1.0));
  let vxY0_14 = clamp(floor(((((sdfUV_3.y - uniforms.n_n0._vfOriginY) / uniforms.n_n0._vfSpanY) * vxRes_13) - 0.5)), 0.0, (vxRes_13 - 1.0));
  let vxUv10_11 = vec2f((uniforms.n_n0._vfOriginX + (((min((vxX0_12 + 1.0), (vxRes_13 - 1.0)) + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanX)), (uniforms.n_n0._vfOriginY + (((vxY0_14 + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanY)));
  let vxTex10_10 = fieldSampleTexelArg(vxUv10_11, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let cz10_15 = floor((vxTex10_10.z / 65536.0));
  let cy10_16 = floor(((vxTex10_10.z - (cz10_15 * 65536.0)) / 256.0));
  let cx10_9 = ((vxTex10_10.z - (cz10_15 * 65536.0)) - (cy10_16 * 256.0));
  let cell10_8 = vec3f(cx10_9, cy10_16, cz10_15);
  let vxUv00_20 = vec2f((uniforms.n_n0._vfOriginX + (((vxX0_12 + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanX)), (uniforms.n_n0._vfOriginY + (((vxY0_14 + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanY)));
  let vxTex00_19 = fieldSampleTexelArg(vxUv00_20, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let cz00_21 = floor((vxTex00_19.z / 65536.0));
  let cy00_22 = floor(((vxTex00_19.z - (cz00_21 * 65536.0)) / 256.0));
  let cx00_18 = ((vxTex00_19.z - (cz00_21 * 65536.0)) - (cy00_22 * 256.0));
  let cell00_17 = vec3f(cx00_18, cy00_22, cz00_21);
  let vxSame1_7 = (select(0.0, 1.0, ((abs((cell10_8 - cell00_17))).x < 0.5)) * (select(0.0, 1.0, ((abs((cell10_8 - cell00_17))).y < 0.5)) * select(0.0, 1.0, ((abs((cell10_8 - cell00_17))).z < 0.5))));
  let vxUv01_27 = vec2f((uniforms.n_n0._vfOriginX + (((vxX0_12 + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanX)), (uniforms.n_n0._vfOriginY + (((min((vxY0_14 + 1.0), (vxRes_13 - 1.0)) + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanY)));
  let vxTex01_26 = fieldSampleTexelArg(vxUv01_27, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let cz01_28 = floor((vxTex01_26.z / 65536.0));
  let cy01_29 = floor(((vxTex01_26.z - (cz01_28 * 65536.0)) / 256.0));
  let cx01_25 = ((vxTex01_26.z - (cz01_28 * 65536.0)) - (cy01_29 * 256.0));
  let cell01_24 = vec3f(cx01_25, cy01_29, cz01_28);
  let vxSame2_23 = (select(0.0, 1.0, ((abs((cell01_24 - cell00_17))).x < 0.5)) * (select(0.0, 1.0, ((abs((cell01_24 - cell00_17))).y < 0.5)) * select(0.0, 1.0, ((abs((cell01_24 - cell00_17))).z < 0.5))));
  let vxUv11_34 = vec2f((uniforms.n_n0._vfOriginX + (((min((vxX0_12 + 1.0), (vxRes_13 - 1.0)) + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanX)), (uniforms.n_n0._vfOriginY + (((min((vxY0_14 + 1.0), (vxRes_13 - 1.0)) + 0.5) / vxRes_13) * uniforms.n_n0._vfSpanY)));
  let vxTex11_33 = fieldSampleTexelArg(vxUv11_34, compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound);
  let cz11_35 = floor((vxTex11_33.z / 65536.0));
  let cy11_36 = floor(((vxTex11_33.z - (cz11_35 * 65536.0)) / 256.0));
  let cx11_32 = ((vxTex11_33.z - (cz11_35 * 65536.0)) - (cy11_36 * 256.0));
  let cell11_31 = vec3f(cx11_32, cy11_36, cz11_35);
  let vxSame3_30 = (select(0.0, 1.0, ((abs((cell11_31 - cell00_17))).x < 0.5)) * (select(0.0, 1.0, ((abs((cell11_31 - cell00_17))).y < 0.5)) * select(0.0, 1.0, ((abs((cell11_31 - cell00_17))).z < 0.5))));
  let cov00_37 = select(0.0, 1.0, (vxTex00_19.x < 0.0));
  let cov10_38 = select(0.0, 1.0, (vxTex10_10.x < 0.0));
  let cov01_39 = select(0.0, 1.0, (vxTex01_26.x < 0.0));
  let cov11_40 = select(0.0, 1.0, (vxTex11_33.x < 0.0));
  let vxAllOne_6 = (((vxSame1_7 * vxSame2_23) * vxSame3_30) * ((cov00_37 * cov10_38) * (cov01_39 * cov11_40)));
  let vxSize_44 = uniforms.n_n0._vxVoxel;
  let vxOrigin_46 = uniforms.n_n0._vxGridOrigin;
  let vxSpan_45 = (vxOrigin_46 * -2.0);
  let heightT00_43 = clamp((((cy00_22 + 0.5) * vxSize_44) / vxSpan_45), 0.0, 1.0);
  let hash00_47 = hash13((cell00_17 + vec3f(0.37, 0.61, 0.83)));
  let albedo_42 = ((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, (1.0 - heightT00_43))).rgb * (1.0 + ((hash00_47 - 0.5) * (uniforms.n_n0.colorVariation * 0.6))));
  let keyRgb_48 = (uniforms.n_n0.lightColor.rgb * uniforms.n_n0.lightIntensity);
  let texHp00_56 = (((uniforms.n_n0._vxMx * (vec3f((vxUv00_20.x - 0.5), (0.5 - vxUv00_20.y), (vxTex00_19.w - 1.2))).x) + (uniforms.n_n0._vxMy * (vec3f((vxUv00_20.x - 0.5), (0.5 - vxUv00_20.y), (vxTex00_19.w - 1.2))).y)) + (uniforms.n_n0._vxMz * (vec3f((vxUv00_20.x - 0.5), (0.5 - vxUv00_20.y), (vxTex00_19.w - 1.2))).z));
  let vxRo_57 = (((uniforms.n_n0._vxMx * (vec3f((sdfUV_3.x - 0.5), (0.5 - sdfUV_3.y), -1.2)).x) + (uniforms.n_n0._vxMy * (vec3f((sdfUV_3.x - 0.5), (0.5 - sdfUV_3.y), -1.2)).y)) + (uniforms.n_n0._vxMz * (vec3f((sdfUV_3.x - 0.5), (0.5 - sdfUV_3.y), -1.2)).z));
  let vxRd_58 = (((uniforms.n_n0._vxMx * (vec3f(0.0, 0.0, 1.0)).x) + (uniforms.n_n0._vxMy * (vec3f(0.0, 0.0, 1.0)).y)) + (uniforms.n_n0._vxMz * (vec3f(0.0, 0.0, 1.0)).z));
  let centre00_60 = (((cell00_17 + 0.5) * vxSize_44) + vxOrigin_46);
  let vxHalf_61 = ((vxSize_44 * 0.5) * uniforms.n_n0.voxelScale);
  let tC00_59 = select(-1.0, voxelRayBox(vxRo_57, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let hp00_55 = select(texHp00_56, (vxRo_57 + (vxRd_58 * tC00_59)), (tC00_59 > -0.5));
  let l00_54 = (hp00_55 - centre00_60);
  let al00_63 = abs(l00_54);
  let isX00_62 = select(0.0, 1.0, (al00_63.x >= max(al00_63.x, max(al00_63.y, al00_63.z))));
  let isY00_64 = (select(0.0, 1.0, (al00_63.y >= max(al00_63.x, max(al00_63.y, al00_63.z)))) * (1.0 - isX00_62));
  let faceN00_53 = (sign(l00_54) * vec3f(isX00_62, isY00_64, ((1.0 - isX00_62) - isY00_64)));
  let vxREff_65 = max((uniforms.n_n0.bevel * vxHalf_61), (vxHalf_61 * 0.04));
  let tMask00_66 = (1.0 - abs(faceN00_53));
  let nG00_52 = normalize(((faceN00_53 * vxREff_65) + ((sign(l00_54) * max((al00_63 - (vxHalf_61 - vxREff_65)), vec3f(0.0, 0.0, 0.0))) * tMask00_66)));
  let nV00_51 = vec3f(dot(uniforms.n_n0._vxMx, nG00_52), dot(uniforms.n_n0._vxMy, nG00_52), dot(uniforms.n_n0._vxMz, nG00_52));
  let n00_50 = vec3f(nV00_51.x, (-(nV00_51.y)), nV00_51.z);
  let cosEl_68 = cos((uniforms.n_n0.lightElevation * 0.017453292519943295));
  let L_67 = normalize(vec3f((cos((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_68), (sin((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_68), (-(sin((uniforms.n_n0.lightElevation * 0.017453292519943295))))));
  let ndl_49 = clamp(((dot(n00_50, L_67) + 0.12) / (1.0 + 0.12)), 0.0, 1.0);
  let texHp11_76 = (((uniforms.n_n0._vxMx * (vec3f((vxUv11_34.x - 0.5), (0.5 - vxUv11_34.y), (vxTex11_33.w - 1.2))).x) + (uniforms.n_n0._vxMy * (vec3f((vxUv11_34.x - 0.5), (0.5 - vxUv11_34.y), (vxTex11_33.w - 1.2))).y)) + (uniforms.n_n0._vxMz * (vec3f((vxUv11_34.x - 0.5), (0.5 - vxUv11_34.y), (vxTex11_33.w - 1.2))).z));
  let centre11_78 = (((cell11_31 + 0.5) * vxSize_44) + vxOrigin_46);
  let tC11_77 = select(-1.0, voxelRayBox(vxRo_57, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let hp11_75 = select(texHp11_76, (vxRo_57 + (vxRd_58 * tC11_77)), (tC11_77 > -0.5));
  let l11_74 = (hp11_75 - centre11_78);
  let al11_80 = abs(l11_74);
  let isX11_79 = select(0.0, 1.0, (al11_80.x >= max(al11_80.x, max(al11_80.y, al11_80.z))));
  let isY11_81 = (select(0.0, 1.0, (al11_80.y >= max(al11_80.x, max(al11_80.y, al11_80.z)))) * (1.0 - isX11_79));
  let faceN11_73 = (sign(l11_74) * vec3f(isX11_79, isY11_81, ((1.0 - isX11_79) - isY11_81)));
  let tMask11_82 = (1.0 - abs(faceN11_73));
  let nG11_72 = normalize(((faceN11_73 * vxREff_65) + ((sign(l11_74) * max((al11_80 - (vxHalf_61 - vxREff_65)), vec3f(0.0, 0.0, 0.0))) * tMask11_82)));
  let texHp01_87 = (((uniforms.n_n0._vxMx * (vec3f((vxUv01_27.x - 0.5), (0.5 - vxUv01_27.y), (vxTex01_26.w - 1.2))).x) + (uniforms.n_n0._vxMy * (vec3f((vxUv01_27.x - 0.5), (0.5 - vxUv01_27.y), (vxTex01_26.w - 1.2))).y)) + (uniforms.n_n0._vxMz * (vec3f((vxUv01_27.x - 0.5), (0.5 - vxUv01_27.y), (vxTex01_26.w - 1.2))).z));
  let centre01_89 = (((cell01_24 + 0.5) * vxSize_44) + vxOrigin_46);
  let tC01_88 = select(-1.0, voxelRayBox(vxRo_57, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let hp01_86 = select(texHp01_87, (vxRo_57 + (vxRd_58 * tC01_88)), (tC01_88 > -0.5));
  let l01_85 = (hp01_86 - centre01_89);
  let al01_91 = abs(l01_85);
  let isX01_90 = select(0.0, 1.0, (al01_91.x >= max(al01_91.x, max(al01_91.y, al01_91.z))));
  let isY01_92 = (select(0.0, 1.0, (al01_91.y >= max(al01_91.x, max(al01_91.y, al01_91.z)))) * (1.0 - isX01_90));
  let faceN01_84 = (sign(l01_85) * vec3f(isX01_90, isY01_92, ((1.0 - isX01_90) - isY01_92)));
  let tMask01_93 = (1.0 - abs(faceN01_84));
  let nG01_83 = normalize(((faceN01_84 * vxREff_65) + ((sign(l01_85) * max((al01_91 - (vxHalf_61 - vxREff_65)), vec3f(0.0, 0.0, 0.0))) * tMask01_93)));
  let tSort01_94 = select(1000000000.0, tC01_88, (tC01_88 > -0.5));
  let tSort11_95 = select(1000000000.0, tC11_77, (tC11_77 > -0.5));
  let texHp10_100 = (((uniforms.n_n0._vxMx * (vec3f((vxUv10_11.x - 0.5), (0.5 - vxUv10_11.y), (vxTex10_10.w - 1.2))).x) + (uniforms.n_n0._vxMy * (vec3f((vxUv10_11.x - 0.5), (0.5 - vxUv10_11.y), (vxTex10_10.w - 1.2))).y)) + (uniforms.n_n0._vxMz * (vec3f((vxUv10_11.x - 0.5), (0.5 - vxUv10_11.y), (vxTex10_10.w - 1.2))).z));
  let centre10_102 = (((cell10_8 + 0.5) * vxSize_44) + vxOrigin_46);
  let tC10_101 = select(-1.0, voxelRayBox(vxRo_57, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let hp10_99 = select(texHp10_100, (vxRo_57 + (vxRd_58 * tC10_101)), (tC10_101 > -0.5));
  let l10_98 = (hp10_99 - centre10_102);
  let al10_104 = abs(l10_98);
  let isX10_103 = select(0.0, 1.0, (al10_104.x >= max(al10_104.x, max(al10_104.y, al10_104.z))));
  let isY10_105 = (select(0.0, 1.0, (al10_104.y >= max(al10_104.x, max(al10_104.y, al10_104.z)))) * (1.0 - isX10_103));
  let faceN10_97 = (sign(l10_98) * vec3f(isX10_103, isY10_105, ((1.0 - isX10_103) - isY10_105)));
  let tMask10_106 = (1.0 - abs(faceN10_97));
  let nG10_96 = normalize(((faceN10_97 * vxREff_65) + ((sign(l10_98) * max((al10_104 - (vxHalf_61 - vxREff_65)), vec3f(0.0, 0.0, 0.0))) * tMask10_106)));
  let tSort00_107 = select(1000000000.0, tC00_59, (tC00_59 > -0.5));
  let tSort10_108 = select(1000000000.0, tC10_101, (tC10_101 > -0.5));
  let vxNShadow_71 = select(select(nG11_72, nG01_83, (tSort01_94 < tSort11_95)), select(nG10_96, nG00_52, (tSort00_107 < tSort10_108)), (select(tSort10_108, tSort00_107, (tSort00_107 < tSort10_108)) < select(tSort11_95, tSort01_94, (tSort01_94 < tSort11_95))));
  let vxHpShadow_109 = select(select(hp11_75, hp01_86, (tSort01_94 < tSort11_95)), select(hp10_99, hp00_55, (tSort00_107 < tSort10_108)), (select(tSort10_108, tSort00_107, (tSort00_107 < tSort10_108)) < select(tSort11_95, tSort01_94, (tSort01_94 < tSort11_95))));
  var shadow_70 = 1.0;
  if (((select(0.0, 1.0, (uniforms.n_n0.shadows > 0.001)) * select(0.0, 1.0, (dot(vxNShadow_71, uniforms.n_n0._vxL) > -0.12))) > 0.5)) {
  
  shadow_70 = voxelShadowLookup(compute_1, (vxHpShadow_109 + (vxNShadow_71 * (vxSize_44 * 0.12))), uniforms.n_n0._vxL, uniforms.n_n0._vxT1, uniforms.n_n0._vxT2, (-(vxOrigin_46)), uniforms.n_n0.shadowSoftness, vxSize_44);
}
  let lit_69 = (1.0 - (shadow_70 * uniforms.n_n0.shadows));
  let sky_111 = uniforms.n_n0.ambientColor.rgb;
  let upness_112 = clamp((-(n00_50.y)), 0.0, 1.0);
  let ao00_114 = (floor(((vxTex00_19.y - (floor((vxTex00_19.y / 65536.0)) * 65536.0)) / 256.0)) / 255.0);
  let occl_113 = (1.0 - (uniforms.n_n0.ao * (1.0 - ao00_114)));
  let ambient_110 = ((mix((sky_111 * 0.35), sky_111, upness_112) * uniforms.n_n0.ambient) * max(occl_113, 0.0));
  let viewI_116 = perspectiveViewRay(uv, (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y), 0.6);
  let H_115 = normalize((L_67 - viewI_116));
  let shininess_117 = exp2(mix(3.0, 7.5, uniforms.n_n0.glossiness));
  let grazing_118 = (1.0 - clamp((-(dot(n00_50, viewI_116))), 0.0, 1.0));
  let edge00_119 = clamp((max(max(((al00_63 * tMask00_66)).x, ((al00_63 * tMask00_66)).y), ((al00_63 * tMask00_66)).z) / vxHalf_61), 0.0, 1.0);
  let vxShade00_41 = (vec4f(clamp((((albedo_42 * ((keyRgb_48 * (ndl_49 * lit_69)) + ambient_110)) + ((keyRgb_48 * (((pow(clamp(dot(n00_50, H_115), 0.0, 1.0), shininess_117) * mix(0.2, 1.4, uniforms.n_n0.glossiness)) * uniforms.n_n0.specular) * lit_69)) + (sky_111 * (((0.02 + (pow(grazing_118, 5.0) * (uniforms.n_n0.glossiness * 0.5))) * (uniforms.n_n0.specular * 0.5)) * max(occl_113, 0.0))))) * (1.0 - ((uniforms.n_n0.seams * 0.75) * smoothstep((1.0 - (0.04 + (uniforms.n_n0.seams * 0.12))), 1.0, edge00_119)))), vec3f(0.0, 0.0, 0.0), vec3f(1.0, 1.0, 1.0)), cov00_37)).rgb;
  let depth00_122 = select(vxTex00_19.w, tC00_59, (tC00_59 > -0.5));
  let depthT00_121 = clamp(((depth00_122 - (1.2 + vxOrigin_46)) / vxSpan_45), 0.0, 1.0);
  let nV10_124 = vec3f(dot(uniforms.n_n0._vxMx, nG10_96), dot(uniforms.n_n0._vxMy, nG10_96), dot(uniforms.n_n0._vxMz, nG10_96));
  let n10_123 = vec3f(nV10_124.x, (-(nV10_124.y)), nV10_124.z);
  let ao10_125 = (floor(((vxTex10_10.y - (floor((vxTex10_10.y / 65536.0)) * 65536.0)) / 256.0)) / 255.0);
  let hash10_126 = hash13((cell10_8 + vec3f(0.37, 0.61, 0.83)));
  let heightT10_127 = clamp((((cy10_16 + 0.5) * vxSize_44) / vxSpan_45), 0.0, 1.0);
  let depth10_129 = select(vxTex10_10.w, tC10_101, (tC10_101 > -0.5));
  let depthT10_128 = clamp(((depth10_129 - (1.2 + vxOrigin_46)) / vxSpan_45), 0.0, 1.0);
  let edge10_130 = clamp((max(max(((al10_104 * tMask10_106)).x, ((al10_104 * tMask10_106)).y), ((al10_104 * tMask10_106)).z) / vxHalf_61), 0.0, 1.0);
  let nV01_132 = vec3f(dot(uniforms.n_n0._vxMx, nG01_83), dot(uniforms.n_n0._vxMy, nG01_83), dot(uniforms.n_n0._vxMz, nG01_83));
  let n01_131 = vec3f(nV01_132.x, (-(nV01_132.y)), nV01_132.z);
  let ao01_133 = (floor(((vxTex01_26.y - (floor((vxTex01_26.y / 65536.0)) * 65536.0)) / 256.0)) / 255.0);
  let hash01_134 = hash13((cell01_24 + vec3f(0.37, 0.61, 0.83)));
  let heightT01_135 = clamp((((cy01_29 + 0.5) * vxSize_44) / vxSpan_45), 0.0, 1.0);
  let depth01_137 = select(vxTex01_26.w, tC01_88, (tC01_88 > -0.5));
  let depthT01_136 = clamp(((depth01_137 - (1.2 + vxOrigin_46)) / vxSpan_45), 0.0, 1.0);
  let edge01_138 = clamp((max(max(((al01_91 * tMask01_93)).x, ((al01_91 * tMask01_93)).y), ((al01_91 * tMask01_93)).z) / vxHalf_61), 0.0, 1.0);
  let nV11_140 = vec3f(dot(uniforms.n_n0._vxMx, nG11_72), dot(uniforms.n_n0._vxMy, nG11_72), dot(uniforms.n_n0._vxMz, nG11_72));
  let n11_139 = vec3f(nV11_140.x, (-(nV11_140.y)), nV11_140.z);
  let ao11_141 = (floor(((vxTex11_33.y - (floor((vxTex11_33.y / 65536.0)) * 65536.0)) / 256.0)) / 255.0);
  let hash11_142 = hash13((cell11_31 + vec3f(0.37, 0.61, 0.83)));
  let heightT11_143 = clamp((((cy11_36 + 0.5) * vxSize_44) / vxSpan_45), 0.0, 1.0);
  let depth11_145 = select(vxTex11_33.w, tC11_77, (tC11_77 > -0.5));
  let depthT11_144 = clamp(((depth11_145 - (1.2 + vxOrigin_46)) / vxSpan_45), 0.0, 1.0);
  let edge11_146 = clamp((max(max(((al11_80 * tMask11_82)).x, ((al11_80 * tMask11_82)).y), ((al11_80 * tMask11_82)).z) / vxHalf_61), 0.0, 1.0);
  var vxShade10_120 = vxShade00_41;
  if ((vxSame1_7 < 0.5)) {
  let albedo_147 = ((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, (1.0 - heightT10_127))).rgb * (1.0 + ((hash10_126 - 0.5) * (uniforms.n_n0.colorVariation * 0.6))));
  let keyRgb_148 = (uniforms.n_n0.lightColor.rgb * uniforms.n_n0.lightIntensity);
  let cosEl_151 = cos((uniforms.n_n0.lightElevation * 0.017453292519943295));
  let L_150 = normalize(vec3f((cos((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_151), (sin((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_151), (-(sin((uniforms.n_n0.lightElevation * 0.017453292519943295))))));
  let ndl_149 = clamp(((dot(n10_123, L_150) + 0.12) / (1.0 + 0.12)), 0.0, 1.0);
  let lit_152 = (1.0 - (shadow_70 * uniforms.n_n0.shadows));
  let sky_154 = uniforms.n_n0.ambientColor.rgb;
  let upness_155 = clamp((-(n10_123.y)), 0.0, 1.0);
  let occl_156 = (1.0 - (uniforms.n_n0.ao * (1.0 - ao10_125)));
  let ambient_153 = ((mix((sky_154 * 0.35), sky_154, upness_155) * uniforms.n_n0.ambient) * max(occl_156, 0.0));
  let H_157 = normalize((L_150 - viewI_116));
  let shininess_158 = exp2(mix(3.0, 7.5, uniforms.n_n0.glossiness));
  let grazing_159 = (1.0 - clamp((-(dot(n10_123, viewI_116))), 0.0, 1.0));
  vxShade10_120 = (vec4f(clamp((((albedo_147 * ((keyRgb_148 * (ndl_149 * lit_152)) + ambient_153)) + ((keyRgb_148 * (((pow(clamp(dot(n10_123, H_157), 0.0, 1.0), shininess_158) * mix(0.2, 1.4, uniforms.n_n0.glossiness)) * uniforms.n_n0.specular) * lit_152)) + (sky_154 * (((0.02 + (pow(grazing_159, 5.0) * (uniforms.n_n0.glossiness * 0.5))) * (uniforms.n_n0.specular * 0.5)) * max(occl_156, 0.0))))) * (1.0 - ((uniforms.n_n0.seams * 0.75) * smoothstep((1.0 - (0.04 + (uniforms.n_n0.seams * 0.12))), 1.0, edge10_130)))), vec3f(0.0, 0.0, 0.0), vec3f(1.0, 1.0, 1.0)), cov10_38)).rgb;
}
  var vxShade01_160 = vxShade00_41;
  if ((vxSame2_23 < 0.5)) {
  let albedo_161 = ((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, (1.0 - heightT01_135))).rgb * (1.0 + ((hash01_134 - 0.5) * (uniforms.n_n0.colorVariation * 0.6))));
  let keyRgb_162 = (uniforms.n_n0.lightColor.rgb * uniforms.n_n0.lightIntensity);
  let cosEl_165 = cos((uniforms.n_n0.lightElevation * 0.017453292519943295));
  let L_164 = normalize(vec3f((cos((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_165), (sin((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_165), (-(sin((uniforms.n_n0.lightElevation * 0.017453292519943295))))));
  let ndl_163 = clamp(((dot(n01_131, L_164) + 0.12) / (1.0 + 0.12)), 0.0, 1.0);
  let lit_166 = (1.0 - (shadow_70 * uniforms.n_n0.shadows));
  let sky_168 = uniforms.n_n0.ambientColor.rgb;
  let upness_169 = clamp((-(n01_131.y)), 0.0, 1.0);
  let occl_170 = (1.0 - (uniforms.n_n0.ao * (1.0 - ao01_133)));
  let ambient_167 = ((mix((sky_168 * 0.35), sky_168, upness_169) * uniforms.n_n0.ambient) * max(occl_170, 0.0));
  let H_171 = normalize((L_164 - viewI_116));
  let shininess_172 = exp2(mix(3.0, 7.5, uniforms.n_n0.glossiness));
  let grazing_173 = (1.0 - clamp((-(dot(n01_131, viewI_116))), 0.0, 1.0));
  vxShade01_160 = (vec4f(clamp((((albedo_161 * ((keyRgb_162 * (ndl_163 * lit_166)) + ambient_167)) + ((keyRgb_162 * (((pow(clamp(dot(n01_131, H_171), 0.0, 1.0), shininess_172) * mix(0.2, 1.4, uniforms.n_n0.glossiness)) * uniforms.n_n0.specular) * lit_166)) + (sky_168 * (((0.02 + (pow(grazing_173, 5.0) * (uniforms.n_n0.glossiness * 0.5))) * (uniforms.n_n0.specular * 0.5)) * max(occl_170, 0.0))))) * (1.0 - ((uniforms.n_n0.seams * 0.75) * smoothstep((1.0 - (0.04 + (uniforms.n_n0.seams * 0.12))), 1.0, edge01_138)))), vec3f(0.0, 0.0, 0.0), vec3f(1.0, 1.0, 1.0)), cov01_39)).rgb;
}
  var vxShade11_174 = vxShade00_41;
  if ((vxSame3_30 < 0.5)) {
  let albedo_175 = ((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, (1.0 - heightT11_143))).rgb * (1.0 + ((hash11_142 - 0.5) * (uniforms.n_n0.colorVariation * 0.6))));
  let keyRgb_176 = (uniforms.n_n0.lightColor.rgb * uniforms.n_n0.lightIntensity);
  let cosEl_179 = cos((uniforms.n_n0.lightElevation * 0.017453292519943295));
  let L_178 = normalize(vec3f((cos((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_179), (sin((uniforms.n_n0.lightAngle * 0.017453292519943295)) * cosEl_179), (-(sin((uniforms.n_n0.lightElevation * 0.017453292519943295))))));
  let ndl_177 = clamp(((dot(n11_139, L_178) + 0.12) / (1.0 + 0.12)), 0.0, 1.0);
  let lit_180 = (1.0 - (shadow_70 * uniforms.n_n0.shadows));
  let sky_182 = uniforms.n_n0.ambientColor.rgb;
  let upness_183 = clamp((-(n11_139.y)), 0.0, 1.0);
  let occl_184 = (1.0 - (uniforms.n_n0.ao * (1.0 - ao11_141)));
  let ambient_181 = ((mix((sky_182 * 0.35), sky_182, upness_183) * uniforms.n_n0.ambient) * max(occl_184, 0.0));
  let H_185 = normalize((L_178 - viewI_116));
  let shininess_186 = exp2(mix(3.0, 7.5, uniforms.n_n0.glossiness));
  let grazing_187 = (1.0 - clamp((-(dot(n11_139, viewI_116))), 0.0, 1.0));
  vxShade11_174 = (vec4f(clamp((((albedo_175 * ((keyRgb_176 * (ndl_177 * lit_180)) + ambient_181)) + ((keyRgb_176 * (((pow(clamp(dot(n11_139, H_185), 0.0, 1.0), shininess_186) * mix(0.2, 1.4, uniforms.n_n0.glossiness)) * uniforms.n_n0.specular) * lit_180)) + (sky_182 * (((0.02 + (pow(grazing_187, 5.0) * (uniforms.n_n0.glossiness * 0.5))) * (uniforms.n_n0.specular * 0.5)) * max(occl_184, 0.0))))) * (1.0 - ((uniforms.n_n0.seams * 0.75) * smoothstep((1.0 - (0.04 + (uniforms.n_n0.seams * 0.12))), 1.0, edge11_146)))), vec3f(0.0, 0.0, 0.0), vec3f(1.0, 1.0, 1.0)), cov11_40)).rgb;
}
  var vxOut_5 = vec4f(vxShade00_41, 1.0);
  if ((vxAllOne_6 < 0.5)) {
  let vxPx_192 = (pxH_4 / max(uniforms.n_n0.scale, 0.001));
  let vxEx_191 = (((uniforms.n_n0._vxMx * (vec3f(vxPx_192, 0.0, 0.0)).x) + (uniforms.n_n0._vxMy * (vec3f(vxPx_192, 0.0, 0.0)).y)) + (uniforms.n_n0._vxMz * (vec3f(vxPx_192, 0.0, 0.0)).z));
  let vxEy_193 = (((uniforms.n_n0._vxMx * (vec3f(0.0, (-(vxPx_192)), 0.0)).x) + (uniforms.n_n0._vxMy * (vec3f(0.0, (-(vxPx_192)), 0.0)).y)) + (uniforms.n_n0._vxMz * (vec3f(0.0, (-(vxPx_192)), 0.0)).z));
  let vxRo0_190 = ((vxRo_57 + (vxEx_191 * 0.0625)) + (vxEy_193 * -0.1875));
  let vxTr0_01_189 = select(-1.0, voxelRayBox(vxRo0_190, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT0_01_188 = select(1000000000.0, vxTr0_01_189, (vxTr0_01_189 > -0.5));
  let vxTr0_11_195 = select(-1.0, voxelRayBox(vxRo0_190, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT0_11_194 = select(1000000000.0, vxTr0_11_195, (vxTr0_11_195 > -0.5));
  let vxTr0_10_197 = select(-1.0, voxelRayBox(vxRo0_190, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT0_10_196 = select(1000000000.0, vxTr0_10_197, (vxTr0_10_197 > -0.5));
  let vxTr0_00_199 = select(-1.0, voxelRayBox(vxRo0_190, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT0_00_198 = select(1000000000.0, vxTr0_00_199, (vxTr0_00_199 > -0.5));
  let vxHit0_200 = select(0.0, 1.0, (min(vxT0_00_198, min(vxT0_10_196, min(vxT0_01_188, vxT0_11_194))) < 500000000.0));
  let vxRo1_203 = ((vxRo_57 + (vxEx_191 * -0.0625)) + (vxEy_193 * 0.1875));
  let vxTr1_01_202 = select(-1.0, voxelRayBox(vxRo1_203, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT1_01_201 = select(1000000000.0, vxTr1_01_202, (vxTr1_01_202 > -0.5));
  let vxTr1_11_205 = select(-1.0, voxelRayBox(vxRo1_203, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT1_11_204 = select(1000000000.0, vxTr1_11_205, (vxTr1_11_205 > -0.5));
  let vxTr1_10_207 = select(-1.0, voxelRayBox(vxRo1_203, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT1_10_206 = select(1000000000.0, vxTr1_10_207, (vxTr1_10_207 > -0.5));
  let vxTr1_00_209 = select(-1.0, voxelRayBox(vxRo1_203, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT1_00_208 = select(1000000000.0, vxTr1_00_209, (vxTr1_00_209 > -0.5));
  let vxHit1_210 = select(0.0, 1.0, (min(vxT1_00_208, min(vxT1_10_206, min(vxT1_01_201, vxT1_11_204))) < 500000000.0));
  let vxRo2_213 = ((vxRo_57 + (vxEx_191 * 0.3125)) + (vxEy_193 * 0.0625));
  let vxTr2_01_212 = select(-1.0, voxelRayBox(vxRo2_213, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT2_01_211 = select(1000000000.0, vxTr2_01_212, (vxTr2_01_212 > -0.5));
  let vxTr2_11_215 = select(-1.0, voxelRayBox(vxRo2_213, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT2_11_214 = select(1000000000.0, vxTr2_11_215, (vxTr2_11_215 > -0.5));
  let vxTr2_10_217 = select(-1.0, voxelRayBox(vxRo2_213, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT2_10_216 = select(1000000000.0, vxTr2_10_217, (vxTr2_10_217 > -0.5));
  let vxTr2_00_219 = select(-1.0, voxelRayBox(vxRo2_213, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT2_00_218 = select(1000000000.0, vxTr2_00_219, (vxTr2_00_219 > -0.5));
  let vxHit2_220 = select(0.0, 1.0, (min(vxT2_00_218, min(vxT2_10_216, min(vxT2_01_211, vxT2_11_214))) < 500000000.0));
  let vxRo3_223 = ((vxRo_57 + (vxEx_191 * -0.1875)) + (vxEy_193 * -0.3125));
  let vxTr3_01_222 = select(-1.0, voxelRayBox(vxRo3_223, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT3_01_221 = select(1000000000.0, vxTr3_01_222, (vxTr3_01_222 > -0.5));
  let vxTr3_11_225 = select(-1.0, voxelRayBox(vxRo3_223, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT3_11_224 = select(1000000000.0, vxTr3_11_225, (vxTr3_11_225 > -0.5));
  let vxTr3_10_227 = select(-1.0, voxelRayBox(vxRo3_223, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT3_10_226 = select(1000000000.0, vxTr3_10_227, (vxTr3_10_227 > -0.5));
  let vxTr3_00_229 = select(-1.0, voxelRayBox(vxRo3_223, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT3_00_228 = select(1000000000.0, vxTr3_00_229, (vxTr3_00_229 > -0.5));
  let vxHit3_230 = select(0.0, 1.0, (min(vxT3_00_228, min(vxT3_10_226, min(vxT3_01_221, vxT3_11_224))) < 500000000.0));
  let vxRo4_233 = ((vxRo_57 + (vxEx_191 * -0.3125)) + (vxEy_193 * 0.3125));
  let vxTr4_01_232 = select(-1.0, voxelRayBox(vxRo4_233, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT4_01_231 = select(1000000000.0, vxTr4_01_232, (vxTr4_01_232 > -0.5));
  let vxTr4_11_235 = select(-1.0, voxelRayBox(vxRo4_233, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT4_11_234 = select(1000000000.0, vxTr4_11_235, (vxTr4_11_235 > -0.5));
  let vxTr4_10_237 = select(-1.0, voxelRayBox(vxRo4_233, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT4_10_236 = select(1000000000.0, vxTr4_10_237, (vxTr4_10_237 > -0.5));
  let vxTr4_00_239 = select(-1.0, voxelRayBox(vxRo4_233, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT4_00_238 = select(1000000000.0, vxTr4_00_239, (vxTr4_00_239 > -0.5));
  let vxHit4_240 = select(0.0, 1.0, (min(vxT4_00_238, min(vxT4_10_236, min(vxT4_01_231, vxT4_11_234))) < 500000000.0));
  let vxRo5_243 = ((vxRo_57 + (vxEx_191 * -0.4375)) + (vxEy_193 * -0.0625));
  let vxTr5_01_242 = select(-1.0, voxelRayBox(vxRo5_243, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT5_01_241 = select(1000000000.0, vxTr5_01_242, (vxTr5_01_242 > -0.5));
  let vxTr5_11_245 = select(-1.0, voxelRayBox(vxRo5_243, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT5_11_244 = select(1000000000.0, vxTr5_11_245, (vxTr5_11_245 > -0.5));
  let vxTr5_10_247 = select(-1.0, voxelRayBox(vxRo5_243, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT5_10_246 = select(1000000000.0, vxTr5_10_247, (vxTr5_10_247 > -0.5));
  let vxTr5_00_249 = select(-1.0, voxelRayBox(vxRo5_243, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT5_00_248 = select(1000000000.0, vxTr5_00_249, (vxTr5_00_249 > -0.5));
  let vxHit5_250 = select(0.0, 1.0, (min(vxT5_00_248, min(vxT5_10_246, min(vxT5_01_241, vxT5_11_244))) < 500000000.0));
  let vxRo6_253 = ((vxRo_57 + (vxEx_191 * 0.1875)) + (vxEy_193 * 0.4375));
  let vxTr6_01_252 = select(-1.0, voxelRayBox(vxRo6_253, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT6_01_251 = select(1000000000.0, vxTr6_01_252, (vxTr6_01_252 > -0.5));
  let vxTr6_11_255 = select(-1.0, voxelRayBox(vxRo6_253, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT6_11_254 = select(1000000000.0, vxTr6_11_255, (vxTr6_11_255 > -0.5));
  let vxTr6_10_257 = select(-1.0, voxelRayBox(vxRo6_253, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT6_10_256 = select(1000000000.0, vxTr6_10_257, (vxTr6_10_257 > -0.5));
  let vxTr6_00_259 = select(-1.0, voxelRayBox(vxRo6_253, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT6_00_258 = select(1000000000.0, vxTr6_00_259, (vxTr6_00_259 > -0.5));
  let vxHit6_260 = select(0.0, 1.0, (min(vxT6_00_258, min(vxT6_10_256, min(vxT6_01_251, vxT6_11_254))) < 500000000.0));
  let vxRo7_263 = ((vxRo_57 + (vxEx_191 * 0.4375)) + (vxEy_193 * -0.4375));
  let vxTr7_01_262 = select(-1.0, voxelRayBox(vxRo7_263, vxRd_58, centre01_89, vxHalf_61), (cov01_39 > 0.5));
  let vxT7_01_261 = select(1000000000.0, vxTr7_01_262, (vxTr7_01_262 > -0.5));
  let vxTr7_11_265 = select(-1.0, voxelRayBox(vxRo7_263, vxRd_58, centre11_78, vxHalf_61), (cov11_40 > 0.5));
  let vxT7_11_264 = select(1000000000.0, vxTr7_11_265, (vxTr7_11_265 > -0.5));
  let vxTr7_10_267 = select(-1.0, voxelRayBox(vxRo7_263, vxRd_58, centre10_102, vxHalf_61), (cov10_38 > 0.5));
  let vxT7_10_266 = select(1000000000.0, vxTr7_10_267, (vxTr7_10_267 > -0.5));
  let vxTr7_00_269 = select(-1.0, voxelRayBox(vxRo7_263, vxRd_58, centre00_60, vxHalf_61), (cov00_37 > 0.5));
  let vxT7_00_268 = select(1000000000.0, vxTr7_00_269, (vxTr7_00_269 > -0.5));
  let vxHit7_270 = select(0.0, 1.0, (min(vxT7_00_268, min(vxT7_10_266, min(vxT7_01_261, vxT7_11_264))) < 500000000.0));
  vxOut_5 = vec4f((((((((((vec3f(0.0, 0.0, 0.0) + (select(select(select(vxShade11_174, vxShade01_160, (vxT0_01_188 < vxT0_11_194)), vxShade10_120, (vxT0_10_196 < min(vxT0_01_188, vxT0_11_194))), vxShade00_41, (vxT0_00_198 < min(vxT0_10_196, min(vxT0_01_188, vxT0_11_194)))) * vxHit0_200)) + (select(select(select(vxShade11_174, vxShade01_160, (vxT1_01_201 < vxT1_11_204)), vxShade10_120, (vxT1_10_206 < min(vxT1_01_201, vxT1_11_204))), vxShade00_41, (vxT1_00_208 < min(vxT1_10_206, min(vxT1_01_201, vxT1_11_204)))) * vxHit1_210)) + (select(select(select(vxShade11_174, vxShade01_160, (vxT2_01_211 < vxT2_11_214)), vxShade10_120, (vxT2_10_216 < min(vxT2_01_211, vxT2_11_214))), vxShade00_41, (vxT2_00_218 < min(vxT2_10_216, min(vxT2_01_211, vxT2_11_214)))) * vxHit2_220)) + (select(select(select(vxShade11_174, vxShade01_160, (vxT3_01_221 < vxT3_11_224)), vxShade10_120, (vxT3_10_226 < min(vxT3_01_221, vxT3_11_224))), vxShade00_41, (vxT3_00_228 < min(vxT3_10_226, min(vxT3_01_221, vxT3_11_224)))) * vxHit3_230)) + (select(select(select(vxShade11_174, vxShade01_160, (vxT4_01_231 < vxT4_11_234)), vxShade10_120, (vxT4_10_236 < min(vxT4_01_231, vxT4_11_234))), vxShade00_41, (vxT4_00_238 < min(vxT4_10_236, min(vxT4_01_231, vxT4_11_234)))) * vxHit4_240)) + (select(select(select(vxShade11_174, vxShade01_160, (vxT5_01_241 < vxT5_11_244)), vxShade10_120, (vxT5_10_246 < min(vxT5_01_241, vxT5_11_244))), vxShade00_41, (vxT5_00_248 < min(vxT5_10_246, min(vxT5_01_241, vxT5_11_244)))) * vxHit5_250)) + (select(select(select(vxShade11_174, vxShade01_160, (vxT6_01_251 < vxT6_11_254)), vxShade10_120, (vxT6_10_256 < min(vxT6_01_251, vxT6_11_254))), vxShade00_41, (vxT6_00_258 < min(vxT6_10_256, min(vxT6_01_251, vxT6_11_254)))) * vxHit6_260)) + (select(select(select(vxShade11_174, vxShade01_160, (vxT7_01_261 < vxT7_11_264)), vxShade10_120, (vxT7_10_266 < min(vxT7_01_261, vxT7_11_264))), vxShade00_41, (vxT7_00_268 < min(vxT7_10_266, min(vxT7_01_261, vxT7_11_264)))) * vxHit7_270)) / max(((((((((0.0 + vxHit0_200) + vxHit1_210) + vxHit2_220) + vxHit3_230) + vxHit4_240) + vxHit5_250) + vxHit6_260) + vxHit7_270), 0.0001)), (((((((((0.0 + vxHit0_200) + vxHit1_210) + vxHit2_220) + vxHit3_230) + vxHit4_240) + vxHit5_250) + vxHit6_260) + vxHit7_270) / 8.0));
}
  voxels_0 = vxOut_5;
}
  var composed_271 = voxels_0;
  return vec4f(linearToSrgb(tone_linear(composed_271.rgb)) * composed_271.a, composed_271.a);
}