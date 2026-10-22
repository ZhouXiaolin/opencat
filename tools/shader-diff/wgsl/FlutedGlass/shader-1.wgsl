struct FlutedGeom {
  refractedUV: vec2f,
  chrOff: vec2f,
  slope: f32,
}

fn flutedGlassGeom(uv: vec2f, aspect: f32, t: f32, angleDeg: f32, frequency: f32, softness: f32, waveAmp: f32, waveFreq: f32, refraction: f32, aberration: f32, shapeParams: vec3f) -> FlutedGeom {
  let expHi = shapeParams.x;
  let expLo = shapeParams.y;
  let wavesFlag = shapeParams.z;
  let r = ((angleDeg * 3.141592653589793f) / 180f);
  let cosA = cos(r);
  let sinA = sin(r);
  let aspCorr = vec2f(((uv.x - 0.5f) * aspect), (uv.y - 0.5f));
  let u = ((aspCorr.x * cosA) + (aspCorr.y * sinA));
  let v = ((-(aspCorr.x) * sinA) + (aspCorr.y * cosA));
  let waveTerm = (sin((((v * waveFreq) * 6.283185307179586f) + (t * 2f))) * waveAmp);
  let fluteU = (u + (wavesFlag * waveTerm));
  let flutePos = ((fluteU * frequency) + t);
  let cellPos = ((fract(flutePos) - 0.5f) * 2f);
  let absCell = max(abs(cellPos), 1e-4f);
  let exponent = mix(expHi, expLo, softness);
  let slope = (sign(cellPos) * pow(absCell, exponent));
  let halfCell = (0.5f / max(frequency, 1e-3f));
  let refrU = -(((slope * refraction) * halfCell));
  let refractedUV = vec2f((uv.x + ((refrU * cosA) / aspect)), (uv.y + (refrU * sinA)));
  let chrU = ((refrU * aberration) * 0.5f);
  let chrOff = vec2f(((chrU * cosA) / aspect), (chrU * sinA));
  return FlutedGeom(refractedUV, chrOff, slope);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  shape: f32,
  angle: f32,
  frequency: f32,
  softness: f32,
  waveAmplitude: f32,
  waveFrequency: f32,
  speed: f32,
  refraction: f32,
  aberration: f32,
  lightAngle: f32,
  highlight: f32,
  highlightSoftness: f32,
  highlightColor: vec4f,
  edges: f32,
  _opacity: f32,
  _animTime: f32,
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

fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

fn edgeMirrorUV(uv: vec2f) -> vec2f {
  let mirrorX = (abs(uv.x) % 2f);
  let mirrorY = (abs(uv.y) % 2f);
  return vec2f(select(mirrorX, (2f - mirrorX), (mirrorX >= 1f)), select(mirrorY, (2f - mirrorY), (mirrorY >= 1f)));
}

fn blinnHighlight(slope: f32, lightAngleDeg: f32, intensity: f32, softness: f32) -> f32 {
  let slopeSq = min((slope * slope), 1f);
  let nz = sqrt((1f - slopeSq));
  let half = ((lightAngleDeg * 3.141592653589793f) / 360f);
  let hx = sin(half);
  let hy = cos(half);
  let nDotH = max(((slope * hx) + (nz * hy)), 0f);
  let shininess = exp2((8f - (softness * 7f)));
  let fresnel = pow((1f - nz), 5f);
  let fresnelMix = (0.04f + (0.96f * fresnel));
  return ((pow(nDotH, shininess) * fresnelMix) * intensity);
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
  let flute_0 = flutedGlassGeom(uv, uniforms._sys.aspect, (uniforms.n_n0._animTime * -1.0), uniforms.n_n0.angle, uniforms.n_n0.frequency, uniforms.n_n0.softness, uniforms.n_n0.waveAmplitude, uniforms.n_n0.waveFrequency, uniforms.n_n0.refraction, uniforms.n_n0.aberration, vec3f(16.0, 4.0, 0.0));
  var composed_1 = vec4f(((unpremultiplyAlpha(vec4f((textureSample(rtt_0, linearClamp, edgeMirrorUV((flute_0.refractedUV + flute_0.chrOff)))).r, (textureSample(rtt_0, linearClamp, edgeMirrorUV(flute_0.refractedUV))).g, (textureSample(rtt_0, linearClamp, edgeMirrorUV((flute_0.refractedUV + (flute_0.chrOff * -1.0))))).b, (textureSample(rtt_0, linearClamp, edgeMirrorUV(flute_0.refractedUV))).a))).rgb + (uniforms.n_n0.highlightColor.rgb * (blinnHighlight(flute_0.slope, uniforms.n_n0.lightAngle, uniforms.n_n0.highlight, uniforms.n_n0.highlightSoftness) * (unpremultiplyAlpha(vec4f((textureSample(rtt_0, linearClamp, edgeMirrorUV((flute_0.refractedUV + flute_0.chrOff)))).r, (textureSample(rtt_0, linearClamp, edgeMirrorUV(flute_0.refractedUV))).g, (textureSample(rtt_0, linearClamp, edgeMirrorUV((flute_0.refractedUV + (flute_0.chrOff * -1.0))))).b, (textureSample(rtt_0, linearClamp, edgeMirrorUV(flute_0.refractedUV))).a))).a))), (unpremultiplyAlpha(vec4f((textureSample(rtt_0, linearClamp, edgeMirrorUV((flute_0.refractedUV + flute_0.chrOff)))).r, (textureSample(rtt_0, linearClamp, edgeMirrorUV(flute_0.refractedUV))).g, (textureSample(rtt_0, linearClamp, edgeMirrorUV((flute_0.refractedUV + (flute_0.chrOff * -1.0))))).b, (textureSample(rtt_0, linearClamp, edgeMirrorUV(flute_0.refractedUV))).a))).a);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}