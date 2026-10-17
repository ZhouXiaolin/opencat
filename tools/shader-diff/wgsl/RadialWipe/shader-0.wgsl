fn applyReveal(color: vec4f, reveal: f32) -> vec4f {
  return vec4f(color.x, color.y, color.z, (color.w * reveal));
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

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  progress: f32,
  startAngle: f32,
  center: vec2f,
  feather: f32,
  invert: f32,
  _opacity: f32,
}

struct struct_2 {
  colorA: vec4f,
  colorB: vec4f,
  colorsArray: array<vec4f, 8>,
  positionsArray: array<vec4f, 2>,
  convertedColorsArray: array<vec3f, 8>,
  stopCount: f32,
  start: vec2f,
  end: vec2f,
  angle: f32,
  edges: f32,
  colorSpace: f32,
  _opacity: f32,
}

struct struct_3 {
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
  n_n0c0: struct_2,
  _sys: struct_3,
}

@group(0) @binding(0) var<uniform> uniforms: combined;

fn gradientClamp01(t: f32) -> f32 {
  return clamp(t, 0f, 1f);
}

fn unflipPosition(p: vec2f) -> vec2f {
  return vec2f(p.x, (1f - p.y));
}

fn geom_rotate2(p: vec2f, cosA: f32, sinA: f32) -> vec2f {
  return vec2f(((p.x * cosA) - (p.y * sinA)), ((p.x * sinA) + (p.y * cosA)));
}

fn gradientRawT(start: vec2f, end: vec2f, angle: f32, uv: vec2f, viewport: vec2f) -> f32 {
  let startPos = unflipPosition(start);
  let endPos = unflipPosition(end);
  let gradientVector = (endPos - startPos);
  let gradientLength = length(gradientVector);
  let gradientDir = normalize(gradientVector);
  let safeViewportY = max(viewport.y, 1e-6f);
  let aspect = (viewport.x / safeViewportY);
  let angleRad = ((angle * 0.017453292519943295f) * -1f);
  let cosAngle = cos(angleRad);
  let sinAngle = sin(angleRad);
  let midpoint = ((startPos + endPos) * 0.5f);
  let centeredUV = (uv - midpoint);
  let rotated = geom_rotate2(vec2f((centeredUV.x * aspect), centeredUV.y), cosAngle, sinAngle);
  let rotatedUV = (vec2f((rotated.x / aspect), rotated.y) + midpoint);
  let relativePos = (rotatedUV - startPos);
  let projection = dot(relativePos, gradientDir);
  return (projection / max(gradientLength, 1e-6f));
}

fn revealMask(coord: f32, progress: f32, feather: f32, invert: f32) -> f32 {
  let c = select(coord, (1f - coord), (invert > 0f));
  let f = max(feather, 1e-4f);
  let front = ((progress * (1f + (2f * f))) - f);
  return smoothstep((front - f), (front + f), c);
}

fn angularCoord(uv: vec2f, aspect: f32, center: vec2f, startDeg: f32, mode: f32) -> f32 {
  let rel = vec2f(((uv.x - center.x) * aspect), (uv.y - (1f - center.y)));
  let ang = atan2(rel.y, rel.x);
  let start = (startDeg * 0.017453292519943295f);
  var frac = ((ang - start) / 6.283185307179586f);
  frac = (frac - floor(frac));
  let ccwCoord = (1f - frac);
  let bothCoord = (min(frac, (1f - frac)) * 2f);
  var coord = select(frac, ccwCoord, (mode > 0.5f));
  coord = select(coord, bothCoord, (mode > 1.5f));
  return coord;
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
  var composed_0 = applyReveal(mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, uv, uniforms._sys.viewportSize)))), revealMask(angularCoord(uv, uniforms._sys.aspect, uniforms.n_n0.center, uniforms.n_n0.startAngle, 0.0), uniforms.n_n0.progress, uniforms.n_n0.feather, uniforms.n_n0.invert));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}