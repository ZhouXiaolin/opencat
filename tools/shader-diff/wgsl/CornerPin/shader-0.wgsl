fn cornerSideOf(p: vec2f, a: vec2f, b: vec2f) -> f32 {
  let bax = (b.x - a.x);
  let bay = (b.y - a.y);
  let pax = (p.x - a.x);
  let pay = (p.y - a.y);
  return ((bax * pay) - (bay * pax));
}

fn cornerInsideTri(p: vec2f, a: vec2f, b: vec2f, c: vec2f) -> f32 {
  let d1 = cornerSideOf(p, a, b);
  let d2 = cornerSideOf(p, b, c);
  let d3 = cornerSideOf(p, c, a);
  let allPos = ((step(0f, d1) * step(0f, d2)) * step(0f, d3));
  let allNeg = ((step(d1, 0f) * step(d2, 0f)) * step(d3, 0f));
  return (allPos + allNeg);
}

fn cornerProjectToDiagonal(p: vec2f, a: vec2f, b: vec2f, refSide: f32) -> vec2f {
  let ab = (b - a);
  let t = clamp((dot((p - a), ab) / max(dot(ab, ab), 1e-8f)), 0f, 1f);
  let foot = (a + (ab * t));
  let perp = (vec2f((ab.y * -1f), ab.x) / max(length(ab), 1e-8f));
  let nudge = ((sign(refSide) * -1f) * 4e-3f);
  return (foot + (perp * nudge));
}

fn cornerConvexify(p: vec2f, a: vec2f, b: vec2f, o: vec2f) -> vec2f {
  let inside = (cornerInsideTri(p, a, b, o) > 0.5f);
  let proj = cornerProjectToDiagonal(p, a, b, cornerSideOf(o, a, b));
  return select(p, proj, inside);
}

fn cornerPinSafeDiv(n: f32, dv: f32) -> f32 {
  return (n / select(dv, 1e-8f, (abs(dv) < 1e-8f)));
}

fn cornerPinSample(coord: vec2f, amount: f32, c0: vec2f, c1: vec2f, c2: vec2f, c3: vec2f) -> vec3f {
  let amt = vec2f(amount, amount);
  let r0 = mix(vec2f(), vec2f(c0.x, (1f - c0.y)), amt);
  let r1 = mix(vec2f(1, 0), vec2f(c1.x, (1f - c1.y)), amt);
  let r2 = mix(vec2f(1), vec2f(c2.x, (1f - c2.y)), amt);
  let r3 = mix(vec2f(0, 1), vec2f(c3.x, (1f - c3.y)), amt);
  let q0 = cornerConvexify(r0, r1, r3, r2);
  let q1 = cornerConvexify(r1, r0, r2, r3);
  let q2 = cornerConvexify(r2, r1, r3, r0);
  let q3 = cornerConvexify(r3, r0, r2, r1);
  let dx1 = (q1.x - q2.x);
  let dx2 = (q3.x - q2.x);
  let dx3 = (((q0.x - q1.x) + q2.x) - q3.x);
  let dy1 = (q1.y - q2.y);
  let dy2 = (q3.y - q2.y);
  let dy3 = (((q0.y - q1.y) + q2.y) - q3.y);
  let denA = ((dx1 * dy2) - (dx2 * dy1));
  let a13 = cornerPinSafeDiv(((dx3 * dy2) - (dx2 * dy3)), denA);
  let a23 = cornerPinSafeDiv(((dx1 * dy3) - (dx3 * dy1)), denA);
  let a11 = ((q1.x - q0.x) + (a13 * q1.x));
  let a21 = ((q3.x - q0.x) + (a23 * q3.x));
  let a31 = q0.x;
  let a12 = ((q1.y - q0.y) + (a13 * q1.y));
  let a22 = ((q3.y - q0.y) + (a23 * q3.y));
  let a32 = q0.y;
  let c00 = (a22 - (a32 * a23));
  let c10 = ((a21 - (a31 * a23)) * -1f);
  let c20 = ((a21 * a32) - (a31 * a22));
  let c01 = ((a12 - (a32 * a13)) * -1f);
  let c11 = (a11 - (a31 * a13));
  let c21 = (((a11 * a32) - (a31 * a12)) * -1f);
  let c02 = ((a12 * a23) - (a22 * a13));
  let c12 = (((a11 * a23) - (a21 * a13)) * -1f);
  let c22 = ((a11 * a22) - (a21 * a12));
  let px = coord.x;
  let py = coord.y;
  let den = (((c02 * px) + (c12 * py)) + c22);
  let su = cornerPinSafeDiv((((c00 * px) + (c10 * py)) + c20), den);
  let sv = cornerPinSafeDiv((((c01 * px) + (c11 * py)) + c21), den);
  let det = (((a11 * c00) + (a21 * c01)) + (a31 * c02));
  let front = step(0f, (den * det));
  return vec3f(su, sv, front);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  topLeft: vec2f,
  topRight: vec2f,
  bottomLeft: vec2f,
  bottomRight: vec2f,
  amount: f32,
  edges: f32,
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
  let foldUV_1 = (cornerPinSample(uv, uniforms.n_n0.amount, uniforms.n_n0.topLeft, uniforms.n_n0.topRight, uniforms.n_n0.bottomRight, uniforms.n_n0.bottomLeft)).xy;
  let sibs_0 = mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, foldUV_1, uniforms._sys.viewportSize))));
  var composed_2 = vec4f(sibs_0.rgb, (sibs_0.a * ((1.0 * edgeTransparentMask((cornerPinSample(uv, uniforms.n_n0.amount, uniforms.n_n0.topLeft, uniforms.n_n0.topRight, uniforms.n_n0.bottomRight, uniforms.n_n0.bottomLeft)).xy)) * (cornerPinSample(uv, uniforms.n_n0.amount, uniforms.n_n0.topLeft, uniforms.n_n0.topRight, uniforms.n_n0.bottomRight, uniforms.n_n0.bottomLeft)).z)));
  return vec4f(linearToSrgb(tone_linear(composed_2.rgb)) * composed_2.a, composed_2.a);
}