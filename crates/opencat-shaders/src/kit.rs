//! 共享 WGSL 函数库（kit）。
//!
//! 逐字收割自 `tools/shader-diff/wgsl/<Component>/shader-N.wgsl`——那些文件是
//! `@coreroot` 编译器（`/Users/solaren/Projects/shaders/packages/core`）的成品输出。
//! 本模块不做任何改写：函数体是权威，改一个字符就可能让像素偏离快照。
//! 顺序 = 依赖拓扑序，调用方按需切片即可直接拼接。

/// `awMix`
pub const AWMIX: &str = r#"fn awMix(a: vec3f, b: vec3f, wA: f32, wB: f32, sw: f32) -> vec3f {
  return (((a * wA) + (b * wB)) / sw);
}"#;

/// `beatShade`
pub const BEATSHADE: &str = r#"fn beatShade(color: vec4f, acBeat: f32) -> vec4f {
  let clamped = clamp((color.xyz * acBeat), vec3f(), vec3f(1));
  return vec4f(clamped.x, clamped.y, clamped.z, color.w);
}"#;

/// `overComposite`
pub const OVERCOMPOSITE: &str = r#"fn overComposite(base: vec4f, blended: vec3f, overlayAlpha: f32) -> vec4f {
  let baseWeight = (base.w * (1f - overlayAlpha));
  let finalAlpha = (overlayAlpha + baseWeight);
  let rgb = ((blended * overlayAlpha) + (base.xyz * baseWeight));
  return vec4f(rgb, finalAlpha);
}"#;

/// `blend_normal`
pub const BLEND_NORMAL: &str = r#"fn blend_normal(base: vec4f, overlay2: vec4f, opacity: f32) -> vec4f {
  let overlayAlpha = (overlay2.w * opacity);
  return overComposite(base, overlay2.xyz, overlayAlpha);
}"#;

/// `floatBitsToUint2`
pub const FLOATBITSTOUINT2: &str = r#"fn floatBitsToUint2(v: vec2f) -> vec2u { return bitcast<vec2u>(v); }"#;

/// `hash12`
pub const HASH12: &str = r#"fn hash12(p: vec2f) -> f32 {
  let u = floatBitsToUint2((p * vec2f(141421360, 2718281728)));
  return (f32(((u.x ^ u.y) * 3141592653u)) / 4294967296f);
}"#;

/// `hash2D`
pub const HASH2D: &str = r#"fn hash2D(p: vec2f) -> f32 {
  return hash12(p);
}"#;

/// `smoothNoise2D`
pub const SMOOTHNOISE2D: &str = r#"fn smoothNoise2D(p: vec2f) -> f32 {
  let i = floor(p);
  let f = fract(p);
  let u = ((f * f) * (vec2f(3) - (f * 2f)));
  let a = hash2D(i);
  let b = hash2D((i + vec2f(1, 0)));
  let c = hash2D((i + vec2f(0, 1)));
  let dd = hash2D((i + vec2f(1)));
  return mix(mix(a, b, u.x), mix(c, dd, u.x), u.y);
}"#;

/// `burstGate`
pub const BURSTGATE: &str = r#"fn burstGate(t: f32) -> f32 {
  return smoothstep(0.45f, 0.8f, smoothNoise2D(vec2f((t * 0.4f), 0f)));
}"#;

/// `creaseShift`
pub const CREASESHIFT: &str = r#"fn creaseShift(uvY: f32, t: f32, wobble: f32) -> f32 {
  let creasePhase = smoothstep(0.92f, 0.99f, sin(((uvY * 8f) - (t * 3.77f))));
  let creaseNoise = smoothstep(0.3f, 1f, smoothNoise2D(vec2f((uvY * 4.77f), t)));
  return (((creasePhase * creaseNoise) * wobble) * -0.018f);
}"#;

/// `floatBitsToUint1`
pub const FLOATBITSTOUINT1: &str = r#"fn floatBitsToUint1(x: f32) -> u32 { return bitcast<u32>(x); }"#;

/// `footprint1`
pub const FOOTPRINT1: &str = r#"fn footprint1(x: f32, softness: f32) -> f32 {
  let baseW = max(abs(dpdx(x)), abs(dpdy(x)));
  return max((baseW + softness), 1e-5f);
}"#;

/// `geom_rotate2`
pub const GEOM_ROTATE2: &str = r#"fn geom_rotate2(p: vec2f, cosA: f32, sinA: f32) -> vec2f {
  return vec2f(((p.x * cosA) - (p.y * sinA)), ((p.x * sinA) + (p.y * cosA)));
}"#;

/// `gradientClamp01`
pub const GRADIENTCLAMP01: &str = r#"fn gradientClamp01(t: f32) -> f32 {
  return clamp(t, 0f, 1f);
}"#;

/// `unflipPosition`
pub const UNFLIPPOSITION: &str = r#"fn unflipPosition(p: vec2f) -> vec2f {
  return vec2f(p.x, (1f - p.y));
}"#;

/// `gradientRawT`
pub const GRADIENTRAWT: &str = r#"fn gradientRawT(start: vec2f, end: vec2f, angle: f32, uv: vec2f, viewport: vec2f) -> f32 {
  let startPos = unflipPosition(start);
  let endPos = unflipPosition(end);
  let gradientVector = (endPos - startPos);
  let gradientLength = length(gradientVector);
  let gradientDir = normalize(gradientVector);
  let safeViewportY = max(viewport.y, 1e-6f);
  let aspect = (viewport.x / safeViewportY);
  let angleRad = ((angle * 0.017453292519943295f) * -1f);
  let cosAngle = cos(angleRad);
  let sinAngle = sin(angleRad);
  let midpoint = ((startPos + endPos) * 0.5f);
  let centeredUV = (uv - midpoint);
  let rotated = geom_rotate2(vec2f((centeredUV.x * aspect), centeredUV.y), cosAngle, sinAngle);
  let rotatedUV = (vec2f((rotated.x / aspect), rotated.y) + midpoint);
  let relativePos = (rotatedUV - startPos);
  let projection = dot(relativePos, gradientDir);
  return (projection / max(gradientLength, 1e-6f));
}"#;

/// `mixInSpace`
pub const MIXINSPACE: &str = r#"fn mixInSpace(convA: vec3f, alphaA: f32, convB: vec3f, alphaB: f32, t: f32) -> MixStep {
  let weightA = (alphaA * (1f - t));
  let weightB = (alphaB * t);
  let totalWeight = (weightA + weightB);
  let safeWeight = max(totalWeight, 1e-3f);
  return MixStep(awMix(convA, convB, weightA, weightB, safeWeight), totalWeight);
}"#;

/// `gradientStopsInSpace`
pub const GRADIENTSTOPSINSPACE: &str = r#"fn gradientStopsInSpace(t: f32, colors: array<vec4f, 8>, positions: array<vec4f, 2>, converted: array<vec3f, 8>, stopCount: f32) -> MixStep {
  let clampedT = clamp(t, 0f, 1f);
  let n = i32(stopCount);
  let c0 = converted[0i];
  var conv = vec3f(c0.x, c0.y, c0.z);
  var alpha = colors[0i].w;
  for (var i = 1; (i < n); i++) {
    let va = positions[((i - 1i) >> 2u)];
    let vb = positions[(i >> 2u)];
    let posA = va[((i - 1i) & 3i)];
    let posB = vb[(i & 3i)];
    let seg = clamp(((clampedT - posA) / max((posB - posA), 1e-6f)), 0f, 1f);
    let step_1 = mixInSpace(conv, alpha, converted[i], colors[i].w, seg);
    conv = vec3f(step_1.conv.x, step_1.conv.y, step_1.conv.z);
    alpha = step_1.alpha;
  }
  return MixStep(conv, alpha);
}"#;

/// `headSwitch`
pub const HEADSWITCH: &str = r#"fn headSwitch(uvY: f32, t: f32, wobble: f32, burst: f32) -> vec2f {
  let switchPhase = (1f - smoothstep(0f, 0.06f, uvY));
  let switchNoise = (smoothNoise2D(vec2f((uvY * 60f), (t * 14f))) - 0.5f);
  let switchX = (((switchPhase * switchNoise) * wobble) * 0.09f);
  let switchY = (((switchPhase * wobble) * burst) * 0.02f);
  return vec2f(switchX, switchY);
}"#;

/// `linearToSrgb`
pub const LINEARTOSRGB: &str = r#"fn linearToSrgb(color: vec3f) -> vec3f {
  let a = ((pow(color, vec3f(0.41666001081466675)) * 1.055f) - 0.055f);
  let b = (color * 12.92f);
  let factor = (color <= vec3f(0.0031308000907301903));
  return select(a, b, factor);
}"#;

/// `mixColors`
pub const MIXCOLORS: &str = r#"fn mixColors(colorA: vec4f, colorB: vec4f, t: f32) -> vec4f {
  let weightA = (colorA.w * (1f - t));
  let weightB = (colorB.w * t);
  let totalWeight = (weightA + weightB);
  let safeWeight = max(totalWeight, 1e-3f);
  let rgb = awMix(colorA.xyz, colorB.xyz, weightA, weightB, safeWeight);
  return vec4f(rgb, totalWeight);
}"#;

/// `mxRotl32`
pub const MXROTL32: &str = r#"fn mxRotl32(x: u32, k: i32) -> u32 {
  return ((x << u32(k)) | (x >> u32((32i - k))));
}"#;

/// `mxBjfinal`
pub const MXBJFINAL: &str = r#"fn mxBjfinal(a0: u32, b0: u32, c0: u32) -> u32 {
  var a = a0;
  var b = b0;
  var c = c0;
  c = (c ^ b);
  c = (c - mxRotl32(b, 14i));
  a = (a ^ c);
  a = (a - mxRotl32(c, 11i));
  b = (b ^ a);
  b = (b - mxRotl32(a, 25i));
  c = (c ^ b);
  c = (c - mxRotl32(b, 16i));
  a = (a ^ c);
  a = (a - mxRotl32(c, 4i));
  b = (b ^ a);
  b = (b - mxRotl32(a, 14i));
  c = (c ^ b);
  c = (c - mxRotl32(b, 24i));
  return c;
}"#;

/// `mxFade`
pub const MXFADE: &str = r#"fn mxFade(t: f32) -> f32 {
  return (((t * t) * t) * ((t * ((t * 6f) - 15f)) + 10f));
}"#;

/// `mxGradientFloat3`
pub const MXGRADIENTFLOAT3: &str = r#"fn mxGradientFloat3(hash: u32, x: f32, y: f32, z: f32) -> f32 {
  let h = (hash & 15u);
  let u = select(y, x, (h < 8u));
  let inner = select(z, x, ((h == 12u) || (h == 14u)));
  let v = select(inner, y, (h < 4u));
  let uu = select(u, -(u), ((h & 1u) != 0u));
  let vv = select(v, -(v), ((h & 2u) != 0u));
  return (uu + vv);
}"#;

/// `mxGradientScale3d`
pub const MXGRADIENTSCALE3D: &str = r#"fn mxGradientScale3d(v: f32) -> f32 {
  return (0.982f * v);
}"#;

/// `mxHashInt2`
pub const MXHASHINT2: &str = r#"fn mxHashInt2(x: i32, y: i32, z: i32) -> u32 {
  const len = 3u;
  let seed = ((3735928559u + (len << 2u)) + 13u);
  let a = (seed + u32(x));
  let b = (seed + u32(y));
  let c = (seed + u32(z));
  return mxBjfinal(a, b, c);
}"#;

/// `mxTrilerp`
pub const MXTRILERP: &str = r#"fn mxTrilerp(v0: f32, v1: f32, v2: f32, v3: f32, v4: f32, v5: f32, v6: f32, v7: f32, s: f32, t: f32, r: f32) -> f32 {
  let s1 = (1f - s);
  let t1 = (1f - t);
  let r1 = (1f - r);
  return ((r1 * ((t1 * ((v0 * s1) + (v1 * s))) + (t * ((v2 * s1) + (v3 * s))))) + (r * ((t1 * ((v4 * s1) + (v5 * s))) + (t * ((v6 * s1) + (v7 * s))))));
}"#;

/// `mxPerlinNoiseFloat3`
pub const MXPERLINNOISEFLOAT3: &str = r#"fn mxPerlinNoiseFloat3(p: vec3f) -> f32 {
  let X = i32(floor(p.x));
  let Y = i32(floor(p.y));
  let Z = i32(floor(p.z));
  let fx = (p.x - f32(X));
  let fy = (p.y - f32(Y));
  let fz = (p.z - f32(Z));
  let u = mxFade(fx);
  let v = mxFade(fy);
  let w = mxFade(fz);
  let result = mxTrilerp(mxGradientFloat3(mxHashInt2(X, Y, Z), fx, fy, fz), mxGradientFloat3(mxHashInt2((X + 1i), Y, Z), (fx - 1f), fy, fz), mxGradientFloat3(mxHashInt2(X, (Y + 1i), Z), fx, (fy - 1f), fz), mxGradientFloat3(mxHashInt2((X + 1i), (Y + 1i), Z), (fx - 1f), (fy - 1f), fz), mxGradientFloat3(mxHashInt2(X, Y, (Z + 1i)), fx, fy, (fz - 1f)), mxGradientFloat3(mxHashInt2((X + 1i), Y, (Z + 1i)), (fx - 1f), fy, (fz - 1f)), mxGradientFloat3(mxHashInt2(X, (Y + 1i), (Z + 1i)), fx, (fy - 1f), (fz - 1f)), mxGradientFloat3(mxHashInt2((X + 1i), (Y + 1i), (Z + 1i)), (fx - 1f), (fy - 1f), (fz - 1f)), u, v, w);
  return mxGradientScale3d(result);
}"#;

/// `mxNoiseFloat3`
pub const MXNOISEFLOAT3: &str = r#"fn mxNoiseFloat3(p: vec3f) -> f32 {
  return mxPerlinNoiseFloat3(p);
}"#;

/// `noise_hash11`
pub const NOISE_HASH11: &str = r#"fn noise_hash11(p: f32) -> f32 {
  let u = floatBitsToUint1((p * 3141592653f));
  return (f32(((u * u) * 3141592653u)) / 4294967296f);
}"#;

/// `oklabToRgb`
pub const OKLABTORGB: &str = r#"fn oklabToRgb(lab: vec3f) -> vec3f {
  let L = lab.x;
  let a = lab.y;
  let b = lab.z;
  let l_ = ((L + (a * 0.3963377774f)) + (b * 0.2158037573f));
  let m_ = ((L - (a * 0.1055613458f)) - (b * 0.0638541728f));
  let s_ = ((L - (a * 0.0894841775f)) - (b * 1.291485548f));
  let l = pow(l_, 3f);
  let m = pow(m_, 3f);
  let s = pow(s_, 3f);
  return vec3f((((l * 4.0767416621f) - (m * 3.3077115913f)) + (s * 0.2309699292f)), (((l * -1.2684380046f) + (m * 2.6097574011f)) - (s * 0.3413193965f)), (((l * -0.0041960863f) - (m * 0.7034186147f)) + (s * 1.707614701f)));
}"#;

/// `rampWrap`
pub const RAMPWRAP: &str = r#"fn rampWrap(t: f32, cycles: f32) -> f32 {
  let s = (t * cycles);
  let m = (fract((s * 0.5f)) * 2f);
  let band = (1f - abs((m - 1f)));
  let fp = (footprint1(t, 0f) * cycles);
  return mix(band, 0.5f, smoothstep(0.5f, 1.5f, fp));
}"#;

/// `rgbDither`
pub const RGBDITHER: &str = r#"fn rgbDither(base: vec4f, uv: vec2f, viewport: vec2f, animTime: f32) -> vec4f {
  let n = hash12(((uv * viewport) + vec2f(animTime, (animTime * 1.618f))));
  let rgb = clamp((base.xyz + ((n - 0.5f) * 4e-3f)), vec3f(), vec3f(1));
  return vec4f(rgb, base.w);
}"#;

/// `rowJitter`
pub const ROWJITTER: &str = r#"fn rowJitter(uvY: f32, t: f32, scanlineNoise: f32, burst: f32) -> vec2f {
  let rowInt = floor((uvY * 487f));
  let h0 = ((hash12(vec2f(rowInt, t)) - 0.5f) * 2f);
  let h1 = ((hash12(vec2f(rowInt, (t + 69.42f))) - 0.5f) * 2f);
  let fineGate = (0.25f + (burst * 0.75f));
  let chromaRowOffset = (((h0 * scanlineNoise) * 8e-3f) * fineGate);
  let lumaRowOffset = (((h1 * scanlineNoise) * 4e-3f) * fineGate);
  return vec2f(lumaRowOffset, chromaRowOffset);
}"#;

/// `sRGBToP3`
pub const SRGBTOP3: &str = r#"fn sRGBToP3(srgb: vec3f) -> vec3f {
  let r = (((srgb.x * 0.8224621f) + (srgb.y * 0.177538f)) + (srgb.z * 0f));
  let g = (((srgb.x * 0.0331941f) + (srgb.y * 0.9668058f)) + (srgb.z * 0f));
  let b = (((srgb.x * 0.0170826f) + (srgb.y * 0.0723974f)) + (srgb.z * 0.9105199f));
  return vec3f(r, g, b);
}"#;

/// `scatterField`
pub const SCATTERFIELD: &str = r#"fn scatterField(q: vec2f, count: f32, seed: f32, anchors: array<vec4f, 4>, power: f32) -> f32 {
  var num = 0f;
  var den = 0f;
  let seedShuffle = noise_hash11(((seed * 0.531f) + 0.77f));
  for (var i = 0; (i < 8i); i++) {
    let fi = f32(i);
    let active_1 = select(0f, 1f, (fi < (count - 0.5f)));
    let pair = anchors[u32((f32(i) / 2f))];
    let anchor = select(pair.xy, pair.zw, ((i & 1i) == 1i));
    let dist = max(length((q - anchor)), 0.01f);
    let w = (active_1 / pow(dist, power));
    let ti = fract(((fi * 0.6180339887f) + seedShuffle));
    num = (num + (w * ti));
    den = (den + w);
  }
  return (num / max(den, 1e-6f));
}"#;

/// `tapeWave`
pub const TAPEWAVE: &str = r#"fn tapeWave(uvY: f32, t: f32, wobble: f32) -> f32 {
  let wave1 = (smoothNoise2D(vec2f((uvY * 3f), (t * 0.8f))) - 0.5f);
  let wave2 = (smoothNoise2D(vec2f((uvY * 30f), (t * 6f))) - 0.5f);
  return (((wave1 * 8e-3f) + (wave2 * 2e-3f)) * wobble);
}"#;

/// `tone_linear`
pub const TONE_LINEAR: &str = r#"fn tone_linear(color: vec3f) -> vec3f {
  return vec3f(color.x, color.y, color.z);
}"#;

/// `unpremultiplyAlpha`
pub const UNPREMULTIPLYALPHA: &str = r#"fn unpremultiplyAlpha(color2: vec4f) -> vec4f {
  let alpha = color2.w;
  let straightRGB = select(vec3f(), (color2.xyz / alpha), (alpha > 1e-3f));
  return vec4f(straightRGB, alpha);
}"#;

/// `vhsAcBeat`
pub const VHSACBEAT: &str = r#"fn vhsAcBeat(uv: vec2f, time: f32, speed: f32, wobble: f32) -> f32 {
  let t = (time * speed);
  return (1f + ((cos((((t % 6.2831853f) * 2f) + (uv.y * 0.5f))) * 0.015f) * wobble));
}"#;

/// `vhsChromaTapUV`
pub const VHSCHROMATAPUV: &str = r#"fn vhsChromaTapUV(chromaUV: vec2f, smearScale: f32, i: f32) -> vec2f {
  return vec2f((chromaUV.x + ((i * -1f) * smearScale)), chromaUV.y);
}"#;

/// `vhsSampleUVs`
pub const VHSSAMPLEUVS: &str = r#"fn vhsSampleUVs(uv: vec2f, time: f32, speed: f32, wobble: f32, scanlineNoise: f32) -> vec4f {
  let t = (time * speed);
  let burst = burstGate(t);
  let jitter = rowJitter(uv.y, t, scanlineNoise, burst);
  let sw = headSwitch(uv.y, t, wobble, burst);
  let globalX = ((tapeWave(uv.y, t, wobble) + creaseShift(uv.y, t, wobble)) + sw.x);
  let sy = (uv.y + sw.y);
  return vec4f(((uv.x + globalX) + jitter.x), sy, ((uv.x + globalX) + jitter.y), sy);
}"#;

/// `warpDomain`
pub const WARPDOMAIN: &str = r#"fn warpDomain(pos: vec3f, t: f32, warpAmount: f32) -> vec3f {
  let qx = mxNoiseFloat3((pos + vec3f((t * 0.3f), (t * -0.2f), 0f)));
  let qy = mxNoiseFloat3((pos + vec3f(5.2f, 1.3f, (t * 0.25f))));
  let warped1 = (pos + (vec3f(qx, qy, 0f) * warpAmount));
  let rx = mxNoiseFloat3((warped1 + vec3f(1.7f, 9.2f, (t * 0.15f))));
  let ry = mxNoiseFloat3((warped1 + vec3f(8.3f, 2.8f, (t * 0.13f))));
  return (pos + (vec3f(rx, ry, 0f) * warpAmount));
}"#;

/// `yiqRecombine`
pub const YIQRECOMBINE: &str = r#"fn yiqRecombine(luma: vec4f, c1: vec4f, c2: vec4f, c3: vec4f, c4: vec4f, c5: vec4f) -> vec3f {
  let sharpY = dot(luma.xyz, vec3f(0.29899999499320984, 0.5870000123977661, 0.11400000005960464));
  let iAxis = vec3f(0.5960000157356262, -0.27399998903274536, -0.32199999690055847);
  let qAxis = vec3f(0.210999995470047, -0.5230000019073486, 0.31200000643730164);
  let accI = (((((dot(c1.xyz, iAxis) * 0.06666666666666667f) + (dot(c2.xyz, iAxis) * 0.13333333333333333f)) + (dot(c3.xyz, iAxis) * 0.2f)) + (dot(c4.xyz, iAxis) * 0.26666666666666666f)) + (dot(c5.xyz, iAxis) * 0.3333333333333333f));
  let accQ = (((((dot(c1.xyz, qAxis) * 0.06666666666666667f) + (dot(c2.xyz, qAxis) * 0.13333333333333333f)) + (dot(c3.xyz, qAxis) * 0.2f)) + (dot(c4.xyz, qAxis) * 0.26666666666666666f)) + (dot(c5.xyz, qAxis) * 0.3333333333333333f));
  return vec3f(((sharpY + (accI * 0.956f)) + (accQ * 0.621f)), ((sharpY - (accI * 0.272f)) - (accQ * 0.647f)), ((sharpY - (accI * 1.106f)) + (accQ * 1.703f)));
}"#;

/// `zoomBlurTapCoord`
pub const ZOOMBLURTAPCOORD: &str = r#"fn zoomBlurTapCoord(center: vec2f, intensity: f32, uv: vec2f, aspect: f32, tapIndex: f32) -> vec2f {
  let centerPos = vec2f(center.x, (1f - center.y));
  let radius = (intensity * 0.01f);
  let scale = (1f + (radius * (tapIndex / 31f)));
  let deltaX = (uv.x - centerPos.x);
  let deltaY = (uv.y - centerPos.y);
  let acdX = (deltaX * aspect);
  let acdY = deltaY;
  let scaledDeltaX = (acdX / scale);
  let scaledDeltaY = (acdY / scale);
  return vec2f(((scaledDeltaX / aspect) + centerPos.x), (scaledDeltaY + centerPos.y));
}"#;

/// 全部 kit 函数，按依赖拓扑序。
pub const ALL: &[(&str, &str)] = &[
    ("awMix", AWMIX),
    ("beatShade", BEATSHADE),
    ("overComposite", OVERCOMPOSITE),
    ("blend_normal", BLEND_NORMAL),
    ("floatBitsToUint2", FLOATBITSTOUINT2),
    ("hash12", HASH12),
    ("hash2D", HASH2D),
    ("smoothNoise2D", SMOOTHNOISE2D),
    ("burstGate", BURSTGATE),
    ("creaseShift", CREASESHIFT),
    ("floatBitsToUint1", FLOATBITSTOUINT1),
    ("footprint1", FOOTPRINT1),
    ("geom_rotate2", GEOM_ROTATE2),
    ("gradientClamp01", GRADIENTCLAMP01),
    ("unflipPosition", UNFLIPPOSITION),
    ("gradientRawT", GRADIENTRAWT),
    ("mixInSpace", MIXINSPACE),
    ("gradientStopsInSpace", GRADIENTSTOPSINSPACE),
    ("headSwitch", HEADSWITCH),
    ("linearToSrgb", LINEARTOSRGB),
    ("mixColors", MIXCOLORS),
    ("mxRotl32", MXROTL32),
    ("mxBjfinal", MXBJFINAL),
    ("mxFade", MXFADE),
    ("mxGradientFloat3", MXGRADIENTFLOAT3),
    ("mxGradientScale3d", MXGRADIENTSCALE3D),
    ("mxHashInt2", MXHASHINT2),
    ("mxTrilerp", MXTRILERP),
    ("mxPerlinNoiseFloat3", MXPERLINNOISEFLOAT3),
    ("mxNoiseFloat3", MXNOISEFLOAT3),
    ("noise_hash11", NOISE_HASH11),
    ("oklabToRgb", OKLABTORGB),
    ("rampWrap", RAMPWRAP),
    ("rgbDither", RGBDITHER),
    ("rowJitter", ROWJITTER),
    ("sRGBToP3", SRGBTOP3),
    ("scatterField", SCATTERFIELD),
    ("tapeWave", TAPEWAVE),
    ("tone_linear", TONE_LINEAR),
    ("unpremultiplyAlpha", UNPREMULTIPLYALPHA),
    ("vhsAcBeat", VHSACBEAT),
    ("vhsChromaTapUV", VHSCHROMATAPUV),
    ("vhsSampleUVs", VHSSAMPLEUVS),
    ("warpDomain", WARPDOMAIN),
    ("yiqRecombine", YIQRECOMBINE),
    ("zoomBlurTapCoord", ZOOMBLURTAPCOORD),
];

/// 按名字取函数体。
pub fn lookup(name: &str) -> Option<&'static str> {
    ALL.iter().find(|(n, _)| *n == name).map(|(_, b)| *b)
}

/// 取一组函数的闭包（含它们依赖的函数），按依赖拓扑序返回。
///
/// 组件配方只列出它直接调用的 kit 名字，闭包展开与排序由这里完成——配方
/// 因此不需要知道 `mxNoiseFloat3` 依赖 `mxPerlinNoiseFloat3` 这类细节。
pub fn closure_for(names: &[&'static str]) -> Vec<&'static str> {
    let mut want: Vec<&'static str> = Vec::new();
    let mut stack: Vec<&'static str> = names.to_vec();
    while let Some(n) = stack.pop() {
        if want.contains(&n) {
            continue;
        }
        let Some(body) = lookup(n) else { continue };
        want.push(n);
        // 依赖 = 函数体里出现的其它 kit 函数名（以 `(` 结尾的调用形态）。
        for (cand, _) in ALL {
            if *cand != n && contains_call(body, cand) {
                stack.push(cand);
            }
        }
    }
    // ALL 已是拓扑序，按它过滤即得合法顺序。
    ALL.iter()
        .map(|(n, _)| *n)
        .filter(|n| want.contains(n))
        .collect()
}

fn contains_call(body: &str, name: &str) -> bool {
    let mut from = 0;
    while let Some(i) = body[from..].find(name) {
        let s = from + i;
        let before_ok = s == 0
            || !body[..s]
                .chars()
                .next_back()
                .map(|c| c.is_ascii_alphanumeric() || c == '_')
                .unwrap_or(false);
        let after = &body[s + name.len()..];
        let after_ok = after.starts_with('(');
        if before_ok && after_ok {
            return true;
        }
        from = s + name.len();
    }
    false
}
