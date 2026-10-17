fn edgeMirrorUV(uv: vec2f) -> vec2f {
  let mirrorX = (abs(uv.x) % 2f);
  let mirrorY = (abs(uv.y) % 2f);
  return vec2f(select(mirrorX, (2f - mirrorX), (mirrorX >= 1f)), select(mirrorY, (2f - mirrorY), (mirrorY >= 1f)));
}

fn mxFade(t: f32) -> f32 {
  return (((t * t) * t) * ((t * ((t * 6f) - 15f)) + 10f));
}

fn mxRotl32(x: u32, k: i32) -> u32 {
  return ((x << u32(k)) | (x >> u32((32i - k))));
}

fn mxBjfinal(a0: u32, b0: u32, c0: u32) -> u32 {
  var a = a0;
  var b = b0;
  var c = c0;
  c = (c ^ b);
  c = (c - mxRotl32(b, 14i));
  a = (a ^ c);
  a = (a - mxRotl32(c, 11i));
  b = (b ^ a);
  b = (b - mxRotl32(a, 25i));
  c = (c ^ b);
  c = (c - mxRotl32(b, 16i));
  a = (a ^ c);
  a = (a - mxRotl32(c, 4i));
  b = (b ^ a);
  b = (b - mxRotl32(a, 14i));
  c = (c ^ b);
  c = (c - mxRotl32(b, 24i));
  return c;
}

fn mxHashInt2(x: i32, y: i32, z: i32) -> u32 {
  const len = 3u;
  let seed = ((3735928559u + (len << 2u)) + 13u);
  let a = (seed + u32(x));
  let b = (seed + u32(y));
  let c = (seed + u32(z));
  return mxBjfinal(a, b, c);
}

fn mxGradientFloat3(hash: u32, x: f32, y: f32, z: f32) -> f32 {
  let h = (hash & 15u);
  let u = select(y, x, (h < 8u));
  let inner = select(z, x, ((h == 12u) || (h == 14u)));
  let v = select(inner, y, (h < 4u));
  let uu = select(u, -(u), ((h & 1u) != 0u));
  let vv = select(v, -(v), ((h & 2u) != 0u));
  return (uu + vv);
}

fn mxTrilerp(v0: f32, v1: f32, v2: f32, v3: f32, v4: f32, v5: f32, v6: f32, v7: f32, s: f32, t: f32, r: f32) -> f32 {
  let s1 = (1f - s);
  let t1 = (1f - t);
  let r1 = (1f - r);
  return ((r1 * ((t1 * ((v0 * s1) + (v1 * s))) + (t * ((v2 * s1) + (v3 * s))))) + (r * ((t1 * ((v4 * s1) + (v5 * s))) + (t * ((v6 * s1) + (v7 * s))))));
}

fn mxGradientScale3d(v: f32) -> f32 {
  return (0.982f * v);
}

fn mxPerlinNoiseFloat3(p: vec3f) -> f32 {
  let X = i32(floor(p.x));
  let Y = i32(floor(p.y));
  let Z = i32(floor(p.z));
  let fx = (p.x - f32(X));
  let fy = (p.y - f32(Y));
  let fz = (p.z - f32(Z));
  let u = mxFade(fx);
  let v = mxFade(fy);
  let w = mxFade(fz);
  let result = mxTrilerp(mxGradientFloat3(mxHashInt2(X, Y, Z), fx, fy, fz), mxGradientFloat3(mxHashInt2((X + 1i), Y, Z), (fx - 1f), fy, fz), mxGradientFloat3(mxHashInt2(X, (Y + 1i), Z), fx, (fy - 1f), fz), mxGradientFloat3(mxHashInt2((X + 1i), (Y + 1i), Z), (fx - 1f), (fy - 1f), fz), mxGradientFloat3(mxHashInt2(X, Y, (Z + 1i)), fx, fy, (fz - 1f)), mxGradientFloat3(mxHashInt2((X + 1i), Y, (Z + 1i)), (fx - 1f), fy, (fz - 1f)), mxGradientFloat3(mxHashInt2(X, (Y + 1i), (Z + 1i)), fx, (fy - 1f), (fz - 1f)), mxGradientFloat3(mxHashInt2((X + 1i), (Y + 1i), (Z + 1i)), (fx - 1f), (fy - 1f), (fz - 1f)), u, v, w);
  return mxGradientScale3d(result);
}

fn mxNoiseFloat3(p: vec3f) -> f32 {
  return mxPerlinNoiseFloat3(p);
}

fn flowFieldUV(uv: vec2f, aspect: f32, scale: f32, strength: f32, time: f32, evolutionTime: f32) -> vec2f {
  let aspectCorrectedUV = vec2f((uv.x * aspect), uv.y);
  let drift1 = (time * 0.7f);
  let evolve1 = (evolutionTime * 0.2f);
  let angle1 = (mxNoiseFloat3(vec3f(((aspectCorrectedUV.x * scale) + (drift1 * 0.3f)), ((aspectCorrectedUV.y * scale) + (drift1 * 0.5f)), evolve1)) * 6.283185307179586f);
  let flow1 = vec2f(cos(angle1), sin(angle1));
  let drift2 = (time * 0.5f);
  let evolve2 = (evolutionTime * 0.15f);
  let angle2 = (mxNoiseFloat3(vec3f((((aspectCorrectedUV.x * (scale * 2.3f)) + (drift2 * -0.4f)) + 173.5f), (((aspectCorrectedUV.y * (scale * 2.3f)) + (drift2 * 0.3f)) + 291.7f), (evolve2 + 50f))) * 6.283185307179586f);
  let flow2 = vec2f(cos(angle2), sin(angle2));
  let drift3 = (time * 0.3f);
  let evolve3 = (evolutionTime * 0.1f);
  let angle3 = (mxNoiseFloat3(vec3f((((aspectCorrectedUV.x * (scale * 4.7f)) + (drift3 * 0.2f)) + 527.3f), (((aspectCorrectedUV.y * (scale * 4.7f)) + (drift3 * -0.6f)) + 839.1f), (evolve3 + 100f))) * 6.283185307179586f);
  let flow3 = vec2f(cos(angle3), sin(angle3));
  let combinedFlow = ((flow1 + (flow2 * 0.5f)) + (flow3 * 0.25f));
  let flowLength = max(length(combinedFlow), 1e-3f);
  let normalizedFlow = (combinedFlow / flowLength);
  let finalFlow = vec2f(((normalizedFlow.x * strength) / aspect), (normalizedFlow.y * strength));
  return vec2f((uv.x + finalFlow.x), (uv.y + finalFlow.y));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct padded_1 {
  strength: f32,
  detail: f32,
  speed: f32,
  evolutionSpeed: f32,
  seed: f32,
  edges: f32,
  _opacity: f32,
  _animTime: f32,
  _animTime_evolution: f32,
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
  let foldUV_0 = edgeMirrorUV(flowFieldUV(uv, uniforms._sys.aspect, uniforms.n_n0.detail, uniforms.n_n0.strength, ((uniforms.n_n0._animTime + uniforms.n_n0.seed) * 0.1), ((uniforms.n_n0._animTime_evolution + uniforms.n_n0.seed) * 0.1)));
  var composed_1 = mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, foldUV_0, uniforms._sys.viewportSize))));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}