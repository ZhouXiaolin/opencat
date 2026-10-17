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

fn noiseToneKColor(noise01: f32, contrast: f32, balance: f32) -> f32 {
  let k = clamp(((((noise01 - 0.5f) * (contrast + 1f)) + 0.5f) + balance), 0f, 1f);
  return (1f - k);
}

fn signedToUnit(v: f32) -> f32 {
  return ((v * 0.5f) + 0.5f);
}

fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }

fn hash22(p: vec2f) -> vec2f {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  let h = (u.x ^ u.y);
  let prod = (vec2u(h, h) * vec2u(3141592653, 1618033988));
  return (vec2f(f32(prod.x), f32(prod.y)) / 4294967296f);
}

fn perlin12d(p: vec2f) -> vec3f {
  let i = floor(p);
  let f = fract(p);
  let u = (((f * f) * f) * ((f * ((f * 6f) - 15f)) + 10f));
  let du = (((f * f) * ((f * (f - 2f)) + 1f)) * 30f);
  let ga = ((hash22((i + vec2f())) * 2f) - 1f);
  let gb = ((hash22((i + vec2f(1, 0))) * 2f) - 1f);
  let gc = ((hash22((i + vec2f(0, 1))) * 2f) - 1f);
  let gd = ((hash22((i + vec2f(1))) * 2f) - 1f);
  let va = dot(ga, (f - vec2f()));
  let vb = dot(gb, (f - vec2f(1, 0)));
  let vc = dot(gc, (f - vec2f(0, 1)));
  let vd = dot(gd, (f - vec2f(1)));
  let k = (((va - vb) - vc) + vd);
  let value = (((va + (u.x * (vb - va))) + (u.y * (vc - va))) + ((u.x * u.y) * k));
  let deriv = ((((ga + ((gb - ga) * u.x)) + ((gc - ga) * u.y)) + ((((ga - gb) - gc) + gd) * (u.x * u.y))) + (du * (((vec2f(u.y, u.x) * k) + vec2f(vb, vc)) - va)));
  return vec3f(value, deriv.x, deriv.y);
}

fn gullies(p: vec2f, slope: vec2f) -> vec3f {
  let sideDir = (vec2f(-(slope.y), slope.x) * 3.141592653589793f);
  let id = floor(p);
  let pp = (p - id);
  var heightSlope = vec2f();
  var wSum = 0f;
  for (var x = -1; (x <= 2i); x++) {
    for (var y = -1; (y <= 2i); y++) {
      let off = vec2f(f32(x), f32(y));
      let c = (((pp - off) - hash22((id + off))) + 0.5f);
      let dist2 = dot(c, c);
      let w = max(0f, (exp((dist2 * -2f)) - 0.01111f));
      wSum = (wSum + w);
      let t = dot(c, sideDir);
      heightSlope = (heightSlope + (vec2f(cos(t), -(sin(t))) * w));
    }
  }
  return (vec3f(heightSlope.x, (heightSlope.y * sideDir.x), (heightSlope.y * sideDir.y)) / wSum);
}

fn erosion12(p: vec2f) -> vec3f {
  var nd = perlin12d(p);
  var strength = 0.25f;
  var freq = 8f;
  var total = 1f;
  for (var i = 0; (i < 4i); i++) {
    let grad = vec2f(nd.y, nd.z);
    let len2 = max(dot(grad, grad), 9.999999747378752e-5f);
    let slope = (grad * pow(len2, -0.25f));
    nd = (nd + ((gullies((p * freq), slope) * strength) * vec3f(1f, freq, freq)));
    total = (total + strength);
    strength = (strength * 0.5f);
    freq = (freq * 2f);
  }
  return (nd / total);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, noiseToneKColor(signedToUnit((erosion12(aspectScaledDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.seed))).x), uniforms.n_n0.contrast, uniforms.n_n0.balance));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}