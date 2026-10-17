fn colorWheelRainbow(t: f32) -> vec4f {
  let h6 = (t * 6f);
  let r = clamp((abs((h6 - 3f)) - 1f), 0f, 1f);
  let g = clamp((2f - abs((h6 - 2f))), 0f, 1f);
  let b = clamp((2f - abs((h6 - 4f))), 0f, 1f);
  return vec4f(r, g, b, 1f);
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn canvasCentredDelta(uv: vec2f, aspect: f32) -> vec2f {
  return vec2f(((uv.x * aspect) - (aspect * 0.5f)), (uv.y - 0.5f));
}

fn directionalProjection(p: vec2f, cosA: f32, sinA: f32) -> f32 {
  return ((p.x * cosA) + (p.y * sinA));
}

fn colorWheelT(uv: vec2f, viewport: vec2f, angle: f32, scale: f32, animTime: f32) -> f32 {
  let angleRad = (angle * 0.017453292519943295f);
  let aspect = geom_aspectOf(viewport);
  let projectedCoord = (directionalProjection(canvasCentredDelta(uv, aspect), cos(angleRad), sin(angleRad)) + (aspect * 0.5f));
  return fract((((projectedCoord * scale) * 0.2f) + animTime));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  colorA: vec4f,
  colorB: vec4f,
  colorC: vec4f,
  scale: f32,
  angle: f32,
  speed: f32,
  colorSpace: f32,
  _opacity: f32,
  _animTime: f32,
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
  var composed_0 = colorWheelRainbow(colorWheelT(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.scale, uniforms.n_n0._animTime));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}