struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  colorSpeed: f32,
  color: vec4f,
  color1: vec4f,
  color2: vec4f,
  color3: vec4f,
  colorSpace: f32,
  radius: f32,
  force: f32,
  curl: f32,
  decay: f32,
  momentum: f32,
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
  _ = uniforms._sys.time;
  var uv = in.uv;
  let inkDye_0 = (textureSample(compute_0, linearClamp, uv)).rgb;
  var composed_1 = vec4f(inkDye_0, clamp((max(inkDye_0.x, max(inkDye_0.y, inkDye_0.z)) * 1.35), 0.0, 1.0));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}