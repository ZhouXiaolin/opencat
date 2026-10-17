struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  trackers: f32,
  threshold: f32,
  variance: f32,
  agility: f32,
  lifespan: f32,
  trail: f32,
  markerSize: f32,
  lineWidth: f32,
  markerColor: vec4f,
  keyframeColor: vec4f,
  pathColor: vec4f,
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

@group(1) @binding(0) var rtt_0: texture_2d<f32>;

@group(2) @binding(0) var linearClamp: sampler;

@group(1) @binding(1) var compute_0: texture_2d<f32>;

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
  let kft_0_res = uniforms._sys.logicalViewportSize;
  let kft_0_sf = kft_0_res.y / 1080.0;
  let kft_0_px = vec2f((uv).x * kft_0_res.x, (uv).y * kft_0_res.y);
  let kft_0_aa = max(max(fwidth(kft_0_px.x), fwidth(kft_0_px.y)), 0.0001);
  let kft_0_lw = max(uniforms.n_n0.lineWidth * kft_0_sf, 0.5);
  let kft_0_ms = max(uniforms.n_n0.markerSize * kft_0_sf, 4.0);
  var kft_0_dst = textureSampleLevel(rtt_0, linearClamp, (uv), 0.0);
  let kft_0_n = u32(clamp(uniforms.n_n0.trackers, 1.0, 64.0));
  for (var ti = 0u; ti < kft_0_n; ti = ti + 1u) {
  let head = textureLoad(compute_0, vec2u(0u, ti), 0);
  if (head.w < 0.5) { continue; }
  let bb = textureLoad(compute_0, vec2u(14u, ti), 0);
  let pad = kft_0_ms + kft_0_lw * 4.0 + kft_0_aa * 2.0;
  let bbMin = vec2f(bb.x * kft_0_res.x, bb.y * kft_0_res.y) - vec2f(pad);
  let bbMax = vec2f(bb.z * kft_0_res.x, bb.w * kft_0_res.y) + vec2f(pad);
  if (all(kft_0_px >= bbMin) && all(kft_0_px <= bbMax)) {
  let mta = textureLoad(compute_0, vec2u(1u, ti), 0);
  let tp = vec2f(head.x * kft_0_res.x, head.y * kft_0_res.y);
  let searching = select(1.0, 0.45, mta.x > 4.0);
  if (uniforms.n_n0.trail > 0.001) {
  var ePrev = textureLoad(compute_0, vec2u(13u, ti), 0);
  for (var j = 0u; j < 12u; j = j + 1u) {
  let e = textureLoad(compute_0, vec2u(2u + j, ti), 0);
  let valid = select(0.0, 1.0, abs(e.w - mta.z) < 0.5) * select(0.0, 1.0, e.z > 0.5);
  let age = head.z - e.z;
  let fade = clamp(1.0 - age / 96.0, 0.0, 1.0) * valid;
  let ep = vec2f(e.x * kft_0_res.x, e.y * kft_0_res.y);
  let dp = kft_0_px - ep;
  let dsz = kft_0_ms * 0.16;
  let dSd = (abs(dp.x) + abs(dp.y)) * 0.7071 - dsz;
  { let av = clamp((1.0 - smoothstep(0.0, kft_0_aa, dSd)) * uniforms.n_n0.keyframeColor.a * fade * uniforms.n_n0.trail, 0.0, 1.0); kft_0_dst = vec4f((uniforms.n_n0.keyframeColor.rgb) * av + kft_0_dst.rgb * (1.0 - av), av + kft_0_dst.a * (1.0 - av)); }
  let validP = select(0.0, 1.0, abs(ePrev.w - mta.z) < 0.5) * select(0.0, 1.0, ePrev.z > 0.5);
  let stampGap = abs(e.z - ePrev.z);
  let contiguous = select(0.0, 1.0, stampGap < 12.0);
  let epp = vec2f(ePrev.x * kft_0_res.x, ePrev.y * kft_0_res.y);
  let pa = kft_0_px - epp; let ba = ep - epp;
  let h = clamp(dot(pa, ba) / max(dot(ba, ba), 0.0001), 0.0, 1.0);
  let segD = length(pa - ba * h);
  { let av = clamp((1.0 - smoothstep(kft_0_lw * 0.4, kft_0_lw * 0.4 + kft_0_aa, segD)) * uniforms.n_n0.pathColor.a * fade * validP * contiguous * uniforms.n_n0.trail * 0.8, 0.0, 1.0); kft_0_dst = vec4f((uniforms.n_n0.pathColor.rgb) * av + kft_0_dst.rgb * (1.0 - av), av + kft_0_dst.a * (1.0 - av)); }
  ePrev = e;
  }
  }
  let gp = kft_0_px - tp;
  let hlf = kft_0_ms * 0.5;
  let q = abs(gp) - vec2f(hlf);
  let boxSd = length(max(q, vec2f(0.0))) + min(max(q.x, q.y), 0.0);
  var strokeA = 1.0 - smoothstep(0.0, kft_0_aa, abs(boxSd) - kft_0_lw * 0.5);
  let bl = hlf * 0.45;
  let inCorner = abs(gp.x) > hlf - bl && abs(gp.y) > hlf - bl;
  strokeA = strokeA * select(0.0, 1.0, inCorner);
  { let av = clamp(strokeA * uniforms.n_n0.markerColor.a * searching, 0.0, 1.0); kft_0_dst = vec4f((uniforms.n_n0.markerColor.rgb) * av + kft_0_dst.rgb * (1.0 - av), av + kft_0_dst.a * (1.0 - av)); }
  let tick = hlf * 0.55;
  let chH = 1.0 - smoothstep(0.0, kft_0_aa, max(abs(gp.y) - kft_0_lw * 0.5, abs(gp.x) - tick));
  let chV = 1.0 - smoothstep(0.0, kft_0_aa, max(abs(gp.x) - kft_0_lw * 0.5, abs(gp.y) - tick));
  { let av = clamp(max(chH, chV) * 0.55 * uniforms.n_n0.markerColor.a * searching, 0.0, 1.0); kft_0_dst = vec4f((uniforms.n_n0.markerColor.rgb) * av + kft_0_dst.rgb * (1.0 - av), av + kft_0_dst.a * (1.0 - av)); }
  { let av = clamp((1.0 - smoothstep(kft_0_lw, kft_0_lw + kft_0_aa, length(gp))) * searching, 0.0, 1.0); kft_0_dst = vec4f((vec3f(1.0, 1.0, 1.0)) * av + kft_0_dst.rgb * (1.0 - av), av + kft_0_dst.a * (1.0 - av)); }
  }
  }
  var composed_1 = vec4f(kft_0_dst.rgb / max(kft_0_dst.a, 0.0001), kft_0_dst.a);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}