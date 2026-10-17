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
  count: f32,
  angle: f32,
  balance: f32,
  softness: f32,
  speed: f32,
  offset: f32,
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

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn canvasCentredDelta(uv: vec2f, aspect: f32) -> vec2f {
  return vec2f(((uv.x * aspect) - (aspect * 0.5f)), (uv.y - 0.5f));
}

fn geom_rotate2(p: vec2f, cosA: f32, sinA: f32) -> vec2f {
  return vec2f(((p.x * cosA) - (p.y * sinA)), ((p.x * sinA) + (p.y * cosA)));
}

fn chevronMask(uv: vec2f, viewport: vec2f, count: f32, angle: f32, balance: f32, softness: f32, offset: f32, animTime: f32) -> f32 {
  let angleRad = (angle * 0.017453292519943295f);
  let cosA = cos(angleRad);
  let sinA = sin(angleRad);
  let rot = geom_rotate2(canvasCentredDelta(uv, geom_aspectOf(viewport)), cosA, -(sinA));
  let zigzag = (abs((fract((rot.y * count)) - 0.5f)) * 2f);
  let stripeCoord = ((((rot.x * count) + (zigzag * 0.5f)) + animTime) + offset);
  let pattern = fract(stripeCoord);
  let fw = fwidth(stripeCoord);
  return smoothstep(((balance - softness) - fw), ((balance + softness) + fw), pattern);
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
  var composed_0 = vec4f((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, chevronMask(uv, uniforms._sys.viewportSize, uniforms.n_n0.count, uniforms.n_n0.angle, uniforms.n_n0.balance, uniforms.n_n0.softness, uniforms.n_n0.offset, uniforms.n_n0._animTime))).rgb, mix(uniforms.n_n0.colorA.a, uniforms.n_n0.colorB.a, chevronMask(uv, uniforms._sys.viewportSize, uniforms.n_n0.count, uniforms.n_n0.angle, uniforms.n_n0.balance, uniforms.n_n0.softness, uniforms.n_n0.offset, uniforms.n_n0._animTime)));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}