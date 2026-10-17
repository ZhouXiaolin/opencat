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

fn floatBitsToUint3(v: vec3f) -> vec3u { return bitcast<vec3u>(v); }

fn hash33(p: vec3f) -> vec3f {
  let u = floatBitsToUint3((p * vec3f(141421360, 2718281728, 1618034048)));
  let h = ((u.x ^ u.y) ^ u.z);
  let prod = (vec3u(h, h, h) * vec3u(1732050807, 2645751311, 3316624790));
  return (vec3f(f32(prod.x), f32(prod.y), f32(prod.z)) / 4294967296f);
}

fn perlinGrad3(i: vec3f, f: vec3f, ox: f32, oy: f32, oz: f32) -> f32 {
  let o = vec3f(ox, oy, oz);
  return dot((f - o), normalize((hash33((i + o)) - 0.5f)));
}

fn perlin13(p: vec3f) -> f32 {
  let i = floor(p);
  let f = (p - i);
  let u = (((f * f) * f) * ((f * ((f * 6f) - 15f)) + 10f));
  let a0 = perlinGrad3(i, f, 0f, 0f, 0f);
  let b0 = perlinGrad3(i, f, 1f, 0f, 0f);
  let c0 = perlinGrad3(i, f, 0f, 1f, 0f);
  let d0 = perlinGrad3(i, f, 1f, 1f, 0f);
  let a1 = perlinGrad3(i, f, 0f, 0f, 1f);
  let b1 = perlinGrad3(i, f, 1f, 0f, 1f);
  let c1 = perlinGrad3(i, f, 0f, 1f, 1f);
  let d1 = perlinGrad3(i, f, 1f, 1f, 1f);
  let z0 = mix(mix(a0, b0, u.x), mix(c0, d0, u.x), u.y);
  let z1 = mix(mix(a1, b1, u.x), mix(c1, d1, u.x), u.y);
  return ((mix(z0, z1, u.z) * 0.7f) + 0.5f);
}

fn curl22z(p: vec2f, z: f32) -> vec2f {
  let pax = (p + vec2f(0.10000000149011612, 0));
  let pay = (p + vec2f(0, 0.10000000149011612));
  let pbx = (p - vec2f(0.10000000149011612, 0));
  let pby = (p - vec2f(0, 0.10000000149011612));
  let a = vec2f(perlin13(vec3f(pax.x, pax.y, z)), perlin13(vec3f(pay.x, pay.y, z)));
  let b = vec2f(perlin13(vec3f(pbx.x, pbx.y, z)), perlin13(vec3f(pby.x, pby.y, z)));
  return (((a - b) / 0.1f) * 0.5f);
}

fn curlMagnitude(pos: vec2f, z: f32) -> f32 {
  return (length(curl22z(pos, z)) / 1.414f);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, noiseToneKColor(curlMagnitude(aspectScaledDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.seed), (uniforms.n_n0._animTime * 0.2)), uniforms.n_n0.contrast, uniforms.n_n0.balance));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}