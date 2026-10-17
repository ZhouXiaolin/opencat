fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn zoomBlurTapCoord(center: vec2f, intensity: f32, uv: vec2f, aspect: f32, tapIndex: f32) -> vec2f {
  let centerPos = vec2f(center.x, (1f - center.y));
  let radius = (intensity * 0.01f);
  let scale = (1f + (radius * (tapIndex / 31f)));
  let deltaX = (uv.x - centerPos.x);
  let deltaY = (uv.y - centerPos.y);
  let acdX = (deltaX * aspect);
  let acdY = deltaY;
  let scaledDeltaX = (acdX / scale);
  let scaledDeltaY = (acdY / scale);
  return vec2f(((scaledDeltaX / aspect) + centerPos.x), (scaledDeltaY + centerPos.y));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  intensity: f32,
  center: vec2f,
  dispersion: f32,
  falloff: f32,
  focus: f32,
  bias: f32,
  highlights: f32,
  spiral: f32,
  jitter: f32,
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
  var composed_0 = unpremultiplyAlpha(((((((((((((((((((((((((((((((((textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 0.0)) * 0.01833924571930286) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 1.0)) * 0.02021849882901347)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 2.0)) * 0.022145824255032632)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 3.0)) * 0.024099625573710187)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 4.0)) * 0.026055790002929013)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 5.0)) * 0.027988118222713798)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 6.0)) * 0.029868860752810644)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 7.0)) * 0.03166934789372045)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 8.0)) * 0.033360694846059885)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 9.0)) * 0.03491455881584358)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 10.0)) * 0.03630392101960902)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 11.0)) * 0.03750386381383466)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 12.0)) * 0.03849231191611351)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 13.0)) * 0.039250707006906665)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 14.0)) * 0.03976458694921195)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 15.0)) * 0.04002404438318769)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 16.0)) * 0.04002404438318769)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 17.0)) * 0.03976458694921195)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 18.0)) * 0.039250707006906665)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 19.0)) * 0.03849231191611351)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 20.0)) * 0.03750386381383466)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 21.0)) * 0.03630392101960902)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 22.0)) * 0.03491455881584358)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 23.0)) * 0.033360694846059885)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 24.0)) * 0.03166934789372045)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 25.0)) * 0.029868860752810648)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 26.0)) * 0.027988118222713798)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 27.0)) * 0.026055790002929013)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 28.0)) * 0.024099625573710187)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 29.0)) * 0.022145824255032632)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 30.0)) * 0.02021849882901347)) + (textureSample(rtt_0, linearClamp, zoomBlurTapCoord(uniforms.n_n0.center, uniforms.n_n0.intensity, uv, uniforms._sys.aspect, 31.0)) * 0.01833924571930286)));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}