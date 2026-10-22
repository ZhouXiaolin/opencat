fn bendRemap(strength: f32, falloff: f32, angle: f32, uv: vec2f, aspect: f32) -> vec2f {
  let angleRad = (angle * 0.017453292519943295f);
  let dir = vec2f(cos(angleRad), sin(angleRad));
  let perp = vec2f((dir.y * -1f), dir.x);
  let p = vec2f(((uv.x - 0.5f) * aspect), (uv.y - 0.5f));
  let sScreen = dot(p, dir);
  let tScreen = dot(p, perp);
  let sHalf = max((((abs(dir.x) * aspect) + abs(dir.y)) * 0.5f), 1e-3f);
  let snScreen = (sScreen / sHalf);
  let b = (strength * 1.2f);
  let f = (clamp(falloff, 0f, 1f) * 0.9f);
  let fSpan = (1f - f);
  let m = abs(snScreen);
  let sig = select(-1f, 1f, (snScreen >= 0f));
  let a2 = (b * m);
  let disc = (((4f * fSpan) * fSpan) + (((4f * a2) * 2f) * (m - f)));
  let nearZero = (abs(a2) < 1e-5f);
  let denom = select((a2 * 2f), 1f, nearZero);
  let uQuad = ((sqrt(max(disc, 0f)) - (2f * fSpan)) / denom);
  let uOn = select(uQuad, ((m - f) / fSpan), nearZero);
  let u = select(6f, max(uOn, 0f), (disc >= 0f));
  let snBend = (sig * (f + (u * fSpan)));
  let inFlat = (m <= f);
  let sn = select(snBend, snScreen, inFlat);
  let z = select(((b * u) * u), 0f, inFlat);
  let tSrc = ((tScreen * (2f - z)) / 2f);
  let srcP = ((dir * (sn * sHalf)) + (perp * tSrc));
  return vec2f(((srcP.x / aspect) + 0.5f), (srcP.y + 0.5f));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct padded_1 {
  strength: f32,
  falloff: f32,
  angle: f32,
  edges: f32,
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
  start: vec2f,
  end: vec2f,
  angle: f32,
  edges: f32,
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
  n_n0: padded_1,
  n_n0c0: struct_1,
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

fn edgeTransparentMask(uv: vec2f) -> f32 {
  let inX = (step(0f, uv.x) * step(uv.x, 1f));
  let inY = (step(0f, uv.y) * step(uv.y, 1f));
  return (inX * inY);
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
  let foldUV_1 = bendRemap(uniforms.n_n0.strength, uniforms.n_n0.falloff, uniforms.n_n0.angle, uv, uniforms._sys.aspect);
  let sibs_0 = mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, foldUV_1, uniforms._sys.viewportSize))));
  var composed_2 = vec4f(sibs_0.rgb, (sibs_0.a * (1.0 * edgeTransparentMask(bendRemap(uniforms.n_n0.strength, uniforms.n_n0.falloff, uniforms.n_n0.angle, uv, uniforms._sys.aspect)))));
  return vec4f(linearToSrgb(tone_linear(composed_2.rgb)) * composed_2.a, composed_2.a);
}