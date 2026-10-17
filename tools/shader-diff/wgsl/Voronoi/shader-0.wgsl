fn cellHash2(p: vec2f) -> vec2f {
  let h = vec2f(dot(p, vec2f(127.0999984741211, 311.70001220703125)), dot(p, vec2f(269.5, 183.3000030517578)));
  return fract((sin(h) * 43758.5453f));
}

fn voronoiNearest2Len(scaledUV: vec2f, animT: f32, seed: f32) -> vec2f {
  let cell = floor(scaledUV);
  let localUV = fract(scaledUV);
  var d1 = 10f;
  var d2 = 10f;
  for (var ny = -1; (ny <= 1i); ny++) {
    for (var nx = -1; (nx <= 1i); nx++) {
      let offset = vec2f(f32(nx), f32(ny));
      let h = cellHash2(vec2f(((cell.x + offset.x) + seed), ((cell.y + offset.y) + seed)));
      let pointX = clamp((h.x + (sin((animT + (h.x * 6.283185307179586f))) * 0.15f)), 0.05f, 0.95f);
      let pointY = clamp((h.y + (cos(((animT * 0.7f) + (h.y * 6.283185307179586f))) * 0.15f)), 0.05f, 0.95f);
      let dd = length(vec2f((localUV.x - (offset.x + pointX)), (localUV.y - (offset.y + pointY))));
      d2 = min(d2, max(dd, d1));
      d1 = min(d1, dd);
    }
  }
  return vec2f(d1, d2);
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn aspectPlane(uv: vec2f, viewport: vec2f) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  return vec2f((uv.x * aspect), uv.y);
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
  colorBorder: vec4f,
  scale: f32,
  speed: f32,
  seed: f32,
  edgeIntensity: f32,
  edgeSoftness: f32,
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

fn cellRatio(d1: f32, d2: f32, power: f32) -> f32 {
  let safeSum = max((d1 + d2), 1e-4f);
  let normalizedDist = clamp(((d1 * 2f) / safeSum), 0f, 1f);
  return clamp(pow(normalizedDist, power), 0f, 1f);
}

fn cellFill(dd: vec2f, edgeIntensity: f32) -> f32 {
  let gradientPower = (4f - (edgeIntensity * 3f));
  return cellRatio(dd.x, dd.y, gradientPower);
}

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

fn cellEdgeMask(d1: f32, d2: f32, softness: f32) -> f32 {
  let safeSum = max((d1 + d2), 1e-4f);
  let edgeMetric = ((d2 - d1) / safeSum);
  return smoothstep(0f, (softness + 1e-3f), edgeMetric);
}

fn cellBorders(dd: vec2f, softness: f32, scale: f32) -> f32 {
  let scaledEdgeSoftness = ((softness * scale) / 6f);
  return cellEdgeMask(dd.x, dd.y, scaledEdgeSoftness);
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
  let field_2 = voronoiNearest2Len((aspectPlane(uv, uniforms._sys.viewportSize) * uniforms.n_n0.scale), uniforms.n_n0._animTime, uniforms.n_n0.seed);
  let field_1 = cellFill(field_2, uniforms.n_n0.edgeIntensity);
  let base_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, field_1);
  var composed_3 = vec4f(mix(uniforms.n_n0.colorBorder.rgb, base_0.rgb, cellBorders(field_2, uniforms.n_n0.edgeSoftness, uniforms.n_n0.scale)), base_0.a);
  return vec4f(linearToSrgb(tone_linear(composed_3.rgb)) * composed_3.a, composed_3.a);
}