fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn lightfields_pointToSegment(p: vec2f, a: vec2f, b: vec2f) -> vec2f {
  let lineVec = vec2f((b.x - a.x), (b.y - a.y));
  let toPoint = vec2f((p.x - a.x), (p.y - a.y));
  let dotProduct = ((toPoint.x * lineVec.x) + (toPoint.y * lineVec.y));
  let lineLengthSq = ((lineVec.x * lineVec.x) + (lineVec.y * lineVec.y));
  let t = clamp((dotProduct / max(lineLengthSq, 1e-4f)), 0f, 1f);
  let closestPoint = vec2f((a.x + (lineVec.x * t)), (a.y + (lineVec.y * t)));
  let distVec = vec2f((p.x - closestPoint.x), (p.y - closestPoint.y));
  return vec2f(t, sqrt(((distVec.x * distVec.x) + (distVec.y * distVec.y))));
}

fn lightfields_taperedSegmentGlow(dist: f32, t: f32, startThickness: f32, endThickness: f32, startSoftness: f32, endSoftness: f32) -> vec2f {
  let thickness = mix(startThickness, endThickness, t);
  let softness = max(mix(startSoftness, endSoftness, t), 1e-4f);
  let normalizedDist = (dist / max(thickness, 1e-4f));
  let edgeEnd = (1f + softness);
  let alphaBase = (1f - smoothstep(1f, edgeEnd, normalizedDist));
  let softnessFactor = (1f + (softness * 1.5f));
  let alpha = pow(alphaBase, softnessFactor);
  let colorGradientStart = (1f - softness);
  let colorGradientEnd = (1f + softness);
  let colorT = smoothstep(colorGradientStart, colorGradientEnd, normalizedDist);
  return vec2f(colorT, alpha);
}

fn beamField(uv: vec2f, vp: vec2f, start: vec2f, end: vec2f, startThickness: f32, endThickness: f32, startSoftness: f32, endSoftness: f32) -> vec2f {
  let aspect = geom_aspectOf(vp);
  let startPos = vec2f(start.x, (1f - start.y));
  let endPos = vec2f(end.x, (1f - end.y));
  let acStart = vec2f((startPos.x * aspect), startPos.y);
  let acEnd = vec2f((endPos.x * aspect), endPos.y);
  let acUV = vec2f((uv.x * aspect), uv.y);
  let proj = lightfields_pointToSegment(acUV, acStart, acEnd);
  let field = lightfields_taperedSegmentGlow(proj.y, proj.x, (startThickness * 0.25f), (endThickness * 0.25f), startSoftness, endSoftness);
  let colorT = field.x;
  let alpha = field.y;
  return vec2f(colorT, alpha);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  startPosition: vec2f,
  endPosition: vec2f,
  startThickness: f32,
  endThickness: f32,
  startSoftness: f32,
  endSoftness: f32,
  insideColor: vec4f,
  outsideColor: vec4f,
  colorSpace: f32,
  _opacity: f32,
  convA: vec3f,
  convB: vec3f,
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
  let beamField_0 = beamField(uv, uniforms._sys.viewportSize, uniforms.n_n0.startPosition, uniforms.n_n0.endPosition, uniforms.n_n0.startThickness, uniforms.n_n0.endThickness, uniforms.n_n0.startSoftness, uniforms.n_n0.endSoftness);
  var composed_1 = vec4f((mixColors(uniforms.n_n0.insideColor, uniforms.n_n0.outsideColor, beamField_0.x)).rgb, (((uniforms.n_n0.insideColor.a + uniforms.n_n0.outsideColor.a) * 0.5) * beamField_0.y));
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}