fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

fn channelBlurCompose(sharp: vec4f, blurred: vec4f, redIntensity: f32, greenIntensity: f32, blueIntensity: f32) -> vec4f {
  let redRadius = (redIntensity * 0.1f);
  let greenRadius = (greenIntensity * 0.1f);
  let blueRadius = (blueIntensity * 0.1f);
  let maxRadius = max(max(max(redRadius, greenRadius), blueRadius), 0.01f);
  let rMix = (redRadius / maxRadius);
  let gMix = (greenRadius / maxRadius);
  let bMix = (blueRadius / maxRadius);
  let finalR = mix(sharp.x, blurred.x, rMix);
  let finalG = mix(sharp.y, blurred.y, gMix);
  let finalB = mix(sharp.z, blurred.z, bMix);
  return vec4f(finalR, finalG, finalB, sharp.w);
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

@group(1) @binding(1) var compute_0: texture_2d<f32>;

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  redIntensity: f32,
  greenIntensity: f32,
  blueIntensity: f32,
  _opacity: f32,
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
  var composed_0 = unpremultiplyAlpha(channelBlurCompose(textureSample(rtt_0, linearClamp, uv), textureSample(compute_0, linearClamp, uv), uniforms.n_n0.redIntensity, uniforms.n_n0.greenIntensity, uniforms.n_n0.blueIntensity));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}