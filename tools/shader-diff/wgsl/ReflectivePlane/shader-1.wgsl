fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

fn planarReflectionCompose(original: vec4f, refl: vec4f, uvY: f32, lineY: f32, distance_1: f32, falloff: f32) -> vec4f {
  let distBelow = max((uvY - lineY), 0f);
  let distanceMax = max(distance_1, 1e-3f);
  let fadeWidth = max((distanceMax * falloff), 1e-4f);
  let fadeStart = (distanceMax - fadeWidth);
  let visibility = (1f - smoothstep(fadeStart, distanceMax, distBelow));
  let effReflA = (refl.w * visibility);
  let oneMinusReflA = (1f - effReflA);
  let belowRgb = ((refl.xyz * visibility) + (original.xyz * oneMinusReflA));
  let belowA = (effReflA + (original.w * oneMinusReflA));
  let belowLine = vec4f(belowRgb, belowA);
  let isBelowLine = step(lineY, uvY);
  return mix(original, belowLine, vec4f(isBelowLine));
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

@group(1) @binding(1) var compute_0: texture_2d<f32>;

fn mirrorRowUV(uv: vec2f, lineY: f32) -> vec2f {
  return vec2f(uv.x, ((lineY * 2f) - uv.y));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct padded_1 {
  height: f32,
  distance: f32,
  falloff: f32,
  blur: f32,
  blurDistance: f32,
  edges: f32,
  _opacity: f32,
  _pad0: f32,
}

struct struct_1 {
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
  n_n0: padded_1,
  n_n0c0: struct_1,
  _sys: struct_2,
}

@group(0) @binding(0) var<uniform> uniforms: combined;

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
  var composed_0 = unpremultiplyAlpha(planarReflectionCompose(textureSample(rtt_0, linearClamp, uv), textureSample(compute_0, linearClamp, mirrorRowUV(uv, uniforms.n_n0.height)), uv.y, uniforms.n_n0.height, uniforms.n_n0.distance, uniforms.n_n0.falloff));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}