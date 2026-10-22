struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  color: vec4f,
  keyColor: vec4f,
  keyIntensity: f32,
  keySoftness: f32,
  fillColor: vec4f,
  fillIntensity: f32,
  fillSoftness: f32,
  fillAngle: f32,
  backColor: vec4f,
  backIntensity: f32,
  backSoftness: f32,
  brightness: f32,
  vignette: f32,
  center: vec2f,
  lightTarget: f32,
  wallCurvature: f32,
  ambientIntensity: f32,
  ambientSpeed: f32,
  seed: f32,
  _opacity: f32,
  ambPosA: vec4f,
  ambPosB: vec4f,
  ambSizeSq: vec4f,
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

fn lightfields_anisotropicGaussianSpot(du: f32, dv: f32, sharpness: f32) -> f32 {
  return exp((((du * du) + (dv * dv)) * -(sharpness)));
}

fn glowAt(uv: vec2f, aspect: f32, lx: f32, ly: f32, sizeSq: f32) -> f32 {
  let dx = ((uv.x - lx) * aspect);
  let dy = (uv.y - ly);
  let dd2 = ((dx * dx) + (dy * dy));
  return exp(((-0.5f * dd2) / sizeSq));
}

fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

fn vignetteMask(uv: vec2f, viewport: vec2f, strength: f32) -> f32 {
  let aspect = geom_aspectOf(viewport);
  let vigX = ((uv.x - 0.5f) * aspect);
  let vigY = (uv.y - 0.5f);
  let vigD2 = ((vigX * vigX) + (vigY * vigY));
  return clamp((1f - (vigD2 * strength)), 0f, 1f);
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
  let coveCenterY_3 = (1.0 - uniforms.n_n0.center.y);
  let coveWallY_2 = clamp((uv.y / max(coveCenterY_3, 0.000001)), 0.0, 1.0);
  let sbCurvature_6 = (uniforms.n_n0.wallCurvature * 0.01);
  let coveTransition_5 = mix(0.015, 0.15, sbCurvature_6);
  let coveIsFloor_4 = smoothstep((coveCenterY_3 - coveTransition_5), (coveCenterY_3 + coveTransition_5), uv.y);
  let sbBrightness_7 = (uniforms.n_n0.brightness * 0.02);
  let sbBase_1 = ((mix(mix(0.2, 0.4, coveWallY_2), mix(0.38, 0.18, clamp(((uv.y - coveCenterY_3) / max((1.0 - coveCenterY_3), 0.000001)), 0.0, 1.0)), coveIsFloor_4) * sbBrightness_7) + (((exp(((-0.5 * ((uv.y - coveCenterY_3) / (coveTransition_5 + 0.001))) * ((uv.y - coveCenterY_3) / (coveTransition_5 + 0.001)))) * sbCurvature_6) * sbBrightness_7) * 0.1));
  let sbLight0_0 = vec3f(sbBase_1, sbBase_1, sbBase_1);
  let coveCenterX_9 = uniforms.n_n0.center.x;
  let coveWallY2_10 = (coveWallY_2 * coveWallY_2);
  let aspect_11 = (uniforms._sys.viewportSize.x / max(uniforms._sys.viewportSize.y, 0.000001));
  let coveCone_12 = mix(0.05, 0.45, coveWallY2_10);
  let coveWallFall_13 = (((1.0 - coveWallY_2) * 1.2) * ((1.0 - coveWallY_2) * 1.2));
  let coveNotFloor_14 = (1.0 - coveIsFloor_4);
  let sbTarget_16 = (uniforms.n_n0.lightTarget * 0.01);
  let sbWallBias_15 = (((1.0 - sbTarget_16) * 1.4) + 0.3);
  let coveFloorFall_17 = (((uv.y - (coveCenterY_3 + ((1.0 - coveCenterY_3) * 0.3))) * 4.0) * ((uv.y - (coveCenterY_3 + ((1.0 - coveCenterY_3) * 0.3))) * 4.0));
  let sbFloorBias_18 = ((sbTarget_16 * 1.4) + 0.3);
  let sbKey_8 = (((exp(((-((((((uv.x - mix(coveCenterX_9, coveCenterX_9, coveWallY2_10)) * aspect_11) / coveCone_12) * (((uv.x - mix(coveCenterX_9, coveCenterX_9, coveWallY2_10)) * aspect_11) / coveCone_12)) + coveWallFall_13))) * mix(5.0, 0.8, (uniforms.n_n0.keySoftness * 0.01)))) * coveNotFloor_14) * sbWallBias_15) + ((exp(((-((((((uv.x - mix(coveCenterX_9, coveCenterX_9, 0.25)) * aspect_11) * 1.4) * (((uv.x - mix(coveCenterX_9, coveCenterX_9, 0.25)) * aspect_11) * 1.4)) + coveFloorFall_17))) * (mix(5.0, 0.8, (uniforms.n_n0.keySoftness * 0.01)) * 1.5))) * coveIsFloor_4) * sbFloorBias_18));
  let sbKeyInt_19 = (uniforms.n_n0.keyIntensity * 0.015);
  let sbFillL_20 = (((exp(((-((((((uv.x - mix((coveCenterX_9 - (uniforms.n_n0.fillAngle * 0.005)), coveCenterX_9, coveWallY2_10)) * aspect_11) / coveCone_12) * (((uv.x - mix((coveCenterX_9 - (uniforms.n_n0.fillAngle * 0.005)), coveCenterX_9, coveWallY2_10)) * aspect_11) / coveCone_12)) + coveWallFall_13))) * mix(5.0, 0.8, (uniforms.n_n0.fillSoftness * 0.01)))) * coveNotFloor_14) * sbWallBias_15) + ((exp(((-((((((uv.x - mix(coveCenterX_9, (coveCenterX_9 - (uniforms.n_n0.fillAngle * 0.005)), 0.25)) * aspect_11) * 1.4) * (((uv.x - mix(coveCenterX_9, (coveCenterX_9 - (uniforms.n_n0.fillAngle * 0.005)), 0.25)) * aspect_11) * 1.4)) + coveFloorFall_17))) * (mix(5.0, 0.8, (uniforms.n_n0.fillSoftness * 0.01)) * 1.5))) * coveIsFloor_4) * sbFloorBias_18));
  let sbFillInt_21 = (uniforms.n_n0.fillIntensity * 0.015);
  let sbFillR_22 = (((exp(((-((((((uv.x - mix((coveCenterX_9 + (uniforms.n_n0.fillAngle * 0.005)), coveCenterX_9, coveWallY2_10)) * aspect_11) / coveCone_12) * (((uv.x - mix((coveCenterX_9 + (uniforms.n_n0.fillAngle * 0.005)), coveCenterX_9, coveWallY2_10)) * aspect_11) / coveCone_12)) + coveWallFall_13))) * mix(5.0, 0.8, (uniforms.n_n0.fillSoftness * 0.01)))) * coveNotFloor_14) * sbWallBias_15) + ((exp(((-((((((uv.x - mix(coveCenterX_9, (coveCenterX_9 + (uniforms.n_n0.fillAngle * 0.005)), 0.25)) * aspect_11) * 1.4) * (((uv.x - mix(coveCenterX_9, (coveCenterX_9 + (uniforms.n_n0.fillAngle * 0.005)), 0.25)) * aspect_11) * 1.4)) + coveFloorFall_17))) * (mix(5.0, 0.8, (uniforms.n_n0.fillSoftness * 0.01)) * 1.5))) * coveIsFloor_4) * sbFloorBias_18));
  let coveX_23 = ((uv.x - coveCenterX_9) * aspect_11);
  let sbBackDecay_24 = mix(5.0, 0.8, (uniforms.n_n0.backSoftness * 0.01));
  var composed_25 = vec4f(clamp(((uniforms.n_n0.color.rgb * clamp((((((sbLight0_0 + (uniforms.n_n0.keyColor.rgb * (sbKey_8 * sbKeyInt_19))) + (uniforms.n_n0.fillColor.rgb * ((sbFillL_20 * sbFillInt_21) + (sbFillR_22 * sbFillInt_21)))) + (uniforms.n_n0.backColor.rgb * (((lightfields_anisotropicGaussianSpot((coveX_23 * 0.5), ((uv.y - mix(1.0, coveCenterY_3, 0.15)) * 4.0), sbBackDecay_24) * coveIsFloor_4) + ((lightfields_anisotropicGaussianSpot((coveX_23 * 0.6), ((coveCenterY_3 - uv.y) * 3.0), (sbBackDecay_24 * 0.8)) * coveNotFloor_14) * coveWallY2_10)) * (uniforms.n_n0.backIntensity * 0.015)))) + vec3f(((lightfields_anisotropicGaussianSpot((coveX_23 * 0.7), (((uv.y - coveCenterY_3) + 0.02) * 10.0), 2.5) * (sbKeyInt_19 * 0.12)) * coveNotFloor_14))) + vec3f(((((glowAt(uv, aspect_11, (coveCenterX_9 + uniforms.n_n0.ambPosA.x), ((coveCenterY_3 - 0.2) + uniforms.n_n0.ambPosA.y), uniforms.n_n0.ambSizeSq.x) + glowAt(uv, aspect_11, (coveCenterX_9 + uniforms.n_n0.ambPosA.z), ((coveCenterY_3 - 0.2) + uniforms.n_n0.ambPosA.w), uniforms.n_n0.ambSizeSq.y)) + glowAt(uv, aspect_11, (coveCenterX_9 + uniforms.n_n0.ambPosB.x), ((coveCenterY_3 - 0.2) + uniforms.n_n0.ambPosB.y), uniforms.n_n0.ambSizeSq.z)) + glowAt(uv, aspect_11, (coveCenterX_9 + uniforms.n_n0.ambPosB.z), ((coveCenterY_3 - 0.2) + uniforms.n_n0.ambPosB.w), uniforms.n_n0.ambSizeSq.w)) * ((uniforms.n_n0.ambientIntensity * 0.012) * 0.25)))), vec3f(0.0), vec3f(3.0))) * vignetteMask(uv, uniforms._sys.viewportSize, (uniforms.n_n0.vignette * 0.025))), vec3f(0.0), vec3f(1.0)), 1.0);
  return vec4f(linearToSrgb(tone_linear(composed_25.rgb)) * composed_25.a, composed_25.a);
}