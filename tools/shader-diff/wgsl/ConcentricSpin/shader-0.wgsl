fn edgeMirrorUV(uv: vec2f) -> vec2f {
  let mirrorX = (abs(uv.x) % 2f);
  let mirrorY = (abs(uv.y) % 2f);
  return vec2f(select(mirrorX, (2f - mirrorX), (mirrorX >= 1f)), select(mirrorY, (2f - mirrorY), (mirrorY >= 1f)));
}

fn legacySinHash11(x: f32) -> f32 {
  return ((fract((sin((x * 12.9898f)) * 43758.5453f)) - 0.5f) * 2f);
}

fn csRotateUV(uv: vec2f, aspect: f32, center: vec2f, angle: f32) -> vec2f {
  let centerPos = vec2f(center.x, (1f - center.y));
  let dx = ((uv.x - centerPos.x) * aspect);
  let dy = (uv.y - centerPos.y);
  let cosA = cos(angle);
  let sinA = sin(angle);
  let rotX = ((dx * cosA) - (dy * sinA));
  let rotY = ((dx * sinA) + (dy * cosA));
  return vec2f(((rotX / aspect) + centerPos.x), (rotY + centerPos.y));
}

fn concentricSpinUV(center: vec2f, intensity: f32, rings: f32, smoothness: f32, seed: f32, speedRandomness: f32, animTime: f32, uv: vec2f, aspect: f32) -> vec2f {
  let centerPos = vec2f(center.x, (1f - center.y));
  let dx = ((uv.x - centerPos.x) * aspect);
  let dy = (uv.y - centerPos.y);
  let dist = length(vec2f(dx, dy));
  let ringCoord = (dist * rings);
  let ringIndex = floor(ringCoord);
  let ringFrac = fract(ringCoord);
  let maxAngle = (intensity * 0.017453292519943295f);
  let seedOffset = (seed * 0.137f);
  let staticA = (legacySinHash11((ringIndex + seedOffset)) * maxAngle);
  let staticB = (legacySinHash11(((ringIndex + 1f) + seedOffset)) * maxAngle);
  let ringSpeedA = mix(1f, legacySinHash11((ringIndex + 42.7f)), speedRandomness);
  let ringSpeedB = mix(1f, legacySinHash11(((ringIndex + 1f) + 42.7f)), speedRandomness);
  let animA = ((animTime * 0.25f) * ringSpeedA);
  let animB = ((animTime * 0.25f) * ringSpeedB);
  let totalA = (staticA + animA);
  let totalB = (staticB + animB);
  let wrapArg = ((totalB - totalA) + 3.141592653589793f);
  let diff = ((wrapArg - (6.283185307179586f * floor((wrapArg / 6.283185307179586f)))) - 3.141592653589793f);
  let halfEdge = ((smoothness * 0.5f) + 1e-3f);
  let blend = smoothstep((0.5f - halfEdge), (0.5f + halfEdge), ringFrac);
  let angle = (totalA + (diff * blend));
  return csRotateUV(uv, aspect, center, angle);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  intensity: f32,
  rings: f32,
  smoothness: f32,
  seed: f32,
  speed: f32,
  speedRandomness: f32,
  edges: f32,
  center: vec2f,
  _opacity: f32,
  _animTime: f32,
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
  let foldUV_0 = edgeMirrorUV(concentricSpinUV(uniforms.n_n0.center, uniforms.n_n0.intensity, uniforms.n_n0.rings, uniforms.n_n0.smoothness, uniforms.n_n0.seed, uniforms.n_n0.speedRandomness, uniforms.n_n0._animTime, uv, uniforms._sys.aspect));
  var composed_1 = mixColors(uniforms.n_n0c0.colorA, uniforms.n_n0c0.colorB, gradientClamp01(gradientClamp01(gradientRawT(uniforms.n_n0c0.start, uniforms.n_n0c0.end, uniforms.n_n0c0.angle, foldUV_0, uniforms._sys.viewportSize))));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}