struct padded {
  _opacity: f32,
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
  size: f32,
  deformation: f32,
  softness: f32,
  highlightIntensity: f32,
  highlightX: f32,
  highlightY: f32,
  highlightZ: f32,
  highlightColor: vec4f,
  speed: f32,
  seed: f32,
  center: vec2f,
  colorSpace: f32,
  _opacity: f32,
  _animTime: f32,
  normLx: f32,
  normLy: f32,
  normLz: f32,
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
  let aspect_1 = (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y);
  let centred_0 = vec2f(((uv.x * aspect_1) - (uniforms.n_n0.center.x * aspect_1)), (uv.y - (1.0 - uniforms.n_n0.center.y)));
  let blobTime_2 = (uniforms.n_n0._animTime + uniforms.n_n0.seed);
  let blobRadius_4 = (uniforms.n_n0.size + (((((((sin(((centred_0.x * 3.2) + (blobTime_2 * 0.8))) * sin(((centred_0.y * 2.8) + (blobTime_2 * 0.6)))) + sin((((centred_0.x * 4.8) + (centred_0.y * -3.6)) + (blobTime_2 * -0.4)))) * 0.15) * uniforms.n_n0.deformation) + (((sin(((centred_0.x * 5.6) + (blobTime_2 * -0.5))) * sin(((centred_0.y * 4.4) + (blobTime_2 * 0.7)))) * 0.12) * uniforms.n_n0.deformation)) + (((sin((((centred_0.x * 7.2) + (centred_0.y * 6.4)) + (blobTime_2 * 0.3))) + sin(((centred_0.x * 2.4) + (blobTime_2 * -0.9)))) * 0.1) * uniforms.n_n0.deformation)) + (((sin(((centred_0.x * 8.8) + (blobTime_2 * 0.2))) * sin(((centred_0.y * 7.6) + (blobTime_2 * -0.8)))) * 0.04) * uniforms.n_n0.deformation)));
  let centredDist_5 = length(centred_0);
  let domeSlope_3 = smoothstep(0.0, blobRadius_4, centredDist_5);
  let blobSoftness_7 = uniforms.n_n0.softness;
  let blobMask_6 = pow((1.0 - smoothstep((blobRadius_4 - (blobSoftness_7 * 0.3)), (blobRadius_4 + (blobSoftness_7 * 0.3)), centredDist_5)), ((blobSoftness_7 * 2.0) + 0.5));
  var composed_8 = vec4f(((mixColors(uniforms.n_n0.colorA, uniforms.n_n0.colorB, smoothstep(0.1, 0.9, ((((((sin(((centred_0.x * 3.0) + (blobTime_2 * 0.4))) * sin(((centred_0.y * 3.0) + (blobTime_2 * 0.3)))) + sin(((((centred_0.x * 3.0) * 0.7) + ((centred_0.y * 3.0) * 0.8)) + (blobTime_2 * 0.2)))) * 0.5) + 0.5) * 0.6) + (((((sin((((centred_0.x * 3.0) * 1.3) + (blobTime_2 * -0.5))) * sin((((centred_0.y * 3.0) * 1.1) + (blobTime_2 * 0.6)))) + sin(((((centred_0.x * 3.0) * 0.5) + ((centred_0.y * 3.0) * -0.6)) + (blobTime_2 * 0.3)))) * 0.5) + 0.5) * 0.4))))).rgb + (uniforms.n_n0.highlightColor.rgb * (((pow(max(0.0, (reflect((vec3f(uniforms.n_n0.normLx, uniforms.n_n0.normLy, uniforms.n_n0.normLz) * -1.0), normalize(vec3f((centred_0.x * (domeSlope_3 + 0.2)), (centred_0.y * (domeSlope_3 + 0.2)), (1.0 - (domeSlope_3 * 0.1)))))).z), 32.0) * ((smoothstep(0.0, blobRadius_4, centredDist_5) * 0.5) + 0.5)) * uniforms.n_n0.highlightIntensity) * blobMask_6))), (((uniforms.n_n0.colorA.a + uniforms.n_n0.colorB.a) * 0.5) * blobMask_6));
  return vec4f(linearToSrgb(tone_linear(composed_8.rgb)) * composed_8.a, composed_8.a);
}