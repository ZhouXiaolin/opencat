fn sliceWipeUV(uv: vec2f, aspect: f32, angleDeg: f32, sliceCount: f32, progress: f32) -> vec2f {
  let a = (angleDeg * 0.017453292519943295f);
  let dir = vec2f(cos(a), sin(a));
  let p = vec2f(((uv.x - 0.5f) * aspect), (uv.y - 0.5f));
  let grad = dot(p, dir);
  let ext = (0.5f * ((aspect * abs(dir.x)) + abs(dir.y)));
  let t = ((grad / (2f * ext)) + 0.5f);
  let idx = floor((t * max(sliceCount, 1f)));
  let sign_1 = ((fract((idx * 0.5f)) * 4f) - 1f);
  let perp = vec2f(-(dir.y), dir.x);
  let extPerp = (0.5f * ((aspect * abs(dir.y)) + abs(dir.x)));
  let slide = ((progress * (2.02f * extPerp)) * sign_1);
  let q = (p + (perp * slide));
  return vec2f(((q.x / aspect) + 0.5f), (q.y + 0.5f));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  progress: f32,
  angle: f32,
  sliceCount: f32,
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
  let foldUV_1 = sliceWipeUV(uv, uniforms._sys.aspect, uniforms.n_n0.angle, uniforms.n_n0.sliceCount, uniforms.n_n0.progress);
  let sibs_0 = mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, foldUV_1, uniforms._sys.viewportSize))));
  var composed_2 = vec4f(sibs_0.rgb, (sibs_0.a * (1.0 * edgeTransparentMask(sliceWipeUV(uv, uniforms._sys.aspect, uniforms.n_n0.angle, uniforms.n_n0.sliceCount, uniforms.n_n0.progress)))));
  return vec4f(linearToSrgb(tone_linear(composed_2.rgb)) * composed_2.a, composed_2.a);
}