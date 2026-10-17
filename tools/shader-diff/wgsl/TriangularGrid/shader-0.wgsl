fn triLattice(p: vec2f) -> vec2f {
  return vec2f((p.x - (p.y / 1.7320508075688772f)), (p.y * 1.1547005383792517f));
}

fn rowSpeedHash(row: f32) -> f32 {
  return fract((sin((row * 127.1f)) * 43758.5453f));
}

fn rowSpeedMultiplier(rowHash: f32, variance: f32) -> f32 {
  return (1f + (((rowHash - 0.5f) * 4f) * variance));
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

fn strokeBandSymmetric(field: f32, lineWidth: f32, aaWidth: f32) -> f32 {
  return lineMaskFromField(field, lineWidth, (lineWidth - aaWidth), (lineWidth + aaWidth));
}

fn triangularGridField(p: vec2f, thickness: f32, softness: f32, variation: f32, speedVariance: f32, animTime: f32) -> vec2f {
  let ps = triLattice(p);
  let rowIndex = floor(ps.y);
  let speedMod = rowSpeedMultiplier(rowSpeedHash(rowIndex), speedVariance);
  let drift = (animTime * speedMod);
  let psShifted = vec2f((ps.x - drift), ps.y);
  let cell = floor(psShifted);
  let f = fract(psShifted);
  let upper = step(1f, (f.x + f.y));
  let triId = vec2f(((cell.x * 2f) + upper), cell.y);
  let variedFactor = cellVariation(triId, variation);
  let dEdge = min(min(min(f.x, (1f - f.x)), min(f.y, (1f - f.y))), abs(((f.x + f.y) - 1f)));
  let lineWidth = (thickness * 0.02f);
  let aaWidth = (max(fwidth(ps.x), fwidth(ps.y)) + (softness * 0.5f));
  let lineMask = strokeBandSymmetric(dEdge, lineWidth, aaWidth);
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
  colorA: vec4f,
  colorB: vec4f,
  cells: f32,
  thickness: f32,
  rotation: f32,
  softness: f32,
  variation: f32,
  speed: f32,
  speedVariance: f32,
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
  let triField_0 = triangularGridField(latticeFrameFlipY(uv, uniforms._sys.viewportSize, uniforms.n_n0.cells, uniforms.n_n0.rotation), uniforms.n_n0.thickness, uniforms.n_n0.softness, uniforms.n_n0.variation, uniforms.n_n0.speedVariance, uniforms.n_n0._animTime);
  var composed_1 = mixColors(vec4f((uniforms.n_n0.colorA.rgb * triField_0.y), uniforms.n_n0.colorA.a), uniforms.n_n0.colorB, triField_0.x);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}