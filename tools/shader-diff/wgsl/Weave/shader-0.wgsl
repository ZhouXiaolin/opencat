struct CellTile {
  cell: vec2f,
  local: vec2f,
}

fn squareTiling(p: vec2f) -> CellTile {
  return CellTile(floor(p), fract(p));
}

fn bandMask(x: f32, lo: f32, hi: f32) -> f32 {
  return (smoothstep(lo, hi, x) * smoothstep(lo, hi, (1f - x)));
}

fn flooredMod1(x: f32, m: f32) -> f32 {
  return (x - (m * floor((x / m))));
}

fn weaveColor(scaledUV: vec2f, gap: f32, colorA: vec4f, colorB: vec4f) -> vec4f {
  let tile = squareTiling(scaledUV);
  let pixel = (fwidth(scaledUV.x) * 0.5f);
  let hBand = bandMask(tile.local.y, gap, (gap + pixel));
  let vBand = bandMask(tile.local.x, gap, (gap + pixel));
  let isHTopFloat = flooredMod1((tile.cell.x + tile.cell.y), 2f);
  let intersection = (hBand * vBand);
  let hVis = clamp((hBand - (intersection * isHTopFloat)), 0f, 1f);
  let vVis = clamp((vBand - (intersection * (1f - isHTopFloat))), 0f, 1f);
  let weightA = (colorA.w * hVis);
  let weightB = (colorB.w * vVis);
  let totalAlpha = clamp((weightA + weightB), 0f, 1f);
  let safeWeight = max((weightA + weightB), 1e-3f);
  let r = (((colorA.x * weightA) + (colorB.x * weightB)) / safeWeight);
  let g = (((colorA.y * weightA) + (colorB.y * weightB)) / safeWeight);
  let b = (((colorA.z * weightA) + (colorB.z * weightB)) / safeWeight);
  return vec4f(r, g, b, totalAlpha);
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

fn latticeFrame(uv: vec2f, viewport: vec2f, cells: f32, rotationDeg: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let correctedUV = vec2f((uv.x * aspect), uv.y);
  let rotRad = (rotationDeg * 0.017453292519943295f);
  let cosR = cos(rotRad);
  let sinR = sin(rotRad);
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
  gap: f32,
  rotation: f32,
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
  var composed_0 = weaveColor(latticeFrame(uv, uniforms._sys.viewportSize, uniforms.n_n0.cells, uniforms.n_n0.rotation), uniforms.n_n0.gap, uniforms.n_n0.colorA, uniforms.n_n0.colorB);
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}