struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  color: vec4f,
  amplitude: f32,
  frequency: f32,
  speed: f32,
  angle: f32,
  position: vec2f,
  thickness: f32,
  softness: f32,
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

fn aspectCenteredDelta(uv: vec2f, center: vec2f, viewport: vec2f) -> vec2f {
  let aspect = (viewport.x / max(viewport.y, 1e-6f));
  return vec2f(((uv.x * aspect) - (center.x * aspect)), (uv.y - (1f - center.y)));
}

fn geom_rotate2(p: vec2f, cosA: f32, sinA: f32) -> vec2f {
  return vec2f(((p.x * cosA) - (p.y * sinA)), ((p.x * sinA) + (p.y * cosA)));
}

fn sineWaveMask(uv: vec2f, viewport: vec2f, angle: f32, position: vec2f, frequency: f32, amplitude: f32, thickness: f32, softness: f32, animTime: f32) -> f32 {
  let centered = aspectCenteredDelta(uv, position, viewport);
  let angleRad = (angle * 0.017453292519943295f);
  let cosAngle = cos(angleRad);
  let sinAngle = sin(angleRad);
  let rotated = geom_rotate2(centered, cosAngle, sinAngle);
  let waveInput = (((rotated.x * frequency) * 6.283185307179586f) + animTime);
  let sineWave = (sin(waveInput) * amplitude);
  let distanceFromWave = abs((rotated.y - sineWave));
  let halfThickness = (thickness * 0.5f);
  let halfSoftness = (softness * 0.5f);
  return (1f - smoothstep((halfThickness - halfSoftness), (halfThickness + halfSoftness), distanceFromWave));
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
  var composed_0 = vec4f(uniforms.n_n0.color.rgb, (uniforms.n_n0.color.a * sineWaveMask(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.position, uniforms.n_n0.frequency, uniforms.n_n0.amplitude, uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0._animTime)));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}