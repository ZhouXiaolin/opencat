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
  colorSpace: f32,
  scale: f32,
  mode: f32,
  distance: f32,
  octaves: f32,
  lacunarity: f32,
  persistence: f32,
  jitter: f32,
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

fn worleyTone(v: f32, contrast: f32, balance: f32) -> f32 {
  let signed = ((v * 2f) - 1f);
  let adjusted = ((signed * contrast) + balance);
  return clamp(((adjusted * 0.5f) + 0.5f), 0f, 1f);
}

fn worleyNormalize(sum: vec2f, modeScale: f32) -> f32 {
  return ((sum.x / max(sum.y, 1e-4f)) * modeScale);
}

fn cellHash2(p: vec2f) -> vec2f {
  let h = vec2f(dot(p, vec2f(127.0999984741211, 311.70001220703125)), dot(p, vec2f(269.5, 183.3000030517578)));
  return fract((sin(h) * 43758.5453f));
}

fn worleyNearest2SelJit(scaledUV: vec2f, animT: f32, seed: f32, jitterAmount: f32, metric: f32) -> vec2f {
  let animTSlow = (animT * 0.7f);
  let cell = floor(scaledUV);
  let localUV = fract(scaledUV);
  var d1 = 10f;
  var d2 = 10f;
  for (var ny = -1; (ny <= 1i); ny++) {
    for (var nx = -1; (nx <= 1i); nx++) {
      let offset = vec2f(f32(nx), f32(ny));
      let h = cellHash2(vec2f(((cell.x + offset.x) + seed), ((cell.y + offset.y) + seed)));
      let px = (h.x + (sin((animT + (h.x * 6.283185307179586f))) * 0.15f));
      let py = (h.y + (cos((animTSlow + (h.y * 6.283185307179586f))) * 0.15f));
      let jx = mix(0.5f, clamp(px, 0.05f, 0.95f), jitterAmount);
      let jy = mix(0.5f, clamp(py, 0.05f, 0.95f), jitterAmount);
      let deltaX = (localUV.x - (offset.x + jx));
      let deltaY = (localUV.y - (offset.y + jy));
      let dEuclid = ((deltaX * deltaX) + (deltaY * deltaY));
      let dManhattan = (abs(deltaX) + abs(deltaY));
      let dChebyshev = max(abs(deltaX), abs(deltaY));
      var dd = dEuclid;
      dd = select(dd, dManhattan, (metric == 1f));
      dd = select(dd, dChebyshev, (metric == 2f));
      d2 = min(d2, max(dd, d1));
      d1 = min(d1, dd);
    }
  }
  return vec2f(d1, d2);
}

fn cellReduceSelectable(d1: f32, d2: f32, metric: f32, mode: f32) -> f32 {
  let isEuclid = (metric == 0f);
  let sd1 = select(d1, sqrt(d1), isEuclid);
  let sd2 = select(d2, sqrt(d2), isEuclid);
  var raw = sd1;
  raw = select(raw, sd2, (mode == 1f));
  raw = select(raw, (sd2 - sd1), (mode == 2f));
  raw = select(raw, (sd1 + sd2), (mode == 3f));
  raw = select(raw, (sd1 * sd2), (mode == 4f));
  return raw;
}

fn octaveField(oUV: vec2f, animT: f32, seedOff: f32, jitter: f32, distNum: f32, modeNum: f32) -> f32 {
  let dd = worleyNearest2SelJit(oUV, animT, seedOff, jitter, distNum);
  return cellReduceSelectable(dd.x, dd.y, distNum, modeNum);
}

fn worleyOctaveSumTimeSeed4(base: vec2f, baseFreq: f32, lacunarity: f32, persistence: f32, animTime: f32, seed: f32, extra: vec3f, octaveCount: f32) -> vec2f {
  var freq = baseFreq;
  var amp = 1f;
  var accumulated = 0f;
  var totalWeight = 0f;
  for (var o = 0; (o < 4i); o++) {
    let fo = f32(o);
    let active_1 = select(0f, 1f, (fo < octaveCount));
    let oUV = vec2f((base.x * freq), (base.y * freq));
    let animT = (animTime + (fo * 17f));
    let seedOff = (seed + (fo * 31f));
    let raw = octaveField(oUV, animT, seedOff, extra.x, extra.y, extra.z);
    accumulated = (accumulated + ((raw * amp) * active_1));
    totalWeight = (totalWeight + (amp * active_1));
    amp = (amp * persistence);
    freq = (freq * lacunarity);
  }
  return vec2f(accumulated, totalWeight);
}

fn worleyCells(pos: vec2f, scale: f32, seed: f32, jitter: f32, lacunarity: f32, persistence: f32, animTime: f32, octaves: f32, distNum: f32, modeNum: f32) -> vec2f {
  return worleyOctaveSumTimeSeed4(pos, scale, lacunarity, persistence, animTime, seed, vec3f(jitter, distNum, modeNum), octaves);
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn aspectPlane(uv: vec2f, viewport: vec2f) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  return vec2f((uv.x * aspect), uv.y);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, worleyTone(worleyNormalize(worleyCells(aspectPlane(uv, uniforms._sys.viewportSize), uniforms.n_n0.scale, uniforms.n_n0.seed, uniforms.n_n0.jitter, uniforms.n_n0.lacunarity, uniforms.n_n0.persistence, uniforms.n_n0._animTime, 1.0, 0.0, 0.0), 1.5), uniforms.n_n0.contrast, uniforms.n_n0.balance));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}