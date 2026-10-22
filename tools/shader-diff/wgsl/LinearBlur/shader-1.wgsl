fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn linearBlurTapCoord(uv: vec2f, angleDeg: f32, intensity: f32, viewport: vec2f, tapT: f32) -> vec2f {
  let angleRad = (angleDeg * 0.017453292519943295f);
  let aspect = (viewport.x / viewport.y);
  let dirX = (cos(angleRad) / aspect);
  let dirY = sin(angleRad);
  let bvX = (((dirX * intensity) / viewport.x) * 2f);
  let bvY = (((dirY * intensity) / viewport.y) * 2f);
  return vec2f((uv.x + (bvX * tapT)), (uv.y + (bvY * tapT)));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct padded_1 {
  intensity: f32,
  angle: f32,
  dispersion: f32,
  falloff: f32,
  focus: f32,
  bias: f32,
  highlights: f32,
  jitter: f32,
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
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
  var composed_0 = unpremultiplyAlpha(((((((((((((((((((((((((((((((((textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.5)) * 0.01833924571930286) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.467741935483871)) * 0.02021849882901347)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.43548387096774194)) * 0.022145824255032632)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.4032258064516129)) * 0.024099625573710187)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.3709677419354839)) * 0.026055790002929013)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.33870967741935487)) * 0.027988118222713798)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.3064516129032258)) * 0.029868860752810644)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.27419354838709675)) * 0.03166934789372045)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.24193548387096775)) * 0.033360694846059885)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.2096774193548387)) * 0.03491455881584358)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.1774193548387097)) * 0.03630392101960902)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.14516129032258063)) * 0.03750386381383466)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.11290322580645162)) * 0.03849231191611351)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.08064516129032256)) * 0.039250707006906665)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.04838709677419356)) * 0.03976458694921195)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, -0.016129032258064502)) * 0.04002404438318769)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.016129032258064502)) * 0.04002404438318769)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.048387096774193505)) * 0.03976458694921195)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.08064516129032262)) * 0.039250707006906665)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.11290322580645162)) * 0.03849231191611351)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.14516129032258063)) * 0.03750386381383466)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.17741935483870963)) * 0.03630392101960902)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.20967741935483875)) * 0.03491455881584358)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.24193548387096775)) * 0.033360694846059885)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.27419354838709675)) * 0.03166934789372045)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.30645161290322576)) * 0.029868860752810648)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.33870967741935487)) * 0.027988118222713798)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.3709677419354839)) * 0.026055790002929013)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.4032258064516129)) * 0.024099625573710187)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.4354838709677419)) * 0.022145824255032632)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.467741935483871)) * 0.02021849882901347)) + (textureSample(rtt_0, linearClamp, linearBlurTapCoord(uv, uniforms.n_n0.angle, uniforms.n_n0.intensity, uniforms._sys.viewportSize, 0.5)) * 0.01833924571930286)));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}