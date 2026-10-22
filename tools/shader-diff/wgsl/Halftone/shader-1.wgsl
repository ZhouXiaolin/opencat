fn halftoneClassic(childColor: vec4f, uv: vec2f, aspect: f32, angleDeg: f32, frequency: f32) -> vec4f {
  let aspectUV = vec2f((uv.x * aspect), uv.y);
  let angleRad = (angleDeg * 0.017453292519943295f);
  let c = cos(angleRad);
  let s = sin(angleRad);
  let rotatedUV = vec2f(dot(aspectUV, vec2f(c, (s * -1f))), dot(aspectUV, vec2f(s, c)));
  let gridUV = (fract((rotatedUV * frequency)) - vec2f(0.5));
  let brightness = dot(vec3f(childColor.x, childColor.y, childColor.z), vec3f(0.29899999499320984, 0.5870000123977661, 0.11400000005960464));
  let dotSize = ((brightness * 0.7f) + 0.15f);
  let dist = length(gridUV);
  let dotPattern = (1f - smoothstep((dotSize - 0.1f), (dotSize + 0.1f), dist));
  return vec4f((childColor.x * dotPattern), (childColor.y * dotPattern), (childColor.z * dotPattern), (childColor.w * dotPattern));
}

fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  style: f32,
  frequency: f32,
  angle: f32,
  cyanAngle: f32,
  magentaAngle: f32,
  yellowAngle: f32,
  blackAngle: f32,
  misprint: f32,
  misprintAngle: f32,
  paperColor: vec4f,
  cyanColor: vec4f,
  magentaColor: vec4f,
  yellowColor: vec4f,
  blackColor: vec4f,
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
  var composed_0 = halftoneClassic(unpremultiplyAlpha(textureSample(rtt_0, linearClamp, uv)), uv, uniforms._sys.aspect, uniforms.n_n0.angle, uniforms.n_n0.frequency);
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}