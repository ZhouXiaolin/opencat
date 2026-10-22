@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn dropShadowTapUV(uv: vec2f, viewport: vec2f, blurRadius: f32, ox: f32, oy: f32) -> vec2f {
  return vec2f((uv.x + ((ox * blurRadius) / viewport.x)), (uv.y + ((oy * blurRadius) / viewport.y)));
}

fn dropShadowUV(uv: vec2f, viewport: vec2f, angle: f32, distance_1: f32) -> vec2f {
  let angleRad = (angle * 0.017453292519943295f);
  let dist = (distance_1 * 0.2f);
  let aspect = (viewport.x / viewport.y);
  let ox = ((sin(angleRad) * dist) / aspect);
  let oy = ((cos(angleRad) * -1f) * dist);
  return vec2f((uv.x - ox), (uv.y - oy));
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  color: vec4f,
  distance: f32,
  angle: f32,
  blur: f32,
  intensity: f32,
  cutout: f32,
  _opacity: f32,
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

struct rtt_rtt_1_Input {
  @location(0) uv: vec2f,
}

@fragment fn rtt_rtt_1(in: rtt_rtt_1_Input) -> @location(0)  vec4f {
  var uv = in.uv;
  return vec4f(((((((((((((((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, -6.0, 0.0))).a * 0.009011908593498551) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, -5.0, 0.0))).a * 0.021725136787898294)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, -4.0, 0.0))).a * 0.04473768908915352)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, -3.0, 0.0))).a * 0.07837141937560346)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, -2.0, 0.0))).a * 0.11683295783714193)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, -1.0, 0.0))).a * 0.14853556485355648)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, 0.0, 0.0))).a * 0.16092693916961698)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, 1.0, 0.0))).a * 0.14853556485355648)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, 2.0, 0.0))).a * 0.11683295783714193)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, 3.0, 0.0))).a * 0.07837141937560346)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, 4.0, 0.0))).a * 0.04473768908915352)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, 5.0, 0.0))).a * 0.021725136787898294)) + ((textureSample(rtt_0, linearClamp, dropShadowTapUV(dropShadowUV(uv, uniforms._sys.viewportSize, uniforms.n_n0.angle, uniforms.n_n0.distance), uniforms._sys.viewportSize, uniforms.n_n0.blur, 6.0, 0.0))).a * 0.009011908593498551)), 0.0, 0.0, 1.0);
}