struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  feed: f32,
  kill: f32,
  diffusionRatio: f32,
  featureSize: f32,
  speed: f32,
  brushSize: f32,
  brushStrength: f32,
  colorA: vec4f,
  colorC: vec4f,
  colorB: vec4f,
  contrast: f32,
  threshold: f32,
  colorSpace: f32,
  relief: f32,
  lightAngle: f32,
  childInfluence: f32,
  childContrast: f32,
  childThreshold: f32,
  childInvert: f32,
  _opacity: f32,
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

@group(2) @binding(0) var linearClamp: sampler;

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

fn oklabToOklch(lab: vec3f) -> vec3f {
  let L = lab.x;
  let a = lab.y;
  let b = lab.z;
  let C = sqrt(((a * a) + (b * b)));
  let h = atan2(b, a);
  return vec3f(L, C, h);
}

fn awMix(a: vec3f, b: vec3f, wA: f32, wB: f32, sw: f32) -> vec3f {
  return (((a * wA) + (b * wB)) / sw);
}

fn oklchToOklab(lch: vec3f) -> vec3f {
  let L = lch.x;
  let C = lch.y;
  let h = lch.z;
  return vec3f(L, (C * cos(h)), (C * sin(h)));
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
  let oklchA = oklabToOklch(rgbToOklab(p3ToSRGB(colorA.xyz)));
  let oklchB = oklabToOklch(rgbToOklab(p3ToSRGB(colorB.xyz)));
  let mixed = awMix(oklchA, oklchB, weightA, weightB, safeWeight);
  let rgb = sRGBToP3(oklabToRgb(oklchToOklab(mixed)));
  return vec4f(rgb, totalWeight);
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
  let rampW_2 = max((0.6 - (uniforms.n_n0.contrast * 0.55)), 0.02);
  let view_4 = (1.0 / uniforms.n_n0.featureSize);
  let sampleUV_3 = vec2f(((((uv.x - 0.5) * min(uniforms._sys.aspect, 1.0)) * view_4) + 0.5), ((((uv.y - 0.5) * min((1.0 / uniforms._sys.aspect), 1.0)) * view_4) + 0.5));
  let rampT_1 = smoothstep((uniforms.n_n0.threshold - rampW_2), (uniforms.n_n0.threshold + rampW_2), (textureSample(compute_0, linearClamp, sampleUV_3)).y);
  let rampColor_0 = select(mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorC, clamp((rampT_1 * 2.0), 0.0, 1.0)), mixColors(uniforms.n_n0.colorC, uniforms.n_n0.colorB, clamp(((rampT_1 * 2.0) - 1.0), 0.0, 1.0)), (rampT_1 > 0.5));
  let reliefN_5 = normalize(vec3f((-((((textureSample(compute_0, linearClamp, (sampleUV_3 + vec2f(0.0029296875, 0.0)))).y - (textureSample(compute_0, linearClamp, (sampleUV_3 + vec2f(-0.0029296875, 0.0)))).y) * 6.0))), (-((((textureSample(compute_0, linearClamp, (sampleUV_3 + vec2f(0.0, 0.0029296875)))).y - (textureSample(compute_0, linearClamp, (sampleUV_3 + vec2f(0.0, -0.0029296875)))).y) * 6.0))), 1.0));
  let lightAng_7 = (uniforms.n_n0.lightAngle * 0.017453292519943295);
  let lightDir_6 = normalize(vec3f((cos(lightAng_7) * 0.85), (sin(lightAng_7) * 0.85), 0.55));
  var composed_8 = vec4f(mix(rampColor_0.rgb, ((rampColor_0.rgb * (0.42 + (0.5800000000000001 * clamp(dot(reliefN_5, lightDir_6), 0.0, 1.0)))) + vec3f((pow(clamp(dot(reliefN_5, normalize((lightDir_6 + vec3f(0.0, 0.0, 1.0)))), 0.0, 1.0), 18.0) * 0.35))), uniforms.n_n0.relief), rampColor_0.a);
  return vec4f(linearToSrgb(tone_linear(composed_8.rgb)) * composed_8.a, composed_8.a);
}