fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn pixelateSample(uv: vec2f, aspect: f32, scale: f32, gap: f32, roundness: f32) -> vec3f {
  let isWide = (aspect > 1f);
  let pixelCountX = max(select((scale * aspect), scale, isWide), 1f);
  let pixelCountY = max(select(scale, (scale / aspect), isWide), 1f);
  let sampleUV = vec2f((floor((uv.x * pixelCountX)) / pixelCountX), (floor((uv.y * pixelCountY)) / pixelCountY));
  let localUV = vec2f((fract((uv.x * pixelCountX)) - 0.5f), (fract((uv.y * pixelCountY)) - 0.5f));
  let halfSize = (0.5f - (gap * 0.5f));
  let cornerRadius = (roundness * halfSize);
  let qx = max(((abs(localUV.x) - halfSize) + cornerRadius), 0f);
  let qy = max(((abs(localUV.y) - halfSize) + cornerRadius), 0f);
  let sdf = (length(vec2f(qx, qy)) - cornerRadius);
  let mask = select(0f, 1f, (sdf <= 0f));
  return vec3f(sampleUV.x, sampleUV.y, mask);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  scale: f32,
  gap: f32,
  roundness: f32,
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
  var composed_0 = vec4f((unpremultiplyAlpha(textureSample(rtt_0, linearClamp, (pixelateSample(uv, uniforms._sys.aspect, uniforms.n_n0.scale, uniforms.n_n0.gap, uniforms.n_n0.roundness)).xy))).rgb, ((unpremultiplyAlpha(textureSample(rtt_0, linearClamp, (pixelateSample(uv, uniforms._sys.aspect, uniforms.n_n0.scale, uniforms.n_n0.gap, uniforms.n_n0.roundness)).xy))).a * (pixelateSample(uv, uniforms._sys.aspect, uniforms.n_n0.scale, uniforms.n_n0.gap, uniforms.n_n0.roundness)).z));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}