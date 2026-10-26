// JSX port of hyperframes-launches/vfx-heygen-combined (index.html + the
// nested compositions/vfx-text-cursor.html intro) — 788 frames, 26.2667 s.
//
// Timeline layout (source master timeline, index.html:1261-1601):
//   0 .. 6.48    intro sub-composition (NOT shifted; == source local time)
//   5.58 + t     every main-timeline beat (tl.shiftChildren(5.58))
//   key beats (source script time -> absolute = +5.58):
//     theater in 0.04 -> 5.62   phY 0.08 -> 5.66    spin 2.2 -> 7.78
//     push 3.43 -> 9.01         phoneExit 4.04 -> 9.62
//     promptTransition 4.08 -> 9.66   typing/pan 4.13 -> 9.71
//     topbar fade 5.59 -> 11.17      send 5.69 -> 11.27
//     cursor 5.8/6.1 -> 11.38/11.68  click/release 6.5/6.59 -> 12.08/12.17
//     send exit 6.68 -> 12.26        game-change 6.96 -> 12.54
//     CHROMATIC_START 7.92 -> 13.5   RGB_ROTATE 9.325 -> 14.905
//     BLOOM_SETTLE 10.15 -> 15.73    RGB_EXIT 10.45 -> 16.03
//     CTA 10.76 -> 16.34             PORTAL_START 11.44 -> 17.02
//     radius->1.8 @18.9 -> 24.48     energy->0 @19.85 -> 25.43
//     portal hidden @20.35 -> 25.93  end 26.2667
//
// Known approximations (documented, unavoidable on this engine):
//   - The Three.js DRACO GLB iPhone is re-authored as a 2D body driven by the
//     exact source camera projection (fov 34, S state incl. drift/cY/clY/phZ).
//   - UnrealBloomPass on the chromatic split has no composer equivalent and
//     is NOT approximated: a 28-ring-tap kernel measured ~6 s/frame (87
//     evals/px) and, with the sting march, blew the 30-min render budget.
//     CHROM stays the 3-eval additive form of the shipped actC XML (avg_p8
//     .1698 over its act); the split reads slightly dimmer than ref.
//   - Render-budget early-outs (bit-exact, no pixel change): STING at
//     effectMix==0 reduces to a 1-eval logo draw (STING_LOGOONLY); the sting
//     canvas skips its draw at t=3.1 where its opacity tween lands on 0.
//   - The intro ray-march grain is a sin-hash field: cross-backend bit
//     differences are inherent (same kernel as the shipped actA XML).
//   - bg.mp4 / bg-2.mp4 play from composition 0 via freeze-frame padded
//     copies (JSX <video> has no data-start): bg-2-pad.mp4 (3.1 s prefix),
//     bg-pad.mp4 (5.6 s prefix) — content frames land on their ref times.
//   - Procedural seg() channels use the GSAP power convention (power1=Quad,
//     power2=Cubic, power3=Quart), matching the engine's ease strings
//     (easing.rs) so hand-written channels and timeline tweens agree.

import { useEffect, useGSAP, useRef } from 'opencat/runtime';
import * as THREE from 'three';
import { GLTFLoader, DRACOLoader } from 'three';
import { gsap } from 'gsap';
import CanvasKitInit from 'canvaskit-wasm';
import { SplitText } from 'gsap/SplitText';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 26.2667 };

export const fonts = {
  default: 'inter',
  faces: [
    { id: 'inter', family: 'Inter', path: 'assets/fonts/Inter-500v.ttf' },
    { id: 'inter400', family: 'Inter', path: 'assets/fonts/Inter-400.ttf' },
    { id: 'inter600', family: 'Inter', path: 'assets/fonts/Inter-600.ttf' },
    { id: 'inter700', family: 'Inter', path: 'assets/fonts/Inter-700.ttf' },
    { id: 'inter800', family: 'Inter', path: 'assets/fonts/Inter-800.ttf' },
    { id: 'inter900', family: 'Inter', path: 'assets/fonts/Inter-900.ttf' },
    { id: 'mono', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-500.ttf' },
    /* the shipped TTF had a broken legacy name table (typo family
       "Big Shoulders Display Thin", nameID1 "...Thin Black"); repaired in
       place to family "Big Shoulders Display" / usWeightClass 900 so the
       manifest family and the measure font string match the source CSS. */
    { id: 'big', family: 'Big Shoulders Display', path: 'assets/BigShouldersDisplay-900.ttf' },
    { id: 'serifd', family: 'DM Serif Display', path: 'assets/DMSerifDisplay-Italic.ttf' },
  ],
};

/* ══ shared per-frame helpers ═════════════════════════════════════════ */
var PI = Math.PI;
function clamp01(v) { return v < 0 ? 0 : (v > 1 ? 1 : v); }
function lerp(a, b, t) { return a + (b - a) * clamp01(t); }
function seg(t, start, dur, ease, from, to) {
  if (t <= start) return from;
  return from + (to - from) * ease((t - start) / dur);
}
/* GSAP power eases (procedural state channels). GSAP convention — power1 =
   Quad, power2 = Cubic, power3 = Quart — the same mapping the engine uses for
   the declarative ease strings (easing.rs:105-126), so the seg()-driven
   channels and the timeline tweens agree. */
function p1in(t)  { return Math.pow(clamp01(t), 2); }
function p1out(t) { t = clamp01(t); return 1 - (1 - t) * (1 - t); }
function p1io(t)  { t = clamp01(t); return t < 0.5 ? 2 * t * t : 1 - Math.pow(-2 * t + 2, 2) / 2; }
function p2out(t) { return 1 - Math.pow(1 - clamp01(t), 3); }
function p2io(t)  { t = clamp01(t); return t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2; }
function p2in(t)  { return Math.pow(clamp01(t), 3); }
function p3out(t) { return 1 - Math.pow(1 - clamp01(t), 4); }
function p3in(t)  { return Math.pow(clamp01(t), 4); }
function p3io(t)  { t = clamp01(t); return t < 0.5 ? 8 * t * t * t * t : 1 - Math.pow(-2 * t + 2, 4) / 2; }
function expoOut(t) { t = clamp01(t); return t >= 1 ? 1 : 1 - Math.pow(2, -10 * t); }
/* source breatheEase = cubic-bezier(.37,0,.63,1) — Newton solver, verbatim
   port of index.html cbz() */
function cbz(p1x, p1y, p2x, p2y) {
  var ax = 3 * p1x - 3 * p2x + 1;
  var bx = 3 * p2x - 6 * p1x;
  var cx = 3 * p1x;
  var ay = 3 * p1y - 3 * p2y + 1;
  var by = 3 * p2y - 6 * p1y;
  var cy = 3 * p1y;
  return function (t) {
    var s = t;
    for (var i = 0; i < 12; i++) {
      var x = ((ax * s + bx) * s + cx) * s - t;
      var dx = (3 * ax * s + 2 * bx) * s + cx;
      if (Math.abs(dx) < 1e-10) break;
      s -= x / dx;
    }
    s = Math.max(0, Math.min(1, s));
    return ((ay * s + by) * s + cy) * s;
  };
}
var breathe = cbz(0.37, 0.0, 0.63, 1.0);

/* THREE.PerspectiveCamera.lookAt basis (== engine geom::camera_basis):
   F = norm(L−C), R = norm(F×up), U = R×F. Screen ray for a pixel:
   D = F + sx·tanH·aspect·R + sy·tanH·U. */
function camBasis(eye, look) {
  var fx = look[0] - eye[0], fy = look[1] - eye[1], fz = look[2] - eye[2];
  var fl = Math.sqrt(fx * fx + fy * fy + fz * fz);
  fx /= fl; fy /= fl; fz /= fl;
  var rx = -fz, rz = fx;                            // F × (0,1,0)
  var rl = Math.sqrt(rx * rx + rz * rz);
  rx /= rl; rz /= rl;
  var ux = 0 * fz - rz * fy;                        // R × F
  var uy = rz * fx - rx * fz;
  var uz = rx * fy - 0 * fx;
  return { F: [fx, fy, fz], R: [rx, 0, rz], U: [ux, uy, uz] };
}

/* prefix advance widths for caret tracks (engine measure == engine layout) */
function measurePrefixes(ctx, key, font, ls, str) {
  if (globalThis[key]) return globalThis[key];
  var MSR = ctx.surface(1, 1);
  MSR.font = font;
  MSR.letterSpacing = ls;
  var out = [];
  for (var i = 0; i <= str.length; i++) out.push(MSR.measureText(str.slice(0, i)).width);
  globalThis[key] = out;
  return out;
}

/* ══ SKSL kernels (official RuntimeEffect) ═══════════════════════════ */

/* intro sting — 1:1 actA port of vfx-text-cursor.html:449-546 (112-sample
   ray-march; hash/spectrum primitives; mask=alpha / colour=rgb texture).
   Child order follows sources: [image, colormask]. */
var STING_SKSL = `
uniform float u_w;
uniform float u_h;
uniform float u_scale;
uniform float u_logoY;
uniform float u_mix;
uniform float u_progress;
uniform shader u_image;
uniform shader u_mask;

half4 main(float2 xy) {
    float gx = xy.x / u_w;
    float gy = 1.0 - xy.y / u_h;
    float scale = u_scale;
    float lx = (gx - 0.5) / scale + 0.5;
    float ly = (gy - 0.5) / scale + 0.5 + u_logoY / u_h;
    float mask = u_mask.eval(float2(lx * u_w, (1.0 - ly) * u_h)).a;
    float4 logo = u_image.eval(float2(lx * u_w, (1.0 - ly) * u_h));
    /* p = xy*2-1 (GLSL y-up), p.x *= aspect — source vfx-text-cursor.html:499 */
    float ax = (gx * 2.0 - 1.0) * (u_w / u_h);
    float ay = gy * 2.0 - 1.0;
    float progress = u_progress;
    float mpx = (progress * 2.0 - 1.0) * (u_w / u_h);
    float mpy = 0.0;
    float dax = (mpx - ax) / 112.0;
    float day = (mpy - ay) / 112.0;
    float acc = 0.0;
    float rx = ax;
    float ry = ay;
    for (float i = 0.0; i < 112.0; i++) {
        rx += dax;
        ry += day;
        float h1 = fract(sin((rx * 489.0 + ry * 589.0 + i * 58.0)) * 492.0) * 2.0 - 1.0;
        float h2 = fract(sin(((rx + 1.0) * 489.0 + (ry + 1.0) * 589.0 + (i + 1.0) * 58.0)) * 492.0) * 2.0 - 1.0;
        rx += h1 * 0.5 / 112.0;
        ry += h2 * 0.5 / 112.0;
        float bx = rx / (u_w / u_h) * 0.5 + 0.5;
        float by = ry * 0.5 + 0.5;
        float rlx = (bx - 0.5) / scale + 0.5;
        float rly = (by - 0.5) / scale + 0.5 + u_logoY / u_h;
        acc += u_mask.eval(float2(rlx * u_w, (1.0 - rly) * u_h)).a / 112.0;
    }
    float cr = cos((cos(acc * 4.9) - 0.0) * 0.6 * 3.141593) * acc * 3.65 - acc * 0.24;
    float cg = cos((cos(acc * 4.9) - 0.5) * 1.0 * 3.141593) * acc * 3.65 - acc * 0.24;
    float cb = cos((cos(acc * 4.9) - 1.0) * 0.5 * 3.141593) * acc * 3.65 - acc * 0.24;
    cr = max(cr - 0.035, 0.0) * 1.16;
    cg = max(cg - 0.035, 0.0) * 1.16;
    cb = max(cb - 0.035, 0.0) * 1.16;
    float sweep = exp(-pow((gx - progress) * 6.2, 2.0));
    float textProx = smoothstep(0.006, 0.34, acc);
    float longRay = smoothstep(0.004, 0.18, acc);
    float sx = progress * 1.8;
    float swR = 0.14 + cos((sx - 0.0) * 0.6 * 3.141593) * 0.28;
    float swG = 0.65 + cos((sx - 0.5) * 1.0 * 3.141593) * 0.28;
    float swB = 0.85 + cos((sx - 1.0) * 0.5 * 3.141593) * 0.28;
    float lrR = cos((progress + acc * 2.0 - 0.0) * 0.6 * 3.141593);
    float lrG = cos((progress + acc * 2.0 - 0.5) * 1.0 * 3.141593);
    float lrB = cos((progress + acc * 2.0 - 1.0) * 0.5 * 3.141593);
    float sweepTerm = sweep * (0.34 + textProx * 2.35);
    cr += swR * sweepTerm + lrR * longRay * 0.58;
    cg += swG * sweepTerm + lrG * longRay * 0.58;
    cb += swB * sweepTerm + lrB * longRay * 0.58;
    float logoTerm = acc * 1.18 + sweep * mask * 0.92;
    cr += logo.r * logoTerm;
    cg += logo.g * logoTerm;
    cb += logo.b * logoTerm;
    float mixv = u_mix;
    cr *= mixv; cg *= mixv; cb *= mixv;
    float lm = smoothstep(0.02, 0.75, logo.a);
    cr = mix(cr, logo.r, lm);
    cg = mix(cg, logo.g, lm);
    cb = mix(cb, logo.b, lm);
    float white = smoothstep(0.04, 0.38, mask) * 0.12 * mixv;
    cr += white; cg += white; cb += white;
    /* static grain field (vfx-text-cursor.html:539-542). hash(vec3(uv.xyy +
       mouse.x*0.0007)) with mouse.x = progress*1920: uv is GL y-up, so
       gy = 1 - xy.y/u_w above. */
    float grain = fract(sin(gx * 489.0 + gy * 647.0 + progress * 1527.168) * 492.0) * 2.0 - 1.0;
    float ag = abs(grain);
    float dust = smoothstep(0.02, 0.38, acc) + smoothstep(0.03, 0.22, mask);
    float dustTerm = grain * 0.07 * (0.42 + dust) * mixv;
    float dustT = smoothstep(0.02, 0.5, acc) * ag * 0.045 * mixv;
    float gsR = cos((progress + grain - 0.0) * 0.6 * 3.141593);
    float gsG = cos((progress + grain - 0.5) * 1.0 * 3.141593);
    float gsB = cos((progress + grain - 1.0) * 0.5 * 3.141593);
    return half4(clamp(cr + dustTerm + gsR * dustT, 0.0, 1.0),
                 clamp(cg + dustTerm + gsG * dustT, 0.0, 1.0),
                 clamp(cb + dustTerm + gsB * dustT, 0.0, 1.0), 1.0);
}`;

/* exact early-out of STING at effectMix==0 (frames 0..0.72s): every ray /
   sweep / grain term carries *mixv = 0, white-core too, so the shader
   reduces to mix(0, logo.rgb, smoothstep(.02,.75,logo.a)) — a single eval.
   Bit-identical to STING with u_mix=0, ~12s/frame → ~0.3s (render-budget). */
var STING_LOGOONLY_SKSL = `
uniform float u_w;
uniform float u_h;
uniform float u_scale;
uniform float u_logoY;
uniform shader u_image;

half4 main(float2 xy) {
    float gx = xy.x / u_w;
    float gy = 1.0 - xy.y / u_h;
    float lx = (gx - 0.5) / u_scale + 0.5;
    float ly = (gy - 0.5) / u_scale + 0.5 + u_logoY / u_h;
    float4 logo = u_image.eval(float2(lx * u_w, (1.0 - ly) * u_h));
    float lm = smoothstep(0.02, 0.75, logo.a);
    return half4(logo.r * lm, logo.g * lm, logo.b * lm, 1.0);
}`;

/* chromatic RGB split — 1:1 actC port of index.html:1017-1090 (three
   additive channel planes, source lum formula, exact perspective).
   PERF: the ref's UnrealBloomPass (index.html:1066) is NOT re-implemented —
   this 3-eval kernel is exactly the shipped actC XML form that measured
   avg_p8 .1698 over its 387-frame act (docs/handoffs §12.4: ref is somewhat
   brighter inside the split glyphs). sy follows the actC-XML-validated
   convention (xy.y/u_h*2-1, the flipY'd CanvasTexture sample below closes
   the mapping). */
var CHROM_SKSL = `
uniform float u_w;
uniform float u_h;
uniform float u_pw;
uniform float u_ph;
uniform float u_scale;
uniform float u_alpha;
uniform float u_cx;
uniform float u_cy;
uniform float u_cz;
uniform float u_fx;
uniform float u_fy;
uniform float u_fz;
uniform float u_rx;
uniform float u_ry;
uniform float u_rz;
uniform float u_ux;
uniform float u_uy;
uniform float u_uz;
uniform float u_px0;
uniform float u_py0;
uniform float u_pz0;
uniform float u_cs0;
uniform float u_sn0;
uniform float u_px1;
uniform float u_py1;
uniform float u_pz1;
uniform float u_cs1;
uniform float u_sn1;
uniform float u_px2;
uniform float u_py2;
uniform float u_pz2;
uniform float u_cs2;
uniform float u_sn2;
uniform shader u_image;

half4 main(float2 xy) {
    float sx = xy.x / u_w * 2.0 - 1.0;
    float sy = xy.y / u_h * 2.0 - 1.0;
    float Dx = u_fx + sx * u_rx + sy * u_ux;
    float Dy = u_fy + sx * u_ry + sy * u_uy;
    float Dz = u_fz + sx * u_rz + sy * u_uz;
    float a = u_alpha;
    float tR = -(((u_cx - u_px0) * u_sn0 + (u_cz - u_pz0) * u_cs0)) /
               (Dx * u_sn0 + Dz * u_cs0);
    float exR = (u_cx + tR * Dx) - u_px0;
    float eyR = (u_cy + tR * Dy) - u_py0;
    float ezR = (u_cz + tR * Dz) - u_pz0;
    float lxR = (exR * u_cs0 - ezR * u_sn0) / u_scale;
    float lyR = eyR / u_scale;
    float4 sR = u_image.eval(float2(clamp(lxR / u_pw + 0.5, 0.0, 1.0) * u_w,
                                    clamp(0.5 - lyR / u_ph, 0.0, 1.0) * u_h));
    float tG = -(((u_cx - u_px1) * u_sn1 + (u_cz - u_pz1) * u_cs1)) /
               (Dx * u_sn1 + Dz * u_cs1);
    float exG = (u_cx + tG * Dx) - u_px1;
    float eyG = (u_cy + tG * Dy) - u_py1;
    float ezG = (u_cz + tG * Dz) - u_pz1;
    float lxG = (exG * u_cs1 - ezG * u_sn1) / u_scale;
    float lyG = eyG / u_scale;
    float4 sG = u_image.eval(float2(clamp(lxG / u_pw + 0.5, 0.0, 1.0) * u_w,
                                    clamp(0.5 - lyG / u_ph, 0.0, 1.0) * u_h));
    float tB = -(((u_cx - u_px2) * u_sn2 + (u_cz - u_pz2) * u_cs2)) /
               (Dx * u_sn2 + Dz * u_cs2);
    float exB = (u_cx + tB * Dx) - u_px2;
    float eyB = (u_cy + tB * Dy) - u_py2;
    float ezB = (u_cz + tB * Dz) - u_pz2;
    float lxB = (exB * u_cs2 - ezB * u_sn2) / u_scale;
    float lyB = eyB / u_scale;
    float4 sB = u_image.eval(float2(clamp(lxB / u_pw + 0.5, 0.0, 1.0) * u_w,
                                    clamp(0.5 - lyB / u_ph, 0.0, 1.0) * u_h));
    float lumR = sR.r * 0.6 + max(sR.g, sR.b) * 0.4;
    float lumG = sG.g * 0.6 + max(sG.r, sG.b) * 0.4;
    float lumB = sB.b * 0.6 + max(sB.r, sB.g) * 0.4;
    float bcR = min(sR.r * lumR * a, 1.0);
    float bcG = min(sG.g * lumG * a, 1.0);
    float bcB = min(sB.b * lumB * a, 1.0);
    return half4(bcR, bcG, bcB, 1.0);
}`;

/* portal reveal — full port of index.html:1161-1218 (edge noise, distortion,
   RGB fringe on the front texture, ring glow, sparks). Source noise() is
   [0,1]-range value noise; oc_noise2 is the same sin-hash lattice mapped
   to [-1,1], so source_noise = (oc_noise2 + 1) * 0.5. Reversed smoothsteps
   are inlined. */
var PORTAL_SKSL = `
uniform float u_w;
uniform float u_h;
uniform float u_radius;
uniform float u_energy;
uniform float u_time;
uniform float u_ew;
uniform float u_chroma;
uniform float u_distort;
uniform shader u_image;

float oc_hash2(float2 p) {
    return fract(sin(dot(p, float2(127.1, 311.7))) * 43758.5453123);
}
float oc_noise2(float2 p) {
    float2 i = floor(p);
    float2 f = fract(p);
    float2 u = f * f * (3.0 - 2.0 * f);
    float a = oc_hash2(i);
    float b = oc_hash2(i + float2(1.0, 0.0));
    float c = oc_hash2(i + float2(0.0, 1.0));
    float d = oc_hash2(i + float2(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y) * 2.0 - 1.0;
}

half4 main(float2 xy) {
    float px = (xy.x / u_w - 0.5) * (u_w / u_h);
    float py = xy.y / u_h - 0.5;
    float dist = length(float2(px, py));
    float ang = atan(py, px);
    float eN = (oc_noise2(float2(ang * 3.0, u_time * 2.0)) + 1.0) * 0.5 * 0.03 * u_energy;
    float edge = u_radius + eN;
    float eD = abs(dist - edge);
    float tD = clamp((eD - u_ew * 1.5) / (0.0 - u_ew * 1.5), 0.0, 1.0);
    tD = tD * tD * (3.0 - 2.0 * tD);
    float dMask = tD * u_distort;
    float dl = length(float2(px + 0.0001, py + 0.0001));
    float dirX = (px + 0.0001) / dl;
    float dirY = (py + 0.0001) / dl;
    float wob = sin(ang * 8.0 + u_time * 5.0) * dMask;
    float offX = dirX * wob * u_w;
    float offY = dirY * wob * u_h;
    float tC = clamp((eD - u_ew * 2.0) / (0.0 - u_ew * 2.0), 0.0, 1.0);
    tC = tC * tC * (3.0 - 2.0 * tC);
    float cMask = tC * u_chroma;
    float chX = dirX * cMask * u_w;
    float chY = dirY * cMask * u_h;
    float4 front = u_image.eval(float2(xy.x + offX, xy.y + offY));
    float fr = u_image.eval(float2(xy.x + offX + chX, xy.y + offY + chY)).r;
    float fg = front.g;
    float fb = u_image.eval(float2(xy.x + offX - chX, xy.y + offY - chY)).b;
    float mK = clamp((dist - (edge - 0.008)) / 0.016, 0.0, 1.0);
    float mask = mK * mK * (3.0 - 2.0 * mK);
    float tI = clamp((eD - u_ew) / (0.0 - u_ew), 0.0, 1.0);
    tI = tI * tI * (3.0 - 2.0 * tI);
    float tO = clamp((eD - u_ew * 2.5) / (u_ew * 0.5 - u_ew * 2.5), 0.0, 1.0);
    tO = tO * tO * (3.0 - 2.0 * tO);
    float glow = tI * tO * u_energy;
    float phase = fract(ang / 6.2832 + u_time * 0.4);
    float m = sin(phase * 6.2832) * 0.5 + 0.5;
    float rr = 0.486 + (0.290 - 0.486) * m;
    float rg = 0.227 + (0.565 - 0.227) * m;
    float rb = 0.929 + (1.000 - 0.929) * m;
    float sp = (oc_noise2(float2(ang * 12.0, u_time * 8.0)) + 1.0) * 0.5;
    sp = pow(sp, 4.0) * tI * u_energy * 2.0;
    float cr = fr * mask + rr * glow * 2.5 + sp * 1.0;
    float cg = fg * mask + rg * glow * 2.5 + sp * 0.9;
    float cb = fb * mask + rb * glow * 2.5 + sp * 1.0;
    float alpha = clamp(mask + glow * 0.5 + sp * 0.3, 0.0, 1.0);
    return half4(cr, cg, cb, alpha);
}`;

/* ══ components (tree order == paint order == hook order) ════════════ */

/* V0: main bg.mp4 (source #bg-video data-start 5.58; padded file plays from
   composition 0, content frame 0 lands at 5.6) */
function MainBg() {
  useGSAP((ctx) => {
    var T = gsap.timeline();
    T.set('bg-video', { opacity: 0 }, 0);
    T.to('bg-video', { opacity: 1, duration: 0.42, ease: 'power1.out' }, 5.58);
  });
  return (
    <video id="bg-video" path="assets/bg-pad.mp4" duration={10.6}
      class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover opacity-0" />
  );
}

/* V1: #theater — the source's THREE render of models/iphone.glb, ported 1:1
   to the official three package (same scene as index.html:507-846):
     · PerspectiveCamera(34, 1920/1080, 0.1, 100) at S.cX/cY/cZ → S.clX/clY/clZ
     · ACESFilmic tone mapping, exposure 1.12, sRGB output
     · AmbientLight 0x222233/0.55 + DirectionalLight 0xffffff/2.3 @(-3.5,4.5,5)
     · one equirect environment (a CanvasTexture gradient in the source; here a
       surface drawn with the same stops and handed over as a PMREM source)
     · the phone's screen material swapped for an unlit canvas texture that
       redraws the HeyGen page every frame at S.scroll
   The GLB loads through the official GLTFLoader (parse + texture decode ride
   the engine's fetch/createImageBitmap shims; see asset_api.js).
   Entry/exit (y 340 blur 18, x −1220 blur 22) still ride CSS on the outer
   wrapper, exactly like the source's #theater tweens. */
function Theater() {
  const canvas = useRef();
  /* 进场/出场（index.html:1264/1298-1307）作用在外层节点上：y 340 blur 18
     → 0、x −1220 blur 22 并淡出。t 已平移 +5.58（见文件头的源时 → 绝对时表）。 */
  useGSAP((ctx) => {
    var T = gsap.timeline();
    T.set('phone', { y: 340, opacity: 0, filter: 'blur(18px)' }, 0);
    T.to('phone', { y: 0, opacity: 1, filter: 'blur(0px)', duration: 0.74, ease: 'expo.out' }, 5.62);
    T.to('phone', { x: -1220, filter: 'blur(22px)', duration: 0.53, ease: 'expo.out' }, 9.62);
    T.to('phone', { opacity: 0, duration: 0.18, ease: 'power2.out' }, 9.62);
  });
  useEffect(() => {
    var t = ctx.current.time;
    var ps = Math.max(0, t) - 5.58; // source script time

    /* ── S state (index.html:829-846 + the master tweens) ─────────────── */
    var phY = seg(ps, 0.08, 0.95, expoOut, -3.45, 0.02);
    var phRX = ps < 0.18 ? 0.01 : lerp(0.01, -0.012, breathe((ps - 0.18) / 0.78));
    var phRY;
    if (ps < 0.18) phRY = PI + 0.04;
    else if (ps < 0.96) phRY = lerp(PI + 0.04, PI - 0.035, breathe((ps - 0.18) / 0.78));
    else if (ps < 2.2) phRY = PI - 0.035;
    else if (ps < 3.93) phRY = (PI - 0.035) + 4 * PI * ((ps - 2.2) / 1.73);
    else phRY = 5 * PI - 0.035;
    /* phRZ: the spin segment writes -0.015 -> 0.006 over 2.2..3.93, but the
       later push tween (index.html:1294, phRZ PI/2 power3.inOut @3.43) wins
       per-frame once running (GSAP no-overwrite). Its from-value is the spin
       channel's value at 3.43: -0.015 + 0.021*(3.43-2.2)/1.73 = -0.000069. */
    var phRZ;
    if (ps < 2.2) phRZ = -0.015;
    else if (ps < 3.43) phRZ = lerp(-0.015, 0.006, (ps - 2.2) / 1.73);
    else phRZ = lerp(-0.000069, PI / 2, p3io((ps - 3.43) / 0.85));
    var cZ = ps < 0.18 ? 6.6 : lerp(6.6, 6.35, breathe((ps - 0.18) / 0.78));
    cZ = seg(ps, 3.43, 0.85, p3io, cZ, 3.85);
    var cY = seg(ps, 3.43, 0.85, p3io, 0.16, 0.12);
    var d = Math.max(0, Math.min(5, ps));
    var scroll = seg(ps, 2.05, 2.45, p1out, 0, 0.9);

    /* ── screen canvas (index.html:636-760 drawHeyGenPage) ────────────── */
    var SCREEN_W = 390, SCREEN_H = 844;
    var screen = ctx.surface(SCREEN_W, SCREEN_H);
    drawHeyGenPage(screen, SCREEN_W, SCREEN_H, scroll);

    /* ── scene graph (built once per realm; the surface handle is stable) ─ */
    var g = ctx.__vhcTheater;
    if (!g) {
      var renderer3d = new THREE.WebGLRenderer({ canvas: canvas.current });
      renderer3d.setSize(1920, 1080);
      renderer3d.setPixelRatio(1);
      renderer3d.toneMapping = THREE.ACESFilmicToneMapping;
      renderer3d.toneMappingExposure = 1.12;
      renderer3d.outputColorSpace = THREE.SRGBColorSpace;
      renderer3d.setClearColor(0x050508, 0);

      var scene3d = new THREE.Scene();
      scene3d.background = null;
      var camera3d = new THREE.PerspectiveCamera(34, 1920 / 1080, 0.1, 100);

      var env = ctx.surface(1024, 512);
      drawEnvGradient(env, 1024, 512);
      var envTex = new THREE.CanvasTexture(env);
      envTex.mapping = THREE.EquirectangularReflectionMapping;
      /* 不设 colorSpace：CanvasTexture 缺省 NoColorSpace（r181
         Texture.js:48），env 字节按线性光进 PMREM——与 r147 的
         LinearEncoding 缺省同义。设成 SRGBColorSpace 会多一次 EOTF，
         渐变 env 的平均能量掉到 1/3，机身随之从 sRGB 67 掉到 20。 */
      var pmrem = new THREE.PMREMGenerator(renderer3d);
      scene3d.environment = pmrem.fromEquirectangular(envTex).texture;
      envTex.dispose();
      pmrem.dispose();

      /* ── r147 补偿（唯一的非原版写法，全部集中在这里）─────────────────
         参考片 renders/vfx-heygen-combined_2026-10-05_swgl.mp4 是
         index.html 用 three r147 渲的（index.html:8）。引擎是 r181 语义，
         两处口径差要在这里显式补回来：

         ① 灯光单位：r147 WebGLLights.js 里
            `scaleFactor = physicallyCorrectLights ? 1 : Math.PI`，而
            `physicallyCorrectLights` 缺省 false ⇒ 四类灯的 intensity 在
            进 uniform 前一律 ×π（r155 删掉了这个缩放，r181 没有）。
         ② 灯光颜色：r147 `ColorManagement.legacyMode = true`
            （ColorManagement.js:r147 顶部）⇒ `new Color(0x222233)` 不做
            sRGB→线性解码，hex 字节直接当线性值用。r181 缺省
            `enabled = true` 会解码，蓝色通道被压暗 3 倍且色相偏。

         原版两侧的写法是 `new THREE.AmbientLight(0x222233, 0.55)` 等四处
         （index.html:538-550）。 */
      var R147_LIGHT_UNIT = Math.PI;
      function r147Color(hex) {
        return new THREE.Color().setRGB(
          ((hex >> 16) & 255) / 255,
          ((hex >> 8) & 255) / 255,
          (hex & 255) / 255
        );
      }
      scene3d.add(new THREE.AmbientLight(r147Color(0x222233), 0.55 * R147_LIGHT_UNIT));
      var keyLight = new THREE.DirectionalLight(r147Color(0xffffff), 2.3 * R147_LIGHT_UNIT);
      keyLight.position.set(-3.5, 4.5, 5.0);
      scene3d.add(keyLight);
      var fill1 = new THREE.PointLight(r147Color(0x7c9cff), 3.2 * R147_LIGHT_UNIT, 15);
      fill1.position.set(4.2, 2.4, 3.0);
      scene3d.add(fill1);
      var fill2 = new THREE.PointLight(r147Color(0xd7ff3f), 1.4 * R147_LIGHT_UNIT, 12);
      fill2.position.set(-2.8, -0.8, 3.8);
      scene3d.add(fill2);

      var screenTex = new THREE.CanvasTexture(screen);
      screenTex.minFilter = THREE.LinearFilter;
      screenTex.magFilter = THREE.LinearFilter;
      screenTex.generateMipmaps = false;

      /* the GLB: official GLTFLoader, normalized to 3.2 world units tall and
         centred, exactly like index.html:788-828 (Box3 -> scale 3.2/size.y ->
         position -= center; screen mesh found by name/material probe). The
         asset is the baseline iphone.glb run through gltf-transform copy:
         byte-identical vertex data with the DRACO compression removed —
         r181 DRACOLoader hard-requires Web Workers, which the engine realm
         doesn't have. */
      var phoneGrp = new THREE.Group();
      scene3d.add(phoneGrp);
      var loader = new GLTFLoader();
      loader.load('assets/iphone-decoded.glb', function (gltf) {
        var model = gltf.scene;
        var box = new THREE.Box3().setFromObject(model);
        var size = box.getSize(new THREE.Vector3());
        var scale = 3.2 / size.y;
        model.scale.setScalar(scale);
        box.setFromObject(model);
        model.position.sub(box.getCenter(new THREE.Vector3()));

        var screenApplied = false;
        model.traverse(function (child) {
          if (!child.isMesh) return;
          var name = (child.name || '').toLowerCase();
          var matName = child.material && child.material.name ? child.material.name.toLowerCase() : '';
          if ((child.material && child.material.emissiveMap) || name.indexOf('screen') >= 0 || matName.indexOf('screen') >= 0 || matName.indexOf('display') >= 0) {
            child.material = new THREE.MeshBasicMaterial({ map: screenTex });
            screenApplied = true;
          }
        });
        if (!screenApplied) {
          model.traverse(function (child) {
            if (!child.isMesh || screenApplied) return;
            if (child.material && child.material.emissive) {
              child.material = new THREE.MeshBasicMaterial({ map: screenTex });
              screenApplied = true;
            }
          });
        }

        phoneGrp.add(model);
      }, undefined, function (err) {
        throw new Error('iphone.glb load failed: ' + err);
      });

      g = ctx.__vhcTheater = {
        scene: scene3d, camera: camera3d, renderer: renderer3d,
        phone: phoneGrp, screenTex: screenTex, grain: 0,
      };
    }

    /* index.html:644/746 — the screen canvas is redrawn every frame, so the
       texture must re-upload every frame too (engine-side texture cache keys
       on content, unchanged pixels skip the copy). */
    g.screenTex.needsUpdate = true;

    g.phone.position.set(
      Math.sin(d * 0.56) * 0.015,
      phY + Math.sin(d * 0.72) * 0.009,
      0.45 + Math.cos(d * 0.44) * 0.008
    );
    g.phone.rotation.set(phRX, phRY, phRZ);
    g.camera.position.set(0, cY, cZ);
    g.camera.lookAt(0, 0.04, 0);
    g.renderer.render(g.scene, g.camera);
  });

  return (
    <div id="phone" class="absolute left-0 top-0 opacity-0">
      <canvas id="theater" ref={canvas} class="absolute left-0 top-0 w-[1920px] h-[1080px]" />
    </div>
  );
}

/* index.html:533-563 的环境贴图：1024×512 线性渐变 + 三条 screen 混合的白线。
   surface 只有纯色/线性渐变填充，`globalCompositeOperation` 用"画出比底色更
   亮的色"近似（参考那三条线的可见效果就是提亮）。 */
function drawEnvGradient(s, W, H) {
  var grad = s.createLinearGradient(0, 0, W, H);
  grad.addColorStop(0, '#020307');
  grad.addColorStop(0.22, '#1f2937');
  grad.addColorStop(0.36, '#ffffff');
  grad.addColorStop(0.52, '#15161a');
  grad.addColorStop(0.68, '#d7ff3f');
  grad.addColorStop(0.84, '#28406f');
  grad.addColorStop(1, '#020307');
  s.fillStyle = grad;
  s.fillRect(0, 0, W, H);
  s.fillStyle = 'rgba(255,255,255,0.18)';
  s.fillRect(160, 98, 640, 4);
  s.fillStyle = 'rgba(255,255,255,0.18)';
  s.fillRect(0, 280, 760, 3);
  s.fillStyle = 'rgba(215,255,63,0.12)';
  s.fillRect(310, 180, 520, 5);
}

/* index.html:596-760 drawHeyGenPage, transcribed: same box geometry, same
   strings, same type scale. Only the primitives change (surface has
   fillRect/roundRect/linear gradients/text at the alphabetic baseline). */
function drawHeyGenPage(s, W, H, scrollProgress) {
  var scroll = scrollProgress * 980;
  s.clearRect(0, 0, W, H);

  s.fillStyle = '#f7f4ee';
  s.fillRect(0, 0, W, H);

  s.save();
  s.translate(0, -scroll);

  s.font = '900 22px Inter';
  s.fillStyle = '#111111';
  s.fillText('HeyGen', 24, 46);
  pageLabel(s, 'Get Started', 264, 20, 102, 34, '#d7ff3f', '#111111');

  var headline = ['Turn your', 'ideas into', 'videos in', 'minutes'];
  s.font = '900 48px Inter';
  s.fillStyle = '#111111';
  for (var h = 0; h < headline.length; h += 1) s.fillText(headline[h], 24, 118 + h * 50);

  s.font = '500 16px Inter';
  s.fillStyle = 'rgba(17,17,17,0.68)';
  var body = [
    'Go from script, image, presentation,',
    'or PDF to finished video. No cameras,',
    'no crew, no editing skills required.',
  ];
  for (var b = 0; b < body.length; b += 1) s.fillText(body[b], 24, 346 + b * 22);

  pageLabel(s, 'AI Video Generator', 24, 420, 154, 38, '#111111', '#f7f4ee');
  pageLabel(s, 'Free to start', 188, 420, 122, 38, '#d7ff3f', '#111111');

  /* hero card (index.html:673-706): rounded rect with a 3-stop gradient */
  var hero = s.createLinearGradient(24, 494, 366, 720);
  hero.addColorStop(0, '#10131c');
  hero.addColorStop(0.5, '#4d78ff');
  hero.addColorStop(1, '#d7ff3f');
  s.fillStyle = hero;
  s.roundRect(24, 494, 342, 226, 28);
  s.fillStyle = 'rgba(255,255,255,0.92)';
  s.font = '800 24px Inter';
  s.fillText('One-Shot Text-to-Video', 44, 556);
  s.font = '600 15px Inter';
  s.fillText('Type your idea. Click generate.', 44, 590);
  s.fillText('Get a share-ready video.', 44, 614);
  s.fillStyle = '#ffffff';
  s.roundRect(44, 642, 144, 44, 14);
  s.fillStyle = '#111111';
  s.font = '800 14px Inter';
  s.fillText('Generate', 76, 669);

  s.font = '850 28px Inter';
  s.fillStyle = '#111111';
  s.fillText('128M+ videos generated', 24, 784);
  s.fillText('102M+ avatars generated', 24, 826);
  s.fillText('17M+ videos translated', 24, 868);

  s.font = '700 13px Inter';
  s.fillStyle = 'rgba(17,17,17,0.54)';
  s.fillText('Trusted by millions worldwide', 24, 930);

  var logos = ['Miro', 'Workday', 'Coursera', 'HubSpot', 'Airtable', 'Wondershare'];
  for (var l = 0; l < logos.length; l += 1) {
    var lx = 24 + (l % 2) * 174;
    var ly = 958 + Math.floor(l / 2) * 58;
    s.fillStyle = '#ffffff';
    s.roundRect(lx, ly, 156, 42, 14);
    s.fillStyle = 'rgba(17,17,17,0.62)';
    s.font = '800 14px Inter';
    s.fillText(logos[l], lx + 18, ly + 27);
  }

  s.font = '900 31px Inter';
  s.fillStyle = '#111111';
  s.fillText('AI video creation', 24, 1196);
  s.fillText('just became your', 24, 1234);
  s.fillText('superpower', 24, 1272);

  pageFeature(s, 24, 1320, 342, 150, '#d7ff3f', 'Text to Video',
    'Create complete AI videos from a script with voiceovers, visuals, and avatars.');
  pageFeature(s, 24, 1490, 342, 150, '#4d78ff', 'Photo to Video',
    'Upload an image and script to create lifelike motion and natural lip-sync.');
  pageFeature(s, 24, 1660, 342, 150, '#ff7a59', 'Translate Video',
    'Dub content into 175+ languages with voice cloning and subtitles.');

  s.restore();

  /* top fade (index.html:748-757) */
  var fade = s.createLinearGradient(0, 0, 0, 92);
  fade.addColorStop(0, 'rgba(247,244,238,1)');
  fade.addColorStop(0.55, 'rgba(247,244,238,0.92)');
  fade.addColorStop(1, 'rgba(247,244,238,0)');
  s.fillStyle = fade;
  s.fillRect(0, 0, W, 92);
}

function pageLabel(s, text, x, y, w, h, fill, color) {
  s.fillStyle = fill;
  s.roundRect(x, y, w, h, h / 2);
  s.fillStyle = color;
  s.font = '700 12px Inter';
  s.fillText(text, x + 14, y + h / 2 + 4);
}

function pageFeature(s, x, y, w, h, accent, title, body) {
  s.fillStyle = '#ffffff';
  s.roundRect(x, y, w, h, 18);
  s.fillStyle = accent;
  s.roundRect(x + 16, y + 18, 42, 42, 14);
  s.fillStyle = '#111111';
  s.font = '800 19px Inter';
  s.fillText(title, x + 16, y + 82);
  s.fillStyle = 'rgba(17,17,17,0.58)';
  s.font = '500 13px Inter';
  var words = body.split(' ');
  var line = '', row = 0;
  for (var i = 0; i < words.length; i += 1) {
    var next = line ? line + ' ' + words[i] : words[i];
    if (s.measureText(next).width > w - 32 && line) {
      s.fillText(line, x + 16, y + 107 + row * 17);
      row += 1;
      line = words[i];
    } else {
      line = next;
    }
  }
  if (line) s.fillText(line, x + 16, y + 107 + row * 17);
}

/* V2: #finger-overlay — static CSS rotate(-75deg) origin 174px 0, enters
   offscreen bottom-right, sweeps to the screen, exits up */
function Finger() {
  useGSAP((ctx) => {
    var T = gsap.timeline();
    T.set('finger', { left: 1260, top: 1220, rotation: -75, transformOrigin: '174px 0' }, 0);
    T.to('finger', { left: 805, top: 532, duration: 0.82, ease: 'power3.out' }, 6.5);
    T.to('finger', { top: -1900, duration: 0.68, ease: 'power3.inOut' }, 7.43);
    T.set('finger', { opacity: 0 }, 8.13);
  });
  return (
    <image id="finger" path="assets/finger.png"
      class="absolute left-[1260px] top-[1220px] w-[820px] h-[1716px] drop-shadow-[0_28px_34px_rgba(0,0,0,0.26)]" />
  );
}

/* V3: #prompt-scene — composer UI, typing, send click, game-change copy */
function PromptScene() {
  useGSAP((ctx) => {
    var T = gsap.timeline();
    T.set('prompt-scene', { opacity: 0, x: 620, filter: 'blur(18px)' }, 0);
    T.to('prompt-scene', { x: 0, filter: 'blur(0px)', duration: 0.27, ease: 'power2.out' }, 9.66);
    T.to('prompt-scene', { opacity: 1, duration: 0.48, ease: 'power2.out' }, 9.66);
    T.set('prompt-app', { x: 560, y: -120, scale: 3.15, transformOrigin: '50% 92%' }, 0);

    /* typing "npx hyperframes add iphone-canvas" @4.13 dur 0.9 snap —
       SplitText reveal + caret on the measured prefix advance. SplitText
       wraps non-space characters only, so the char list is walked against
       the source string: the reveal time is keyed to the offset in CMD. */
    var CMD = 'npx hyperframes add iphone-canvas';
    var widths = measurePrefixes(ctx, '__vhc_cmdW', '400 34px "Inter"', '0px', CMD);
    var charEls = new SplitText(ctx.getNode('prompt-command'), { type: 'chars' }).chars;
    var i, ci = 0;
    for (i = 0; i < CMD.length; i++) {
      if (CMD.charAt(i) === ' ') continue;
      var el = charEls[ci++];
      if (!el) break;
      T.set(el, { opacity: 0 }, 0);
      T.set(el, { opacity: 1 }, 9.71 + (i / CMD.length) * 0.9);
    }
    T.set('prompt-caret', { opacity: 0, left: 34, top: 31 }, 0);
    T.set('prompt-caret', { opacity: 1 }, 9.71);
    for (i = 0; i < CMD.length; i++) {
      T.set('prompt-caret', { left: 34 + widths[i + 1] }, 9.71 + (i / CMD.length) * 0.9);
    }
    /* the source caret is a child of .prompt-composer-text, which fades out
       whole at 11.17 (opacity/blur/scale 0.86) — so the caret blink tween
       (@11.83 in the source) never renders; mirror the effective result */
    T.to('prompt-caret', { opacity: 0, duration: 0.2, ease: 'power3.inOut' }, 11.17);
    T.to('prompt-app', { x: -1260, duration: 1.78, ease: 'power1.in' }, 9.71);

    T.to('prompt-meta', { opacity: 1, duration: 0.18, ease: 'power1.out' }, 10.48);
    T.to('prompt-topbar', { opacity: 0, filter: 'blur(16px)', scale: 0.86, duration: 0.2, ease: 'power3.inOut', transformOrigin: '50% 50%' }, 11.17);
    T.to('prompt-command', { opacity: 0, filter: 'blur(16px)', scale: 0.86, transformOrigin: '50% 50%', duration: 0.2, ease: 'power3.inOut' }, 11.17);
    T.to('pc-attach', { opacity: 0, filter: 'blur(16px)', duration: 0.2, ease: 'power3.inOut' }, 11.17);
    T.to('prompt-meta', { opacity: 0, filter: 'blur(16px)', duration: 0.2, ease: 'power3.inOut' }, 11.17);
    /* composer washes out at 11.17 (index.html:1353): bg/border fade while the
       shadow holder fades. bg lands on the opaque page color — the composer
       keeps covering the holder's interior so the shadow only ever shows
       outside the box, like the browser's outer-shadow clipping. */
    T.to('prompt-composer', { backgroundColor: '#F5F4ED', borderColor: 'rgba(229,224,209,0)', duration: 0.2, ease: 'power3.inOut' }, 11.17);
    T.to('composer-shadow', { opacity: 0, duration: 0.2, ease: 'power3.inOut' }, 11.17);

    T.set('prompt-send', { x: 0, y: 0, scale: 1 }, 0);
    T.to('prompt-send', { y: -95, duration: 0.22, ease: 'power2.out' }, 11.27);
    T.to('prompt-send', { scale: 3, duration: 0.22, ease: 'power3.out', transformOrigin: '50% 50%' }, 11.27);
    T.to('send-glow', { opacity: 1, duration: 0.22, ease: 'power3.out' }, 11.27);

    T.set('click-cursor', { x: 500, y: 620, scale: 0.95, opacity: 0, rotation: -6 }, 0);
    T.to('click-cursor', { x: 280, y: 210, opacity: 1, duration: 0.28, ease: 'power3.out' }, 11.38);
    T.to('click-cursor', { x: 26, y: 38, duration: 0.46, ease: 'power3.inOut' }, 11.68);
    T.to('prompt-send', { scale: 2.62, duration: 0.08, ease: 'power2.in' }, 12.08);
    T.to('click-cursor', { scale: 0.82, duration: 0.08, ease: 'power2.in' }, 12.08);
    T.to('prompt-send', { scale: 3.12, duration: 0.16, ease: 'back.out(2.2)' }, 12.17);
    T.to('click-cursor', { scale: 0.95, duration: 0.16, ease: 'back.out(2.2)' }, 12.17);
    T.to('prompt-send', { y: -260, filter: 'blur(8px)', opacity: 0, duration: 0.28, ease: 'power2.in' }, 12.26);
    T.to('click-cursor', { y: -165, filter: 'blur(8px)', opacity: 0, duration: 0.28, ease: 'power2.in' }, 12.26);

    T.set('game-change-copy', { y: 220, opacity: 0, filter: 'blur(8px)' }, 0);
    T.to('game-change-copy', { y: 0, filter: 'blur(0px)', duration: 0.46, ease: 'expo.out', transformOrigin: '50% 50%' }, 12.54);
    T.to('game-change-copy', { opacity: 1, duration: 0.24, ease: 'power1.out' }, 12.54);
    T.set('game-change-copy', { opacity: 0 }, 13.5);  /* index.html:1432 */
  });
  return (
    <div id="prompt-scene" class="absolute left-0 top-0 w-[1920px] h-[1080px] overflow-hidden bg-[#F5F4ED] opacity-0">
      <div id="prompt-app" class="absolute left-0 top-0 w-[1920px] h-[1080px] flex flex-col">
        <div id="prompt-topbar" class="h-[64px] px-[28px] flex items-center justify-between border-b-[1px] border-[#EBE7DB] bg-[#FAF9F5]">
          <div id="ptb-left" class="flex items-center gap-[14px]">
            <text id="ptb-star" class="text-[#D97757] text-[26px] font-[inter900] font-black">{'\u2726'}</text>
            <text id="ptb-name" class="text-[#3D3929] text-[18px] font-[inter] font-medium">HyperFrames Project</text>
          </div>
          <div id="ptb-right" class="flex items-center gap-[14px]">
            <div id="ptb-pill" class="px-[14px] py-[8px] rounded-full bg-[#F0ECE0] border-[1px] border-[#E5E0D1]"><text id="ptb-pill-t" class="text-[#6B6757] text-[15px] font-[inter] font-medium">Claude Opus 4.7</text></div>
            <div id="ptb-avatar" class="w-[36px] h-[36px] rounded-full bg-[#D97757] flex items-center justify-center"><text id="ptb-avatar-t" class="text-[#ffffff] text-[15px] font-[inter600]">J</text></div>
          </div>
        </div>
        <div id="ptb-thread" class="flex-1"></div>
        <div id="ptb-wrap" class="pb-[44px] flex flex-col items-center gap-[14px]">
          <div id="composer-wrap" class="relative w-[900px] h-[240px]">
            <div id="composer-shadow" class="absolute inset-0 rounded-[22px] bg-[#F5F4ED] shadow-[0_18px_48px_rgba(61,57,41,0.12)]"></div>
            <div id="prompt-composer" class="absolute inset-0 bg-[#FFFFFF] border-[1px] border-[#E5E0D1] rounded-[22px] px-[30px] pt-[30px] pb-[24px] flex flex-col justify-between">
              <div id="pc-textrow" class="flex items-center">
                <text id="prompt-command" class="text-[#3D3929] text-[34px] font-[inter400] whitespace-nowrap">npx hyperframes add iphone-canvas</text>
              </div>
              <div id="pc-actions" class="flex justify-end items-center gap-[10px]">
                <text id="pc-attach" class="text-[#8C8674] text-[18px] font-[inter] font-medium">{'\uD83D\uDCCE'}</text>
                <div id="prompt-send" class="relative w-[36px] h-[36px] rounded-[10px] bg-[#D97757] flex items-center justify-center">
                  <div id="send-glow" class="absolute inset-0 rounded-[10px] shadow-[0_12px_34px_rgba(217,119,87,0.42)] opacity-0"></div>
                  <text id="pc-send-arrow" class="text-[#ffffff] font-[inter800] text-[20px]">{'\u2191'}</text>
                </div>
              </div>
              <div id="prompt-caret" class="absolute w-[14px] h-[38px] rounded-[2px] bg-[#D97757] opacity-0"></div>
            </div>
          </div>
          <text id="prompt-meta" class="text-[#3D3929] text-[13px] font-[inter400] opacity-0">Claude can make mistakes. Please double-check responses.</text>
        </div>
      </div>
      <div id="game-change-copy" class="absolute left-0 top-[0px] w-[1920px] h-[1080px] flex items-center justify-center opacity-0">
        <text id="game-change-line" class="text-[#3D3929] text-[136px] font-[inter800] font-extrabold leading-[0.94] text-center">You can make this too</text>
      </div>
      <image id="click-cursor" path="assets/cursor.png"
        class="absolute left-[940px] top-[528px] w-[118px] h-[118px] opacity-0 drop-shadow-[0_18px_18px_rgba(61,57,41,0.22)]" />
    </div>
  );
}

/* V4: #chromatic-split — full-frame ray-marched RGB channel planes over the
   rasterised still (black + radial gradients + glowing 136px headline) */
function ChromaticSplit() {
  let CS_CK = null;
  CanvasKitInit({}).then((ck) => { CS_CK = ck; });
  useGSAP((ctx) => {
    if (!CS_CK) return;
    var st = Math.max(0, ctx.global.time);
    var chromCanvas = CS_CK.MakeCanvasSurface('chromatic-canvas').getCanvas();
    var chromNode = ctx.getNode('chromatic-canvas');
    /* source #chromatic-split (bg #000, z7) is shown at CHROMATIC_START and
       never hidden — PORTAL_START (17.02) is when the white portal-logo-frame
       (z9) covers it, so black persists through 17.02 (index.html:1433,1551) */
    if (st < 13.5 || st > 17.02) { chromNode.opacity(0); return; }
    chromNode.opacity(1);

    /* chromaticState channel (source defaults + master tweens, abs times) */
    var CH0 = 13.5, ROT = 14.905, EXIT = 16.03;
    var zSplit = 0, xSplit = 0, redRotY = 0, greenRotY = 0, blueRotY = 0;
    var depthX = 0, depthZ = 0, planeScale = 1;
    var camX = 0, camY = 0, camZ = 5, lookX = 0, lookY = 0;
    var sceneAlpha = 1;
    zSplit = seg(st, CH0, 0.44, p2out, 0, 0.42);
    redRotY = seg(st, CH0, 0.44, p2out, 0, -0.08);
    blueRotY = seg(st, CH0, 0.44, p2out, 0, 0.08);
    camZ = seg(st, CH0, 0.44, p2out, 5, 5.55);
    camY = seg(st, CH0, 0.44, p2out, 0, 0.16);
    camX = seg(st, 13.92, 1.0, p2io, camX, 3.2);
    camY = seg(st, 13.92, 1.0, p2io, camY, 0.3);
    camZ = seg(st, 13.92, 1.0, p2io, camZ, 4.0);
    lookX = seg(st, 13.92, 1.0, p2io, lookX, -0.3);
    zSplit = seg(st, 14.005, 0.425, p2io, zSplit, 0.56);
    zSplit = seg(st, 14.455, 0.45, p2io, zSplit, 0.36);
    camX = seg(st, ROT, EXIT - ROT, p2io, camX, 4.7);
    camY = seg(st, ROT, EXIT - ROT, p2io, camY, 0.55);
    camZ = seg(st, ROT, EXIT - ROT, p2io, camZ, 2.55);
    lookX = seg(st, ROT, EXIT - ROT, p2io, lookX, -0.1);
    lookY = seg(st, ROT, EXIT - ROT, p2io, lookY, 0.05);
    zSplit = seg(st, ROT, EXIT - ROT, p2io, zSplit, 1.05);
    xSplit = seg(st, ROT, EXIT - ROT, p2io, xSplit, 0.46);
    redRotY = seg(st, ROT, EXIT - ROT, p2io, redRotY, -1.42);
    greenRotY = seg(st, ROT, EXIT - ROT, p2io, greenRotY, -1.22);
    blueRotY = seg(st, ROT, EXIT - ROT, p2io, blueRotY, -1.02);
    depthX = seg(st, EXIT, 0.31, p3in, depthX, -2.2);
    depthZ = seg(st, EXIT, 0.31, p3in, depthZ, -2.65);
    xSplit = seg(st, EXIT, 0.31, p3in, xSplit, 0.48);
    zSplit = seg(st, EXIT, 0.31, p3in, zSplit, 0.48);
    planeScale = seg(st, EXIT, 0.31, p3in, planeScale, 0.3);
    sceneAlpha = seg(st, EXIT, 0.31, p3in, sceneAlpha, 0.76);
    camZ = seg(st, EXIT, 0.31, p2in, camZ, 4.35);
    lookX = seg(st, EXIT, 0.31, p2in, lookX, -0.16);
    sceneAlpha = seg(st, EXIT + 0.31, 0.11, p1in, sceneAlpha, 0);

    var PW = 1920.0, PH = 1080.0, aspectC = PW / PH;
    var planeW = 3.0 * aspectC, planeH = 3.0;
    var tanH = Math.tan(40.0 * PI / 180.0 / 2.0);
    var B = camBasis([camX, camY, camZ], [lookX, lookY, 0.0]);
    var Rx = B.R[0] * tanH * aspectC, Ry = B.R[1] * tanH * aspectC, Rz = B.R[2] * tanH * aspectC;
    var Ux = B.U[0] * tanH, Uy = B.U[1] * tanH, Uz = B.U[2] * tanH;
    var c0 = [Math.cos(redRotY), Math.sin(redRotY)];
    var c1 = [Math.cos(greenRotY), Math.sin(greenRotY)];
    var c2 = [Math.cos(blueRotY), Math.sin(blueRotY)];
    var chromEffect = CS_CK.RuntimeEffect.Make(CHROM_SKSL);
    if (!chromEffect) throw new Error('CHROM_SKSL compile failed');
    var chromPaint = new CS_CK.Paint();
    chromPaint.setShader(chromEffect.makeShaderWithChildren([
      PW, PH, planeW, planeH, planeScale, sceneAlpha,
      camX, camY, camZ,
      B.F[0], B.F[1], B.F[2], Rx, Ry, Rz, Ux, Uy, Uz,
      depthX - xSplit, 0.0, depthZ + zSplit, c0[0], c0[1],
      depthX,          0.0, depthZ,          c1[0], c1[1],
      depthX + xSplit, 0.0, depthZ - zSplit, c2[0], c2[1]
    ], [chromCanvas.getSubTree().makeShader()]));
    chromCanvas.drawRect(CS_CK.XYWHRect(0, 0, 1920, 1080), chromPaint);
  });
  return (
    <canvas id="chromatic-canvas" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#000000] opacity-0">
      <div id="chrom-still" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#08060e]">
        <div id="chrom-warm" class="absolute left-[540px] top-[-270px] w-[1300px] h-[1300px] rounded-full bg-[radial-gradient(closest-side,rgba(255,49,93,0.26),rgba(217,119,87,0))]"></div>
        <div id="chrom-blue" class="absolute left-[70px] top-[115px] w-[1240px] h-[1240px] rounded-full bg-[radial-gradient(closest-side,rgba(37,200,255,0.24),rgba(37,200,255,0))]"></div>
        <div id="chrom-green" class="absolute left-[440px] top-[35px] w-[1040px] h-[1040px] rounded-full bg-[radial-gradient(closest-side,rgba(49,255,131,0.13),rgba(49,255,131,0))]"></div>
        <text id="chrom-text" class="absolute left-0 top-[472px] w-[1920px] text-center text-[#F5F4ED] text-[136px] font-[inter800] font-extrabold leading-[0.94] [text-shadow:0_0_34px_rgba(245,244,237,0.42)]">You can make this too</text>
      </div>
    </canvas>
  );
}

/* V5: #upgrade-cta */
function Cta() {
  useGSAP((ctx) => {
    var T = gsap.timeline();
    T.set('upgrade-cta', { y: 240, opacity: 0, filter: 'blur(12px)' }, 0);
    T.to('upgrade-cta', { y: 0, filter: 'blur(0px)', duration: 0.38, ease: 'expo.out' }, 16.34);
    T.to('upgrade-cta', { opacity: 1, duration: 0.2, ease: 'power1.out' }, 16.34);
    T.set('upgrade-cta', { opacity: 0 }, 17.02);
  });
  return (
    <div id="upgrade-cta" class="absolute left-0 top-0 w-[1920px] h-[1080px] flex flex-col items-center justify-center gap-[38px] opacity-0">
      <text id="cta-main" class="text-[#F5F4ED] text-[178px] font-[inter900] font-black leading-[0.88] text-center">Full catalog</text>
      <text id="cta-sub" class="text-[rgba(245,244,237,0.82)] text-[92px] font-[inter800] font-extrabold leading-[0.92] text-center">available now</text>
    </div>
  );
}

/* V6: #portal-logo-frame + #portal-canvas — the portal shader keeps the CTA
   front texture outside the growing disc (transparent inside, revealing the
   white brand card below) */
function Portal() {
  let PT_CK = null;
  CanvasKitInit({}).then((ck) => { PT_CK = ck; });
  useGSAP((ctx) => {
    var st = Math.max(0, ctx.global.time);
    var T = gsap.timeline();
    T.set('portal-logo-frame', { opacity: 0 }, 0);
    T.set('portal-canvas', { opacity: 0 }, 0);
    T.set('portal-logo-frame', { opacity: 1 }, 17.02);
    T.set('portal-canvas', { opacity: 1 }, 17.02);
    T.set('portal-canvas', { opacity: 0 }, 25.93);
    if (st < 17.02) return;

    /* portalState channel (abs times) */
    var PS = 17.02;
    var energy = seg(st, PS, 0.28, p3out, 0, 1.0);
    var chroma = seg(st, PS, 0.28, p3out, 0, 0.04);
    var distort = seg(st, PS, 0.28, p3out, 0, 0.06);
    var ew = seg(st, PS, 0.28, p3out, 0.06, 0.08);
    var radius = seg(st, PS + 0.05, 7.59, p1io, 0, 0.72);
    radius = seg(st, 24.48, 1.45, p3in, radius, 1.8);
    energy = seg(st, 25.43, 0.55, p2out, energy, 0.0);
    chroma = seg(st, 25.43, 0.55, p2out, chroma, 0.0);
    distort = seg(st, 25.43, 0.55, p2out, distort, 0.0);

    if (!PT_CK) return;
    var portalCanvas = PT_CK.MakeCanvasSurface('portal-canvas').getCanvas();
    var portalEffect = PT_CK.RuntimeEffect.Make(PORTAL_SKSL);
    if (!portalEffect) throw new Error('PORTAL_SKSL compile failed');
    var portalPaint = new PT_CK.Paint();
    portalPaint.setShader(portalEffect.makeShaderWithChildren(
      [1920.0, 1080.0, radius, energy, st - PS, ew, chroma, distort],
      [portalCanvas.getSubTree().makeShader()]));
    portalCanvas.drawRect(PT_CK.XYWHRect(0, 0, 1920, 1080), portalPaint);
  });
  return (
    <>
      <div id="portal-logo-frame" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#FFFFFF] flex flex-col items-center justify-center gap-[42px] opacity-0">
        <image id="end-logo" class="w-[650px] h-[195px]" path="assets/hyperframes-dark.png" />
        <text id="end-url" class="text-[#111111] font-[mono] font-extrabold text-[32px] leading-none opacity-[0.72]">hyperframes.heygen.com/catalog</text>
      </div>
      <canvas id="portal-canvas" class="absolute left-0 top-0 w-[1920px] h-[1080px] opacity-0">
        <div id="chrome-front" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#000000]">
          <text id="cf-main" class="absolute left-0 top-[426px] w-[1920px] text-center text-[#F5F4ED] text-[178px] font-[inter900] font-black leading-[0.88]">Full catalog</text>
          <text id="cf-sub" class="absolute left-0 top-[600px] w-[1920px] text-center text-[rgba(245,244,237,0.82)] text-[92px] font-[inter800] font-extrabold leading-[0.92]">available now</text>
        </div>
      </canvas>
    </>
  );
}

/* V7: intro sting (nested vfx-text-cursor.html 0..3.12; NOT shifted) */
function IntroSting() {
  let IS_CK = null;
  CanvasKitInit({}).then((ck) => { IS_CK = ck; });
  useGSAP((ctx) => {
    var T = gsap.timeline();
    var TT = Math.max(0, ctx.global.time);
    var effectMix = TT <= 0.72 ? 0 : (TT >= 1.06 ? 1 : 1 - Math.pow(1 - (TT - 0.72) / 0.34, 4));
    var progress = TT <= 0.72 ? 0 : (TT >= 3.0 ? 1 : (TT - 0.72) / 2.28);
    var logoScale = TT < 2.9 ? 1 : (TT >= 3.1 ? 0.4 : 1 - 0.6 * Math.pow(2, 10 * ((TT - 2.9) / 0.2 - 1)));
    var logoT = TT >= 1.0 ? 1 : 1 - Math.pow(1 - TT, 5);
    var logoY = 760 * (1 - logoT);
    /* gate < 3.1: opacity tweens to 0 exactly at 3.1 (2.9+0.2) — frame 93 is
       invisible, skip its ~12s ray-march (render-budget) */
    if (TT < 3.1) {
      if (!IS_CK) return;
      var stingCanvas = IS_CK.MakeCanvasSurface('sting-canvas').getCanvas();
      if (effectMix <= 0) {
        var loEffect = IS_CK.RuntimeEffect.Make(STING_LOGOONLY_SKSL);
        if (!loEffect) throw new Error('STING_LOGOONLY_SKSL compile failed');
        var loPaint = new IS_CK.Paint();
        loPaint.setShader(loEffect.makeShaderWithChildren(
          [1920.0, 1080.0, logoScale, logoY],
          [stingCanvas.getSubTree().makeShader()]));
        stingCanvas.drawRect(IS_CK.XYWHRect(0, 0, 1920, 1080), loPaint);
      } else {
        var stingEffect = IS_CK.RuntimeEffect.Make(STING_SKSL);
        if (!stingEffect) throw new Error('STING_SKSL compile failed');
        var stingPaint = new IS_CK.Paint();
        stingPaint.setShader(stingEffect.makeShaderWithChildren(
          [1920.0, 1080.0, logoScale, logoY, effectMix, progress],
          [stingCanvas.getSubTree().makeShader(), stingCanvas.getSubTree().makeShader()]));
        stingCanvas.drawRect(IS_CK.XYWHRect(0, 0, 1920, 1080), stingPaint);
      }
    }
    T.set('sting-canvas', { opacity: 1 }, 0);
    T.to('sting-canvas', { filter: 'blur(16px)', duration: 0.2, ease: 'expo.in' }, 2.9);
    T.to('sting-canvas', { opacity: 0, duration: 0.2, ease: 'expo.in' }, 2.9);
  });
  return (
    <canvas id="sting-canvas" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
      <div id="logo-field" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
        <image id="lf-img" class="absolute left-0 top-0 w-[1920px] h-[1080px]" path="assets/logo-field.png" />
      </div>
    </canvas>
  );
}

/* V8: intro refraction bg + glass card with the typed phrase + lockup */
function IntroCard() {
  useGSAP((ctx) => {
    var T = gsap.timeline();

    /* bg-2.mp4 (source data-start 3.1; padded copy plays from 0) */
    T.set('next-bg', { opacity: 0 }, 0);
    T.set('next-bg', { opacity: 1 }, 3.1);
    T.to('next-bg', { opacity: 0, duration: 0.36, ease: 'power1.in' }, 5.99);

    /* card entry. `backdrop-filter` stays a class, not a tween: CSSPlugin does
       not know `backdropBlur`, so a tween on it never reaches the store. */
    T.set('next-card', { opacity: 0, scale: 1.45, filter: 'blur(14px)', transformOrigin: '50% 50%' }, 0);
    T.set('next-card', { opacity: 1, scale: 1.45, filter: 'blur(14px)', transformOrigin: '50% 50%' }, 3.1);
    /* upstream `entryEase` (vfx-text-cursor.html:332) is the quadratic
       `-0.806t² + 1.806t`, not a cubic-bezier — the two differ mid-tween. */
    T.to('next-card', { scale: 0.88, filter: 'blur(0px)', duration: 0.9, ease: (t) => -0.806 * t * t + 1.806 * t }, 3.1);
    T.to('next-card', { scale: 0.84, filter: 'blur(1.5px)', duration: 0.4, ease: 'linear' }, 4.0);

    /* phrase A: "HTML" types (typeStep .085, titleHold .25), caret rides the
       measured advance; supporting words enter staggered 0.08 */
    var HTML_TEXT = 'HTML';
    var typeStep = 0.085, titleHold = 0.25;
    var htmlStart = 3.15;
    var typeEnd = htmlStart + (HTML_TEXT.length - 1) * typeStep;   // 3.405
    var slideStart = typeEnd + titleHold;                          // 3.655
    var slideEnd = slideStart + 0.05;                              // 3.705
    var widths = measurePrefixes(ctx, '__vhc_htmlW', '900 292px "Big Shoulders Display"', '4.38px', HTML_TEXT);
    var parts = new SplitText(ctx.getNode('html-type'), { type: 'chars' }).chars;
    T.set('html-type', { opacity: 1 }, 0);
    /* upstream `getTitleOffset()` (vfx-text-cursor.html:336-344) centres the
       laid-out word box on the card. Our `getBoundingClientRect` now reports
       real layout geometry, but the offset still lands ~160px right of the
       reference: our `-in` tail is set in Big Shoulders (upstream leaves
       `.html-tail` on the card's default face), so the centred line sits at a
       different x. Calibrate against the reference until the tail face matches. */
    T.set('html-word', { x: 85, y: 130, scale: 1.8, transformOrigin: '50% 50%' }, 0);
    T.set('html-caret', { opacity: 0, left: 0 }, 0);
    T.set('html-caret', { opacity: 1 }, htmlStart - 0.05);
    for (var i = 0; i < parts.length; i++) {
      T.set(parts[i], { opacity: 0 }, 0);
      T.set(parts[i], { opacity: 1 }, htmlStart + i * typeStep);
      T.set('html-caret', { left: widths[i + 1] }, htmlStart + i * typeStep);
    }
    T.set('html-caret', { opacity: 0 }, typeEnd);
    T.to('html-word', { x: 0, y: 0, scale: 1, duration: 0.05, ease: 'power2.out' }, slideStart);

    T.set('html-tail', { opacity: 0, y: 54, scale: 0.96, filter: 'blur(10px)' }, 0);
    T.set('canvas-word', { opacity: 0, y: 54, scale: 0.96, filter: 'blur(10px)' }, 0);
    T.to('html-tail', { opacity: 1, y: 0, scale: 1, filter: 'blur(0px)', duration: 0.42, ease: 'expo.out' }, slideEnd);
    T.to('canvas-word', { opacity: 1, y: 0, scale: 1, filter: 'blur(0px)', duration: 0.42, ease: 'expo.out' }, slideEnd + 0.08);

    /* phrase A exit -> phrase B (availability lockup) */
    var phraseAExit = slideEnd + 0.42 + 0.08 + 0.5;   // 4.705
    var phraseBStart = phraseAExit + 0.36;            // 5.065
    T.to('phrase-a', { x: -1480, filter: 'blur(8px)', duration: 0.36, ease: 'power2.in' }, phraseAExit);
    T.set('phrase-a', { opacity: 0 }, phraseBStart);
    T.set('phrase-b', { opacity: 1, x: 1480, filter: 'blur(8px)' }, phraseBStart);
    T.to('phrase-b', { x: 0, filter: 'blur(0px)', duration: 0.48, ease: 'expo.out' }, phraseBStart);

    /* bridge (handoff to the main timeline), 5.85..6.35 */
    T.to('next-card', { y: -1180, filter: 'blur(24px)', duration: 0.5, ease: 'power2.in' }, 5.85);
    T.to('next-card', { opacity: 0, duration: 0.2, ease: 'power1.in' }, 6.15);
  });
  return (
    <>
      <video id="next-bg" path="assets/bg-2-pad.mp4" duration={8.1}
        class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover opacity-0 saturate-[1.1] contrast-[1.03]" />
      <div id="next-card" class="absolute left-[120px] top-[67.5px] w-[1680px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.17)] bg-[linear-gradient(160deg,rgba(255,255,255,0.11)_0%,rgba(255,255,255,0.045)_42%,rgba(255,255,255,0.018)_70%,rgba(255,255,255,0.075)_100%)] backdrop-blur-[14px] backdrop-saturate-[1.12] shadow-[0_30px_90px_rgba(0,0,0,0.42)] opacity-0 overflow-hidden">
        <div id="glow-a" class="absolute left-[-40px] top-[520px] w-[560px] h-[560px] rounded-full bg-[radial-gradient(closest-side,rgba(73,242,255,0.14),transparent)]"></div>
        <div id="glow-b" class="absolute left-[1260px] top-[-10px] w-[440px] h-[440px] rounded-full bg-[radial-gradient(closest-side,rgba(255,79,216,0.11),transparent)]"></div>
        <div id="phrase-a" class="absolute left-[126px] right-[126px] top-[96px] bottom-[96px] flex flex-col items-center justify-center text-center text-[#ffffff] text-[292px] font-[inter900] font-black leading-[0.88] tracking-[0]">
          <div id="pa-line1" class="flex items-center justify-center">
            <div id="html-word" class="relative text-left text-[#f6ff7a] font-[big] font-black tracking-[4.38px]">
              <text id="html-type" class="font-[big] font-black tracking-[4.38px]" style={{ textShadow: '-6px 0 16px rgba(73,242,255,0.48), 6px 0 16px rgba(255,79,216,0.34), 0 0 24px rgba(246,255,122,0.34), 0 24px 72px rgba(0,0,0,0.56)' }}>HTML</text>
              <text id="html-caret" class="absolute left-[0px] top-[0px] text-[#f6ff7a] font-[big] font-black text-[292px] leading-[0.88] opacity-0">|</text>
            </div>
            <text id="html-tail" class="opacity-0">{'\u2013'}in</text>
          </div>
          <text id="canvas-word" class="opacity-0">canvas</text>
        </div>
        <div id="phrase-b" class="absolute left-[126px] right-[126px] top-[96px] bottom-[96px] text-[#ffffff] opacity-0">
          <text id="avail-now" class="absolute left-[0px] top-[76px] text-[161px] font-[inter800] font-extrabold leading-[0.88]">Now</text>
          <text id="avail-natively" class="absolute left-[92px] top-[212px] text-[#f6ff7a] font-[serifd] text-[304px] italic font-normal leading-[0.78]">natively</text>
          {/* source `.availability > span{display:block}` outranks
              `.availability-supported{display:flex}`, so the lockup is a
              left-aligned block, not a flex row. */}
          <div id="avail-supported-wrap" class="absolute right-[0px] top-[526px] w-[940px]">
            <text id="avail-supported" class="text-[161px] font-[inter800] font-extrabold leading-[0.88] whitespace-nowrap">supported.</text>
          </div>
        </div>
      </div>
    </>
  );
}

/* ══ composition (paint order bottom→top; #vfx-intro is z12 in the source,
   so the intro components come LAST) ══════════════════════════════════ */
export default function VfxHeygenCombined() {
  return (
    <div id="root" class="relative w-[1920px] h-[1080px] bg-[#050508] overflow-hidden font-[inter]">
      <MainBg />
      <Theater />
      <Finger />
      <PromptScene />
      <ChromaticSplit />
      <Cta />
      <Portal />
      <IntroSting />
      <IntroCard />
    </div>
  );
}
