fn awMix(a: vec3f, b: vec3f, wA: f32, wB: f32, sw: f32) -> vec3f {
  return (((a * wA) + (b * wB)) / sw);
}

fn mixColors(colorA: vec4f, colorB: vec4f, t: f32) -> vec4f {
  let weightA = (colorA.w * (1f - t));
  let weightB = (colorB.w * t);
  let totalWeight = (weightA + weightB);
  let safeWeight = max(totalWeight, 1e-3f);
  let rgb = awMix(colorA.xyz, colorB.xyz, weightA, weightB, safeWeight);
  return vec4f(rgb, totalWeight);
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
  colorsArray: array<vec4f, 8>,
  positionsArray: array<vec4f, 2>,
  convertedColorsArray: array<vec3f, 8>,
  stopCount: f32,
  center: vec2f,
  radius: f32,
  repeat: f32,
  aspect: f32,
  skewAngle: f32,
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
  n_n0: struct_1,
  _sys: struct_2,
}

@group(0) @binding(0) var<uniform> uniforms: combined;

fn repeatRings(t: f32, repeat: f32) -> f32 {
  let s = (t * repeat);
  return select(clamp(s, 0f, 1f), fract(s), (repeat > 1.0001f));
}

fn aspectCenteredDelta(uv: vec2f, center: vec2f, viewport: vec2f) -> vec2f {
  let aspect = (viewport.x / max(viewport.y, 1e-6f));
  return vec2f(((uv.x * aspect) - (center.x * aspect)), (uv.y - (1f - center.y)));
}

fn geom_rotate2(p: vec2f, cosA: f32, sinA: f32) -> vec2f {
  return vec2f(((p.x * cosA) - (p.y * sinA)), ((p.x * sinA) + (p.y * cosA)));
}

fn radialDist(center: vec2f, skewAngle: f32, ellipseAspect: f32, radius: f32, uv: vec2f, viewport: vec2f) -> f32 {
  let delta = aspectCenteredDelta(uv, center, viewport);
  let skewRad = (skewAngle * 0.017453292519943295f);
  let cosS = cos(skewRad);
  let sinS = sin(skewRad);
  let rotated = geom_rotate2(delta, cosS, -(sinS));
  let stretched = vec2f((rotated.x * ellipseAspect), rotated.y);
  let dist = length(stretched);
  let safeRadius = max(radius, 1e-6f);
  return (dist / safeRadius);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, repeatRings(radialDist(uniforms.n_n0.center, uniforms.n_n0.skewAngle, uniforms.n_n0.aspect, uniforms.n_n0.radius, uv, uniforms._sys.viewportSize), uniforms.n_n0.repeat));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}