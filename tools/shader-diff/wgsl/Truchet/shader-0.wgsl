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
  cells: f32,
  thickness: f32,
  rotation: f32,
  softness: f32,
  seed: f32,
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

struct CellTile {
  cell: vec2f,
  local: vec2f,
}

fn squareTiling(p: vec2f) -> CellTile {
  return CellTile(floor(p), fract(p));
}

fn truchetHash(p: vec2f) -> f32 {
  let h = dot(p, vec2f(127.0999984741211, 311.70001220703125));
  return fract((sin(h) * 43758.5453f));
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

fn truchetField(scaledUV: vec2f, thickness: f32, softness: f32, seed: f32) -> f32 {
  let tile = squareTiling(scaledUV);
  let h = truchetHash(vec2f((tile.cell.x + seed), (tile.cell.y + seed)));
  let flip = (h > 0.5f);
  let c1x = select(0f, 1f, flip);
  let c2x = select(1f, 0f, flip);
  let d1 = abs((length(vec2f((tile.local.x - c1x), tile.local.y)) - 0.5f));
  let d2 = abs((length(vec2f((tile.local.x - c2x), (tile.local.y - 1f))) - 0.5f));
  let arcDist = min(d1, d2);
  let lineWidth = (thickness * 5e-3f);
  let pixel = (fwidth(scaledUV.x) * 0.5f);
  return strokeBandFloored(arcDist, lineWidth, pixel, softness);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, truchetField(latticeFrameCW(uv, uniforms._sys.viewportSize, uniforms.n_n0.cells, uniforms.n_n0.rotation), uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.seed));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}