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

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  colorA: vec4f,
  colorB: vec4f,
  center: vec2f,
  scale: f32,
  rotation: f32,
  emitFrom: vec2f,
  direction: f32,
  speed: f32,
  spread: f32,
  emitRadius: f32,
  intensity: f32,
  dissipation: f32,
  detail: f32,
  gravity: f32,
  colorDecay: f32,
  mouseInfluence: f32,
  mouseRadius: f32,
  colorSpace: f32,
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

@group(1) @binding(0) var compute_1: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

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

@group(1) @binding(1) var compute_0: texture_2d<f32>;

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
  var composed_0 = vec4f((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, (textureSample(compute_1, linearClamp, uv)).y)).rgb, (((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, (textureSample(compute_1, linearClamp, uv)).y)).a * (textureSample(compute_1, linearClamp, uv)).x) * clamp(((fieldSampleArg(sdfSpaceUV(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.rotation, uv, uniforms._sys.aspect), compute_0, uniforms.n_n0._vfOriginX, uniforms.n_n0._vfOriginY, uniforms.n_n0._vfSpanX, uniforms.n_n0._vfSpanY, uniforms.n_n0._vfActiveRes, uniforms.n_n0._vfRBound)).x * -1024.0), 0.0, 1.0)));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}