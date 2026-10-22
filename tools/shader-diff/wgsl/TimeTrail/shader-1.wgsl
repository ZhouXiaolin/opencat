@group(1) @binding(0) var compute_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

fn overComposite(base: vec4f, blended: vec3f, overlayAlpha: f32) -> vec4f {
  let baseWeight = (base.w * (1f - overlayAlpha));
  let finalAlpha = (overlayAlpha + baseWeight);
  let rgb = ((blended * overlayAlpha) + (base.xyz * baseWeight));
  return vec4f(rgb, finalAlpha);
}

fn blend_normal(base: vec4f, overlay2: vec4f, opacity: f32) -> vec4f {
  let overlayAlpha = (overlay2.w * opacity);
  return overComposite(base, overlay2.xyz, overlayAlpha);
}

@group(1) @binding(1) var rtt_0: texture_2d<f32>;

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  trailSource: f32,
  motionThreshold: f32,
  trailLength: f32,
  diffusion: f32,
  trailOpacity: f32,
  driftX: f32,
  driftY: f32,
  zoom: f32,
  tintMode: f32,
  tint: vec4f,
  speed: f32,
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
  let trail_0 = textureSample(compute_0, linearClamp, uv);
  var composed_1 = unpremultiplyAlpha(blend_normal(unpremultiplyAlpha(textureSample(rtt_0, linearClamp, uv)), unpremultiplyAlpha(trail_0), uniforms.n_n0.trailOpacity));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}