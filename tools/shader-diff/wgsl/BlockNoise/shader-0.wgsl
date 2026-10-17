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
  colorSpace: f32,
  scale: f32,
  contrast: f32,
  balance: f32,
  seed: f32,
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

fn noiseToneKColor(noise01: f32, contrast: f32, balance: f32) -> f32 {
  let k = clamp(((((noise01 - 0.5f) * (contrast + 1f)) + 0.5f) + balance), 0f, 1f);
  return (1f - k);
}

fn perm4(x: vec4f) -> vec4f {
  let xx = (x * ((x * 34f) + 1f));
  return (xx - (floor((xx / 289f)) * 289f));
}

fn value13(p: vec3f) -> f32 {
  let a = floor(p);
  let d0 = (p - a);
  let sd = ((d0 * d0) * (3f - (d0 * 2f)));
  let b = (vec4f(a.x, a.x, a.y, a.y) + vec4f(0, 1, 0, 1));
  let k1 = perm4(vec4f(b.x, b.y, b.x, b.y));
  let k2 = (perm4((vec4f(k1.x, k1.y, k1.x, k1.y) + vec4f(b.z, b.z, b.w, b.w))) + vec4f(a.z, a.z, a.z, a.z));
  let k3 = perm4(k2);
  let k4 = perm4((k2 + 1f));
  let o1 = fract((k3 * 0.02439024f));
  let o2 = fract((k4 * 0.02439024f));
  let o3 = mix(o1, o2, vec4f(sd.z));
  let o4 = mix(vec2f(o3.x, o3.z), vec2f(o3.y, o3.w), vec2f(sd.x));
  return mix(o4.x, o4.y, sd.y);
}

fn timeAxisDomain(pos: vec2f, t: f32, rate: f32) -> vec3f {
  return vec3f(pos.x, pos.y, (t * rate));
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn aspectScaledDomain(uv: vec2f, viewport: vec2f, scale: f32, seed: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let aspectUV = vec2f((uv.x * aspect), uv.y);
  return ((aspectUV * exp(scale)) + seed);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, noiseToneKColor(value13(timeAxisDomain(aspectScaledDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.seed), uniforms.n_n0._animTime, 0.3)), uniforms.n_n0.contrast, uniforms.n_n0.balance));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}