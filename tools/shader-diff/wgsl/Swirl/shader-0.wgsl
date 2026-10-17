fn flowLayers(uv: vec2f, detail: f32, animTime: f32) -> f32 {
  let t = animTime;
  let freq1 = detail;
  let d1 = vec2f((uv.x + ((sin(((uv.y * (freq1 * 1.7f)) + (t * 0.8f))) * 0.12f) + (cos(((uv.x * (freq1 * 0.9f)) - (t * 0.5f))) * 0.05f))), (uv.y + ((cos(((uv.x * (freq1 * 1.3f)) - (t * 0.6f))) * 0.12f) + (sin(((uv.y * (freq1 * 1.1f)) + (t * 0.7f))) * 0.05f))));
  let pattern1 = sin((((d1.x * (freq1 * 2.1f)) + (d1.y * (freq1 * 1.8f))) + (t * 0.4f)));
  let freq2 = (detail * 2.1f);
  let d2 = vec2f((d1.x + ((cos(((d1.y * (freq2 * 2.7f)) - (t * 0.45f))) * 0.07f) + (sin(((d1.x * (freq2 * 1.9f)) + (t * 0.6f))) * 0.04f))), (d1.y + ((sin(((d1.x * (freq2 * 2.3f)) + (t * 0.65f))) * 0.07f) + (cos(((d1.y * (freq2 * 1.6f)) - (t * 0.4f))) * 0.04f))));
  let pattern2 = cos((((d2.x * (freq2 * 1.4f)) - (d2.y * (freq2 * 1.9f))) + (t * 0.35f)));
  let freq3 = (detail * 3.7f);
  let d3 = vec2f((d2.x + (((sin(((d2.y * (freq3 * 1.8f)) + (t * 0.85f))) * 0.04f) + (cos(((d2.x * (freq3 * 1.3f)) - (t * 0.55f))) * 0.025f)) + (sin((((d2.x + d2.y) * (freq3 * 0.7f)) + (t * 0.9f))) * 0.02f))), (d2.y + (((cos(((d2.x * (freq3 * 1.6f)) - (t * 0.75f))) * 0.04f) + (sin(((d2.y * (freq3 * 1.1f)) + (t * 0.5f))) * 0.025f)) + (cos((((d2.x + d2.y) * (freq3 * 0.8f)) - (t * 0.95f))) * 0.02f))));
  let pattern3 = sin((((d3.x * (freq3 * 1.1f)) + (d3.y * (freq3 * 1.5f))) - (t * 0.55f)));
  return (((pattern1 * 0.45f) + (pattern2 * 0.35f)) + (pattern3 * 0.2f));
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
  speed: f32,
  detail: f32,
  blend: f32,
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

fn unitThreshold(pattern: f32, bias: f32, low: f32, high: f32) -> f32 {
  let normalized = (((pattern * 0.5f) + 0.5f) + bias);
  return smoothstep(low, high, normalized);
}

fn shimmerPulse(pattern: f32, animTime: f32, speed: f32, span: f32, depth: f32) -> f32 {
  return ((sin(((animTime * speed) + (pattern * span))) * depth) + 1f);
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
  let field_0 = flowLayers(uv, uniforms.n_n0.detail, uniforms.n_n0._animTime);
  var composed_1 = (mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, unitThreshold(field_0, ((uniforms.n_n0.blend + -50.0) * 0.006), 0.3, 0.7)) * shimmerPulse(field_0, uniforms.n_n0._animTime, 2.5, 8.0, 0.015));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}