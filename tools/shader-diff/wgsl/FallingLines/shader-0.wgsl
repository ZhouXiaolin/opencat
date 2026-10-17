fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn colHash(idx: f32) -> vec2f {
  return vec2f(fract((sin((idx * 127.1f)) * 43758.5453123f)), fract((sin((idx * 311.7f)) * 38291.3421f)));
}

fn fallingCapAlpha(vPeriodic: f32, trailLen: f32, density: f32, hw: f32, rounding: f32, fwU: f32, distFromCenter: f32) -> f32 {
  let maxDepth = (hw * rounding);
  let gapDist = (max((vPeriodic - trailLen), 0f) * density);
  let tcap = clamp((gapDist / max(maxDepth, 1e-4f)), 0f, 1f);
  let capHW = (hw * sqrt(max((1f - (tcap * tcap)), 0f)));
  return ((1f - smoothstep(max((capHW - fwU), 0f), max((capHW + fwU), 1e-3f), distFromCenter)) * smoothstep(0f, fwU, capHW));
}

fn fallingBalanceRemap(withinStreak: f32, balance: f32) -> f32 {
  let balanceNum = ((2f * balance) - 1f);
  let balanceDen = max((2f * (1f - balance)), 1e-3f);
  return clamp(((withinStreak + balanceNum) / balanceDen), 0f, 1f);
}

fn fallingLinesField(uv: vec2f, vp: vec2f, angle: f32, density: f32, speedVariance: f32, trailLength: f32, strokeWidth: f32, rounding: f32, balance: f32, animTime: f32) -> vec2f {
  let aspect = geom_aspectOf(vp);
  let corrUV = vec2f((uv.x * aspect), uv.y);
  let centeredUV = (corrUV - vec2f((aspect * 0.5f), 0.5f));
  let angleRad = (angle * 0.017453292519943295f);
  let cosA = cos(angleRad);
  let sinA = sin(angleRad);
  let u = ((centeredUV.y * cosA) - (centeredUV.x * sinA));
  let v = ((centeredUV.x * cosA) + (centeredUV.y * sinA));
  let colF = (u * density);
  let colIdx = floor(colF);
  let colLocalU = fract(colF);
  let h = colHash(colIdx);
  let speedMod = (1f + (((h.x - 0.5f) * speedVariance) * 2f));
  let phase = h.y;
  let animOffset = fract((animTime * speedMod));
  let vPeriodic = fract(((v - animOffset) - phase));
  let trailLen = trailLength;
  let withinStreak = clamp((vPeriodic / trailLen), 0f, 1f);
  let fwV = clamp(fwidth(v), 1e-3f, 0.1f);
  let fwU = clamp((fwidth(u) * density), 1e-3f, 0.1f);
  let streakMask = (1f - smoothstep((trailLen - fwV), (trailLen + fwV), vPeriodic));
  let hw = (strokeWidth * 0.5f);
  let distFromCenter = abs((colLocalU - 0.5f));
  let widthMask = (1f - smoothstep((hw - fwU), (hw + fwU), distFromCenter));
  let rectMask = (streakMask * widthMask);
  let capAlpha = fallingCapAlpha(vPeriodic, trailLen, density, hw, rounding, fwU, distFromCenter);
  let finalMask = max(rectMask, capAlpha);
  return vec2f(finalMask, fallingBalanceRemap(withinStreak, balance));
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
  colorSpace: f32,
  angle: f32,
  speed: f32,
  speedVariance: f32,
  density: f32,
  trailLength: f32,
  balance: f32,
  strokeWidth: f32,
  rounding: f32,
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
  let fallingField_0 = fallingLinesField(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.density, uniforms.n_n0.speedVariance, uniforms.n_n0.trailLength, uniforms.n_n0.strokeWidth, uniforms.n_n0.rounding, uniforms.n_n0.balance, uniforms.n_n0._animTime);
  var composed_1 = vec4f((mixColors(uniforms.n_n0.colorB, uniforms.n_n0.colorA, fallingField_0.y)).rgb, ((mixColors(uniforms.n_n0.colorB, uniforms.n_n0.colorA, fallingField_0.y)).a * fallingField_0.x));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}