struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  position: vec2f,
  beamWidth: f32,
  intensity: f32,
  beamColor: vec4f,
  startFalloff: f32,
  endFalloff: f32,
  splitPosition: vec2f,
  spread: f32,
  softness: f32,
  saturation: f32,
  speed: f32,
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
  let aspect_5 = (uniforms._sys.viewportSize.x / uniforms._sys.viewportSize.y);
  let rel_4 = vec2f(((uv.x * aspect_5) - (uniforms.n_n0.position.x * aspect_5)), (uv.y - (1.0 - uniforms.n_n0.position.y)));
  let toRel_7 = vec2f(((uniforms.n_n0.splitPosition.x * aspect_5) - (uniforms.n_n0.position.x * aspect_5)), ((1.0 - uniforms.n_n0.splitPosition.y) - (1.0 - uniforms.n_n0.position.y)));
  let segLen_8 = max(length(toRel_7), 0.0001);
  let segDir_6 = (toRel_7 / segLen_8);
  let across_3 = ((rel_4.x * (-(segDir_6.y))) + (rel_4.y * segDir_6.x));
  let along_11 = dot(rel_4, segDir_6);
  let sp_10 = (along_11 - segLen_8);
  let soft_12 = max(uniforms.n_n0.softness, 0.001);
  let past_9 = ((sp_10 + sqrt(((sp_10 * sp_10) + ((soft_12 * 0.6) * (soft_12 * 0.6))))) * 0.5);
  let an_2 = (across_3 / max((uniforms.n_n0.beamWidth + (past_9 * uniforms.n_n0.spread)), 0.001));
  let hue6_1 = (fract(((an_2 * 0.42) + uniforms.n_n0._animTime)) * 6.0);
  let longEnv_14 = (smoothstep(0.0, max(uniforms.n_n0.startFalloff, 0.001), along_11) * exp((-((past_9 / max(uniforms.n_n0.endFalloff, 0.001))))));
  let amt_13 = (exp((-(((an_2 * an_2) * 2.5)))) * longEnv_14);
  let beamRgb_0 = ((mix(uniforms.n_n0.beamColor.rgb, vec3f(clamp((abs((hue6_1 - 3.0)) - 1.0), 0.0, 1.0), clamp((2.0 - abs((hue6_1 - 2.0))), 0.0, 1.0), clamp((2.0 - abs((hue6_1 - 4.0))), 0.0, 1.0)), vec3f((smoothstep(segLen_8, (segLen_8 + soft_12), along_11) * uniforms.n_n0.saturation))) * amt_13) * uniforms.n_n0.intensity);
  var composed_15 = vec4f(beamRgb_0.xyz, clamp(amt_13, 0.0, 1.0));
  return vec4f(linearToSrgb(tone_linear(composed_15.rgb)) * composed_15.a, composed_15.a);
}