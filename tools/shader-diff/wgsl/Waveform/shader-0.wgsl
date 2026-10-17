struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  style: f32,
  colorA: vec4f,
  colorB: vec4f,
  colorsArray: array<vec4f, 8>,
  positionsArray: array<vec4f, 2>,
  convertedColorsArray: array<vec3f, 8>,
  stopCount: f32,
  colorSpace: f32,
  from_: vec2f,
  to: vec2f,
  amplitude: f32,
  frequency: f32,
  height: f32,
  align: f32,
  count: f32,
  barWidth: f32,
  rounding: f32,
  dotSize: f32,
  lineWidth: f32,
  softness: f32,
  speed: f32,
  seed: f32,
  _opacity: f32,
  _animTime: f32,
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

fn roundedRectSdf(dx: f32, dy: f32, width: f32, height: f32, rounding: f32) -> f32 {
  let qx = ((abs(dx) - width) + rounding);
  let qy = ((abs(dy) - height) + rounding);
  let mqx = max(qx, 0f);
  let mqy = max(qy, 0f);
  let outerDist = sqrt(((mqx * mqx) + (mqy * mqy)));
  let innerDist = min(max(qx, qy), 0f);
  return ((outerDist + innerDist) - rounding);
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
  let aspect_4 = (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y);
  let rel_3 = vec2f(((uv.x * aspect_4) - (uniforms.n_n0.from_.x * aspect_4)), (uv.y - (1.0 - uniforms.n_n0.from_.y)));
  let toRel_6 = vec2f(((uniforms.n_n0.to.x * aspect_4) - (uniforms.n_n0.from_.x * aspect_4)), ((1.0 - uniforms.n_n0.to.y) - (1.0 - uniforms.n_n0.from_.y)));
  let segLen_7 = max(length(toRel_6), 0.0001);
  let segDir_5 = (toRel_6 / segLen_7);
  let along_2 = dot(rel_3, segDir_5);
  let wfX_1 = (along_2 / segLen_7);
  let wfColor_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, wfX_1);
  let wfPx_8 = (1.0 / uniforms._sys.viewportSize.y);
  let wfXCells_10 = (wfX_1 * uniforms.n_n0.count);
  let wfCell_11 = (segLen_7 / uniforms.n_n0.count);
  let wfColDx_9 = ((fract(wfXCells_10) - 0.5) * wfCell_11);
  let across_13 = ((rel_3.x * (-(segDir_5.y))) + (rel_3.y * segDir_5.x));
  let wfDy_12 = across_13;
  let wfHalfW_14 = ((uniforms.n_n0.barWidth * wfCell_11) * 0.5);
  let wfColX_16 = ((floor(wfXCells_10) + 0.5) / uniforms.n_n0.count);
  let wfTime_17 = (uniforms.n_n0._animTime + uniforms.n_n0.seed);
  let wfExtent_15 = max(((clamp((((0.45 + (((((((sin((((wfColX_16 * uniforms.n_n0.frequency) * 5.1) + (wfTime_17 * 1.3))) * 0.28) + ((sin((((wfColX_16 * uniforms.n_n0.frequency) * 9.7) + (wfTime_17 * -1.9))) * sin((((wfColX_16 * uniforms.n_n0.frequency) * 1.7) + (wfTime_17 * 0.6)))) * 0.24)) + (sin((((wfColX_16 * uniforms.n_n0.frequency) * 17.3) + (wfTime_17 * 2.7))) * 0.18)) + ((sin((((wfColX_16 * uniforms.n_n0.frequency) * 31.9) + (wfTime_17 * -3.4))) * sin((((wfColX_16 * uniforms.n_n0.frequency) * 3.3) + (wfTime_17 * -0.9)))) * 0.14)) + (sin((((wfColX_16 * uniforms.n_n0.frequency) * 67.7) + (wfTime_17 * 4.1))) * 0.09)) + ((sin((((wfColX_16 * uniforms.n_n0.frequency) * 131.3) + (wfTime_17 * -5.3))) * sin((((wfColX_16 * uniforms.n_n0.frequency) * 7.9) + (wfTime_17 * 1.1)))) * 0.07)) * 0.6)) * mix(0.4, 1.0, exp((-((((wfColX_16 - 0.5) * (wfColX_16 - 0.5)) * 4.0)))))) * uniforms.n_n0.amplitude), 0.0, 1.0) * uniforms.n_n0.height) * 0.5), wfHalfW_14);
  var composed_18 = vec4f(wfColor_0.rgb, ((wfColor_0.a * (1.0 - smoothstep((-((wfPx_8 * 0.5))), (wfPx_8 * 0.5), roundedRectSdf(wfColDx_9, wfDy_12, wfHalfW_14, wfExtent_15, (uniforms.n_n0.rounding * wfHalfW_14))))) * (1.0 - smoothstep((-((wfPx_8 * 0.5))), (wfPx_8 * 0.5), max((-(along_2)), (along_2 - segLen_7))))));
  return vec4f(linearToSrgb(tone_linear(composed_18.rgb)) * composed_18.a, composed_18.a);
}