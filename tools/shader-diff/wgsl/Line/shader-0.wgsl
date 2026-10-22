struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  color: vec4f,
  pointA: vec2f,
  pointB: vec2f,
  thickness: f32,
  style: f32,
  dashLength: f32,
  gapLength: f32,
  capStart: f32,
  capEnd: f32,
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

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn geom_aspectCorrectedUV(uv: vec2f, viewport: vec2f) -> vec2f {
  let aspect = (viewport.x / max(viewport.y, 1e-6f));
  return vec2f((uv.x * aspect), uv.y);
}

fn geom_aspectCentrePosition(pos: vec2f, aspect: f32) -> vec2f {
  return vec2f((pos.x * aspect), (1f - pos.y));
}

fn segmentArcPerp(p: vec2f, a: vec2f, b: vec2f) -> vec3f {
  let pax = (p.x - a.x);
  let pay = (p.y - a.y);
  let bax = (b.x - a.x);
  let bay = (b.y - a.y);
  let len = max(length(vec2f(bax, bay)), 9.999999747378752e-6f);
  let t = (((pax * bax) + (pay * bay)) / len);
  let overshoot = max(max(-(t), (t - len)), 0f);
  let h = clamp((t / len), 0f, 1f);
  let dSeg = length(vec2f((pax - (bax * h)), (pay - (bay * h))));
  let perp = sqrt(max(((dSeg * dSeg) - (overshoot * overshoot)), 0f));
  return vec3f(t, perp, len);
}

fn lineMask(uv: vec2f, viewport: vec2f, pointA: vec2f, pointB: vec2f, thickness: f32, style: f32, dashLength: f32, gapLength: f32, capStart: f32, capEnd: f32) -> f32 {
  let aspect = geom_aspectOf(viewport);
  let p = geom_aspectCorrectedUV(uv, viewport);
  let a = geom_aspectCentrePosition(pointA, aspect);
  let b = geom_aspectCentrePosition(pointB, aspect);
  let seg = segmentArcPerp(p, a, b);
  let t = seg.x;
  let perp = seg.y;
  let len = seg.z;
  let halfT = (thickness * 0.5f);
  let cSolid = (t - (len * 0.5f));
  let axSolid = (len * 0.5f);
  let rawDash = max(dashLength, 9.999999747378752e-6f);
  let rawPeriod = (rawDash + gapLength);
  let nDash = max(floor((((len + gapLength) / rawPeriod) + 0.5f)), 1f);
  let scale = (len / max(((nDash * rawDash) + ((nDash - 1f) * gapLength)), 9.999999747378752e-6f));
  let dashL = (rawDash * scale);
  let period = (rawPeriod * scale);
  let kDash = clamp(floor((t / period)), 0f, (nDash - 1f));
  let cDash = (t - ((kDash * period) + (dashL * 0.5f)));
  let axDashRound = max(((dashL * 0.5f) - halfT), 0f);
  let axDashSquare = (dashL * 0.5f);
  let rawDotPeriod = max((thickness + gapLength), 9.999999747378752e-6f);
  let nDot = max(floor(((len / rawDotPeriod) + 0.5f)), 1f);
  let dotPeriod = (len / nDot);
  let kDot = clamp(floor(((t / dotPeriod) + 0.5f)), 0f, nDot);
  let cDot = (t - (kDot * dotPeriod));
  let isPattern = (style >= 0.5f);
  let isDotted = (style >= 1.5f);
  let c = select(cSolid, select(cDash, cDot, isDotted), isPattern);
  let axRound = select(axSolid, select(axDashRound, 0f, isDotted), isPattern);
  let axSquare = select(axSolid, select(axDashSquare, halfT, isDotted), isPattern);
  let capHere = select(capStart, capEnd, (c > 0f));
  let sdRound = (length(vec2f(max((abs(c) - axRound), 0f), perp)) - halfT);
  let qx = (abs(c) - axSquare);
  let qy = (perp - halfT);
  let sdSquare = (length(vec2f(max(qx, 0f), max(qy, 0f))) + min(max(qx, qy), 0f));
  let sd = select(sdSquare, sdRound, (capHere >= 0.5f));
  let feather = (1f / viewport.y);
  return (1f - smoothstep(-(feather), feather, sd));
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
  var composed_0 = vec4f(uniforms.n_n0.color.rgb, (uniforms.n_n0.color.a * lineMask(uv, uniforms._sys.viewportSize, uniforms.n_n0.pointA, uniforms.n_n0.pointB, uniforms.n_n0.thickness, uniforms.n_n0.style, uniforms.n_n0.dashLength, uniforms.n_n0.gapLength, uniforms.n_n0.capStart, uniforms.n_n0.capEnd)));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}