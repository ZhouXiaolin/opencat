struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  color: vec4f,
  density: f32,
  dotSize: f32,
  offset: f32,
  speed: f32,
  speedVariance: f32,
  twinkle: f32,
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

fn aspectCorrectedUVFlipY(uv: vec2f, viewport: vec2f) -> vec2f {
  let aspect = (viewport.x / max(viewport.y, 1e-6f));
  return vec2f((uv.x * aspect), (1f - uv.y));
}

fn rowSpeedHash(row: f32) -> f32 {
  return fract((sin((row * 127.1f)) * 43758.5453f));
}

fn rowSpeedMultiplier(rowHash: f32, variance: f32) -> f32 {
  return (1f + (((rowHash - 0.5f) * 4f) * variance));
}

fn flooredMod1(x: f32, m: f32) -> f32 {
  return (x - (m * floor((x / m))));
}

struct CellTile {
  cell: vec2f,
  local: vec2f,
}

fn squareTiling(p: vec2f) -> CellTile {
  return CellTile(floor(p), fract(p));
}

fn discCoverage(dist: f32, radius: f32, footprint: f32) -> f32 {
  return (1f - smoothstep((radius - (footprint * 0.5f)), radius, dist));
}

fn dotGridAlpha(uv: vec2f, viewport: vec2f, density: f32, dotSize: f32, offset: f32, speedVariance: f32, twinkle: f32, animTime: f32, time: f32) -> f32 {
  let correctedUV = aspectCorrectedUVFlipY(uv, viewport);
  let gridUV = (correctedUV * density);
  let rowIndex = floor(gridUV.y);
  let speedMod = rowSpeedMultiplier(rowSpeedHash(rowIndex), speedVariance);
  let drift = (animTime * speedMod);
  let staticStagger = (offset * flooredMod1(rowIndex, 2f));
  let shiftedGrid = vec2f(((gridUV.x - staticStagger) - drift), gridUV.y);
  let tile = squareTiling(shiftedGrid);
  let centerDistance = length((tile.local - 0.5f));
  let pixelSize = length(fwidth((correctedUV * density)));
  let coverage = discCoverage(centerDistance, (dotSize * 0.5f), pixelSize);
  let dotPhase = fract((sin(((tile.cell.x * 12.9898f) + (tile.cell.y * 78.233f))) * 43758.5453f));
  let twinkleTime = ((time * 2f) + (dotPhase * 6.283185307179586f));
  let twinkleValue = ((sin(twinkleTime) * 0.5f) + 0.5f);
  let twinkleModifier = mix(1f, twinkleValue, twinkle);
  return (coverage * twinkleModifier);
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
  var composed_0 = vec4f(uniforms.n_n0.color.rgb, dotGridAlpha(uv, uniforms._sys.viewportSize, uniforms.n_n0.density, uniforms.n_n0.dotSize, uniforms.n_n0.offset, uniforms.n_n0.speedVariance, uniforms.n_n0.twinkle, uniforms.n_n0._animTime, uniforms._sys.time));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}