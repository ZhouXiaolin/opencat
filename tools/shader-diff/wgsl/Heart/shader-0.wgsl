fn shapeLocalCoords(center: vec2f, rotation: f32, uv: vec2f, aspect: f32) -> vec2f {
  let aspectUV = vec2f((uv.x * aspect), uv.y);
  let centerPos = vec2f((center.x * aspect), (1f - center.y));
  let dx = (aspectUV.x - centerPos.x);
  let dy = (aspectUV.y - centerPos.y);
  let rotRad = (rotation * 0.017453292519943295f);
  let cosR = cos(rotRad);
  let sinR = sin(rotRad);
  let rdx = ((dx * cosR) + (dy * sinR));
  let rdy = ((dy * cosR) - (dx * sinR));
  return vec2f(rdx, rdy);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  color: vec4f,
  center: vec2f,
  radius: f32,
  rotation: f32,
  softness: f32,
  strokeThickness: f32,
  strokeColor: vec4f,
  strokePosition: f32,
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
  n_n0: struct_1,
  _sys: struct_2,
}

@group(0) @binding(0) var<uniform> uniforms: combined;

struct ShapeMask {
  overallMask: f32,
  strokeBlend: f32,
}

fn strokeMaskFromSdf(dist: f32, softness: f32, strokeThickness: f32, strokePosition: f32) -> ShapeMask {
  let hasStroke = (strokeThickness > 0f);
  let isInside = (strokePosition >= 1.5f);
  let isCenterOrAbove = (strokePosition >= 0.5f);
  let halfThickness = (strokeThickness * 0.5f);
  let negThickness = (strokeThickness * -1f);
  let negHalf = (halfThickness * -1f);
  let strokeInner = select(select(0f, negHalf, isCenterOrAbove), negThickness, isInside);
  let strokeOuter = select(select(strokeThickness, halfThickness, isCenterOrAbove), 0f, isInside);
  let overallMask = (1f - smoothstep((strokeOuter - softness), strokeOuter, dist));
  let blend = smoothstep((strokeInner - softness), strokeInner, dist);
  let strokeBlend = select(0f, blend, hasStroke);
  return ShapeMask(overallMask, strokeBlend);
}

fn heartSdf(dx: f32, dy: f32, radius: f32) -> f32 {
  let S = (radius / 0.75f);
  let px = (abs(dx) / S);
  let py = (((dy * -1f) / S) + 0.6f);
  let ax = (px - 0.25f);
  let ay = (py - 0.75f);
  let distA = (sqrt(((ax * ax) + (ay * ay))) - 0.35355339059327373f);
  let b1y = (py - 1f);
  let dot1 = ((px * px) + (b1y * b1y));
  let m = (max((px + py), 0f) * 0.5f);
  let b2x = (px - m);
  let b2y = (py - m);
  let dot2v = ((b2x * b2x) + (b2y * b2y));
  let distB = (sqrt(min(dot1, dot2v)) * sign((px - py)));
  return (select(distB, distA, ((px + py) > 1f)) * S);
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
  let shapeLocal_1 = shapeLocalCoords(uniforms.n_n0.center, uniforms.n_n0.rotation, uv, uniforms._sys.aspect);
  let shapeMask_0 = strokeMaskFromSdf(heartSdf(shapeLocal_1.x, shapeLocal_1.y, uniforms.n_n0.radius), uniforms.n_n0.softness, uniforms.n_n0.strokeThickness, uniforms.n_n0.strokePosition);
  var composed_2 = vec4f((mixColors(uniforms.n_n0.color, uniforms.n_n0.strokeColor, shapeMask_0.strokeBlend)).rgb, ((mixColors(uniforms.n_n0.color, uniforms.n_n0.strokeColor, shapeMask_0.strokeBlend)).a * shapeMask_0.overallMask));
  return vec4f(linearToSrgb(tone_linear(composed_2.rgb)) * composed_2.a, composed_2.a);
}