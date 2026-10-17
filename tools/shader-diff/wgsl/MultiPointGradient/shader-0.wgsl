fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn geom_aspectCentrePosition(pos: vec2f, aspect: f32) -> vec2f {
  return vec2f((pos.x * aspect), (1f - pos.y));
}

fn mpgWeight(uvAspect: vec2f, pos: vec2f, effPower: f32) -> f32 {
  let dx = (uvAspect.x - pos.x);
  let dy = (uvAspect.y - pos.y);
  let dist = (sqrt(((dx * dx) + (dy * dy))) + 1e-3f);
  return (1f / pow(dist, effPower));
}

fn mpgFactors(uv: vec2f, vp: vec2f, posA: vec2f, posB: vec2f, posC: vec2f, posD: vec2f, posE: vec2f, smoothness: f32) -> vec4f {
  let aspect = geom_aspectOf(vp);
  let uvAspect = vec2f((uv.x * aspect), uv.y);
  let pA = geom_aspectCentrePosition(posA, aspect);
  let pB = geom_aspectCentrePosition(posB, aspect);
  let pC = geom_aspectCentrePosition(posC, aspect);
  let pD = geom_aspectCentrePosition(posD, aspect);
  let pE = geom_aspectCentrePosition(posE, aspect);
  let effPower = min((8f / (smoothness + 0.5f)), 8f);
  let wA = mpgWeight(uvAspect, pA, effPower);
  let wB = mpgWeight(uvAspect, pB, effPower);
  let wC = mpgWeight(uvAspect, pC, effPower);
  let wD = mpgWeight(uvAspect, pD, effPower);
  let wE = mpgWeight(uvAspect, pE, effPower);
  let accW1 = wA;
  let t1 = (wB / (accW1 + wB));
  let accW2 = (accW1 + wB);
  let t2 = (wC / (accW2 + wC));
  let accW3 = (accW2 + wC);
  let t3 = (wD / (accW3 + wD));
  let accW4 = (accW3 + wD);
  let t4 = (wE / (accW4 + wE));
  return vec4f(t1, t2, t3, t4);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  colorA: vec4f,
  positionA: vec2f,
  colorB: vec4f,
  positionB: vec2f,
  colorC: vec4f,
  positionC: vec2f,
  colorD: vec4f,
  positionD: vec2f,
  colorE: vec4f,
  positionE: vec2f,
  colorSpace: f32,
  smoothness: f32,
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
  let mpgFactors_0 = mpgFactors(uv, uniforms._sys.viewportSize, uniforms.n_n0.positionA, uniforms.n_n0.positionB, uniforms.n_n0.positionC, uniforms.n_n0.positionD, uniforms.n_n0.positionE, uniforms.n_n0.smoothness);
  var composed_1 = mixColors(mixColors(mixColors(mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, mpgFactors_0.x), uniforms.n_n0.colorC, mpgFactors_0.y), uniforms.n_n0.colorD, mpgFactors_0.z), uniforms.n_n0.colorE, mpgFactors_0.w);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}