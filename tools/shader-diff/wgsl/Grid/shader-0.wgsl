fn quilezLineFilterAxis(p: f32, w: f32, N: f32) -> f32 {
  let a = (p + (w * 0.5f));
  let b = (p - (w * 0.5f));
  return ((((floor(a) + min((fract(a) * N), 1f)) - floor(b)) - min((fract(b) * N), 1f)) / (N * w));
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

fn gridField(gridUV: vec2f, thickness: f32, softness: f32, variation: f32) -> vec2f {
  let lineWidth = (thickness * 0.01f);
  let safeLine = max(lineWidth, 1e-4f);
  let lineFraction = (safeLine * 2f);
  let N = (1f / lineFraction);
  let p = (gridUV + safeLine);
  let dpx = dpdx(p);
  let dpy = dpdy(p);
  let wx = (max(max(abs(dpx.x), abs(dpy.x)), 1e-5f) + softness);
  let wy = (max(max(abs(dpx.y), abs(dpy.y)), 1e-5f) + softness);
  let iX = quilezLineFilterAxis(p.x, wx, N);
  let iY = quilezLineFilterAxis(p.y, wy, N);
  let gapBoth = ((1f - iX) * (1f - iY));
  let gridMask = ((1f - gapBoth) * step(1e-4f, lineWidth));
  let variedFactor = cellVariation(floor(gridUV), variation);
  return vec2f(gridMask, variedFactor);
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

fn latticeFrameFlipY(uv: vec2f, viewport: vec2f, cells: f32, rotationDeg: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let correctedUV = vec2f((uv.x * aspect), (1f - uv.y));
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
  color: vec4f,
  cellColor: vec4f,
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
  let gridField_0 = gridField(latticeFrameFlipY(uv, uniforms._sys.viewportSize, uniforms.n_n0.cells, uniforms.n_n0.rotation), uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.variation);
  var composed_1 = mixColors(vec4f((uniforms.n_n0.cellColor.rgb * gridField_0.y), uniforms.n_n0.cellColor.a), uniforms.n_n0.color, gridField_0.x);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}