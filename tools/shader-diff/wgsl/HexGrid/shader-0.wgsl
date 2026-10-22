fn flooredMod2(p: vec2f, m: vec2f) -> vec2f {
  return vec2f((p.x - (m.x * floor((p.x / m.x)))), (p.y - (m.y * floor((p.y / m.y)))));
}

struct HexCell {
  gv: vec2f,
  id: vec2f,
}

fn hexTiling(p: vec2f, s: vec2f) -> HexCell {
  let sh = (s * 0.5f);
  let a = (flooredMod2(p, s) - sh);
  let b = (flooredMod2((p - sh), s) - sh);
  let gv = select(b, a, (dot(a, a) < dot(b, b)));
  return HexCell(gv, (p - gv));
}

fn cellHash(id: vec2f) -> f32 {
  let q0 = fract((id * vec2f(123.33999633789062, 345.45001220703125)));
  let q1 = (q0 + dot(q0, (q0 + 34.345f)));
  return fract((q1.x * q1.y));
}

fn variationFactor(rand: f32, variation: f32) -> f32 {
  return max((1f + (((rand - 0.5f) * 2f) * variation)), 0f);
}

fn cellVariation(cellId: vec2f, variation: f32) -> f32 {
  return variationFactor(cellHash(cellId), variation);
}

fn lineMaskFromField(field: f32, lineWidth: f32, lo: f32, hi: f32) -> f32 {
  return ((1f - smoothstep(lo, hi, field)) * step(1e-4f, lineWidth));
}

fn strokeBandFloored(field: f32, lineWidth: f32, pixel: f32, softness: f32) -> f32 {
  let soft = (softness * 0.5f);
  let lo = max((lineWidth - soft), 0f);
  let hi = ((lineWidth + pixel) + soft);
  return lineMaskFromField(field, lineWidth, lo, hi);
}

fn hexGridField(scaledUV: vec2f, thickness: f32, softness: f32, variation: f32) -> vec2f {
  let hex = hexTiling(scaledUV, vec2f(1.7320507764816284, 1));
  let ix = round((hex.id.x * 1.1547005383792517f));
  let iy = round((hex.id.y * 2f));
  let variedFactor = cellVariation(vec2f(ix, iy), variation);
  let pAbs = vec2f(abs(hex.gv.x), abs(hex.gv.y));
  let hexDist = max(dot(pAbs, vec2f(0.8660253882408142, 0.5)), pAbs.y);
  let edgeDist = (0.5f - hexDist);
  let lineWidth = (thickness * 0.01f);
  let pixel = (fwidth(scaledUV.x) * 0.5f);
  let lineMask = strokeBandFloored(edgeDist, lineWidth, pixel, softness);
  return vec2f(lineMask, variedFactor);
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn rotateAboutCanvasCentre(p: vec2f, aspect: f32, cosA: f32, sinA: f32) -> vec2f {
  let centerX = (aspect * 0.5f);
  const centerY = 0.5;
  let cx = (p.x - centerX);
  let cy = (p.y - centerY);
  return vec2f((((cx * cosA) - (cy * sinA)) + centerX), (((cx * sinA) + (cy * cosA)) + centerY));
}

fn latticeFrameCW(uv: vec2f, viewport: vec2f, cells: f32, rotationDeg: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let correctedUV = vec2f((uv.x * aspect), uv.y);
  let ang = -((rotationDeg * 0.017453292519943295f));
  let cosR = cos(ang);
  let sinR = sin(ang);
  return (rotateAboutCanvasCentre(correctedUV, aspect, cosR, sinR) * cells);
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
  cells: f32,
  thickness: f32,
  rotation: f32,
  softness: f32,
  variation: f32,
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
  let hexField_0 = hexGridField(latticeFrameCW(uv, uniforms._sys.viewportSize, uniforms.n_n0.cells, uniforms.n_n0.rotation), uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.variation);
  var composed_1 = mixColors(vec4f((uniforms.n_n0.colorA.rgb * hexField_0.y), uniforms.n_n0.colorA.a), uniforms.n_n0.colorB, hexField_0.x);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}