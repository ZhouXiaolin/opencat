fn ditherCoord(uv: vec2f, gridRes: vec2f, pixelSize: f32) -> vec2f {
  return floor(vec2f(((uv.x * gridRes.x) / pixelSize), ((uv.y * gridRes.y) / pixelSize)));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  pattern: f32,
  pixelSize: f32,
  threshold: f32,
  spread: f32,
  colorMode: f32,
  colorA: vec4f,
  colorB: vec4f,
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

fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn ditherPixUV(coord: vec2f, pixelSize: f32, gridRes: vec2f) -> vec2f {
  return vec2f((((coord.x + 0.5f) * pixelSize) / gridRes.x), (((coord.y + 0.5f) * pixelSize) / gridRes.y));
}

fn ditherComposeCustom(colorA: vec4f, colorB: vec4f, ditherResult: f32) -> vec4f {
  let rgb = mix(vec3f(colorA.x, colorA.y, colorA.z), vec3f(colorB.x, colorB.y, colorB.z), vec3f(ditherResult));
  let a = mix(colorA.w, colorB.w, ditherResult);
  return vec4f(rgb.x, rgb.y, rgb.z, a);
}

fn ditherOrderedResult(ditherValue: f32, luminance: f32, threshold: f32, spread: f32) -> f32 {
  let adjustedDither = (0.5f + ((ditherValue - 0.5f) * spread));
  return step(adjustedDither, (luminance + (threshold - 0.5f)));
}

fn ditherBayerQuad(a: f32, b: f32) -> f32 {
  return (((b * 3f) + (a * 2f)) - ((a * b) * 4f));
}

fn ditherPeriodic(coord: vec2f) -> vec4f {
  let tx = (coord.x - (8f * floor((coord.x / 8f))));
  let ty = (coord.y - (8f * floor((coord.y / 8f))));
  let x2 = (tx - (2f * floor((tx / 2f))));
  let y2 = (ty - (2f * floor((ty / 2f))));
  let x4 = (tx - (4f * floor((tx / 4f))));
  let y4 = (ty - (4f * floor((ty / 4f))));
  let x4h = floor((x4 / 2f));
  let y4h = floor((y4 / 2f));
  let x8h = floor((tx / 4f));
  let y8h = floor((ty / 4f));
  let b00 = ditherBayerQuad(x2, y2);
  let b11 = ditherBayerQuad(x4h, y4h);
  let b22 = ditherBayerQuad(x8h, y8h);
  let bayer2 = (b00 / 4f);
  let bayer4 = (((b00 * 4f) + b11) / 16f);
  let bayer8 = ((((b00 * 16f) + (b11 * 4f)) + b22) / 64f);
  let idx = ((i32(y4) * 4i) + i32(x4));
  var cd = 0.75;
  cd = select(cd, 0.3125f, (idx == 1i));
  cd = select(cd, 0.375f, (idx == 2i));
  cd = select(cd, 0.8125f, (idx == 3i));
  cd = select(cd, 0.25f, (idx == 4i));
  cd = select(cd, 0f, (idx == 5i));
  cd = select(cd, 0.0625f, (idx == 6i));
  cd = select(cd, 0.4375f, (idx == 7i));
  cd = select(cd, 0.6875f, (idx == 8i));
  cd = select(cd, 0.1875f, (idx == 9i));
  cd = select(cd, 0.125f, (idx == 10i));
  cd = select(cd, 0.5f, (idx == 11i));
  cd = select(cd, 0.9375f, (idx == 12i));
  cd = select(cd, 0.625f, (idx == 13i));
  cd = select(cd, 0.5625f, (idx == 14i));
  cd = select(cd, 0.875f, (idx == 15i));
  return vec4f(bayer2, bayer4, bayer8, cd);
}

fn ditherLuma(c: vec4f) -> f32 {
  return (dot(vec3f(c.x, c.y, c.z), vec3f(0.29899999499320984, 0.5870000123977661, 0.11400000005960464)) * c.w);
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
  let ditherCoord_0 = ditherCoord(uv, uniforms._sys.logicalViewportSize, uniforms.n_n0.pixelSize);
  let ditherSource_1 = unpremultiplyAlpha(textureSample(rtt_0, linearClamp, ditherPixUV(ditherCoord_0, uniforms.n_n0.pixelSize, uniforms._sys.logicalViewportSize)));
  var composed_2 = ditherComposeCustom(uniforms.n_n0.colorA, uniforms.n_n0.colorB, ditherOrderedResult((ditherPeriodic(ditherCoord_0)).y, ditherLuma(ditherSource_1), uniforms.n_n0.threshold, uniforms.n_n0.spread));
  return vec4f(linearToSrgb(tone_linear(composed_2.rgb)) * composed_2.a, composed_2.a);
}