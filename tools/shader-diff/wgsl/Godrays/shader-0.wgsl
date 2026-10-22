fn coverageOver(rayColor: vec4f, bgColor: vec4f, coverage: f32) -> vec4f {
  let rayA = (coverage * rayColor.w);
  let finalAlpha = (rayA + (bgColor.w * (1f - rayA)));
  let safeAlpha = clamp(finalAlpha, 1e-3f, 1f);
  let rayContribution = (rayColor.xyz * rayA);
  let bgContribution = ((bgColor.xyz * bgColor.w) * (1f - rayA));
  let finalColor = ((rayContribution + bgContribution) / safeAlpha);
  return vec4f(finalColor, finalAlpha);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  center: vec2f,
  density: f32,
  intensity: f32,
  spotty: f32,
  speed: f32,
  rayColor: vec4f,
  backgroundColor: vec4f,
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

fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }

fn hash12(p: vec2f) -> f32 {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  return (f32(((u.x ^ u.y) * 3141592653u)) / 4294967296f);
}

fn godraysHash(p: vec2f) -> f32 {
  return hash12(p);
}

fn godraysValueNoise(st: vec2f) -> f32 {
  let i = floor(st);
  let f = fract(st);
  let a = godraysHash(i);
  let b = godraysHash(vec2f((i.x + 1f), i.y));
  let c = godraysHash(vec2f(i.x, (i.y + 1f)));
  let e = godraysHash(vec2f((i.x + 1f), (i.y + 1f)));
  let u = ((f * f) * ((f * -2f) + 3f));
  let x1 = mix(a, b, u.x);
  let x2 = mix(c, e, u.x);
  return mix(x1, x2, u.y);
}

fn godraysRaysShape(a: f32, aWrapped: f32, blend: f32, r: f32, freq: f32, intensity: f32) -> f32 {
  let nLeft = pow(godraysValueNoise(vec2f((a * freq), r)), intensity);
  let nRight = pow(godraysValueNoise(vec2f((aWrapped * freq), r)), intensity);
  return mix(nRight, nLeft, blend);
}

fn godraysRayLayer(frame: vec4f, animTime: f32, spots: f32, intensityCalc: f32, dens: f32, sched: vec4f, freqs: vec2f) -> f32 {
  let rA = ((frame.w * sched.x) - (animTime * sched.y));
  let rB = (((frame.w * sched.z) * (1f + spots)) - (animTime * sched.w));
  let f = (dens * freqs.x);
  let shapeA = godraysRaysShape(frame.x, frame.y, frame.z, rA, f, intensityCalc);
  let shapeB = godraysRaysShape(frame.x, frame.y, frame.z, rB, (f * freqs.y), intensityCalc);
  return (shapeA * shapeB);
}

fn godraysRayStack(frame: vec4f, animTimeRaw: f32, density: f32, intensity: f32, spotty: f32) -> f32 {
  let animTime = (animTimeRaw * 0.2f);
  let spots = (6.5f * abs(spotty));
  let intensityCalc = (4f - (3f * clamp(intensity, 0f, 1f)));
  let dens = (6f * density);
  let layer1 = godraysRayLayer(frame, animTime, spots, intensityCalc, dens, vec4f(1, 3, 0.5, 2), vec2f(5, 4));
  let layer2 = godraysRayLayer(frame, animTime, spots, intensityCalc, dens, vec4f(1.399999976158142, 2.5, 0.699999988079071, 1.7999999523162842), vec2f(4.5, 3.5));
  return clamp((layer1 + (layer2 * 0.7f)), 0f, 1f);
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn lightfields_seamlessAngularField(delta: vec2f) -> vec3f {
  let angle = atan2(delta.y, delta.x);
  let angleWrapped = (angle - (6.283185307179586f * floor((angle / 6.283185307179586f))));
  let blend = smoothstep(-0.15f, 0.15f, delta.x);
  return vec3f(angle, angleWrapped, blend);
}

fn godraysFrame(uv: vec2f, viewport: vec2f, center: vec2f) -> vec4f {
  let aspect = geom_aspectOf(viewport);
  let centerPos = vec2f(center.x, (1f - center.y));
  let delta = (uv - centerPos);
  let shapeUV = vec2f((delta.x * aspect), delta.y);
  let ang = lightfields_seamlessAngularField(shapeUV);
  return vec4f(ang.x, ang.y, ang.z, length(shapeUV));
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
  var composed_0 = coverageOver(uniforms.n_n0.rayColor, uniforms.n_n0.backgroundColor, godraysRayStack(godraysFrame(uv, uniforms._sys.viewportSize, uniforms.n_n0.center), uniforms.n_n0._animTime, uniforms.n_n0.density, uniforms.n_n0.intensity, uniforms.n_n0.spotty));
  return vec4f(linearToSrgb(tone_linear(composed_0.rgb)) * composed_0.a, composed_0.a);
}