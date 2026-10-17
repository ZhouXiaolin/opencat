fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn radialFrame(uv: vec2f, viewport: vec2f, center: vec2f) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let cx = center.x;
  let cy = (1f - center.y);
  let dx = ((uv.x * aspect) - (cx * aspect));
  let dy = (uv.y - cy);
  let dist = sqrt(((dx * dx) + (dy * dy)));
  let angle = atan2(dy, dx);
  return vec2f(dist, angle);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  color: vec4f,
  background: vec4f,
  center: vec2f,
  rayCount: f32,
  softness: f32,
  radius: f32,
  feather: f32,
  speed: f32,
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

fn angularSineLobes(angle: f32, count: f32, phase: f32, sharpness: f32) -> f32 {
  let t = ((sin(((angle * count) + phase)) * 0.5f) + 0.5f);
  return pow(t, sharpness);
}

fn softRayLobes(angle: f32, count: f32, phase: f32, softness: f32) -> f32 {
  let sharpness = ((1f / (softness + 0.05f)) * 0.3f);
  return angularSineLobes(angle, count, phase, sharpness);
}

fn featherMask(dist: f32, radius: f32, feather: f32) -> f32 {
  let falloffStart = (radius * (1f - feather));
  return (1f - smoothstep(falloffStart, radius, dist));
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
  let burstFrame_1 = radialFrame(uv, uniforms._sys.viewportSize, uniforms.n_n0.center);
  let burstAlpha_0 = (softRayLobes(burstFrame_1.y, uniforms.n_n0.rayCount, (uniforms.n_n0._animTime * -1.0), uniforms.n_n0.softness) * featherMask(burstFrame_1.x, uniforms.n_n0.radius, uniforms.n_n0.feather));
  var composed_2 = vec4f(mix(uniforms.n_n0.background.rgb, uniforms.n_n0.color.rgb, burstAlpha_0), mix(uniforms.n_n0.background.a, uniforms.n_n0.color.a, burstAlpha_0));
  return vec4f(linearToSrgb(tone_linear(composed_2.rgb)) * composed_2.a, composed_2.a);
}