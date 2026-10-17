fn geom_aspectOf(viewport: vec2f) -> f32 {
  return (viewport.x / max(viewport.y, 1e-6f));
}

struct FlareFrame {
  aspect: f32,
  lightPos: vec2f,
  flareAxis: vec2f,
  masterFade: f32,
  lightDist: f32,
  lightAngle: f32,
}

fn flareFrame(uv: vec2f, viewport: vec2f, t: f32, lightPosRaw: vec2f, intensity: f32, edgeFade: f32) -> FlareFrame {
  let aspect = geom_aspectOf(viewport);
  let lightPos = vec2f(lightPosRaw.x, (1f - lightPosRaw.y));
  let flareAxis = vec2f((0.5f - lightPos.x), (0.5f - lightPos.y));
  let shimmer = ((((sin((t * 3.7f)) * 0.04f) + (sin((t * 7.3f)) * 0.03f)) + (sin((t * 13.1f)) * 0.02f)) + 0.97f);
  let edgeDist = max((abs((lightPos.x - 0.5f)) * 2f), (abs((lightPos.y - 0.5f)) * 2f));
  let fadeStart = (1f - (edgeFade * 0.5f));
  let edgeFadeV = mix(1f, (1f - smoothstep(fadeStart, 1f, edgeDist)), edgeFade);
  let masterFade = ((intensity * edgeFadeV) * shimmer);
  let toLightAR = vec2f(((uv.x - lightPos.x) * aspect), (uv.y - lightPos.y));
  let lightDist = length(toLightAR);
  let lightAngle = atan2(toLightAR.y, toLightAR.x);
  return FlareFrame(aspect, lightPos, flareAxis, masterFade, lightDist, lightAngle);
}

struct padded {
  _opacity: f32,
  _pad0: f32,
  _pad1: f32,
  _pad2: f32,
}

struct struct_1 {
  lightPosition: vec2f,
  intensity: f32,
  ghostIntensity: f32,
  ghostSpread: f32,
  ghostChroma: f32,
  haloIntensity: f32,
  haloRadius: f32,
  haloChroma: f32,
  haloSoftness: f32,
  starburstIntensity: f32,
  starburstPoints: f32,
  streakIntensity: f32,
  streakLength: f32,
  glareIntensity: f32,
  glareSize: f32,
  edgeFade: f32,
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

fn tone_luma601Dot(rgb: vec3f) -> f32 {
  return dot(rgb, vec3f(0.29899999499320984, 0.5870000123977661, 0.11400000005960464));
}

fn flareComposite(flareTotal: vec3f, masterFade: f32) -> vec4f {
  let finalColor = (flareTotal * masterFade);
  let flareAlpha = clamp((tone_luma601Dot(finalColor) * 2f), 0f, 1f);
  return vec4f(finalColor, flareAlpha);
}

fn lightfields_chromaticRingBand(dist: f32, center: f32, spread: f32, width: f32) -> vec3f {
  let w = max(width, 1e-4f);
  let r = (1f - smoothstep(0f, w, abs((dist - (center + spread)))));
  let g = (1f - smoothstep(0f, w, abs((dist - center))));
  let b = (1f - smoothstep(0f, w, abs((dist - (center - spread)))));
  return vec3f(r, g, b);
}

fn lensFlareGhost(uv: vec2f, aspect: f32, lightPos: vec2f, flareAxis: vec2f, t: f32, ghostSpread: f32, ghostChroma: f32, ghostInt: f32, disc: vec4f, tint: vec4f) -> vec3f {
  let offset = disc.x;
  let size = disc.y;
  let bright = disc.z;
  let hollow = disc.w;
  let r = tint.x;
  let g = tint.y;
  let b = tint.z;
  let gPhase = tint.w;
  let ghostPos = (lightPos + (flareAxis * ((offset * ghostSpread) * 2f)));
  let toGhost = vec2f(((uv.x - ghostPos.x) * aspect), (uv.y - ghostPos.y));
  let dist = length(toGhost);
  let sz = size;
  let discShape = (1f - smoothstep((sz * 0.12f), sz, dist));
  let innerFade = (1f - smoothstep((sz * 0.05f), (sz * 0.45f), dist));
  let shape = (discShape * (1f - (innerFade * hollow)));
  let gShimmer = (((sin(((t * 4.3f) + gPhase)) * 0.08f) + (sin(((t * 9.1f) + (gPhase * 1.7f))) * 0.05f)) + 0.95f);
  let ringCenter = (sz * 0.78f);
  let chromaSpread = ((sz * ghostChroma) * 0.14f);
  let ringWidth = (sz * 0.1f);
  let ring = lightfields_chromaticRingBand(dist, ringCenter, chromaSpread, ringWidth);
  let rRing = ring.x;
  let gRing = ring.y;
  let bRing = ring.z;
  let brightness = ((bright * ghostInt) * gShimmer);
  return vec3f((((shape * r) + (rRing * ghostChroma)) * brightness), (((shape * g) + (gRing * ghostChroma)) * brightness), (((shape * b) + (bRing * ghostChroma)) * brightness));
}

fn flareHalo(uv: vec2f, aspect: f32, lightPos: vec2f, t: f32, haloInt: f32, haloRad: f32, haloChroma: f32, haloSoft: f32) -> vec3f {
  let antiLight = vec2f((1f - lightPos.x), (1f - lightPos.y));
  let toAntiLight = vec2f(((uv.x - antiLight.x) * aspect), (uv.y - antiLight.y));
  let antiLightDist = length(toAntiLight);
  let haloChromaSpread = ((haloRad * haloChroma) * 0.06f);
  let haloWidth = ((haloSoft * 0.12f) + 5e-3f);
  let halo = lightfields_chromaticRingBand(antiLightDist, haloRad, haloChromaSpread, haloWidth);
  let haloAngle = atan2(toAntiLight.y, toAntiLight.x);
  let haloAngVar = ((sin(((haloAngle * 3f) + (t * 0.5f))) * 0.12f) + 0.88f);
  return ((halo * haloInt) * haloAngVar);
}

fn lightfields_angularCosineSpikes(angle: f32, halfCount: f32, power: f32) -> f32 {
  return pow(abs(cos((angle * halfCount))), power);
}

fn lensFlareSpectral(t: f32) -> vec3f {
  return vec3f(((cos((6.283185307179586f * t)) * 0.5f) + 0.5f), ((cos((6.283185307179586f * (t - 0.333f))) * 0.5f) + 0.5f), ((cos((6.283185307179586f * (t - 0.666f))) * 0.5f) + 0.5f));
}

fn flareStarburst(lightAngle: f32, flareAxis: vec2f, aspect: f32, t: f32, lightDist: f32, starburstPts: f32, starburstInt: f32) -> vec3f {
  let axisAng = atan2(flareAxis.y, (flareAxis.x * aspect));
  let rotation = ((axisAng * 0.5f) + (t * 0.06f));
  let spikeAngle = (lightAngle + rotation);
  let primarySpikes = lightfields_angularCosineSpikes(spikeAngle, (starburstPts / 2f), 3.5f);
  let secondarySpikes = (lightfields_angularCosineSpikes((spikeAngle + 0.25f), (starburstPts / 2f), 7f) * 0.4f);
  let radialFall = (1f / (1f + ((lightDist * lightDist) * 60f)));
  let starSpectral = lensFlareSpectral(((spikeAngle / 6.283185307179586f) + 0.5f));
  let starTint = mix(vec3f(1, 0.9700000286102295, 0.9200000166893005), starSpectral, 0.12f);
  return (starTint * (((primarySpikes + secondarySpikes) * radialFall) * starburstInt));
}

fn flareStreak(uv: vec2f, lightPos: vec2f, streakLen: f32, streakInt: f32) -> vec3f {
  let dy = abs((uv.y - lightPos.y));
  let dx = abs((uv.x - lightPos.x));
  let vertFall = exp(((-800f * dy) * dy));
  let lenSq = (((streakLen * streakLen) * 0.25f) + 1e-3f);
  let rStreak = exp((((-1f * dx) * dx) / (lenSq * 0.7f)));
  let gStreak = exp((((-1f * dx) * dx) / lenSq));
  let bStreak = exp((((-1f * dx) * dx) / (lenSq * 1.4f)));
  return (vec3f(((vertFall * rStreak) * 0.5f), ((vertFall * gStreak) * 0.65f), ((vertFall * bStreak) * 0.9f)) * streakInt);
}

fn lightfields_radialGaussianFalloff(dist: f32, sharpness: f32) -> f32 {
  return exp(-(((dist * dist) * sharpness)));
}

fn flareGlare(lightDist: f32, glareSz: f32, glareInt: f32) -> vec3f {
  let glareSharpness = (1f / ((glareSz * glareSz) + 0.01f));
  let glareFalloff = lightfields_radialGaussianFalloff(lightDist, glareSharpness);
  return (vec3f(1, 0.9800000190734863, 0.949999988079071) * (glareFalloff * glareInt));
}

fn flareCore(lightDist: f32, intensity: f32) -> vec3f {
  let coreSoft = exp(((-500f * lightDist) * lightDist));
  let coreSharp = (1f - smoothstep(0f, 0.025f, lightDist));
  return (vec3f(1, 0.9800000190734863, 0.9599999785423279) * ((coreSoft + (coreSharp * 1.5f)) * intensity));
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
  let flare_0 = flareFrame(uv, uniforms._sys.viewportSize, uniforms.n_n0._animTime, uniforms.n_n0.lightPosition, uniforms.n_n0.intensity, uniforms.n_n0.edgeFade);
  var composed_1 = flareComposite((((((((((((lensFlareGhost(uv, flare_0.aspect, flare_0.lightPos, flare_0.flareAxis, uniforms.n_n0._animTime, uniforms.n_n0.ghostSpread, uniforms.n_n0.ghostChroma, uniforms.n_n0.ghostIntensity, vec4f(0.15, 0.2, 0.3, 0.1), vec4f(1.0, 0.95, 0.9, 0.0)) + lensFlareGhost(uv, flare_0.aspect, flare_0.lightPos, flare_0.flareAxis, uniforms.n_n0._animTime, uniforms.n_n0.ghostSpread, uniforms.n_n0.ghostChroma, uniforms.n_n0.ghostIntensity, vec4f(0.3, 0.12, 0.25, 0.65), vec4f(1.0, 0.85, 0.55, 2.17))) + lensFlareGhost(uv, flare_0.aspect, flare_0.lightPos, flare_0.flareAxis, uniforms.n_n0._animTime, uniforms.n_n0.ghostSpread, uniforms.n_n0.ghostChroma, uniforms.n_n0.ghostIntensity, vec4f(0.5, 0.08, 0.35, 0.15), vec4f(0.62, 0.82, 1.0, 4.34))) + lensFlareGhost(uv, flare_0.aspect, flare_0.lightPos, flare_0.flareAxis, uniforms.n_n0._animTime, uniforms.n_n0.ghostSpread, uniforms.n_n0.ghostChroma, uniforms.n_n0.ghostIntensity, vec4f(0.72, 0.14, 0.18, 0.75), vec4f(0.75, 1.0, 0.7, 6.51))) + lensFlareGhost(uv, flare_0.aspect, flare_0.lightPos, flare_0.flareAxis, uniforms.n_n0._animTime, uniforms.n_n0.ghostSpread, uniforms.n_n0.ghostChroma, uniforms.n_n0.ghostIntensity, vec4f(0.95, 0.06, 0.3, 0.0), vec4f(1.0, 0.6, 0.85, 8.68))) + lensFlareGhost(uv, flare_0.aspect, flare_0.lightPos, flare_0.flareAxis, uniforms.n_n0._animTime, uniforms.n_n0.ghostSpread, uniforms.n_n0.ghostChroma, uniforms.n_n0.ghostIntensity, vec4f(1.25, 0.1, 0.15, 0.8), vec4f(0.55, 0.88, 1.0, 10.85))) + lensFlareGhost(uv, flare_0.aspect, flare_0.lightPos, flare_0.flareAxis, uniforms.n_n0._animTime, uniforms.n_n0.ghostSpread, uniforms.n_n0.ghostChroma, uniforms.n_n0.ghostIntensity, vec4f(1.55, 0.07, 0.2, 0.25), vec4f(1.0, 0.92, 0.6, 13.02))) + flareHalo(uv, flare_0.aspect, flare_0.lightPos, uniforms.n_n0._animTime, uniforms.n_n0.haloIntensity, uniforms.n_n0.haloRadius, uniforms.n_n0.haloChroma, uniforms.n_n0.haloSoftness)) + flareStarburst(flare_0.lightAngle, flare_0.flareAxis, flare_0.aspect, uniforms.n_n0._animTime, flare_0.lightDist, uniforms.n_n0.starburstPoints, uniforms.n_n0.starburstIntensity)) + flareStreak(uv, flare_0.lightPos, uniforms.n_n0.streakLength, uniforms.n_n0.streakIntensity)) + flareGlare(flare_0.lightDist, uniforms.n_n0.glareSize, uniforms.n_n0.glareIntensity)) + flareCore(flare_0.lightDist, uniforms.n_n0.intensity)), flare_0.masterFade);
  return vec4f(linearToSrgb(tone_linear(composed_1.rgb)) * composed_1.a, composed_1.a);
}