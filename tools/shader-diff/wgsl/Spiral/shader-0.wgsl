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
  colorA: vec4f,
  colorB: vec4f,
  strokeWidth: f32,
  strokeFalloff: f32,
  softness: f32,
  speed: f32,
  center: vec2f,
  scale: f32,
  colorSpace: f32,
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

fn spiralOffset(center: vec2f, scale: f32, animTime: f32, uv: vec2f, viewport: vec2f) -> f32 {
  let aspect = (viewport.x / viewport.y);
  let dx = ((uv.x * aspect) - (center.x * aspect));
  let dy = (uv.y - (1f - center.y));
  let l = length(vec2f(dx, dy));
  let angle = (atan2(dy, dx) - animTime);
  return ((l * scale) + (angle / 6.283185307179586f));
}

fn spiralMask(center: vec2f, scale: f32, strokeWidth: f32, strokeFalloff: f32, softness: f32, animTime: f32, uv: vec2f, viewport: vec2f) -> f32 {
  let aspect = (viewport.x / viewport.y);
  let dx = ((uv.x * aspect) - (center.x * aspect));
  let dy = (uv.y - (1f - center.y));
  let l = length(vec2f(dx, dy));
  let offset = spiralOffset(center, scale, animTime, uv, viewport);
  let stripe = fract(offset);
  let shape = (abs((stripe - 0.5f)) * 2f);
  let baseWidth = clamp(strokeWidth, (strokeFalloff * 5e-3f), 1f);
  let falloffAmount = (1f - (clamp(strokeFalloff, 0f, 1f) * l));
  let width = (baseWidth * falloffAmount);
  let fw = fwidth(offset);
  let fwMult = (4f - ((smoothstep(0.05f, 0.4f, (strokeWidth * 2f)) * smoothstep(0.05f, 0.4f, ((1f - strokeWidth) * 2f))) * 3f));
  let pixelSize = mix((fwMult * fw), fwidth(shape), clamp(fw, 0f, 1f));
  return smoothstep(((width - pixelSize) - softness), ((width + pixelSize) + softness), shape);
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
  var composed_0 = mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, spiralMask(uniforms.n_n0.center, uniforms.n_n0.scale, uniforms.n_n0.strokeWidth, uniforms.n_n0.strokeFalloff, uniforms.n_n0.softness, uniforms.n_n0._animTime, uv, uniforms._sys.viewportSize));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}