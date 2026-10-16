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

import { useGSAP } from 'opencat/runtime';

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
  var MSR = ly.surface(1, 1);
  MSR.font = font;
  MSR.letterSpacing = ls;
  var out = [];
  for (var i = 0; i <= str.length; i++) out.push(MSR.measureText(str.slice(0, i)).width);
  globalThis[key] = out;
  return out;
}

/* ══ SKSL kernels ════════════════════════════════════════════════════ */

/* intro sting — 1:1 actA port of vfx-text-cursor.html:449-546 (112-sample
   ray-march; hash/spectrum primitives; mask=alpha / colour=rgb texture) */
var STING = (uv, image, colormask, u) => {
  const gx = uv.x / u.w;
  const gy = 1.0 - uv.y / u.h;
  const scale = u.scale;
  const lx = (gx - 0.5) / scale + 0.5;
  const ly = (gy - 0.5) / scale + 0.5 + u.logoY / u.h;
  const mask = colormask.eval([lx * u.w, (1.0 - ly) * u.h]).a;
  const logo = image.eval([lx * u.w, (1.0 - ly) * u.h]);
  /* p = uv*2-1 (GLSL y-up), p.x *= aspect — source vfx-text-cursor.html:499 */
  const ax = (gx * 2.0 - 1.0) * (u.w / u.h);
  const ay = gy * 2.0 - 1.0;
  const progress = u.progress;
  const mpx = (progress * 2.0 - 1.0) * (u.w / u.h);
  const mpy = 0.0;
  const dax = (mpx - ax) / 112.0;
  const day = (mpy - ay) / 112.0;
  let acc = 0.0;
  let rx = ax;
  let ry = ay;
  for (let i = 0.0; i < 112.0; i++) {
    rx += dax;
    ry += day;
    const h1 = fract(sin((rx * 489.0 + ry * 589.0 + i * 58.0)) * 492.0) * 2.0 - 1.0;
    const h2 = fract(sin(((rx + 1.0) * 489.0 + (ry + 1.0) * 589.0 + (i + 1.0) * 58.0)) * 492.0) * 2.0 - 1.0;
    rx += h1 * 0.5 / 112.0;
    ry += h2 * 0.5 / 112.0;
    const bx = rx / (u.w / u.h) * 0.5 + 0.5;
    const by = ry * 0.5 + 0.5;
    const rlx = (bx - 0.5) / scale + 0.5;
    const rly = (by - 0.5) / scale + 0.5 + u.logoY / u.h;
    acc += colormask.eval([rlx * u.w, (1.0 - rly) * u.h]).a / 112.0;
  }
  let cr = cos((cos(acc * 4.9) - 0.0) * 0.6 * 3.141593) * acc * 3.65 - acc * 0.24;
  let cg = cos((cos(acc * 4.9) - 0.5) * 1.0 * 3.141593) * acc * 3.65 - acc * 0.24;
  let cb = cos((cos(acc * 4.9) - 1.0) * 0.5 * 3.141593) * acc * 3.65 - acc * 0.24;
  cr = max(cr - 0.035, 0.0) * 1.16;
  cg = max(cg - 0.035, 0.0) * 1.16;
  cb = max(cb - 0.035, 0.0) * 1.16;
  const sweep = exp(-pow((gx - progress) * 6.2, 2.0));
  const textProx = smoothstep(0.006, 0.34, acc);
  const longRay = smoothstep(0.004, 0.18, acc);
  const sx = progress * 1.8;
  const swR = 0.14 + cos((sx - 0.0) * 0.6 * 3.141593) * 0.28;
  const swG = 0.65 + cos((sx - 0.5) * 1.0 * 3.141593) * 0.28;
  const swB = 0.85 + cos((sx - 1.0) * 0.5 * 3.141593) * 0.28;
  const lrR = cos((progress + acc * 2.0 - 0.0) * 0.6 * 3.141593);
  const lrG = cos((progress + acc * 2.0 - 0.5) * 1.0 * 3.141593);
  const lrB = cos((progress + acc * 2.0 - 1.0) * 0.5 * 3.141593);
  const sweepTerm = sweep * (0.34 + textProx * 2.35);
  cr += swR * sweepTerm + lrR * longRay * 0.58;
  cg += swG * sweepTerm + lrG * longRay * 0.58;
  cb += swB * sweepTerm + lrB * longRay * 0.58;
  const logoTerm = acc * 1.18 + sweep * mask * 0.92;
  cr += logo.r * logoTerm;
  cg += logo.g * logoTerm;
  cb += logo.b * logoTerm;
  const mixv = u.mix;
  cr *= mixv; cg *= mixv; cb *= mixv;
  const lm = smoothstep(0.02, 0.75, logo.a);
  cr = mix(cr, logo.r, lm);
  cg = mix(cg, logo.g, lm);
  cb = mix(cb, logo.b, lm);
  const white = smoothstep(0.04, 0.38, mask) * 0.12 * mixv;
  cr += white; cg += white; cb += white;
  /* static grain field (vfx-text-cursor.html:539-542). hash(vec3(uv.xyy +
     mouse.x*0.0007)) with mouse.x = progress*1920 (state.progress*WIDTH):
     GLSL vec3+float adds to EVERY component, so
     dot((uv.x+m, uv.y+m, uv.y+m), (489,589,58)) = uv.x*489 + uv.y*647 +
     m*(489+589+58) = uv.x*489 + uv.y*647 + progress*1.344*1136 — uv is GL
     y-up, so use gy = 1 - uv.y/u.h. */
  const grain = fract(sin(gx * 489.0 + gy * 647.0 + progress * 1527.168) * 492.0) * 2.0 - 1.0;
  const ag = abs(grain);
  const dust = smoothstep(0.02, 0.38, acc) + smoothstep(0.03, 0.22, mask);
  const dustTerm = grain * 0.07 * (0.42 + dust) * mixv;
  const dustT = smoothstep(0.02, 0.5, acc) * ag * 0.045 * mixv;
  const gsR = cos((progress + grain - 0.0) * 0.6 * 3.141593);
  const gsG = cos((progress + grain - 0.5) * 1.0 * 3.141593);
  const gsB = cos((progress + grain - 1.0) * 0.5 * 3.141593);
  return [clamp(cr + dustTerm + gsR * dustT, 0.0, 1.0),
          clamp(cg + dustTerm + gsG * dustT, 0.0, 1.0),
          clamp(cb + dustTerm + gsB * dustT, 0.0, 1.0), 1.0];
};
var STING_SPEC = {
  uniforms: { w: 'float', h: 'float', scale: 'float',
              logoY: 'float', mix: 'float', progress: 'float' },
  backend: 'sksl'
};

/* exact early-out of STING at effectMix==0 (frames 0..0.72s): every ray /
   sweep / grain term carries *mixv = 0, white-core too, so the shader
   reduces to mix(0, logo.rgb, smoothstep(.02,.75,logo.a)) — a single eval.
   Bit-identical to STING with u.mix=0, ~12s/frame → ~0.3s (render-budget). */
var STING_LOGOONLY = (uv, image, u) => {
  const gx = uv.x / u.w;
  const gy = 1.0 - uv.y / u.h;
  const lx = (gx - 0.5) / u.scale + 0.5;
  const ly = (gy - 0.5) / u.scale + 0.5 + u.logoY / u.h;
  const logo = image.eval([lx * u.w, (1.0 - ly) * u.h]);
  const lm = smoothstep(0.02, 0.75, logo.a);
  return [logo.r * lm, logo.g * lm, logo.b * lm, 1.0];
};
var STING_LOGOONLY_SPEC = {
  uniforms: { w: 'float', h: 'float', scale: 'float', logoY: 'float' },
  backend: 'sksl'
};

/* chromatic RGB split — 1:1 actC port of index.html:1017-1090 (three
   additive channel planes, source lum formula, exact perspective).
   PERF: the ref's UnrealBloomPass (index.html:1066) is NOT re-implemented —
   a ring-tap approximation cost 87 evals/px ≈ 6s/frame (105 frames ≈ 10.5min,
   measured via frame mtimes) and blew the 30min render budget with the STING
   march. This 3-eval kernel is exactly the shipped actC XML form that
   measured avg_p8 .1698 over its 387-frame act (docs/handoffs §12.4: ref is
   somewhat brighter inside the split glyphs). sy follows the
   actC-XML-validated convention (uv.y/u.h*2-1, the flipY'd CanvasTexture
   sample below closes the mapping). */
var CHROM = (uv, image, u) => {
  const sx = uv.x / u.w * 2.0 - 1.0;
  const sy = uv.y / u.h * 2.0 - 1.0;
  const Dx = u.fx + sx * u.rx + sy * u.ux;
  const Dy = u.fy + sx * u.ry + sy * u.uy;
  const Dz = u.fz + sx * u.rz + sy * u.uz;
  const a = u.alpha;
  const tR = -(((u.cx - u.px0) * u.sn0 + (u.cz - u.pz0) * u.cs0)) /
             (Dx * u.sn0 + Dz * u.cs0);
  const exR = (u.cx + tR * Dx) - u.px0;
  const eyR = (u.cy + tR * Dy) - u.py0;
  const ezR = (u.cz + tR * Dz) - u.pz0;
  const lxR = (exR * u.cs0 - ezR * u.sn0) / u.scale;
  const lyR = eyR / u.scale;
  const sR = image.eval([clamp(lxR / u.pw + 0.5, 0.0, 1.0) * u.w,
                         clamp(0.5 - lyR / u.ph, 0.0, 1.0) * u.h]);
  const tG = -(((u.cx - u.px1) * u.sn1 + (u.cz - u.pz1) * u.cs1)) /
             (Dx * u.sn1 + Dz * u.cs1);
  const exG = (u.cx + tG * Dx) - u.px1;
  const eyG = (u.cy + tG * Dy) - u.py1;
  const ezG = (u.cz + tG * Dz) - u.pz1;
  const lxG = (exG * u.cs1 - ezG * u.sn1) / u.scale;
  const lyG = eyG / u.scale;
  const sG = image.eval([clamp(lxG / u.pw + 0.5, 0.0, 1.0) * u.w,
                         clamp(0.5 - lyG / u.ph, 0.0, 1.0) * u.h]);
  const tB = -(((u.cx - u.px2) * u.sn2 + (u.cz - u.pz2) * u.cs2)) /
             (Dx * u.sn2 + Dz * u.cs2);
  const exB = (u.cx + tB * Dx) - u.px2;
  const eyB = (u.cy + tB * Dy) - u.py2;
  const ezB = (u.cz + tB * Dz) - u.pz2;
  const lxB = (exB * u.cs2 - ezB * u.sn2) / u.scale;
  const lyB = eyB / u.scale;
  const sB = image.eval([clamp(lxB / u.pw + 0.5, 0.0, 1.0) * u.w,
                         clamp(0.5 - lyB / u.ph, 0.0, 1.0) * u.h]);
  const lumR = sR.r * 0.6 + max(sR.g, sR.b) * 0.4;
  const lumG = sG.g * 0.6 + max(sG.r, sG.b) * 0.4;
  const lumB = sB.b * 0.6 + max(sB.r, sB.g) * 0.4;
  const bcR = min(sR.r * lumR * a, 1.0);
  const bcG = min(sG.g * lumG * a, 1.0);
  const bcB = min(sB.b * lumB * a, 1.0);
  return [bcR, bcG, bcB, 1.0];
};
var CHROM_SPEC = {
  uniforms: { w: 'float', h: 'float', pw: 'float', ph: 'float',
              scale: 'float', alpha: 'float',
              cx: 'float', cy: 'float', cz: 'float',
              fx: 'float', fy: 'float', fz: 'float',
              rx: 'float', ry: 'float', rz: 'float',
              ux: 'float', uy: 'float', uz: 'float',
              px0: 'float', py0: 'float', pz0: 'float', cs0: 'float', sn0: 'float',
              px1: 'float', py1: 'float', pz1: 'float', cs1: 'float', sn1: 'float',
              px2: 'float', py2: 'float', pz2: 'float', cs2: 'float', sn2: 'float' },
  backend: 'sksl'
};

/* portal reveal — full port of index.html:1161-1218 (edge noise, distortion,
   RGB fringe on the front texture, ring glow, sparks). Source noise() is
   [0,1]-range value noise; engine noise2 is the same sin-hash lattice mapped
   to [-1,1], so source_noise = (noise2 + 1) * 0.5. Reversed smoothsteps are
   inlined (the lambda whitelist has no nested functions; GLSL edge0>edge1 is
   UB anyway). */
var PORTAL = (uv, image, u) => {
  const px = (uv.x / u.w - 0.5) * (u.w / u.h);
  const py = uv.y / u.h - 0.5;
  const dist = length([px, py]);
  const ang = atan2(py, px);
  const eN = (noise2([ang * 3.0, u.time * 2.0]) + 1.0) * 0.5 * 0.03 * u.energy;
  const edge = u.radius + eN;
  const eD = abs(dist - edge);
  let tD = clamp((eD - u.ew * 1.5) / (0.0 - u.ew * 1.5), 0.0, 1.0);
  tD = tD * tD * (3.0 - 2.0 * tD);
  const dMask = tD * u.distort;
  const dl = length([px + 0.0001, py + 0.0001]);
  const dirX = (px + 0.0001) / dl;
  const dirY = (py + 0.0001) / dl;
  const wob = sin(ang * 8.0 + u.time * 5.0) * dMask;
  const offX = dirX * wob * u.w;
  const offY = dirY * wob * u.h;
  let tC = clamp((eD - u.ew * 2.0) / (0.0 - u.ew * 2.0), 0.0, 1.0);
  tC = tC * tC * (3.0 - 2.0 * tC);
  const cMask = tC * u.chroma;
  const chX = dirX * cMask * u.w;
  const chY = dirY * cMask * u.h;
  const front = image.eval([uv.x + offX, uv.y + offY]);
  const fr = image.eval([uv.x + offX + chX, uv.y + offY + chY]).r;
  const fg = front.g;
  const fb = image.eval([uv.x + offX - chX, uv.y + offY - chY]).b;
  const mK = clamp((dist - (edge - 0.008)) / 0.016, 0.0, 1.0);
  const mask = mK * mK * (3.0 - 2.0 * mK);
  let tI = clamp((eD - u.ew) / (0.0 - u.ew), 0.0, 1.0);
  tI = tI * tI * (3.0 - 2.0 * tI);
  let tO = clamp((eD - u.ew * 2.5) / (u.ew * 0.5 - u.ew * 2.5), 0.0, 1.0);
  tO = tO * tO * (3.0 - 2.0 * tO);
  const glow = tI * tO * u.energy;
  const phase = fract(ang / 6.2832 + u.time * 0.4);
  const m = sin(phase * 6.2832) * 0.5 + 0.5;
  const rr = 0.486 + (0.290 - 0.486) * m;
  const rg = 0.227 + (0.565 - 0.227) * m;
  const rb = 0.929 + (1.000 - 0.929) * m;
  let sp = (noise2([ang * 12.0, u.time * 8.0]) + 1.0) * 0.5;
  sp = pow(sp, 4.0) * tI * u.energy * 2.0;
  let cr = fr * mask + rr * glow * 2.5 + sp * 1.0;
  let cg = fg * mask + rg * glow * 2.5 + sp * 0.9;
  let cb = fb * mask + rb * glow * 2.5 + sp * 1.0;
  const alpha = clamp(mask + glow * 0.5 + sp * 0.3, 0.0, 1.0);
  return [cr, cg, cb, alpha];
};
var PORTAL_SPEC = {
  uniforms: { w: 'float', h: 'float', radius: 'float', energy: 'float',
              time: 'float', ew: 'float', chroma: 'float', distort: 'float' },
  backend: 'sksl'
};

/* ══ components (tree order == paint order == hook order) ════════════ */

/* V0: main bg.mp4 (source #bg-video data-start 5.58; padded file plays from
   composition 0, content frame 0 lands at 5.6) */
function MainBg() {
  useGSAP((ctx) => {
    var T = ctx.timeline();
    T.set('bg-video', { opacity: 0 }, 0);
    T.to('bg-video', { opacity: 1, duration: 0.42, ease: 'power1.out' }, 5.58);
  });
  return (
    <video id="bg-video" path="assets/bg-pad.mp4" duration={10.6}
      class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover opacity-0" />
  );
}

/* V1: #theater — the DRACO GLB iPhone re-authored in 2D. Per frame the exact
   source camera (PerspectiveCamera 34° at S.cX/cY/cZ looking at clX/clY/clZ)
   projects the phone quad (P0 = S.phX/phY/phZ + drift jitter, half extents
   1.6 × 0.7393 world); entry/exit (y 340 blur 18, x −1220 blur 22) ride the
   timeline on the outer node like the source's #theater tweens. */
function Theater() {
  useGSAP((ctx) => {
    var st = Math.max(0, ctx.global.time);
    var ps = st - 5.58; // source script time
    var T = ctx.timeline();
    T.set('phone', { y: 340, opacity: 0, filter: 'blur(18px)' }, 0);
    T.to('phone', { y: 0, opacity: 1, filter: 'blur(0px)', duration: 0.74, ease: 'expo.out' }, 5.62);
    T.to('phone', { x: -1220, filter: 'blur(22px)', duration: 0.53, ease: 'expo.out' }, 9.62);
    T.to('phone', { opacity: 0, duration: 0.18, ease: 'power2.out' }, 9.62);

    /* S state (index.html:830-845 + master tweens) */
    var phY = seg(ps, 0.08, 0.95, expoOut, -3.45, 0.02);
    var phRX = ps < 0.18 ? 0.01 : lerp(0.01, -0.012, breathe((ps - 0.18) / 0.78));
    var phRY;
    if (ps < 0.18) phRY = PI + 0.04;
    else if (ps < 0.96) phRY = lerp(PI + 0.04, PI - 0.035, breathe((ps - 0.18) / 0.78));
    else if (ps < 2.2) phRY = PI - 0.035;
    else if (ps < 3.93) phRY = (PI - 0.035) + 4 * PI * ((ps - 2.2) / 1.73);
    else phRY = 5 * PI - 0.035;
    /* phRZ: spin segment writes -0.015 -> 0.006 over 2.2..3.93, but the later
       push tween (source index.html:1294, phRZ: PI/2 power3.inOut @3.43) wins
       per-frame once running (GSAP no-overwrite). Its from-value is the spin
       channel's value at 3.43: -0.015 + 0.021 * (3.43-2.2)/1.73 = -0.000069. */
    var phRZ;
    if (ps < 2.2) phRZ = -0.015;
    else if (ps < 3.43) phRZ = lerp(-0.015, 0.006, (ps - 2.2) / 1.73);
    else phRZ = lerp(-0.000069, PI / 2, p3io((ps - 3.43) / 0.85));
    var cZ = ps < 0.18 ? 6.6 : lerp(6.6, 6.35, breathe((ps - 0.18) / 0.78));
    cZ = seg(ps, 3.43, 0.85, p3io, cZ, 3.85);
    var cY = seg(ps, 3.43, 0.85, p3io, 0.16, 0.12);
    var d = Math.max(0, Math.min(5, ps));
    var P0 = [Math.sin(d * 0.56) * 0.015, phY + Math.sin(d * 0.72) * 0.009,
              0.45 + Math.cos(d * 0.44) * 0.008];

    /* phone local axes under Euler XYZ: v' = RX(RY(RZ v)) */
    function rotV(v) {
      var x1 = v[0] * Math.cos(phRZ) - v[1] * Math.sin(phRZ);
      var y1 = v[0] * Math.sin(phRZ) + v[1] * Math.cos(phRZ);
      var z1 = v[2];
      var x2 = x1 * Math.cos(phRY) + z1 * Math.sin(phRY);
      var y2 = y1;
      var z2 = -x1 * Math.sin(phRY) + z1 * Math.cos(phRY);
      var y3 = y2 * Math.cos(phRX) - z2 * Math.sin(phRX);
      var z3 = y2 * Math.sin(phRX) + z2 * Math.cos(phRX);
      return [x2, y3, z3];
    }
    var uA = rotV([0, 1, 0]), rA = rotV([1, 0, 0]);
    var HL = 1.6, HW = 0.7393; // model 3.2 tall, screen aspect 390:844

    var C = [0, cY, cZ], L = [0, 0.04, 0];
    var B = camBasis(C, L);
    var tanH = Math.tan(17 * PI / 180), aspect = 1920 / 1080;
    function proj(P) {
      var vx = P[0] - C[0], vy = P[1] - C[1], vz = P[2] - C[2];
      var dep = vx * B.F[0] + vy * B.F[1] + vz * B.F[2];
      var sx = (vx * B.R[0] + vy * B.R[1] + vz * B.R[2]) / (dep * tanH * aspect);
      var sy = (vx * B.U[0] + vy * B.U[1] + vz * B.U[2]) / (dep * tanH);
      return [(sx + 1) * 960, (1 - sy) * 540];
    }
    var minX = 1e9, minY = 1e9, maxX = -1e9, maxY = -1e9, i, s, t2, c;
    for (i = 0; i < 4; i++) {
      s = i < 2 ? 1 : -1;
      t2 = (i === 0 || i === 3) ? 1 : -1;
      c = proj([P0[0] + s * HL * uA[0] + t2 * HW * rA[0],
                P0[1] + s * HL * uA[1] + t2 * HW * rA[1],
                P0[2] + s * HL * uA[2] + t2 * HW * rA[2]]);
      if (c[0] < minX) minX = c[0];
      if (c[0] > maxX) maxX = c[0];
      if (c[1] < minY) minY = c[1];
      if (c[1] > maxY) maxY = c[1];
    }
    var tc = proj([P0[0] + HL * uA[0], P0[1] + HL * uA[1], P0[2] + HL * uA[2]]);
    var bc = proj([P0[0] - HL * uA[0], P0[1] - HL * uA[1], P0[2] - HL * uA[2]]);
    var rc = proj([P0[0] + HW * rA[0], P0[1] + HW * rA[1], P0[2] + HW * rA[2]]);
    var cc = proj(P0);
    var hpx = Math.sqrt((tc[0] - bc[0]) * (tc[0] - bc[0]) + (tc[1] - bc[1]) * (tc[1] - bc[1]));
    var wpx = Math.sqrt((rc[0] - cc[0]) * (rc[0] - cc[0]) + (rc[1] - cc[1]) * (rc[1] - cc[1])) * 2;
    var rot = Math.atan2(tc[0] - bc[0], -(tc[1] - bc[1])) * 180 / PI;
    var W = Math.max(2, maxX - minX), H = Math.max(2, maxY - minY);

    var phone = ctx.getNode('phone');
    phone.left(minX); phone.top(minY); phone.width(W); phone.height(H);
    ctx.getNode('phone-spin').rotate(rot);
    /* face switch: screen normal n_z = -cos(phRY); back visible when n_z < 0
       (phRY=PI is front-facing). The crossing sits at the 90deg silhouette,
       where the projected body is at its narrowest. */
    ctx.getNode('pb-back').opacity(Math.cos(phRY) > 0 ? 1 : 0);
    var anch = ctx.getNode('phone-anch');
    anch.left(W / 2); anch.top(H / 2);
    anch.scaleX(Math.max(0.02, wpx / 396));
    anch.scaleY(Math.max(0.02, hpx / 856));

    /* screen scroll: S.scroll 0→0.9 power1.out @2.05; source fallback page
       scrolls 980px per unit in 390px space → 920px in this 366px space */
    var scroll = seg(ps, 2.05, 2.45, p1out, 0, 0.9);
    ctx.getNode('phone-page').top(-scroll * 920);
  });
  return (
    <div id="phone" class="absolute left-0 top-0 opacity-0">
      <div id="phone-spin" class="absolute inset-0">
        <div id="phone-anch" class="absolute left-[0px] top-[0px]">
          <div id="phone-body" class="absolute left-[-198px] top-[-428px] w-[396px] h-[856px]">
            <div id="pb-shell" class="absolute inset-0 rounded-[58px] bg-[#07080c] border-[5px] border-[rgba(255,255,255,0.20)]"></div>
            <div id="pb-notch" class="absolute left-[70px] top-[16px] w-[256px] h-[30px] rounded-full bg-[#000000]"></div>
            <div id="pb-screen" class="absolute left-[15px] top-[22px] w-[366px] h-[812px] rounded-[44px] overflow-hidden bg-[#f7f4ee]">
              <div id="phone-page" class="absolute left-0 top-0 w-[366px] h-[1720px]">
                <text id="pp-brand" class="absolute left-[22px] top-[30px] text-[#111111] font-[inter900] font-black text-[22px]">HeyGen</text>
                <div id="pp-cta" class="absolute left-[236px] top-[26px] px-[16px] py-[8px] rounded-full bg-[#d7ff3f]"><text id="pp-cta-t" class="text-[#111111] font-[inter800] font-extrabold text-[12px]">Get Started</text></div>
                <text id="pp-h1" class="absolute left-[22px] top-[92px] text-[#111111] font-[inter900] font-black text-[46px] leading-[0.98]">Turn your</text>
                <text id="pp-h2" class="absolute left-[22px] top-[140px] text-[#111111] font-[inter900] font-black text-[46px] leading-[0.98]">ideas into</text>
                <text id="pp-h3" class="absolute left-[22px] top-[188px] text-[#111111] font-[inter900] font-black text-[46px] leading-[0.98]">videos in</text>
                <text id="pp-h4" class="absolute left-[22px] top-[236px] text-[#111111] font-[inter900] font-black text-[46px] leading-[0.98]">minutes</text>
                <text id="pp-b1" class="absolute left-[22px] top-[318px] text-[rgba(17,17,17,0.66)] font-[inter400] text-[15px]">Go from script, image, presentation,</text>
                <text id="pp-b2" class="absolute left-[22px] top-[340px] text-[rgba(17,17,17,0.66)] font-[inter400] text-[15px]">or PDF to finished video. No cameras,</text>
                <text id="pp-b3" class="absolute left-[22px] top-[362px] text-[rgba(17,17,17,0.66)] font-[inter400] text-[15px]">no crew, no editing skills required.</text>
                <div id="pp-pill1" class="absolute left-[22px] top-[400px] px-[14px] py-[9px] rounded-full bg-[#111111]"><text id="pp-pill1-t" class="text-[#f7f4ee] font-[inter700] text-[12px]">AI Video Generator</text></div>
                <div id="pp-pill2" class="absolute left-[178px] top-[400px] px-[14px] py-[9px] rounded-full bg-[#d7ff3f]"><text id="pp-pill2-t" class="text-[#111111] font-[inter700] text-[12px]">Free to start</text></div>
                <div id="pp-hero" class="absolute left-[22px] top-[470px] w-[322px] h-[220px] rounded-[28px] overflow-hidden bg-[linear-gradient(135deg,#10131c_0%,#4d78ff_52%,#d7ff3f_100%)]">
                  <text id="pp-hero-t1" class="absolute left-[20px] top-[44px] text-[#ffffff] font-[inter800] font-extrabold text-[22px]">One-Shot Text-to-Video</text>
                  <text id="pp-hero-t2" class="absolute left-[20px] top-[80px] text-[rgba(255,255,255,0.92)] font-[inter600] text-[13px]">Type your idea. Click generate.</text>
                  <text id="pp-hero-t3" class="absolute left-[20px] top-[100px] text-[rgba(255,255,255,0.92)] font-[inter600] text-[13px]">Get a share-ready video.</text>
                  <div id="pp-hero-btn" class="absolute left-[20px] top-[136px] px-[18px] py-[11px] rounded-[12px] bg-[rgba(255,255,255,0.92)]"><text id="pp-hero-btn-t" class="text-[#111111] font-[inter800] text-[13px]">Generate</text></div>
                </div>
                <text id="pp-s1" class="absolute left-[22px] top-[712px] text-[#111111] font-[inter800] text-[26px]">128M+ videos generated</text>
                <text id="pp-s2" class="absolute left-[22px] top-[750px] text-[#111111] font-[inter800] text-[26px]">102M+ avatars generated</text>
                <text id="pp-s3" class="absolute left-[22px] top-[788px] text-[#111111] font-[inter800] text-[26px]">17M+ videos translated</text>
                <text id="pp-trust" class="absolute left-[22px] top-[840px] text-[rgba(17,17,17,0.54)] font-[inter700] text-[12px]">Trusted by millions worldwide</text>
                <div id="pp-lg1" class="absolute left-[22px] top-[899px] w-[148px] h-[40px] rounded-[13px] bg-[#ffffff]"><text id="pp-lg1-t" class="absolute left-[16px] top-[24px] text-[rgba(17,17,17,0.62)] font-[inter800] text-[14px]">Miro</text></div>
                <div id="pp-lg2" class="absolute left-[196px] top-[899px] w-[148px] h-[40px] rounded-[13px] bg-[#ffffff]"><text id="pp-lg2-t" class="absolute left-[16px] top-[24px] text-[rgba(17,17,17,0.62)] font-[inter800] text-[14px]">Workday</text></div>
                <div id="pp-lg3" class="absolute left-[22px] top-[953px] w-[148px] h-[40px] rounded-[13px] bg-[#ffffff]"><text id="pp-lg3-t" class="absolute left-[16px] top-[24px] text-[rgba(17,17,17,0.62)] font-[inter800] text-[14px]">Coursera</text></div>
                <div id="pp-lg4" class="absolute left-[196px] top-[953px] w-[148px] h-[40px] rounded-[13px] bg-[#ffffff]"><text id="pp-lg4-t" class="absolute left-[16px] top-[24px] text-[rgba(17,17,17,0.62)] font-[inter800] text-[14px]">HubSpot</text></div>
                <div id="pp-lg5" class="absolute left-[22px] top-[1008px] w-[148px] h-[40px] rounded-[13px] bg-[#ffffff]"><text id="pp-lg5-t" class="absolute left-[16px] top-[24px] text-[rgba(17,17,17,0.62)] font-[inter800] text-[14px]">Airtable</text></div>
                <div id="pp-lg6" class="absolute left-[196px] top-[1008px] w-[148px] h-[40px] rounded-[13px] bg-[#ffffff]"><text id="pp-lg6-t" class="absolute left-[16px] top-[24px] text-[rgba(17,17,17,0.62)] font-[inter800] text-[14px]">Wondershare</text></div>
                <text id="pp-f1" class="absolute left-[22px] top-[1090px] text-[#111111] font-[inter900] font-black text-[30px]">AI video creation</text>
                <text id="pp-f2" class="absolute left-[22px] top-[1128px] text-[#111111] font-[inter900] font-black text-[30px]">just became your</text>
                <text id="pp-f3" class="absolute left-[22px] top-[1166px] text-[#111111] font-[inter900] font-black text-[30px]">superpower</text>
                <div id="pp-feat1" class="absolute left-[22px] top-[1239px] w-[321px] h-[141px] rounded-[19px] bg-[#ffffff] border-[1px] border-[rgba(17,17,17,0.08)]">
                  <div id="pp-feat1-ic" class="absolute left-[15px] top-[15px] w-[39px] h-[39px] rounded-[13px] bg-[#d7ff3f]"></div>
                  <text id="pp-feat1-t" class="absolute left-[15px] top-[75px] text-[#111111] font-[inter800] text-[18px]">Text to Video</text>
                  <text id="pp-feat1-b1" class="absolute left-[15px] top-[99px] text-[rgba(17,17,17,0.58)] font-[inter] font-medium text-[12px]">Create complete AI videos from a script</text>
                  <text id="pp-feat1-b2" class="absolute left-[15px] top-[115px] text-[rgba(17,17,17,0.58)] font-[inter] font-medium text-[12px]">with voiceovers, visuals, and avatars.</text>
                </div>
                <div id="pp-feat2" class="absolute left-[22px] top-[1399px] w-[321px] h-[141px] rounded-[19px] bg-[#ffffff] border-[1px] border-[rgba(17,17,17,0.08)]">
                  <div id="pp-feat2-ic" class="absolute left-[15px] top-[15px] w-[39px] h-[39px] rounded-[13px] bg-[#4d78ff]"></div>
                  <text id="pp-feat2-t" class="absolute left-[15px] top-[75px] text-[#111111] font-[inter800] text-[18px]">Photo to Video</text>
                  <text id="pp-feat2-b1" class="absolute left-[15px] top-[99px] text-[rgba(17,17,17,0.58)] font-[inter] font-medium text-[12px]">Upload an image and script to create</text>
                  <text id="pp-feat2-b2" class="absolute left-[15px] top-[115px] text-[rgba(17,17,17,0.58)] font-[inter] font-medium text-[12px]">lifelike motion and natural lip-sync.</text>
                </div>
                <div id="pp-feat3" class="absolute left-[22px] top-[1558px] w-[321px] h-[141px] rounded-[19px] bg-[#ffffff] border-[1px] border-[rgba(17,17,17,0.08)]">
                  <div id="pp-feat3-ic" class="absolute left-[15px] top-[15px] w-[39px] h-[39px] rounded-[13px] bg-[#ff7a59]"></div>
                  <text id="pp-feat3-t" class="absolute left-[15px] top-[75px] text-[#111111] font-[inter800] text-[18px]">Translate Video</text>
                  <text id="pp-feat3-b1" class="absolute left-[15px] top-[99px] text-[rgba(17,17,17,0.58)] font-[inter] font-medium text-[12px]">Dub content into 175+ languages with</text>
                  <text id="pp-feat3-b2" class="absolute left-[15px] top-[115px] text-[rgba(17,17,17,0.58)] font-[inter] font-medium text-[12px]">voice cloning and subtitles.</text>
                </div>
              </div>
              <div id="pp-topfade" class="absolute left-0 top-0 w-[366px] h-[86px] bg-[linear-gradient(180deg,rgba(247,244,238,1)_0%,rgba(247,244,238,0.92)_55%,rgba(247,244,238,0)_100%)]"></div>
            </div>
            {/* back face (Three.js GLB shows the real back panel while the spin
               passes phRY 90..270deg; a 2D body must switch faces by hand) */}
            <div id="pb-back" class="absolute inset-0 rounded-[58px] bg-[linear-gradient(170deg,#5a605c_0%,#454b48_45%,#363c39_100%)] border-[5px] border-[rgba(255,255,255,0.20)] opacity-0">
              <div id="pbb-cam" class="absolute left-[10px] top-[12px] w-[170px] h-[173px] rounded-[40px] bg-[#141817]">
                <div id="pbb-l1" class="absolute left-[12px] top-[13px] w-[57px] h-[57px] rounded-full bg-[#060908] border-[4px] border-[rgba(210,220,215,0.35)]"></div>
                <div id="pbb-l2" class="absolute left-[94px] top-[18px] w-[57px] h-[57px] rounded-full bg-[#060908] border-[4px] border-[rgba(210,220,215,0.35)]"></div>
                <div id="pbb-l3" class="absolute left-[16px] top-[92px] w-[57px] h-[57px] rounded-full bg-[#060908] border-[4px] border-[rgba(210,220,215,0.35)]"></div>
                <div id="pbb-flash" class="absolute left-[104px] top-[104px] w-[22px] h-[22px] rounded-full bg-[#262c29]"></div>
              </div>
              <path id="pbb-logo" view-box="0 0 384 512" class="absolute left-[168px] top-[388px] w-[60px] h-[74px] fill-[#232823]" d="M318.7 268.7c-.2-36.7 16.4-64.4 50-84.8-18.8-26.9-47.2-41.7-84.7-44.6-35.5-2.8-74.3 20.7-88.5 20.7-15 0-49.4-19.7-76.4-19.7C63.3 141.2 4 184.8 4 273.5q0 39.3 14.4 81.2c12.8 36.7 59 126.7 107.2 125.2 25.2-.6 43-17.9 75.8-17.9 31.8 0 48.3 17.9 76.4 17.9 48.6-.7 90.4-82.5 102.6-119.3-65.2-30.7-61.7-90-61.7-91.9zm-56.6-164.2c27.3-32.4 24.8-61.9 24-72.5-24.1 1.4-52 16.4-67.9 34.9-17.5 19.8-27.8 44.3-25.6 71.9 26.1 2 49.9-11.4 69.5-34.3z" />
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* V2: #finger-overlay — static CSS rotate(-75deg) origin 174px 0, enters
   offscreen bottom-right, sweeps to the screen, exits up */
function Finger() {
  useGSAP((ctx) => {
    var T = ctx.timeline();
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
    var T = ctx.timeline();
    T.set('prompt-scene', { opacity: 0, x: 620, filter: 'blur(18px)' }, 0);
    T.to('prompt-scene', { x: 0, filter: 'blur(0px)', duration: 0.27, ease: 'power2.out' }, 9.66);
    T.to('prompt-scene', { opacity: 1, duration: 0.48, ease: 'power2.out' }, 9.66);
    T.set('prompt-app', { x: 560, y: -120, scale: 3.15, transformOrigin: '50% 92%' }, 0);

    /* typing "npx hyperframes add iphone-canvas" @4.13 dur 0.9 snap —
       splitText reveal + caret on the measured prefix advance */
    var CMD = 'npx hyperframes add iphone-canvas';
    var widths = measurePrefixes(ctx, '__vhc_cmdW', '400 34px "Inter"', '0px', CMD);
    var chars = ctx.splitText('prompt-command', { type: 'chars' });
    var i;
    for (i = 0; i < chars.length; i++) {
      T.set(chars[i], { opacity: 0 }, 0);
      T.set(chars[i], { opacity: 1 }, 9.71 + (i / CMD.length) * 0.9);
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
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit;
    var st = Math.max(0, ctx.global.time);
    var chromCanvas = ctx.getCanvasById('chromatic-canvas');
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
    var chromEffect = ly.pixel(CHROM, CHROM_SPEC);
    ly.draw(chromCanvas, chromEffect, {
      rect: [0, 0, 1920, 1080],
      uniforms: [
        PW, PH, planeW, planeH, planeScale, sceneAlpha,
        camX, camY, camZ,
        B.F[0], B.F[1], B.F[2], Rx, Ry, Rz, Ux, Uy, Uz,
        depthX - xSplit, 0.0, depthZ + zSplit, c0[0], c0[1],
        depthX,          0.0, depthZ,          c1[0], c1[1],
        depthX + xSplit, 0.0, depthZ - zSplit, c2[0], c2[1]
      ],
      sources: [chromCanvas],
    });
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
    var T = ctx.timeline();
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
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit;
    var st = Math.max(0, ctx.global.time);
    var T = ctx.timeline();
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

    var portalCanvas = ctx.getCanvasById('portal-canvas');
    var portalEffect = ly.pixel(PORTAL, PORTAL_SPEC);
    ly.draw(portalCanvas, portalEffect, {
      rect: [0, 0, 1920, 1080],
      uniforms: [1920.0, 1080.0, radius, energy, st - PS, ew, chroma, distort],
      sources: [portalCanvas],
    });
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
  useGSAP((ctx) => {
    var T = ctx.timeline();
    var CK = ctx.CanvasKit;
    var TT = Math.max(0, ctx.global.time);
    var effectMix = TT <= 0.72 ? 0 : (TT >= 1.06 ? 1 : 1 - Math.pow(1 - (TT - 0.72) / 0.34, 4));
    var progress = TT <= 0.72 ? 0 : (TT >= 3.0 ? 1 : (TT - 0.72) / 2.28);
    var logoScale = TT < 2.9 ? 1 : (TT >= 3.1 ? 0.4 : 1 - 0.6 * Math.pow(2, 10 * ((TT - 2.9) / 0.2 - 1)));
    var logoT = TT >= 1.0 ? 1 : 1 - Math.pow(1 - TT, 5);
    var logoY = 760 * (1 - logoT);
    /* gate < 3.1: opacity tweens to 0 exactly at 3.1 (2.9+0.2) — frame 93 is
       invisible, skip its ~12s ray-march (render-budget) */
    if (TT < 3.1) {
      var stingCanvas = ctx.getCanvasById('sting-canvas');
      if (effectMix <= 0) {
        var loEffect = ly.pixel(STING_LOGOONLY, STING_LOGOONLY_SPEC);
        ly.draw(stingCanvas, loEffect, {
          rect: [0, 0, 1920, 1080],
          uniforms: [1920.0, 1080.0, logoScale, logoY],
          sources: [stingCanvas],
        });
      } else {
        var stingEffect = ly.pixel(STING, STING_SPEC);
        ly.draw(stingCanvas, stingEffect, {
          rect: [0, 0, 1920, 1080],
          uniforms: [1920.0, 1080.0, logoScale, logoY, effectMix, progress],
          sources: [stingCanvas, stingCanvas],
        });
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
    var T = ctx.timeline();

    /* bg-2.mp4 (source data-start 3.1; padded copy plays from 0) */
    T.set('next-bg', { opacity: 0 }, 0);
    T.set('next-bg', { opacity: 1 }, 3.1);
    T.to('next-bg', { opacity: 0, duration: 0.36, ease: 'power1.in' }, 5.99);

    /* card entry */
    T.set('next-card', { opacity: 0, scale: 1.45, filter: 'blur(14px)', backdropBlur: 0, transformOrigin: '50% 50%' }, 0);
    T.set('next-card', { opacity: 1, scale: 1.45, filter: 'blur(14px)', transformOrigin: '50% 50%' }, 3.1);
    /* ref frames: f93 (3.1) shows a sharp backdrop with only the blurred caret,
       f95 (3.1667) already blurred — the swgl renderer brings the backdrop
       blur up inside (3.1, 3.1667]; approximate with 3.13 */
    T.set('next-card', { backdropBlur: 12 }, 3.13);
    T.to('next-card', { scale: 0.88, filter: 'blur(0px)', duration: 0.9, ease: [0.266, 0.47, 0.58, 0.908] }, 3.1);
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
    var parts = ctx.splitText('html-type', { type: 'chars' });
    T.set('html-type', { opacity: 1 }, 0);
    T.set('html-word', { x: 300, y: -6, scale: 1.8, transformOrigin: '50% 50%' }, 0);
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
        class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover opacity-0" />
      <div id="next-card" class="absolute left-[120px] top-[67.5px] w-[1680px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.17)] bg-[linear-gradient(160deg,rgba(255,255,255,0.11)_0%,rgba(255,255,255,0.045)_42%,rgba(255,255,255,0.018)_70%,rgba(255,255,255,0.075)_100%)] shadow-[0_30px_90px_rgba(0,0,0,0.42)] opacity-0 overflow-hidden">
        <div id="glow-a" class="absolute left-[-40px] top-[520px] w-[560px] h-[560px] rounded-full bg-[radial-gradient(closest-side,rgba(73,242,255,0.14),transparent)]"></div>
        <div id="glow-b" class="absolute left-[1260px] top-[-10px] w-[440px] h-[440px] rounded-full bg-[radial-gradient(closest-side,rgba(255,79,216,0.11),transparent)]"></div>
        <div id="phrase-a" class="absolute left-[126px] right-[126px] top-[96px] bottom-[96px] flex flex-col items-center justify-center text-center text-[#ffffff] text-[292px] font-[inter900] font-black leading-[0.88] tracking-[0]">
          <div id="pa-line1" class="flex items-center justify-center">
            <div id="html-word" class="relative text-left text-[#f6ff7a] font-[big] font-black tracking-[4.38px]">
              <text id="html-type" class="font-[big] font-black tracking-[4.38px]">HTML</text>
              <text id="html-caret" class="absolute left-[0px] top-[0px] text-[#f6ff7a] font-[big] font-black text-[292px] leading-[0.88] opacity-0">|</text>
            </div>
            <text id="html-tail" class="opacity-0">{'\u2013'}in</text>
          </div>
          <text id="canvas-word" class="opacity-0">canvas</text>
        </div>
        <div id="phrase-b" class="absolute left-[126px] right-[126px] top-[96px] bottom-[96px] text-[#ffffff] opacity-0">
          <text id="avail-now" class="absolute left-[0px] top-[76px] text-[161px] font-[inter800] font-extrabold leading-[0.88]">Now</text>
          <text id="avail-natively" class="absolute left-[92px] top-[212px] text-[#f6ff7a] font-[serifd] text-[304px] italic font-normal leading-[0.78]">natively</text>
          <div id="avail-supported-wrap" class="absolute left-[560px] top-[526px] w-[940px] flex items-baseline justify-end">
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
