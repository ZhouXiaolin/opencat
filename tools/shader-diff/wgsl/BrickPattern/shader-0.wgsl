fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn canvasCentredDelta(uv: vec2f, aspect: f32) -> vec2f {
  return vec2f(((uv.x * aspect) - (aspect * 0.5f)), (uv.y - 0.5f));
}

fn geom_rotate2(p: vec2f, cosA: f32, sinA: f32) -> vec2f {
  return vec2f(((p.x * cosA) - (p.y * sinA)), ((p.x * sinA) + (p.y * cosA)));
}

fn brickFrame(uv: vec2f, viewport: vec2f, rotation: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let centred = canvasCentredDelta(uv, aspect);
  let rotRad = (rotation * 0.017453292519943295f);
  let cosR = cos(rotRad);
  let sinR = sin(rotRad);
  let rot = geom_rotate2(centred, cosR, -(sinR));
  return vec2f(((rot.x / aspect) + 0.5f), (rot.y + 0.5f));
}

fn flooredMod1(x: f32, m: f32) -> f32 {
  return (x - (m * floor((x / m))));
}

fn brickRowStagger(row: f32, seed: f32, speedVariance: f32) -> vec2f {
  let rowMod2 = flooredMod1(row, 2f);
  let rowOffset = (rowMod2 * 0.5f);
  let rowDir = ((rowMod2 * 2f) - 1f);
  let rowHash = ((fract((sin((row + (seed * 0.137f))) * 43758.5453f)) - 0.5f) * 2f);
  return vec2f(rowOffset, mix(rowDir, rowHash, speedVariance));
}

fn bandMask(x: f32, lo: f32, hi: f32) -> f32 {
  return (smoothstep(lo, hi, x) * smoothstep(lo, hi, (1f - x)));
}

fn brickMortarMask(cellUV: vec2f, mortarUV: vec2f, aaUV: vec2f) -> f32 {
  let brickU = bandMask(cellUV.x, (mortarUV.x - aaUV.x), (mortarUV.x + aaUV.x));
  let brickV = bandMask(cellUV.y, (mortarUV.y - aaUV.y), (mortarUV.y + aaUV.y));
  return (brickU * brickV);
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

fn brickField(uv: vec2f, viewport: vec2f, cellsX: f32, cellsY: f32, mortar: f32, softness: f32, variation: f32, rotation: f32, offset: f32, speedVariance: f32, seed: f32, animTime: f32) -> vec2f {
  let aspect = geom_aspectOf(viewport);
  let frame = brickFrame(uv, viewport, rotation);
  let u = frame.x;
  let v = frame.y;
  let mortarU = (mortar * 0.3f);
  let mortarV = (((mortarU * aspect) * cellsY) / cellsX);
  let row = floor((v * cellsY));
  let stagger = brickRowStagger(row, seed, speedVariance);
  let brickArg = ((((u * cellsX) + stagger.x) + (animTime * stagger.y)) + offset);
  let vScaled = (v * cellsY);
  let cellU = fract(brickArg);
  let cellV = fract(vScaled);
  let soft = (softness * 0.5f);
  let aaU = (fwidth(brickArg) + soft);
  let aaV = (fwidth(vScaled) + soft);
  let brickMask = brickMortarMask(vec2f(cellU, cellV), vec2f(mortarU, mortarV), vec2f(aaU, aaV));
  let brickId = vec2f(floor(brickArg), row);
  let variedFactor = cellVariation(brickId, variation);
  return vec2f(brickMask, variedFactor);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  colorBrick: vec4f,
  colorMortar: vec4f,
  cellsX: f32,
  cellsY: f32,
  mortar: f32,
  softness: f32,
  variation: f32,
  rotation: f32,
  speed: f32,
  offset: f32,
  speedVariance: f32,
  seed: f32,
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
  let brickField_0 = brickField(uv, uniforms._sys.viewportSize, uniforms.n_n0.cellsX, uniforms.n_n0.cellsY, uniforms.n_n0.mortar, uniforms.n_n0.softness, uniforms.n_n0.variation, uniforms.n_n0.rotation, uniforms.n_n0.offset, uniforms.n_n0.speedVariance, uniforms.n_n0.seed, uniforms.n_n0._animTime);
  var composed_1 = mixColors(uniforms.n_n0.colorMortar, vec4f((uniforms.n_n0.colorBrick.rgb * brickField_0.y), uniforms.n_n0.colorBrick.a), brickField_0.x);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}