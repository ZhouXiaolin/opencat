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
  detail: f32,
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

fn signedToUnit(v: f32) -> f32 {
  return ((v * 0.5f) + 0.5f);
}

fn wavelet12(p: vec2f, phase: f32, scale: f32) -> f32 {
  var dsum = 0f;
  var s = 1f;
  var m = 0f;
  var pp = vec2f(p.x, p.y);
  for (var idx = 0; (idx < 4i); idx++) {
    let q0 = (pp * s);
    var g = fract((floor(q0) * vec2f(123.33999633789062, 233.52999877929688)));
    g = (g + dot(g, (g + 23.234f)));
    let a = (fract((g.x * g.y)) * 1000f);
    let qc = (fract(q0) - 0.5f);
    let ca = cos(a);
    let sa = sin(a);
    let q = vec2f(((qc.x * ca) - (qc.y * sa)), ((qc.x * sa) + (qc.y * ca)));
    dsum = (dsum + ((sin(((q.x * 10f) + phase)) * smoothstep(0.25f, 0f, dot(q, q))) / s));
    pp = (vec2f(((pp.x * 0.54f) - (pp.y * 0.84f)), ((pp.x * 0.84f) + (pp.y * 0.54f))) + f32(idx));
    m = (m + (1f / s));
    s = (s * scale);
  }
  return (dsum / m);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, noiseToneKColor(signedToUnit(wavelet12(aspectScaledDomain(uv, uniforms._sys.viewportSize, uniforms.n_n0.scale, uniforms.n_n0.seed), uniforms.n_n0._animTime, uniforms.n_n0.detail)), uniforms.n_n0.contrast, uniforms.n_n0.balance));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}