/* inspector-launch — HyperFrames "Inspector Launch V1" port (44.97s @ 30fps).
 * Reference: F:/hyperframes-launches/inspector-launch/index.html
 *
 * Architecture:
 *  - #global-halftone: reference paints a 1600x1220 canvas upscaled to
 *    4320x3300 (CSS 2.7x). Here the canvas node is 4320x3300 at (-1200,-1100);
 *    a cheap SKSL RuntimeEffect paints bg gradient + screen-blended wash,
 *    then ~12k VISIBLE dots (culled through the node transform) are filled as
 *    native Skia ovals from JS — the field math (mulberry32/hash2/fbm) is
 *    copied verbatim from the reference, so dot values are bit-exact.
 *  - All GSAP timings/easings are ported 1:1. The three CustomEase springs
 *    are precomputed physics tables registered through the engine's
 *    `points:<name>` sampled-ease support (crates parse/easing.rs Samples).
 *  - bgState is a plain object; its numeric keys are registered as animation
 *    properties so timelines can tween it like the reference proxy.
 */
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 44.97 };

export const fonts = {
  default: 'if4',
  faces: [
    { id: 'if4', family: 'Inter', path: 'assets/fonts/Inter-400.ttf' },
    { id: 'if5', family: 'Inter', path: 'assets/fonts/Inter-500.ttf' },
    { id: 'if6', family: 'Inter', path: 'assets/fonts/Inter-600.ttf' },
    { id: 'if7', family: 'Inter', path: 'assets/fonts/Inter-700.ttf' },
    { id: 'if8', family: 'Inter', path: 'assets/fonts/Inter-800.ttf' },
    { id: 'if9', family: 'Inter', path: 'assets/fonts/Inter-900.ttf' },
    { id: 'jb4', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Regular.ttf' },
    { id: 'dmi', family: 'DM Serif Display', path: 'assets/fonts/DMSerifDisplay-Italic.ttf' },
    { id: 'ge4', family: 'Georgia', path: 'assets/fonts/georgia.ttf' },
    { id: 'ge7', family: 'Georgia Bold', path: 'assets/fonts/georgiab.ttf' },
    { id: 'gei', family: 'Georgia Italic', path: 'assets/fonts/georgiai.ttf' }
  ]
};

var TIMELINE_DUR = 44.97;
var INTRO_DUR = 11.61;
var MAG_SCALE = 2.4, MAG_R = 110, TEXT_X = 1123, TEXT_Y = 507;
var SWEEP_START = 90, SWEEP_END = 55, SWEEP_TOP = 54;

function clamp(v, a, b) { return Math.max(a, Math.min(b, v)); }

/* ── scene table (index.html:2276-2295) ─────────────────────────────── */
var SCENES = [
  { id: 'scene-00',  at: 10.74, dur: 1.72, entry: 'none',     bg: { x: 60,   y: 40,   scale: 1.04, hue: 0,   sat: 1.04, density: 0.94, radius: 1.0,  palette: 0.4,  wash: 0.06 } },
  { id: 'scene-01',  at: 12.47, dur: 2.2,  entry: 'zoom',     bg: { x: 0,    y: 0,    scale: 1.06, hue: 0,   sat: 1.08, density: 0.98, radius: 1.04, palette: 0.46 } },
  { id: 'scene-01b', at: 14.68, dur: 1.8,  entry: 'zoom',     bg: { x: -60,  y: -10,  scale: 1.12, hue: 6,   sat: 1.18, density: 1.04, radius: 1.10, palette: 0.58 } },
  { id: 'scene-01c', at: 16.48, dur: 1.59, entry: 'zoom',     bg: { x: -90,  y: -14,  scale: 1.16, hue: 12,  sat: 1.22, density: 1.06, radius: 1.14, palette: 0.62 } },
  { id: 'scene-02',  at: 18.07, dur: 2,    entry: 'left',     bg: { x: -120, y: -18,  scale: 1.22, hue: 10,  sat: 1.24, density: 1.08, radius: 1.2,  palette: 0.68 } },
  { id: 'scene-03',  at: 20.07, dur: 1.7,  entry: 'up',       bg: { x: -120, y: -175, scale: 1.12, hue: -8,  sat: 1.14, density: 1.16, radius: 1.02, palette: 0.34 } },
  { id: 'scene-04',  at: 21.77, dur: 2.4,  entry: 'up',       exitDur: 0.15, bg: { x: -70,  y: -320, scale: 1.06, hue: 0,   sat: 1.05, density: 1.08, radius: 0.96, palette: 0.48 } },
  { id: 'scene-05',  at: 24.17, dur: 3.0,  entry: 'continue', bg: { x: -35,  y: -500, scale: 1.3,  hue: 18,  sat: 1.42, density: 1.34, radius: 0.9,  palette: 0.92, wash: 0.42 } },
  { id: 'scene-06',  at: 27.18, dur: 2.08, entry: 'left',     bg: { x: -300, y: -500, scale: 1.15, hue: -4,  sat: 1.2,  density: 1.12, radius: 1.04, palette: 0.42 } },
  { id: 'scene-07',  at: 29.27, dur: 2.09, entry: 'left',     bg: { x: -540, y: -470, scale: 1.18, hue: 8,   sat: 1.18, density: 1.02, radius: 0.96, palette: 0.58 } },
  { id: 'scene-08',  at: 31.37, dur: 2.19, entry: 'up',       bg: { x: -540, y: -635, scale: 1.16, hue: 20,  sat: 1.27, density: 1.16, radius: 1.08, palette: 0.86 } },
  { id: 'scene-10',  at: 33.57, dur: 2,    entry: 'zoom',     bg: { x: -470, y: -780, scale: 1.26, hue: -12, sat: 1.32, density: 1.14, radius: 1.14, palette: 0.24 } },
  { id: 'scene-11',  at: 35.57, dur: 2.8,  entry: 'left',     bg: { x: -700, y: -730, scale: 1.14, hue: 0,   sat: 1.18, density: 1.1,  radius: 1,    palette: 0.5 } },
  { id: 'scene-12',  at: 38.38, dur: 1.78, entry: 'zoom',     bg: { x: -700, y: -730, scale: 1.24, hue: -18, sat: 1.3,  density: 1.2,  radius: 1.18, palette: 0.14, lift: 0.12 } },
  { id: 'scene-13',  at: 40.17, dur: 2,    entry: 'zoom',     bg: { x: -700, y: -730, scale: 1.32, hue: 4,   sat: 1.36, density: 0.98, radius: 1.1,  palette: 0.56 } },
  { id: 'scene-14',  at: 42.17, dur: 2.8,  entry: 'zoom',     bg: { x: -615, y: -850, scale: 1.18, hue: 0,   sat: 1.18, density: 1.04, radius: 1,    palette: 0.5 } }
];
var VECTORS = {
  left:  { x: -220, y: 0,    inX: 220,  inY: 0 },
  right: { x: 220,  y: 0,    inX: -220, inY: 0 },
  up:    { x: 0,    y: -170, inX: 0,    inY: 170 },
  down:  { x: 0,    y: 170,  inX: 0,    inY: -170 }
};

function restField(bg) {
  return {
    density:     bg.density     != null ? bg.density     : clamp(0.82 + bg.scale * 0.24 + Math.abs(bg.hue) * 0.004, 0.92, 1.34),
    radius:      bg.radius      != null ? bg.radius      : clamp(0.88 + (bg.sat - 1) * 1.1, 0.88, 1.34),
    lift:        bg.lift        != null ? bg.lift        : clamp((bg.sat - 1) * 0.32, 0, 0.18),
    centerX:     bg.centerX     != null ? bg.centerX     : clamp(0.5 - bg.x / 2200, 0.35, 0.7),
    centerY:     bg.centerY     != null ? bg.centerY     : clamp(0.53 - bg.y / 1900, 0.38, 0.78),
    palette:     bg.palette     != null ? bg.palette     : clamp((bg.hue + 24) / 48, 0, 1),
    wash:        bg.wash        != null ? bg.wash        : clamp(bg.sat - 1, 0, 0.45),
    warp:        bg.warp        != null ? bg.warp        : 0.22,
    shapeScale:  bg.shapeScale  != null ? bg.shapeScale  : 0.24,
    ribbonScale: bg.ribbonScale != null ? bg.ribbonScale : 0.06,
    blend:       0
  };
}

var INIT_BG = SCENES[0].bg;
var INIT_FIELD = restField(INIT_BG);
var FINAL_FLOW = 6.89, FINAL_PHASE = 5.31, FINAL_RIBBON = 5.89;

/* ── halftone state (drawHalftone's `s` folds the resting baseline in;
      we keep the raw animated state here and fold in JS before drawing) ── */
var bgState = {
  phase: 0, flow: 0, density: 1.02, radius: 1.0, lift: 0.03,
  centerX: 0.5, centerY: 0.53, palette: 0.46, wash: 0.14, warp: 0.22,
  shapeScale: 0.24, ribbonScale: 0.06, blend: 0, ribbon: 0, massShiftY: 0
};
/* plate view transform for #global-halftone (reference tweens the node's
   x/y/scale/filter directly; we tween this proxy and apply it in the draw
   hook so the dot culler can read the exact same values) */
var htView = { x: 60, y: 40, scale: 1.04, hue: 0, sat: 1.04, blur: 0 };

/* ── CustomEase spring tables (index.html springEase) ───────────────── */
function springPoints(stiffness, damping) {
  var points = [], pos = 0, vel = 0, dt = 1 / 120, i, force;
  for (i = 0; i < 240; i += 1) {
    force = -stiffness * (pos - 1) - damping * vel;
    vel += force * dt;
    pos += vel * dt;
    points.push(clamp(pos, -0.2, 1.3));
  }
  return points;
}
/* Native shims (__ease_register_points) and the animation runtime
 * (__opencatAnimation) are injected by the script realm at install time, but
 * module top-level code runs earlier (the extraction pass) where they are
 * still undefined. Defer to a frame callback: __ocOnFrame runs before every
 * useGSAP build inside the realm, so the tables/properties are in place
 * before any tween parses 'points:spring*' or a bgState key. The typeof
 * guards make the extraction pass a no-op instead of a crash. */
function initInspectorRuntime() {
  if (globalThis.__IL_INIT) return;
  if (typeof globalThis.__ease_register_points !== 'function') return;
  if (typeof globalThis.__opencatAnimation !== 'object' || !globalThis.__opencatAnimation) return;
  globalThis.__IL_INIT = true;
  globalThis.__ease_register_points('springHard', springPoints(180, 14));
  globalThis.__ease_register_points('springSoft', springPoints(120, 18));
  globalThis.__ease_register_points('springHeavy', springPoints(80, 22));
  var animation = globalThis.__opencatAnimation.animation;
  ['flow', 'phase', 'density', 'radius', 'lift', 'centerX', 'centerY', 'palette',
   'wash', 'warp', 'shapeScale', 'ribbonScale', 'blend', 'ribbon', 'massShiftY',
   'htX', 'htY', 'htScale', 'htHue', 'htSat', 'htBlur'].forEach(function (k) {
    animation.registerProperty(k, {
      interpolate: 'number',
      defaultValue: 0,
      apply: function (target, value) { target[k] = value; }
    });
  });
}
globalThis.__ocOnFrame(initInspectorRuntime);
var E_HARD = 'points:springHard', E_SOFT = 'points:springSoft', E_HEAVY = 'points:springHeavy';

/* ── SKSL: base layer — bg linear gradient + screen-blended wash ────── */
var SKSL_BASE = [
'uniform float uAX; uniform float uAY; uniform float uBX; uniform float uBY;',
'uniform float uCX; uniform float uCY; uniform float uBlend; uniform float uPhase;',
'const float PI = 3.141592653589793;',
'float3 scr(float3 cb, float3 cs) { return cb + cs - cb * cs; }',
'float ell(float2 p, float2 c, float2 r, float a) {',
'  float d = length((p - c) / r);',
'  float al;',
'  if (d < 0.52) { al = a * (1.0 - (1.0 - 0.42) * (d / 0.52)); }',
'  else { al = a * 0.42 * (1.0 - (d - 0.52) / 0.48); }',
'  return clamp(al, 0.0, 1.0);',
'}',
'float ribbonDist(float2 p) {',
'  float ry = 1220.0 * (0.5 + sin(uPhase * 0.72) * 0.16);',
'  float2 p0 = float2(-0.14 * 1600.0, ry + sin(uPhase * 0.9) * 1220.0 * 0.08);',
'  float2 p1 = float2(0.2 * 1600.0, ry - 1220.0 * 0.24);',
'  float2 p2 = float2(0.62 * 1600.0, ry + 1220.0 * 0.26);',
'  float2 p3 = float2(1.14 * 1600.0, ry - cos(uPhase * 0.7) * 1220.0 * 0.10);',
'  float best = 1e9;',
'  for (int i = 0; i <= 14; i++) {',
'    float t = float(i) / 14.0;',
'    float mt = 1.0 - t;',
'    float2 b = mt*mt*mt*p0 + 3.0*mt*mt*t*p1 + 3.0*mt*t*t*p2 + t*t*t*p3;',
'    best = min(best, length(p - b));',
'  }',
'  return best;',
'}',
'half4 main(float2 frag) {',
'  float2 p = frag / 2.7;',
'  float gt = clamp(dot(p, float2(1600.0, 1220.0)) / dot(float2(1600.0, 1220.0), float2(1600.0, 1220.0)), 0.0, 1.0);',
'  if (gt < 0.52) { bg = mix(float3(0.078431, 0.125490, 0.090196), float3(0.050980, 0.082353, 0.062745), gt / 0.52); }',
'  else { bg = mix(float3(0.050980, 0.082353, 0.062745), float3(0.023529, 0.043137, 0.035294), (gt - 0.52) / 0.48); }',
'  float3 cb = bg;',
'  float3 green = float3(0.270588, 0.847059, 0.431373);',
'  float3 cyan  = float3(0.098039, 0.717647, 0.835294);',
'  float3 amber = float3(0.945098, 0.713726, 0.290196);',
'  float2 pp = float2(p.x * 1600.0, p.y * 1220.0);',
'  float aA = ell(pp, float2(uAX * 1600.0, uAY * 1220.0), float2(1600.0 * (0.32 + uBlend * 0.08), 1220.0 * 0.42), 0.18 + uBlend * 0.10);',
'  cb = mix(cb, scr(cb, green), aA);',
'  float aB = ell(pp, float2(uBX * 1600.0, uBY * 1220.0), float2(1600.0 * 0.30, 1220.0 * (0.40 + uBlend * 0.06)), 0.15 + uBlend * 0.09);',
'  cb = mix(cb, scr(cb, cyan), aB);',
'  float aC = ell(pp, float2(uCX * 1600.0, uCY * 1220.0), float2(1600.0 * 0.46, 1220.0 * 0.20), 0.09 + uBlend * 0.05);',
'  cb = mix(cb, scr(cb, amber), aC);',
'  float d = ribbonDist(pp);',
'  float hw = 1220.0 * (0.18 + uBlend * 0.08) * 0.5;',
'  float cov = 1.0 - smoothstep(hw - 40.0, hw + 40.0, d);',
'  if (cov > 0.001) {',
'    float t = clamp(pp.x / 1600.0, 0.0, 1.0);',
'    float3 rc; float ra;',
'    if (t < 0.28) { rc = green; ra = mix(0.0, 0.12 + uBlend * 0.05, t / 0.28); }',
'    else if (t < 0.58) { rc = mix(green, cyan, (t - 0.28) / 0.30); ra = mix(0.12 + uBlend * 0.05, 0.13 + uBlend * 0.06, (t - 0.28) / 0.30); }',
'    else if (t < 0.86) { rc = mix(cyan, amber, (t - 0.58) / 0.28); ra = mix(0.13 + uBlend * 0.06, 0.07 + uBlend * 0.03, (t - 0.58) / 0.28); }',
'    else { rc = mix(amber, cyan, (t - 0.86) / 0.14); ra = mix(0.07 + uBlend * 0.03, 0.0, (t - 0.86) / 0.14); }',
'    cb = mix(cb, scr(cb, rc), cov * clamp(ra, 0.0, 1.0));',
'  }',
'  return half4(half3(cb), 1.0);',
'}'
].join('\n');

/* SKSL: top layer — vignette (black, source-over) */
var SKSL_VIG = [
'uniform float uVCX; uniform float uVCY;',
'half4 main(float2 frag) {',
'  float2 p = frag / 2.7;',
'  float2 c = float2(uVCX, uVCY);',
'  float len = distance(p, c);',
'  float t = clamp((len - 1220.0 * 0.05) / (length(float2(800.0, 610.0)) * 0.92 - 1220.0 * 0.05), 0.0, 1.0);',
'  float a;',
'  if (t < 0.46) { a = 0.10 + (0.03 - 0.10) * (t / 0.46); }',
'  else { a = 0.03 + (0.40 - 0.03) * ((t - 0.46) / 0.54); }',
'  return half4(half3(0.0), half(a));',
'}'
].join('\n');

/* ── verbatim reference field math (bit-exact JS) ───────────────────── */
function hash2(ix, iy) {
  var n = ix * 374761393 + iy * 668265263;
  n = (n ^ (n >>> 13)) * 1274126177;
  return ((n ^ (n >>> 16)) >>> 0) / 4294967295;
}
function smoothstep(e0, e1, v) {
  var t = clamp((v - e0) / (e1 - e0), 0, 1);
  return t * t * (3 - 2 * t);
}
function valueNoise(x, y) {
  var ix = Math.floor(x), iy = Math.floor(y);
  var fx = smoothstep(0, 1, x - ix), fy = smoothstep(0, 1, y - iy);
  var n00 = hash2(ix, iy), n10 = hash2(ix + 1, iy);
  var n01 = hash2(ix, iy + 1), n11 = hash2(ix + 1, iy + 1);
  var nx0 = n00 * (1 - fx) + n10 * fx;
  var nx1 = n01 * (1 - fx) + n11 * fx;
  return nx0 * (1 - fy) + nx1 * fy;
}
function fbm(x, y) {
  var value = 0, amp = 0.56, total = 0, px = x, py = y, i, rx, ry;
  for (i = 0; i < 4; i += 1) {
    value += valueNoise(px, py) * amp;
    total += amp;
    rx = px * 1.62 + py * 0.42; ry = py * 1.48 - px * 0.36;
    px = rx + 13.7; py = ry - 8.9;
    amp *= 0.52;
  }
  return value / total;
}
function gradientMass(x, y, cx, cy, rx, ry) {
  var dx = (x - cx) / rx, dy = (y - cy) / ry;
  return Math.exp(-(dx * dx + dy * dy));
}
function shaderField(x, y, s, mass) {
  var t = s.phase;
  var warpA = (fbm(x * 2.3 + t * 0.24, y * 1.8 - t * 0.16) - 0.5) * s.warp;
  var warpB = (fbm(x * 1.7 - t * 0.12 + 4.6, y * 2.15 + t * 0.2) - 0.5) * s.warp;
  var ux = x + warpA + Math.sin(y * Math.PI * 2.1 + t * 0.9) * 0.035;
  var uy = y + warpB + Math.cos(x * Math.PI * 1.55 - t * 0.75) * 0.03;
  var mA = gradientMass(ux, uy, mass.AX, mass.AY, 0.36, 0.48);
  var mB = gradientMass(ux, uy, mass.BX, mass.BY, 0.3, 0.46);
  var mC = gradientMass(ux, uy, mass.CX, mass.CY, 0.54, 0.22);
  var ribbon = smoothstep(0.18, 0.88, Math.sin((ux * 1.12 - uy * 0.22 + s.ribbon) * Math.PI * 2));
  var nested = fbm(ux * 2.2 + mA * 0.7 + t * 0.12, uy * 2.0 + mB * 0.7 - t * 0.1);
  var shape = clamp(mA * 0.5 + mB * 0.38 + mC * 0.24 + ribbon * 0.18 + nested * 0.22, 0, 1);
  var hue = clamp(shape * 0.65 + ribbon * 0.2 + nested * 0.28 + s.palette * 0.3, 0, 1);
  return { shape: shape, hue: hue, ribbon: ribbon };
}
/* closed-form mulberry32: call i reads state seed + (i+1)*0x6D2B79F5 */
function randAt(i) {
  var t = (9400 + (i + 1) * 0x6D2B79F5) >>> 0;
  var r = Math.imul(t ^ (t >>> 15), 1 | t);
  r ^= (r + Math.imul(r ^ (r >>> 7), 61 | r)) >>> 0;
  return ((r ^ (r >>> 14)) >>> 0) / 4294967296;
}

var DOT_COLORS = [
  [69, 216, 110], [25, 183, 213], [255, 248, 233], [241, 182, 74], [92, 132, 99]
];
var DARK = [5, 12, 10], LIGHT = [244, 239, 226];
function mixc(a, b, t) {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

function halftoneS() {
  return {
    phase: bgState.phase, flow: bgState.flow, density: bgState.density,
    radius: bgState.radius, lift: bgState.lift + 0.09,
    centerX: bgState.centerX, centerY: bgState.centerY,
    palette: clamp(bgState.palette + 0.16, 0, 1),
    wash: bgState.wash + 0.12, warp: bgState.warp,
    shapeScale: bgState.shapeScale + 0.14, ribbonScale: bgState.ribbonScale + 0.05,
    blend: bgState.blend, ribbon: bgState.ribbon, massShiftY: bgState.massShiftY || 0
  };
}
function halftoneMass(s) {
  return {
    AX: -0.20 - s.flow * 0.045 + Math.sin(s.phase * 0.55) * 0.06,
    AY: 0.46 + s.massShiftY + Math.sin(s.phase * 0.62 + 1.1) * 0.10,
    BX: 1.08 - s.flow * 0.055 + Math.cos(s.phase * 0.48 + 2.3) * 0.06,
    BY: 0.56 + s.massShiftY + Math.cos(s.phase * 0.52 + 0.4) * 0.10,
    CX: 0.58 - s.flow * 0.030 + Math.sin(s.phase * 0.36 + 1.4) * 0.18,
    CY: 0.38 + s.massShiftY + Math.cos(s.phase * 0.34 + 0.8) * 0.08
  };
}

function vis(ctx, id, on) { var n = ctx.getNode(id); if (n) n.opacity(on ? 1 : 0); }

/* ════════════ global halftone background ════════════ */
function Halftone() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.global.time;
    /* replay every queued timeline write so bgState/htView are final */
    ctx.__flushTimelines();
    var node = ctx.getNode('global-halftone');
    if (node) {
      node.translateX(htView.x);
      node.translateY(htView.y);
      node.scale(htView.scale);
      node.filter('hue-rotate(' + htView.hue + 'deg) saturate(' + htView.sat + ') blur(' + htView.blur + 'px)');
    }
    var gcv = ctx.getCanvasById('global-halftone');
    if (!gcv) return;
    gcv.clear();
    var s = halftoneS();
    var mass = halftoneMass(s);

    /* base layer: bg gradient + wash (SKSL) */
    if (!globalThis.__IL_BASEFX) globalThis.__IL_BASEFX = CK.RuntimeEffect.Make(SKSL_BASE);
    if (!globalThis.__IL_VIGFX) globalThis.__IL_VIGFX = CK.RuntimeEffect.Make(SKSL_VIG);
    var baseFx = globalThis.__IL_BASEFX;
    if (baseFx) {
      var bp = new CK.Paint();
      bp.setStyle(CK.PaintStyle.Fill);
      var sh = baseFx.makeShader([mass.AX, mass.AY, mass.BX, mass.BY, mass.CX, mass.CY, s.blend, s.phase]);
      if (sh) { bp.setShader(sh); gcv.drawRect([0, 0, 4320, 3300], bp); }
      else {
        bp.setColor(CK.Color(13, 21, 16, 1));
        gcv.drawRect([0, 0, 4320, 3300], bp);
      }
    }

    /* dots: only the visible source-window (reference canvas coords /2.7) */
    var spacing = 5.15 / s.density;
    var radius = 1.02 * s.radius;
    var cols = Math.ceil((1600 + 2 * spacing) / spacing);
    var rows = Math.ceil((1220 + 2 * spacing) / spacing);
    /* node local center (2160,1650); screen center (-1200+htX+2160, -1100+htY+1650) */
    var csx = 960 + htView.x, csy = 550 + htView.y;
    var inv = 1 / (htView.scale * 2.7);
    var x0 = (2160 + (0 - csx) * inv) - 8, x1 = (2160 + (1920 - csx) * inv) + 8;
    var y0 = (1650 + (0 - csy) * inv) - 8, y1 = (1650 + (1080 - csy) * inv) + 8;
    var k0 = Math.max(0, Math.ceil((x0 + spacing) / spacing));
    var k1 = Math.min(cols - 1, Math.floor((x1 + spacing) / spacing));
    var j0 = Math.max(0, Math.ceil((y0 + spacing) / spacing));
    var j1 = Math.min(rows - 1, Math.floor((y1 + spacing) / spacing));
    var cx = 1600 * s.centerX, cy = 1220 * s.centerY;
    var maxD = Math.hypot(800, 610);
    var paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);
    var w = 1600, h = 1220, j, k, X, Y, col, row, field, dist, centerDark, topLight, noise;
    var energy, colorSeed, colorIndex, tonal, fieldColor, color, alpha, sizeBase, size, px;
    for (j = j0; j <= j1; j += 1) {
      Y = (j - 1) * spacing;
      row = Y / h;
      for (k = k0; k <= k1; k += 1) {
        X = (k - 1) * spacing;
        col = X / w;
        dist = Math.hypot(X - cx, Y - cy) / maxD;
        field = shaderField(col, row, s, mass);
        centerDark = clamp(1 - dist * 2.25, 0, 1) * 0.62;
        topLight = clamp(1 - row * 1.6, 0, 1);
        noise = randAt(j * cols + k);
        energy = 0.18 + topLight * 0.18 + field.shape * (0.54 + s.blend * 0.26) + s.lift - centerDark * 0.36;
        energy = clamp(energy + (noise - 0.5) * 0.025, 0.05, 0.98);
        colorSeed = field.hue * 3.2 + s.palette * 1.4 + row * 0.42;
        colorIndex = ((Math.floor(colorSeed) % 5) + 5) % 5;
        tonal = mixc(DARK, LIGHT, energy);
        fieldColor = mixc(DOT_COLORS[colorIndex], DOT_COLORS[(colorIndex + 1) % 5], smoothstep(0.24, 0.78, field.ribbon));
        color = mixc(tonal, fieldColor, 0.36 + field.shape * 0.34 + s.wash * 0.24);
        alpha = clamp(0.18 + energy * 0.72 + s.blend * 0.12, 0.14, 0.98);
        sizeBase = 0.40 + energy * 1.10 + field.shape * s.shapeScale + field.ribbon * s.ribbonScale;
        size = radius * clamp(sizeBase + (noise - 0.5) * 0.045, 0.32, 4.4);
        px = X + (noise - 0.5) * 0.12;
        paint.setColor(CK.Color(Math.floor(color[0]), Math.floor(color[1]), Math.floor(color[2]), Math.round(alpha * 1000) / 1000));
        gcv.drawCircle(px * 2.7, Y * 2.7, size * 2.7, paint);
      }
    }

    /* cut-frame wash (flat fills over the dots) */
    if (s.blend > 0.01) {
      paint.setColor(CK.Color(69, 216, 110, Math.round(0.08 * s.blend * 1000) / 1000));
      gcv.drawRect([0, 0, 4320, 3300], paint);
      paint.setColor(CK.Color(25, 183, 213, Math.round(0.07 * s.blend * 1000) / 1000));
      gcv.drawRect([1600 * 0.18 * 2.7, 0, 4320, 3300], paint);
    }

    /* vignette (SKSL, source-over) */
    var vigFx = globalThis.__IL_VIGFX;
    if (vigFx) {
      var vp = new CK.Paint();
      vp.setStyle(CK.PaintStyle.Fill);
      var vsh = vigFx.makeShader([cx * 2.7, cy * 2.7]);
      if (vsh) { vp.setShader(vsh); gcv.drawRect([0, 0, 4320, 3300], vp); }
    }
  });
  return <canvas id="global-halftone" class="absolute left-[-1200px] top-[-1100px] w-[4320px] h-[3300px] z-[0]" />;
}

/* ════════════ background system (stages + bg transitions + master) ════ */
/* static precompute of every cut (setBgTransition), including the absolute
   massShiftY accumulator (reference uses `+=` on the proxy) */
var CUTS = [];
(function () {
  var msY = 0, i, curr, next, exitDur, exitStart, start;
  for (i = 0; i < SCENES.length - 1; i += 1) {
    curr = SCENES[i]; next = SCENES[i + 1];
    exitDur = next.exitDur != null ? next.exitDur : (next.entry === 'zoom' ? 0.22 : 0.33);
    exitStart = next.at - exitDur;
    start = Math.max(curr.at + 0.05, exitStart - 0.1);
    CUTS.push({
      curr: curr, next: next, exitStart: exitStart, exitDur: exitDur, start: start,
      cutFrame: start + 0.33,
      currField: restField(curr.bg), nextField: restField(next.bg),
      dirY: VECTORS[next.entry] && VECTORS[next.entry].y ? Math.sign(VECTORS[next.entry].y) : 0,
      msBefore: msY
    });
    msY += (VECTORS[next.entry] && VECTORS[next.entry].y ? Math.sign(VECTORS[next.entry].y) : 0) * 0.036;
  }
})();

function magFilter(f) { return 'hue-rotate(' + f.hue + 'deg) saturate(' + f.sat + ') blur(' + f.blur + 'px)'; }

function setStageEntry(tl, scene, index) {
  var sid = scene.id + '-stage';
  if (index === 0 || scene.entry === 'none' || scene.entry === 'continue') {
    tl.set(sid, { opacity: 1, x: 0, y: 0, scale: 1, filter: 'blur(0px)' }, scene.at);
    return;
  }
  if (scene.entry === 'zoom') {
    tl.set(sid, { opacity: 0.15, x: 0, y: 0, scale: 0.75, filter: 'blur(20px)' }, scene.at);
    tl.to(sid, { opacity: 1, scale: 1, filter: 'blur(0px)', duration: 0.5, ease: 'expo.out' }, scene.at);
    return;
  }
  var v = VECTORS[scene.entry];
  tl.set(sid, { opacity: 1, x: v.inX, y: v.inY, scale: 1, filter: 'blur(8px)' }, scene.at);
  tl.to(sid, { x: 0, y: 0, filter: 'blur(0px)', duration: 0.42, ease: 'expo.out' }, scene.at);
}

function setStageExit(tl, cut) {
  var sid = cut.curr.id + '-stage';
  if (cut.next.entry === 'continue') return;
  if (cut.next.entry === 'zoom') {
    tl.to(sid, { scale: 1.2, filter: 'blur(20px)', duration: cut.exitDur, ease: 'power3.in' }, cut.exitStart);
    tl.to(sid, { opacity: 0.15, duration: cut.exitDur, ease: 'none' }, cut.exitStart);
    return;
  }
  var v = VECTORS[cut.next.entry];
  tl.to(sid, { x: v.x, y: v.y, filter: 'blur(8px)', duration: cut.exitDur, ease: 'power3.in' }, cut.exitStart);
}

function setBgTransition(tl, cut) {
  var nf = cut.nextField, cf = cut.currField;
  if (cut.next.entry === 'continue') {
    tl.to(htView, { htX: cut.next.bg.x, htY: cut.next.bg.y, htScale: cut.next.bg.scale, htHue: cut.next.bg.hue, htSat: cut.next.bg.sat, htBlur: 0, duration: 1.0, ease: 'power1.inOut' }, cut.start);
    tl.to(bgState, {
      density: nf.density, radius: nf.radius, lift: nf.lift, centerX: nf.centerX, centerY: nf.centerY,
      palette: nf.palette, wash: nf.wash, warp: nf.warp, shapeScale: nf.shapeScale, ribbonScale: nf.ribbonScale,
      blend: 0, duration: 1.0, ease: 'power1.inOut'
    }, cut.start);
    return;
  }
  var peakX = cut.curr.bg.x + (cut.next.bg.x - cut.curr.bg.x) * 0.55;
  var peakY = cut.curr.bg.y + (cut.next.bg.y - cut.curr.bg.y) * 0.55;
  var peakScale = (cut.curr.bg.scale + cut.next.bg.scale) * 0.5 + 0.08;
  var peakHue = (cut.curr.bg.hue + cut.next.bg.hue) * 0.5;
  var peakSat = Math.max(cut.curr.bg.sat, cut.next.bg.sat) + 0.14;
  tl.to(htView, { htX: peakX, htY: peakY, htScale: peakScale, htHue: peakHue, htSat: peakSat, htBlur: 3, duration: 0.36, ease: 'power2.in' }, cut.start);
  tl.to(htView, { htX: cut.next.bg.x, htY: cut.next.bg.y, htScale: cut.next.bg.scale, htHue: cut.next.bg.hue, htSat: cut.next.bg.sat, htBlur: 0, duration: 0.52, ease: 'back.out(0.4)' }, cut.cutFrame);
  tl.to(bgState, {
    density: nf.density + 0.03, radius: nf.radius + 0.10, lift: nf.lift + 0.04,
    centerX: (cf.centerX + nf.centerX) * 0.5, centerY: (cf.centerY + nf.centerY) * 0.5,
    palette: clamp(nf.palette + 0.08, 0, 1), wash: nf.wash + 0.06, warp: nf.warp + 0.05,
    shapeScale: nf.shapeScale + 0.14, ribbonScale: nf.ribbonScale + 0.05,
    blend: 0.42, massShiftY: cut.msBefore + cut.dirY * 0.018,
    duration: 0.36, ease: 'power2.in'
  }, cut.start);
  tl.to(bgState, {
    density: nf.density, radius: nf.radius, lift: nf.lift, centerX: nf.centerX, centerY: nf.centerY,
    palette: nf.palette, wash: nf.wash, warp: nf.warp, shapeScale: nf.shapeScale, ribbonScale: nf.ribbonScale,
    blend: 0, massShiftY: cut.msBefore + cut.dirY * 0.036,
    duration: 0.52, ease: 'back.out(0.4)'
  }, cut.cutFrame);
}

function addBgDrift(tl, scene, index) {
  var next = SCENES[index + 1];
  var driftStart = scene.at + (scene.entry === 'none' || scene.entry === 'continue' ? 0.18 : 0.5);
  var transitionStart = next ? next.at - (next.entry === 'zoom' ? 0.32 : 0.43) : scene.at + scene.dur - 0.2;
  var duration = Math.max(0.28, transitionStart - driftStart);
  var f = restField(scene.bg);
  tl.to(bgState, {
    centerX: clamp(f.centerX + (index % 2 === 0 ? 0.018 : -0.014), 0.34, 0.72),
    centerY: clamp(f.centerY + (index % 3 === 0 ? 0.012 : -0.01), 0.36, 0.8),
    duration: duration, ease: 'sine.inOut'
  }, driftStart);
}

function BgSystem() {
  useGSAP((ctx) => {
    var tl = ctx.timeline();
    SCENES.forEach((scene, index) => {
      setStageEntry(tl, scene, index);
      var cut = CUTS[index];
      if (cut) { setBgTransition(tl, cut); setStageExit(tl, cut); }
    });
    SCENES.forEach((scene, index) => addBgDrift(tl, scene, index));
    tl.to(bgState, { flow: FINAL_FLOW, phase: FINAL_PHASE, ribbon: FINAL_RIBBON, duration: TIMELINE_DUR, ease: 'none' }, 0);
    tl.set(htView, { htX: INIT_BG.x, htY: INIT_BG.y, htScale: INIT_BG.scale, htHue: INIT_BG.hue, htSat: INIT_BG.sat, htBlur: 0 }, 0);
    tl.set(bgState, Object.assign({}, INIT_FIELD, { phase: 0, flow: 0, ribbon: 0, massShiftY: 0 }), 0);
  });
  return <div id="il-bgsystem" class="absolute left-0 top-0 w-[0px] h-[0px]" />;
}

/* ════════════ intro: background beats (reference intro block order) ═══ */
function IntroBg() {
  useGSAP((ctx) => {
    ctx.timeline()
      .to(bgState, { density: 1.02, wash: 0.14, palette: 0.46, duration: 1.33, ease: 'sine.inOut' }, 0)
      .to(bgState, { blend: 0.42, radius: 1.1, shapeScale: 0.38, density: 1.05, lift: 0.07, duration: 0.60, ease: E_SOFT }, 0.58)
      .to(bgState, { blend: 0, radius: 1.0, shapeScale: 0.24, density: 1.02, lift: 0.03, duration: 0.87, ease: E_HEAVY }, 1.18)
      .to(htView, { htScale: 1.08, htHue: 4, htSat: 1.12, htBlur: 0, duration: 0.60, ease: E_SOFT }, 0.58)
      .to(htView, { htScale: 1.04, htHue: 0, htSat: 1.08, htBlur: 0, duration: 0.87, ease: E_HEAVY }, 1.18)
      .to(htView, { htX: -10, duration: 1.33, ease: 'sine.inOut' }, 1.42)
      .to(bgState, { centerX: 0.52, centerY: 0.55, duration: 1.33, ease: 'sine.inOut' }, 1.42)
      .to(htView, { htHue: -4, htSat: 1.06, htBlur: 0, duration: 1.34, ease: 'sine.inOut' }, 2.75)
      .to(bgState, { centerX: 0.48, centerY: 0.52, palette: 0.5, duration: 1.60, ease: 'sine.inOut' }, 2.75)
      .to(bgState, { blend: 0.42, radius: 1.1, shapeScale: 0.38, lift: 0.07, wash: 0.2, warp: 0.27, duration: 0.60, ease: E_SOFT }, 4.21)
      .to(bgState, { blend: 0, radius: 1.0, shapeScale: 0.24, lift: 0.03, wash: 0.14, warp: 0.22, duration: 0.87, ease: E_HEAVY }, 4.78)
      .to(htView, { htScale: 1.12, htHue: -6, htSat: 1.18, htBlur: 2, duration: 0.60, ease: E_SOFT }, 4.21)
      .to(htView, { htScale: 1.06, htHue: 0, htSat: 1.08, htBlur: 0, duration: 0.87, ease: E_HEAVY }, 4.78)
      .to(htView, { htX: -15, duration: 1.67, ease: 'sine.inOut' }, 5.18)
      .to(bgState, { centerX: 0.51, centerY: 0.54, duration: 1.67, ease: 'sine.inOut' }, 5.18)
      .to(htView, { htX: 5, duration: 4.33, ease: 'sine.inOut' }, 6.93)
      .to(bgState, { centerX: 0.49, centerY: 0.52, palette: 0.48, duration: 4.33, ease: 'sine.inOut' }, 6.93)
      /* handoff into scene-00's resting state (index.html:2944-2962) */
      .to(htView, { htX: -120, htScale: 1.1, htHue: 4, htSat: 1.14, htBlur: 2, duration: 0.60, ease: E_SOFT }, 10.64)
      .to(bgState, { blend: 0.42, radius: 1.1, shapeScale: 0.38, lift: 0.07, wash: 0.2, duration: 0.60, ease: E_SOFT }, 10.64)
      .to(htView, { htX: INIT_BG.x, htY: INIT_BG.y, htScale: INIT_BG.scale, htHue: INIT_BG.hue, htSat: INIT_BG.sat, htBlur: 0, duration: 0.37, ease: E_HEAVY }, 11.24)
      .to(bgState, Object.assign({}, INIT_FIELD, { blend: 0, massShiftY: 0, duration: 0.37, ease: E_HEAVY }), 11.24)
      .set(htView, { htX: INIT_BG.x, htY: INIT_BG.y, htScale: INIT_BG.scale, htHue: INIT_BG.hue, htSat: INIT_BG.sat, htBlur: 0 }, INTRO_DUR)
      .set(bgState, Object.assign({}, INIT_FIELD, { massShiftY: 0 }), INTRO_DUR);
  });
  return <div id="il-introbg" class="absolute left-0 top-0 w-[0px] h-[0px]" />;
}

/* ════════════ green flash ════════════ */
function GreenFlash() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'green-flash', T < 1.33);
    ctx.timeline()
      .set('green-flash', { opacity: 1 }, 0)
      .to('green-flash', { opacity: 0, duration: 1.0, ease: E_SOFT }, 0);
  });
  return <div id="green-flash" class="absolute inset-0 z-[50] bg-[#142017] opacity-0" />;
}

/* ════════════ logo (19 svg paths; the two gradient strokes become the
   mid-color #2ADFAC — the RGB-split beat is emulated with tinted copies) */
var LOGO_PATHS = [
'M0 16.6738H4.96V23.3838H11.53V16.6738H16.49V35.4538H11.53V27.6738H4.96V35.4538H0V16.6738Z',
'M17.8899 28.0137C17.8899 23.4237 21.2399 20.0237 25.7999 20.0237C30.0099 20.0237 32.9399 23.0837 32.9399 27.5637C32.9399 28.2037 32.8599 28.8237 32.8299 29.3337H22.6099C23.0399 31.1337 24.6199 32.1237 27.1399 32.1237C28.7199 32.1237 30.2199 31.7237 31.2999 31.0237V34.6737C30.1699 35.3937 28.2999 35.8537 26.3599 35.8537C21.2599 35.8537 17.8799 32.6037 17.8799 28.0237L17.8899 28.0137ZM28.5699 26.1937C28.4899 24.5337 27.3899 23.5637 25.7499 23.5637C24.1099 23.5637 22.9599 24.5337 22.6099 26.1937H28.5699Z',
'M37.85 33.8638L32 20.4238H37.29L40.46 28.1538L43.6 20.4238H48.48L40.33 39.5738H35.26L37.86 33.8638H37.85Z',
'M48.1797 26.1138C48.1797 20.4238 52.7097 16.2438 58.9397 16.2438C61.2997 16.2438 63.5297 16.8038 64.8197 17.7238V21.9338C63.5597 20.9138 61.6797 20.2938 59.7497 20.2938C55.7797 20.2938 53.2597 22.6238 53.2597 26.1438C53.2597 29.6638 55.5697 31.8838 58.8897 31.8838C59.6397 31.8838 60.4997 31.7538 61.1997 31.5038V27.8838H57.4697V24.2338H65.5197V33.7038C63.6697 35.0938 61.1997 35.8738 58.5997 35.8738C52.5397 35.8738 48.1897 31.7438 48.1897 26.1138H48.1797Z',
'M66.6604 28.0137C66.6604 23.4237 70.0104 20.0237 74.5704 20.0237C78.7804 20.0237 81.7104 23.0837 81.7104 27.5637C81.7104 28.2037 81.6304 28.8237 81.6004 29.3337H71.3804C71.8104 31.1337 73.3904 32.1237 75.9104 32.1237C77.4904 32.1237 78.9904 31.7237 80.0704 31.0237V34.6737C78.9404 35.3937 77.0704 35.8537 75.1304 35.8537C70.0304 35.8537 66.6504 32.6037 66.6504 28.0237L66.6604 28.0137ZM77.3404 26.1937C77.2604 24.5337 76.1604 23.5637 74.5204 23.5637C72.8804 23.5637 71.7304 24.5337 71.3804 26.1937H77.3404Z',
'M82.9697 20.4238H87.8497V22.4138C88.7597 20.9638 90.3497 20.1038 92.3297 20.1038C95.4997 20.1038 97.3997 22.3338 97.3997 25.8138V35.4438H92.5197V26.8638C92.5197 25.0438 91.8197 24.1838 90.4797 24.1838C88.9497 24.1838 87.8497 25.3638 87.8497 27.2638V35.4438H82.9697V20.4238Z',
'M195.219 26.1937L213.529 38.9937C216.009 40.7237 220.239 38.7637 221.009 35.5337L228.419 4.33374C229.189 1.10374 225.879 -0.856262 222.589 0.873738L198.199 13.6737C192.649 16.5837 191.059 23.2837 195.219 26.1937Z',
'M256.97 25.9638L232.58 38.7638C229.28 40.4938 225.98 38.5338 226.75 35.3038L234.16 4.10376C234.93 0.873757 239.16 -1.08624 241.64 0.643757L259.95 13.4438C264.12 16.3538 262.52 23.0538 256.97 25.9638Z',
'M252.115 72.502C250.192 72.502 248.268 72.0838 246.93 71.3728V65.6853C247.808 66.3126 248.937 66.689 250.066 66.689C251.739 66.689 252.784 65.8526 252.784 64.5562C252.784 63.8452 252.492 63.0925 251.655 62.0888L249.899 59.956C248.728 58.4923 248.226 56.9031 248.226 55.2721C248.226 51.0065 251.864 48.0791 256.883 48.0791C258.723 48.0791 260.438 48.4555 261.65 49.0828V54.7703C260.898 54.2266 259.768 53.8503 258.723 53.8503C257.092 53.8503 256.046 54.6867 256.046 55.8994C256.046 56.6522 256.423 57.405 257.259 58.4086L258.974 60.4996C260.187 62.047 260.73 63.5943 260.73 65.2671C260.73 69.5746 257.05 72.502 252.115 72.502Z',
'M236.338 72.6269C228.392 72.6269 223.123 67.5667 223.123 60.4155C223.123 53.2643 228.351 47.9531 235.46 47.9531C242.026 47.9531 246.584 52.7206 246.584 59.7045C246.584 60.7082 246.459 61.6701 246.417 62.4647H230.483C231.152 65.2666 233.62 66.8139 237.551 66.8139C240.018 66.8139 242.36 66.1866 244.033 65.0993V70.7868C242.277 71.916 239.349 72.6269 236.338 72.6269ZM230.483 57.5717H239.767C239.642 54.9789 237.927 53.4734 235.376 53.4734C232.867 53.4734 231.027 54.9789 230.483 57.5717Z',
'M184.998 72.0001V48.5809H192.609V51.6756C193.989 49.4173 196.373 48.0791 199.426 48.0791C202.521 48.0791 204.779 49.5428 206.075 52.052C207.497 49.5846 210.132 48.0791 213.352 48.0791C218.245 48.0791 221.214 51.592 221.214 57.0286V72.0001H213.603V58.5341C213.603 55.774 212.557 54.4357 210.634 54.4357C208.459 54.4357 206.912 56.234 206.912 59.0778V72.0001H199.3V58.5341C199.3 55.774 198.255 54.4357 196.373 54.4357C194.157 54.4357 192.609 56.234 192.609 59.0778V72.0001H184.998Z',
'M167.26 72.4602C161.154 72.4602 156.596 67.2745 156.596 60.3324C156.596 53.3066 161.196 48.0791 167.302 48.0791C170.438 48.0791 173.031 49.4173 174.536 51.8429V48.5809H182.148V72.0001H174.536V68.4873C173.031 71.0801 170.438 72.4602 167.26 72.4602ZM169.393 66.1872C172.78 66.1872 174.536 63.3434 174.536 60.876V59.6632C174.536 57.2377 172.78 54.4357 169.393 54.4357C166.298 54.4357 164.207 56.7777 164.207 60.2905C164.207 63.7616 166.298 66.1872 169.393 66.1872Z',
'M141.258 72V48.5808H148.827V52.9719C149.831 50.0445 151.796 48.2881 154.306 48.2881C154.975 48.2881 155.602 48.4135 156.02 48.6645V55.8993C155.435 55.5648 154.724 55.4393 153.887 55.4393C150.751 55.4393 148.869 57.6558 148.869 61.2941V72H141.258Z',
'M122.643 71.9996V42.7256H139.371V49.124H130.379V54.477H138.911V60.5827H130.379V71.9996H122.643Z',
'M105.645 72V48.5808H113.214V52.9719C114.218 50.0445 116.183 48.2881 118.692 48.2881C119.361 48.2881 119.989 48.4135 120.407 48.6645V55.8993C119.822 55.5648 119.111 55.4393 118.274 55.4393C115.138 55.4393 113.256 57.6558 113.256 61.2941V72H105.645Z',
'M93.4397 72.6269C85.4939 72.6269 80.2246 67.5667 80.2246 60.4155C80.2246 53.2643 85.4521 47.9531 92.5615 47.9531C99.1273 47.9531 103.686 52.7206 103.686 59.7045C103.686 60.7082 103.56 61.6701 103.518 62.4647H87.5849C88.2541 65.2666 90.7214 66.8139 94.6525 66.8139C97.1199 66.8139 99.4618 66.1866 101.135 65.0993V70.7868C99.3782 71.916 96.4508 72.6269 93.4397 72.6269ZM87.5849 57.5717H96.869C96.7435 54.9789 95.0289 53.4734 92.4779 53.4734C89.9687 53.4734 88.1286 54.9789 87.5849 57.5717Z',
'M53.3711 78.4404V48.5809H60.9823V52.052C62.446 49.501 65.0807 48.0791 68.3008 48.0791C74.3647 48.0791 78.9649 53.3066 78.9649 60.2487C78.9649 67.2745 74.3229 72.4602 68.259 72.4602C65.0807 72.4602 62.446 71.1637 60.9823 68.78V78.4404H53.3711ZM66.1262 66.1872C69.2627 66.1872 71.3119 63.7616 71.3119 60.2905C71.3119 56.8195 69.2627 54.4357 66.1262 54.4357C62.7388 54.4357 60.9823 57.2795 60.9823 59.7051V60.9178C60.9823 63.3434 62.7388 66.1872 66.1262 66.1872Z',
'M31.9606 78.4405L36.0171 69.5329L26.9004 48.5811H35.1389L40.0737 60.6252L44.9666 48.5811H52.5779L39.8646 78.4405H31.9606Z',
'M0 71.9996V42.7256H7.7367V53.1806H17.9826V42.7256H25.7193V71.9996H17.9826V59.8718H7.7367V71.9996H0Z'
];
function LogoSvg({ id }) {
  return (
    <div id={id} class="absolute left-[347px] top-[356px] w-[1225px] h-[368px]">
      {LOGO_PATHS.map((d, i) => (
        <path key={i} d={d} view-box="0 0 263 79" fill={i === 6 || i === 7 ? '#2ADFAC' : '#FFFFFF'} />
      ))}
    </div>
  );
}
function Logo() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'logo-wrap', T >= 0.42 && T < 5.42);
    /* RGB-split beat (ref drop-shadow chromatic aberration 0.67→1.05s) is
       emulated with two tinted copies fading in/out */
    ctx.timeline()
      .set('logo-img', { scale: 0.05, opacity: 0, filter: 'blur(16px)' }, 0)
      .set('logo-cyan', { opacity: 0 }, 0)
      .set('logo-amber', { opacity: 0 }, 0)
      .to('logo-img', { opacity: 0.3, duration: 0.08, ease: 'none' }, 0.42)
      .to('logo-img', { scale: 1.0, opacity: 1, filter: 'blur(0px)', duration: 0.68, ease: E_HARD }, 0.42)
      .fromTo('logo-cyan', { opacity: 0 }, { opacity: 0.4, duration: 0.13, ease: 'none' }, 0.67)
      .fromTo('logo-amber', { opacity: 0 }, { opacity: 0.3, duration: 0.13, ease: 'none' }, 0.67)
      .to('logo-cyan', { opacity: 0, duration: 0.25, ease: E_SOFT }, 0.80)
      .to('logo-amber', { opacity: 0, duration: 0.25, ease: E_SOFT }, 0.80)
      .to('logo-img', { scale: 0.57, x: -435, duration: 1.33, ease: E_SOFT }, 0.92)
      .to('logo-img', { x: -1200, opacity: 0, filter: 'blur(12px)', duration: 0.83, ease: E_SOFT }, 3.75);
  });
  return (
    <div id="logo-wrap" class="absolute inset-0 z-[2] opacity-0">
      <div id="logo-cyan" class="absolute inset-0 opacity-0">
        <div id="logo-cyan-svg" class="absolute left-[350px] top-[356px] w-[1225px] h-[368px]">
          {LOGO_PATHS.map((d, i) => (<path key={i} d={d} view-box="0 0 263 79" fill="#19B7D5" />))}
        </div>
      </div>
      <div id="logo-amber" class="absolute inset-0 opacity-0">
        <div id="logo-amber-svg" class="absolute left-[344px] top-[356px] w-[1225px] h-[368px]">
          {LOGO_PATHS.map((d, i) => (<path key={i} d={d} view-box="0 0 263 79" fill="#D97757" />))}
        </div>
      </div>
      <LogoSvg id="logo-img" />
    </div>
  );
}

/* ════════════ inspector text + letters ════════════ */
var LETTERS = ['I', 'n', 's', 'p', 'e', 'c', 't', 'o', 'r'];
var LETTER_IDS = ['l-I', 'l-n', 'l-s', 'l-p', 'l-e', 'l-c', 'l-t', 'l-o', 'l-r'];
var LT_FACES = ['lt-a', 'lt-b', 'lt-c', 'lt-d', 'lt-e'];
var LT_CLASSES = ['font-[if9]', 'font-[ge7]', 'font-[jb4]', 'font-[ge4]', 'font-[jb4]'];
function InspectorText() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'it-wrap', T >= 1.42 && T < 11.33);
    var tl = ctx.timeline();
    tl.set('inspector-text', { opacity: 1, x: 435, y: 40, filter: 'blur(0px)' }, 2.08);
    LETTERS.forEach((letter, i) => {
      tl.fromTo(LETTER_IDS[i], { opacity: 0, x: 40, filter: 'blur(4px)' }, { opacity: 1, x: 0, filter: 'blur(0px)', duration: 0.83, ease: E_HEAVY }, 2.17 + i * 0.067);
    });
    /* fontSize 120→180 on letterT (ref) ≈ scale 1→1.5 about the baseline
       (origin 50% 80%); the row never reflows, so 'o r' keep their positions */
    tl.set('lt-zoom', { transformOrigin: '50% 80%' }, 0);
    tl.to('lt-zoom', { scale: 1.5, duration: 1.0, ease: E_HEAVY }, 4.43);
    /* color beats on the letter-t faces */
    tl.to(LT_FACES, { color: '#8ed8dc', duration: 0.16, ease: 'none' }, 7.78);
    tl.to(LT_FACES, { color: '#4cc8d9', duration: 0.16, ease: 'none' }, 7.94);
    tl.to(LT_FACES, { color: '#19b7d5', duration: 0.14, ease: 'none' }, 8.10);
    /* face swaps (ref fontFamily swaps): Inter→Fraunces→Plex→Georgia→Plex */
    tl.set('lt-a', { opacity: 0 }, 8.69); tl.set('lt-b', { opacity: 1 }, 8.69);
    tl.set('lt-b', { opacity: 0 }, 8.90); tl.set('lt-c', { opacity: 1 }, 8.90);
    tl.set('lt-c', { opacity: 0 }, 9.12); tl.set('lt-d', { opacity: 1 }, 9.12);
    tl.set('lt-d', { opacity: 0 }, 9.22); tl.set('lt-e', { opacity: 1 }, 9.22);
    /* size-field beats (ref scale on letterT; folded with the 1.5x wrapper) */
    tl.to('lt-t', { scale: 0.33, duration: 0.06, ease: 'none' }, 9.98);
    tl.to('lt-t', { scale: 0.4, duration: 0.12, ease: E_HEAVY }, 10.08);
    tl.to('lt-t', { scale: 0.25, duration: 0.06, ease: 'none' }, 10.38);
    tl.to('lt-t', { scale: 3.0, duration: 0.15, ease: E_HEAVY }, 10.58);
    tl.set('lt-t', { transformOrigin: '50% 50%' }, 0);
    tl.to('inspector-text', { x: -90, duration: 0.66, ease: E_SOFT }, 6.52);
    tl.to('inspector-text', { x: -2000, filter: 'blur(8px)', duration: 0.58, ease: E_SOFT }, 10.74);
  });
  return (
    <div id="it-wrap" class="absolute inset-0 z-[2] opacity-0">
      <div class="absolute inset-0 flex items-center justify-center">
        <div id="inspector-text" class="flex items-end opacity-0">
          {LETTERS.map((ch, i) => {
            if (i !== 6) {
              return <text key={i} id={LETTER_IDS[i]} class="text-[120px] font-[if9] text-[#fff8e9] tracking-[-4.2px] leading-none">{ch}</text>;
            }
            return (
              <div key={i} id="lt-zoom" class="relative">
                <div id="lt-t" class="relative">
                  <text id="lt-a" class={'text-[120px] tracking-[-4.2px] leading-none text-[#fff8e9] ' + LT_CLASSES[0]}>t</text>
                  <text id="lt-b" class="absolute left-0 top-0 text-[120px] tracking-[-4.2px] leading-none text-[#fff8e9] opacity-0 font-[ge7]">t</text>
                  <text id="lt-c" class="absolute left-0 top-0 text-[120px] tracking-[-4.2px] leading-none text-[#fff8e9] opacity-0 font-[jb4]">t</text>
                  <text id="lt-d" class="absolute left-0 top-0 text-[120px] tracking-[-4.2px] leading-none text-[#fff8e9] opacity-0 font-[ge4]">t</text>
                  <text id="lt-e" class="absolute left-0 top-0 text-[120px] tracking-[-4.2px] leading-none text-[#fff8e9] opacity-0 font-[jb4]">t</text>
                </div>
              </div>
            );
          })}
        </div>
      </div>
    </div>
  );
}

/* ════════════ magnifier sweep (buildMagKeyframes, px-converted) ═══════ */
function hash1(n) {
  var s = Math.sin(n * 127.1 + 311.7) * 43758.5453;
  return s - Math.floor(s);
}
function valueNoise1d(x, seed) {
  var i = Math.floor(x), f = x - i;
  var a = hash1(i + seed), b = hash1(i + 1 + seed);
  var u = 0.5 - 0.5 * Math.cos(f * Math.PI);
  return a * (1 - u) + b * u;
}
function fbm1d(x, seed) {
  var sum = 0, amp = 1, freq = 1, norm = 0, i;
  for (i = 0; i < 4; i += 1) {
    sum += valueNoise1d(x * freq, seed + i * 17) * amp;
    norm += amp;
    amp *= 0.5;
    freq *= 2;
  }
  return sum / norm;
}
var MAG_KF = (function () {
  var glassKF = [], cloneKF = [], i;
  for (i = 1; i <= 12; i += 1) {
    var p = i / 12, nt = p * 4.0;
    var jx = (fbm1d(nt, 0) * 2 - 1) * 4;
    var jy = (fbm1d(nt, 101) * 2 - 1) * 1.5;
    var gl = SWEEP_START + (SWEEP_END - SWEEP_START) * p + jx;
    var gt = SWEEP_TOP + jy;
    var gxPx = gl / 100 * 1920, gyPx = gt / 100 * 1080;
    var dur = 1.06 / 12;
    glassKF.push({ x: gl / 100 * 1920 - MAG_R, y: gt / 100 * 1080 - MAG_R, duration: dur, ease: 'none' });
    cloneKF.push({ x: MAG_R - (gxPx - TEXT_X) * MAG_SCALE, y: MAG_R - (gyPx - TEXT_Y) * MAG_SCALE, duration: dur, ease: 'none' });
  }
  return { glassKF: glassKF, cloneKF: cloneKF };
})();

function MagGlass() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'mag-wrap', T >= 2.75 && T < 4.35);
    var initX = SWEEP_START / 100 * 1920, initY = SWEEP_TOP / 100 * 1080;
    ctx.timeline()
      .set('mag-glass', { x: initX - MAG_R, y: initY - MAG_R, scale: 0, opacity: 1 }, 2.75)
      .set('mag-text-clone', { x: MAG_R - (initX - TEXT_X) * MAG_SCALE, y: MAG_R - (initY - TEXT_Y) * MAG_SCALE }, 2.75)
      .to('mag-glass', { scale: 1, duration: 0.40, ease: E_SOFT }, 2.75)
      .to('mag-glass', { keyframes: MAG_KF.glassKF }, 2.89)
      .to('mag-text-clone', { keyframes: MAG_KF.cloneKF }, 2.89)
      .to('mag-glass', { y: 1.2 * 1080 - MAG_R, scale: 0.3, opacity: 0, duration: 0.40, ease: 'power2.in' }, 3.95);
  });
  return (
    <div id="mag-wrap" class="absolute inset-0 z-[3] opacity-0">
      <div id="mag-glass" class="absolute left-0 top-0 w-[220px] h-[220px] rounded-full overflow-hidden opacity-0 bg-[radial-gradient(circle,rgba(69,216,110,0.18)_1.2px,transparent_1.2px),radial-gradient(circle,rgba(25,183,213,0.12)_1px,transparent_1px),radial-gradient(circle_at_30%_40%,rgba(69,216,110,0.06),transparent_60%),#080f0b] bg-[length:10px_10px,14px_14px,100%_100%,100%_100%]">
        <text id="mag-text-clone" class="absolute left-0 top-0 text-[288px] font-[if8] text-[#fff8e9] whitespace-nowrap">Inspector</text>
        <div id="mag-overlay" class="absolute inset-0 rounded-full bg-[radial-gradient(circle_at_32%_28%,rgba(255,255,255,0.22),transparent_45%),radial-gradient(circle_at_22%_18%,rgba(255,255,255,0.35),transparent_12%),radial-gradient(circle_at_center,transparent_65%,rgba(0,0,0,0.4)_100%)] shadow-[inset_0_0_0_3px_rgba(255,248,233,0.55),inset_0_0_0_5px_rgba(7,16,12,0.5),inset_0_0_1px_6px_rgba(4,8,6,0.3),0_16px_50px_rgba(0,0,0,0.5)]"></div>
        <div id="mag-ring" class="absolute left-[-1px] top-[-1px] right-[-1px] bottom-[-1px] rounded-full border-[4px] border-[rgba(255,248,233,0.6)]"></div>
        <div id="mag-handle" class="absolute left-[240px] top-[204px] w-[12px] h-[48px] bg-[rgba(255,248,233,0.55)] rounded-[6px] rotate-[-45deg] origin-top"></div>
      </div>
    </div>
  );
}

/* ════════════ selection highlight ════════════ */
function SelectionHighlight() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'sel-wrap', T >= 5.18 && T < 11.0);
    ctx.timeline()
      .set('selection-highlight', { x: 0.583 * 1920, y: 0.443 * 1080, width: 0, height: 0, opacity: 0 }, 5.85)
      .to('selection-highlight', { opacity: 1, width: 0.045 * 1920, height: 0.18 * 1080, duration: 0.42, ease: E_SOFT }, 5.90)
      .to('selection-highlight', { x: 0.583 * 1920 - 90, duration: 0.66, ease: E_SOFT }, 6.52)
      .to('selection-highlight', { opacity: 0, duration: 0.25, ease: 'none' }, 10.74);
  });
  return (
    <div id="sel-wrap" class="absolute inset-0 z-[2] opacity-0">
      <div id="selection-highlight" class="absolute left-0 top-0 bg-[rgba(25,183,213,0.28)] border border-[rgba(25,183,213,0.45)] rounded-[4px] opacity-0"></div>
    </div>
  );
}

/* ════════════ cursor ════════════ */
function Cursor() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'cursor-wrap', T >= 5.18 && T < 11.32);
    ctx.timeline()
      .set('cursor', { x: 0.08 * 1920, y: 0.88 * 1080, opacity: 0 }, 5.18)
      .to('cursor', { opacity: 1, duration: 0.08, ease: 'none' }, 5.18)
      .to('cursor', { x: 0.35 * 1920, y: 0.55 * 1080, duration: 0.25, ease: E_SOFT }, 5.22)
      .to('cursor', { x: 0.595 * 1920, y: 0.437 * 1080, duration: 0.33, ease: E_SOFT }, 5.47)
      .to('cursor', { x: 0.615 * 1920, y: 0.617 * 1080, duration: 0.42, ease: E_SOFT }, 5.90)
      .to('cursor', { x: 0.72 * 1920, y: 0.55 * 1080, duration: 0.54, ease: E_SOFT }, 6.65)
      .to('cursor', { x: 0.793 * 1920, y: 0.30 * 1080, duration: 0.26, ease: E_SOFT }, 7.32)
      .to('cursor', { x: 0.66 * 1920, y: 0.42 * 1080, duration: 0.20, ease: E_SOFT }, 7.58)
      .to('cursor', { x: 0.72 * 1920, y: 0.435 * 1080, duration: 0.46, ease: E_SOFT }, 7.78)
      .to('cursor', { x: 0.885 * 1920, y: 0.40 * 1080, duration: 0.26, ease: E_SOFT }, 8.32)
      .to('cursor', { x: 0.63 * 1920, y: 0.533 * 1080, duration: 0.20, ease: E_SOFT }, 8.61)
      .to('cursor', { x: 0.63 * 1920, y: 0.572 * 1080, duration: 0.14, ease: E_SOFT }, 8.82)
      .to('cursor', { x: 0.63 * 1920, y: 0.612 * 1080, duration: 0.14, ease: E_SOFT }, 9.04)
      .to('cursor', { x: 0.63 * 1920, y: 0.572 * 1080, duration: 0.14, ease: E_SOFT }, 9.17)
      .to('cursor', { x: 0.886 * 1920, y: 0.492 * 1080, duration: 0.25, ease: E_SOFT }, 9.44)
      .to('cursor', { x: -0.10 * 1920, duration: 0.50, ease: E_SOFT }, 10.74)
      .to('cursor', { opacity: 0, duration: 0.25, ease: 'none' }, 11.06);
  });
  return (
    <div id="cursor-wrap" class="absolute inset-0 z-[10] opacity-0">
      <div id="cursor" class="absolute left-0 top-0 opacity-0 drop-shadow-[0_2px_8px_rgba(0,0,0,0.6)]">
        <div id="cursor-svg" class="w-[32px] h-[38px]">
          <path d="M2 1L2 25.5L8.5 19.5L14.5 31L18.5 29L12.5 17.5L21 17.5L2 1Z" view-box="0 0 28 34" fill="#FFFFFF" stroke="#000000" stroke-width="1.5" />
        </div>
      </div>
    </div>
  );
}

/* ════════════ inspector panel ════════════ */
function PanelLabel({ children }) {
  return <text class="block text-[10px] font-[if7] tracking-[1px] text-[rgba(255,248,233,0.3)] mb-[10px]">{children}</text>;
}
function PanelField({ children, fl, id }) {
  return (
    <div id={id} class="flex-1 p-[8px_10px] bg-[rgba(255,248,233,0.04)] border border-[rgba(255,248,233,0.08)] rounded-[10px] text-[13px] text-[rgba(255,248,233,0.7)] font-[jb4] relative">
      <text class="text-[10px] text-[rgba(255,248,233,0.3)] mr-[4px]">{fl}</text>
      {children}
    </div>
  );
}
function InspectorPanel() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'panel-wrap', T >= 6.52 && T < 11.25);
    ctx.timeline()
      .set('inspector-panel', { right: -460 }, 0)
      .to('inspector-panel', { right: 0, duration: 0.80, ease: E_SOFT }, 6.52)
      .to('inspector-panel', { right: -460, duration: 0.50, ease: E_SOFT }, 10.74)
      .set('cl-1', { opacity: 1 }, 7.78)
      .set('cl-2', { opacity: 1 }, 7.94)
      .set('cl-3', { opacity: 1 }, 8.10)
      .to('color-swatch', { backgroundColor: '#8ed8dc', duration: 0.16, ease: 'none' }, 7.78)
      .to('color-swatch', { backgroundColor: '#4cc8d9', duration: 0.16, ease: 'none' }, 7.94)
      .to('color-swatch', { backgroundColor: '#19b7d5', duration: 0.14, ease: 'none' }, 8.10)
      .set('fd-fraunces', { opacity: 1 }, 8.69)
      .set('fd-plex', { opacity: 1 }, 8.90)
      .set('fd-georgia', { opacity: 1 }, 9.12)
      .set('fd-plex2', { opacity: 1 }, 9.22)
      .set('size-field-hl', { opacity: 1 }, 9.71)
      .set('size-field-hl', { opacity: 0 }, 10.74)
      .set('sv-sel', { opacity: 1 }, 9.78)
      .set('sv-sel', { opacity: 0 }, 9.91)
      .set('sv-caret', { opacity: 1 }, 9.91)
      .set('sv-caret', { opacity: 0 }, 9.98)
      .set('sv-4a', { opacity: 1 }, 9.98)
      .set('sv-4a', { opacity: 0 }, 10.08)
      .set('sv-48', { opacity: 1 }, 10.08)
      .set('sv-48', { opacity: 0 }, 10.31)
      .set('sv-caret', { opacity: 1 }, 10.31)
      .set('sv-caret', { opacity: 0 }, 10.38)
      .set('sv-3a', { opacity: 1 }, 10.38)
      .set('sv-3a', { opacity: 0 }, 10.48)
      .set('sv-36', { opacity: 1 }, 10.48)
      .set('sv-36', { opacity: 0 }, 10.58)
      .set('sv-360', { opacity: 1 }, 10.58);
  });
  return (
    <div id="panel-wrap" class="absolute inset-0 z-[5] opacity-0">
      <div id="inspector-panel" class="absolute right-[-460px] top-0 bottom-0 w-[440px] bg-[#111b15] border-l border-[rgba(255,248,233,0.08)] rounded-l-[24px] p-[32px_24px] text-[13px] text-[rgba(255,248,233,0.6)] font-[if4]">
        <div class="mb-[24px] pb-[18px] border-b border-[rgba(255,248,233,0.06)]">
          <PanelLabel>Document</PanelLabel>
          <text class="block text-[16px] font-[if7] text-[#fff8e9] mb-[4px]">Inspector</text>
          <text class="block text-[11px] text-[rgba(255,248,233,0.3)]">#text-layer</text>
        </div>
        <div class="mb-[24px] pb-[18px] border-b border-[rgba(255,248,233,0.06)]">
          <PanelLabel>Layout</PanelLabel>
          <div class="flex gap-[10px] mb-[8px]">
            <PanelField fl="X">{" 192px"}</PanelField>
            <PanelField fl="Y">{" 420px"}</PanelField>
          </div>
          <div class="flex gap-[10px] mb-[8px]">
            <PanelField fl="W">{" 1344px"}</PanelField>
            <PanelField fl="H">{" 156px"}</PanelField>
          </div>
        </div>
        <div class="mb-[24px] pb-[18px] border-b border-[rgba(255,248,233,0.06)]">
          <PanelLabel>Color</PanelLabel>
          <div class="flex items-center gap-[12px]">
            <div id="color-swatch" class="w-[36px] h-[36px] rounded-[10px] border border-[rgba(255,248,233,0.12)] bg-[#fff8e9]"></div>
            <div class="relative font-[jb4] text-[12px]">
              <text id="cl-0" class="text-[rgba(255,248,233,0.6)]">rgb(255,248,233)</text>
              <text id="cl-1" class="absolute left-0 top-0 text-[rgba(255,248,233,0.6)] opacity-0">rgb(142,216,220)</text>
              <text id="cl-2" class="absolute left-0 top-0 text-[rgba(255,248,233,0.6)] opacity-0">rgb(76,200,217)</text>
              <text id="cl-3" class="absolute left-0 top-0 text-[rgba(255,248,233,0.6)] opacity-0">rgb(25,183,213)</text>
            </div>
          </div>
        </div>
        <div class="mb-[24px] pb-[18px] border-b border-[rgba(255,248,233,0.06)]">
          <PanelLabel>Font</PanelLabel>
          <div class="p-[8px_12px] bg-[rgba(255,248,233,0.04)] border border-[rgba(255,248,233,0.08)] rounded-[10px] text-[13px] text-[rgba(255,248,233,0.7)] flex justify-between items-center">
            <div class="relative">
              <text id="fd-inter">Inter</text>
              <text id="fd-fraunces" class="absolute left-0 top-0 opacity-0">Fraunces</text>
              <text id="fd-plex" class="absolute left-0 top-0 opacity-0">IBM Plex Mono</text>
              <text id="fd-georgia" class="absolute left-0 top-0 opacity-0">Georgia</text>
              <text id="fd-plex2" class="absolute left-0 top-0 opacity-0">IBM Plex Mono</text>
            </div>
            <text class="text-[12px] text-[rgba(255,248,233,0.3)]">v</text>
          </div>
        </div>
        <div class="mb-[24px]">
          <PanelLabel>Size</PanelLabel>
          <div id="size-field" class="p-[8px_10px] bg-[rgba(255,248,233,0.04)] border border-[rgba(255,248,233,0.08)] rounded-[10px] text-[13px] text-[rgba(255,248,233,0.7)] font-[jb4] relative">
            <div id="size-field-hl" class="absolute inset-0 border border-[rgba(25,183,213,0.42)] rounded-[10px] opacity-0"></div>
            <text class="text-[10px] text-[rgba(255,248,233,0.3)] mr-[4px]">Size</text>
            <text id="sv-120">{" 120px"}</text>
            <text id="sv-sel" class="absolute right-[10px] top-[8px] bg-[rgba(25,183,213,0.3)] rounded-[3px] px-[2px] opacity-0">{" 120"}</text>
            <text id="sv-caret" class="absolute right-[10px] top-[6px] text-[#19b7d5] opacity-0">{" |"}</text>
            <text id="sv-3a" class="absolute right-[10px] top-[8px] opacity-0">{" 3|"}</text>
            <text id="sv-36" class="absolute right-[10px] top-[8px] opacity-0">{" 36|"}</text>
            <text id="sv-360" class="absolute right-[10px] top-[8px] opacity-0">{" 360px"}</text>
            <text id="sv-4a" class="absolute right-[10px] top-[8px] opacity-0">{" 4|"}</text>
            <text id="sv-48" class="absolute right-[10px] top-[8px] opacity-0">{" 48px"}</text>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ color picker ════════════ */
function ColorPicker() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'picker-wrap', T >= 7.32 && T < 8.38);
    ctx.timeline()
      .set('color-picker', { scale: 0.9, opacity: 0 }, 0)
      .to('color-picker', { opacity: 1, scale: 1, duration: 0.34, ease: E_SOFT }, 7.45)
      .to('picker-circle', { x: 51.5, y: 21.2, duration: 0.20, ease: E_SOFT }, 7.58)
      .to('picker-circle', { x: 182.5, y: 33.0, duration: 0.46, ease: E_SOFT }, 7.78)
      .set('hx-1', { opacity: 1 }, 7.78)
      .set('hx-2', { opacity: 1 }, 7.94)
      .set('hx-3', { opacity: 1 }, 8.10)
      .to('hue-thumb', { x: 172.5, duration: 0.46, ease: E_SOFT }, 7.78)
      .to('color-picker', { opacity: 0, scale: 0.9, duration: 0.20, ease: E_SOFT }, 8.18);
  });
  return (
    <div id="picker-wrap" class="absolute inset-0 z-[6] opacity-0">
      <div class="absolute right-[460px] top-0 bottom-0 flex items-center">
        <div id="color-picker" class="w-[280px] bg-[#0a0a0a] border border-[rgba(255,248,233,0.1)] rounded-[18px] p-[22px] opacity-0">
          <text class="block text-[15px] font-[if7] text-[#fff8e9] mb-[14px]">Stroke color</text>
          <div class="h-[120px] rounded-[10px] mb-[14px] border border-[rgba(255,248,233,0.08)] relative bg-[linear-gradient(to_bottom,transparent,black),linear-gradient(to_right,white,#19b7d5)]">
            <div id="picker-circle" class="absolute left-0 top-0 w-[16px] h-[16px] rounded-full border-2 border-white shadow-[0_0_4px_rgba(0,0,0,0.5)]"></div>
          </div>
          <div class="h-[14px] rounded-[7px] mb-[14px] relative bg-[linear-gradient(to_right,red,yellow,lime,cyan,blue,magenta,red)]">
            <div id="hue-thumb" class="absolute left-[114px] top-[-3px] w-[20px] h-[20px] rounded-full bg-[#19b7d5] border-2 border-white shadow-[0_0_4px_rgba(0,0,0,0.5)]"></div>
          </div>
          <text class="block text-[10px] tracking-[0.8px] text-[rgba(255,248,233,0.3)] mb-[6px]">HEX</text>
          <div class="p-[10px_12px] bg-[rgba(255,248,233,0.04)] border border-[rgba(255,248,233,0.08)] rounded-[10px] font-[jb4] text-[15px] text-[rgba(255,248,233,0.8)] relative">
            <text id="hx-0">#FFF8E9</text>
            <text id="hx-1" class="absolute left-[12px] top-[10px] opacity-0">#8ED8DC</text>
            <text id="hx-2" class="absolute left-[12px] top-[10px] opacity-0">#4CC8D9</text>
            <text id="hx-3" class="absolute left-[12px] top-[10px] opacity-0">#19B7D5</text>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ font list ════════════ */
var FONT_GREEN = 'rgba(69,216,110,0.08)', FONT_DEFAULT_BG = 'rgba(0,0,0,0)';
function FontList() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    vis(ctx, 'fontlist-wrap', T >= 8.38 && T < 9.48);
    ctx.timeline()
      .set('font-list', { scale: 0.95, opacity: 0 }, 0)
      .to('font-list', { opacity: 1, scale: 1, duration: 0.26, ease: E_SOFT }, 8.45)
      .set('fo-inter', { backgroundColor: FONT_GREEN, color: '#45d86e' }, 8.45)
      .set('fs-inter', { color: 'rgba(69,216,110,0.5)' }, 8.45)
      .set('fo-inter', { backgroundColor: FONT_DEFAULT_BG, color: 'rgba(255,248,233,0.5)' }, 8.69)
      .set('fs-inter', { color: 'rgba(255,248,233,0.2)' }, 8.69)
      .set('fo-fraunces', { backgroundColor: FONT_GREEN, color: '#45d86e' }, 8.69)
      .set('fs-fraunces', { color: 'rgba(69,216,110,0.5)' }, 8.69)
      .set('fo-fraunces', { backgroundColor: FONT_DEFAULT_BG, color: 'rgba(255,248,233,0.5)' }, 8.90)
      .set('fs-fraunces', { color: 'rgba(255,248,233,0.2)' }, 8.90)
      .set('fo-plex', { backgroundColor: FONT_GREEN, color: '#45d86e' }, 8.90)
      .set('fs-plex', { color: 'rgba(69,216,110,0.5)' }, 8.90)
      .set('fo-plex', { backgroundColor: FONT_DEFAULT_BG, color: 'rgba(255,248,233,0.5)' }, 9.12)
      .set('fs-plex', { color: 'rgba(255,248,233,0.2)' }, 9.12)
      .set('fo-georgia', { backgroundColor: FONT_GREEN, color: '#45d86e' }, 9.12)
      .set('fs-georgia', { color: 'rgba(69,216,110,0.5)' }, 9.12)
      .set('fo-georgia', { backgroundColor: FONT_DEFAULT_BG, color: 'rgba(255,248,233,0.5)' }, 9.22)
      .set('fs-georgia', { color: 'rgba(255,248,233,0.2)' }, 9.22)
      .set('fo-plex2', { backgroundColor: FONT_GREEN, color: '#45d86e' }, 9.22)
      .set('fs-plex2', { color: 'rgba(69,216,110,0.5)' }, 9.22)
      .to('font-list', { opacity: 0, scale: 0.95, duration: 0.20, ease: E_SOFT }, 9.28);
  });
  function Opt({ id, fsId, face, name, src }) {
    return (
      <div id={id} class="p-[12px_16px] text-[14px] text-[rgba(255,248,233,0.5)] border-b border-[rgba(255,248,233,0.04)] flex justify-between items-center">
        <text class={face}>{name}</text>
        <text id={fsId} class="text-[9px] tracking-[0.72px] text-[rgba(255,248,233,0.2)]">{src}</text>
      </div>
    );
  }
  return (
    <div id="fontlist-wrap" class="absolute inset-0 z-[6] opacity-0">
      <div class="absolute right-[460px] top-0 bottom-0 flex items-center">
        <div id="font-list" class="w-[320px] bg-[#0d1510] border border-[rgba(255,248,233,0.1)] rounded-[18px] overflow-hidden opacity-0">
          <text class="block p-[16px_16px_10px] text-[10px] font-[if7] tracking-[1px] text-[rgba(255,248,233,0.3)]">Font Family</text>
          <div class="mx-[16px] mb-[10px] p-[8px_12px] bg-[rgba(255,248,233,0.04)] border border-[rgba(255,248,233,0.08)] rounded-[10px]">
            <text class="text-[12px] text-[rgba(255,248,233,0.25)] font-[if4]">Search fonts</text>
          </div>
          <div class="flex gap-[6px] px-[16px] pb-[10px]">
            <div class="p-[4px_10px] rounded-[6px] text-[11px] text-[rgba(255,248,233,0.4)] bg-[rgba(255,248,233,0.04)] border border-[rgba(255,248,233,0.06)]"><text>Local</text></div>
            <div class="p-[4px_10px] rounded-[6px] text-[11px] text-[rgba(255,248,233,0.4)] bg-[rgba(255,248,233,0.04)] border border-[rgba(255,248,233,0.06)]"><text>Import</text></div>
          </div>
          <Opt id="fo-inter" fsId="fs-inter" face="font-[if4]" name="Inter" src="Current" />
          <Opt id="fo-fraunces" fsId="fs-fraunces" face="font-[dmi]" name="Fraunces" src="Google" />
          <Opt id="fo-plex" fsId="fs-plex" face="font-[jb4]" name="IBM Plex Mono" src="Google" />
          <Opt id="fo-georgia" fsId="fs-georgia" face="font-[ge4]" name="Georgia" src="Local" />
          <Opt id="fo-plex2" fsId="fs-plex2" face="font-[jb4]" name="IBM Plex Mono" src="Google" />
        </div>
      </div>
    </div>
  );
}

/* ════════════ shared marks / icons ════════════ */
function ClaudeMark({ id, size, classExtra }) {
  return (
    <div id={id} class={size + ' ' + (classExtra || '')}>
      <path d="M16 2 L17.4 12.6 L21.2 4.8 L18.7 13.3 L26.2 8.2 L19.5 14.8 L29.4 14 L19.6 16.6 L28 21.6 L18.8 17.8 L22.5 27.2 L16.8 18.4 L15.4 28.8 L14.7 18.2 L9.2 27.2 L13 17.8 L3.8 21.6 L12.3 16.6 L2.6 14 L12.4 14.8 L5.8 8.2 L13.3 13.3 L10.8 4.8 L14.6 12.6 Z" view-box="0 0 32 32" fill="#D97757" />
    </div>
  );
}

/* ════════════ scene 00: typed dark prompt ════════════ */
function Scene00() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= 10.74 && T < 12.46;
    vis(ctx, 'scene-00', ON);
    if (ON) {
      ctx.timeline()
        .fromTo('sc0-prompt', { x: 1700, opacity: 1, filter: 'blur(6px)' }, { x: 0, filter: 'blur(0px)', duration: 0.85, ease: 'expo.out' }, 10.78)
        .fromTo('sc0-actions', { opacity: 0 }, { opacity: 1, duration: 0.28, ease: 'power3.out' }, 10.94)
        .fromTo('sc0-send', { scale: 0.78 }, { scale: 1, duration: 0.2, ease: 'back.out(2.2)' }, 12.06);
      /* typewriter (ref tl.to proxy + onUpdate) — computed per frame */
      var fullText = 'we created HyperFrames because we believe';
      var p = clamp((T - 11.04) / 1.0, 0, 1);
      ctx.getNode('sc0-typed').text(fullText.slice(0, Math.floor(p * fullText.length)));
      /* caret: fromTo(op 1→0, 0.16, repeat 5, yoyo) from 10.86 */
      var tl6 = (T - 10.86) / 0.16, on = 1;
      if (tl6 >= 0 && tl6 < 6) { var k = Math.floor(tl6), f = tl6 - k; on = (k % 2 === 0) ? 1 - f : f; }
      ctx.getNode('sc0-caret').opacity(on);
    }
  });
  return (
    <div id="scene-00" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-00-stage" class="absolute inset-0 grid place-items-center p-[120px]">
        <div class="relative w-full h-full flex items-center justify-center">
          <div id="sc0-prompt" class="relative w-[1480px] min-h-[224px] p-[44px_48px_36px] border border-[rgba(255,248,233,0.18)] rounded-[44px] bg-[linear-gradient(180deg,rgba(22,28,22,0.98),rgba(9,15,12,0.98)),#10140f] shadow-[0_70px_200px_rgba(0,0,0,0.52),inset_0_1px_0_rgba(255,248,233,0.08)]">
            <div class="min-h-[72px] pr-[160px] text-[46px] font-[if6] leading-[1.18] tracking-[-0.83px] whitespace-nowrap overflow-hidden text-[#fff8e9]">
              <text id="sc0-typed"></text>
              <text id="sc0-caret" class="inline-block relative top-[7px] w-[7px] h-[44px] ml-[6px] bg-[#d97757]"></text>
            </div>
            <div id="sc0-actions" class="absolute right-[32px] bottom-[28px] flex items-center gap-[14px]">
              <div class="w-[60px] h-[60px] grid place-items-center text-[rgba(255,248,233,0.54)]">
                <div class="w-[26px] h-[26px]"><path d="M21 11.5l-8.5 8.5a5 5 0 1 1-7-7l8.5-8.5a3.5 3.5 0 1 1 5 5L11 17.5a2 2 0 1 1-3-3l7-7" view-box="0 0 24 24" fill="none" stroke="rgba(255,248,233,0.54)" stroke-width="1.6" /></div>
              </div>
              <div id="sc0-send" class="w-[60px] h-[60px] grid place-items-center bg-[#d97757] rounded-[16px]">
                <div class="w-[22px] h-[22px]"><path d="M12 5v14M5 12l7-7 7 7" view-box="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="2.2" /></div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ scene 01: opener with gradient "video" ════════════ */
function Scene01() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= 12.47 && T < 14.67;
    vis(ctx, 'scene-01', ON);
    if (ON) {
      ctx.timeline()
        .fromTo('sc1-w2-ov', { clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)', duration: 0.75, ease: 'power2.out' }, 13.07)
        .fromTo('sc1-h1', { x: 0 }, { x: 170, duration: 1.1, ease: 'power2.out' }, 13.37);
    }
  });
  return (
    <div id="scene-01" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-01-stage" class="absolute inset-0 grid place-items-center p-[120px]">
        <div class="relative w-full h-full flex items-center justify-center">
          <div id="sc1-h1" class="flex items-end gap-[26px] text-[116px] font-[if9] leading-none tracking-[-5.22px] text-[#fff8e9] whitespace-nowrap [text-shadow:2px_0_rgba(25,183,213,0.36),-2px_0_rgba(217,119,87,0.28),0_30px_90px_rgba(0,0,0,0.42)]">
            <text>Creating</text>
            <text class="relative inline-block">
              <text>video</text>
              <text id="sc1-w2-ov" class="absolute left-0 top-0">
                <text class="text-transparent bg-clip-text bg-[linear-gradient(90deg,#62e58c_0%,#3cd3ef_33%,#ffc858_66%,#62e58c_100%)] bg-[length:220%_100%]">video</text>
              </text>
            </text>
            <text>should be easy.</text>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ scene 01b: Agent→robot flip (lottie as divs) ════════════ */
var AG_ROBOT = null; /* robot keyframe helpers filled in hook via globalThis */
function Scene01b() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= 14.68 && T < 16.48;
    vis(ctx, 'scene-01b', ON);
    if (ON) {
      ctx.timeline()
        .set('ag-flip', { transformPerspective: 1600 }, 0)
        .fromTo('ag-inner', { rotationY: 0 }, { rotationY: 180, duration: 0.55, ease: 'power2.inOut' }, 15.32);
      /* drive faces + robot parts per frame (ref lottie @60fps, 241 frames,
         loop 4.0167s from 14.68) */
      var f = ((T - 14.68) * 60) % 241;
      var r = 0; /* rotationY mirror of the tween (recomputed below) */
      var p = clamp((T - 15.32) / 0.55, 0, 1);
      r = 180 * (p < 0.5 ? 2 * p * p : 1 - Math.pow(-2 * p + 2, 2) / 2); /* power2.inOut */
      ctx.getNode('ag-inner').rotationY(r);
      var front = ctx.getNode('ag-face'), back = ctx.getNode('ag-back');
      if (front && back) { front.opacity(r < 90 ? 1 : 0); back.opacity(r >= 90 ? 1 : 0); }
      /* N bob: frames 37-50 540→512, 95-108 512→591, 212-225 591→540 */
      var nx = 540;
      if (f >= 37 && f < 50) nx = 540 + (512 - 540) * (f - 37) / 13;
      else if (f >= 50 && f < 95) nx = 512;
      else if (f >= 95 && f < 108) nx = 512 + (591 - 512) * (f - 95) / 13;
      else if (f >= 108 && f < 154) nx = 591;
      else if (f >= 154 && f < 167) nx = 591 + (512 - 591) * (f - 154) / 13;
      else if (f >= 167 && f < 212) nx = 512;
      else if (f >= 212 && f <= 225) nx = 512 + (540 - 512) * (f - 212) / 13;
      else if (f > 225) nx = 540;
      var k = ctx.getNode('ag-robot');
      if (k) k.translateX((nx - 540) * 0.2222);
      /* eyes blink: h 89.6→11.6→89.6 over frames (22,26,33) etc */
      var eyes = [[22, 26, 33], [76, 80, 87], [128, 132, 139], [196, 200, 207]];
      var eh = 89.598, e;
      for (var ei = 0; ei < eyes.length; ei += 1) {
        e = eyes[ei];
        if (f >= e[0] && f < e[1]) eh = 89.598 + (11.598 - 89.598) * (f - e[0]) / (e[1] - e[0]);
        else if (f >= e[1] && f < e[2]) eh = 11.598 + (89.598 - 11.598) * (f - e[1]) / (e[2] - e[1]);
      }
      var e1 = ctx.getNode('ag-eye1'), e2 = ctx.getNode('ag-eye2');
      if (e1) e1.height(eh * 0.2222);
      if (e2) e2.height(eh * 0.2222);
      /* antenna ball sweep 663↔422 every 60f */
      var cyc = (f % 120) / 60;
      var sw = cyc < 1 ? cyc : 2 - cyc; /* 0..1..0 */
      var se = sw < 0.5 ? 2 * sw * sw : 1 - Math.pow(-2 * sw + 2, 2) / 2;
      var bx = 663 + (422 - 663) * se;
      var ball = ctx.getNode('ag-ball');
      if (ball) ball.translateX(((bx - 663) + (nx - 540)) * 0.2222);
    }
  });
  return (
    <div id="scene-01b" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-01b-stage" class="absolute inset-0 grid place-items-center p-[120px]">
        <div class="relative w-full h-full flex items-center justify-center">
          <div class="flex items-center text-[116px] font-[if9] leading-none tracking-[-5.22px] text-[#fff8e9] whitespace-nowrap [text-shadow:2px_0_rgba(25,183,213,0.36),-2px_0_rgba(217,119,87,0.28),0_30px_90px_rgba(0,0,0,0.42)]">
            <text>For You and Your</text>
            <div id="ag-flip" class="relative w-[360px] h-[150px] ml-[18px]">
              <div id="ag-inner" class="absolute inset-0">
                <div id="ag-face" class="absolute inset-0 grid place-items-center">
                  <text class="text-[116px] font-[if9] tracking-[-5.22px] text-[#fff8e9]">Agent</text>
                </div>
                <div id="ag-back" class="absolute inset-0 grid place-items-center opacity-0">
                  <div id="ag-robot" class="relative w-[240px] h-[240px]">
                    <div id="ag-body" class="absolute left-[64.6px] top-[91.8px] w-[110.7px] h-[85.8px] rounded-[42px] bg-[#FFFFFF]"></div>
                    <div id="ag-ball" class="absolute left-[136.3px] top-[154.9px] w-[23.6px] h-[23.6px] rounded-full bg-[#FFFFFF]"></div>
                    <div class="absolute left-[118px] top-[86px] w-[4.9px] h-[52px] bg-[#FFFFFF] rounded-full rotate-[10deg]"></div>
                    <div id="ag-head" class="absolute left-[74.2px] top-[92.4px] w-[91.1px] h-[63.1px] rounded-[31.5px] bg-[#0A0A0A]"></div>
                    <div id="ag-eye1" class="absolute left-[127.6px] top-[119.2px] w-[6.3px] h-[19.9px] rounded-[4.4px] bg-[#FFFFFF]"></div>
                    <div id="ag-eye2" class="absolute left-[106.7px] top-[119.2px] w-[6.3px] h-[19.9px] rounded-[4.4px] bg-[#FFFFFF]"></div>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ scene 01c: But / for / now ════════════ */
function Scene01c() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= 16.48 && T < 18.07;
    vis(ctx, 'scene-01c', ON);
    if (ON) {
      ctx.timeline()
        .fromTo('bfn-but', { y: -42, opacity: 0, filter: 'blur(10px)' }, { y: 0, opacity: 1, filter: 'blur(0px)', duration: 0.5, ease: 'expo.out' }, 16.53)
        .fromTo('bfn-for', { y: 28, opacity: 0, filter: 'blur(8px)' }, { y: 0, opacity: 1, filter: 'blur(0px)', duration: 0.46, ease: 'expo.out' }, 16.69)
        .fromTo('bfn-now', { scale: 0.86, opacity: 0.001, filter: 'blur(22px)', y: 40 }, { scale: 1, opacity: 1, filter: 'blur(0px)', y: 0, duration: 0.6, ease: 'expo.out' }, 16.85);
    }
  });
  return (
    <div id="scene-01c" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-01c-stage" class="absolute inset-0 grid place-items-center p-[120px]">
        <div class="relative w-full h-full flex items-center justify-center">
          <div class="absolute inset-0 p-[90px_220px] flex flex-col justify-between">
            <div id="bfn-but" class="text-[200px] font-[dmi] leading-[0.94] tracking-[-4px] text-[#f5c66a] [text-shadow:0_22px_60px_rgba(0,0,0,0.44)]">But</div>
            <div id="bfn-for" class="text-center text-[180px] font-[dmi] leading-[0.94] tracking-[-3.6px] text-[#d97757] [text-shadow:0_18px_50px_rgba(0,0,0,0.4)]">for</div>
            <div id="bfn-now" class="self-end text-[320px] font-[if9] leading-[0.86] tracking-[-16px] text-transparent bg-clip-text bg-[linear-gradient(105deg,#f7d27a_0%,#f1b64a_32%,#d97757_66%,#b8542a_100%)]">now</div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ scene 02: glass video player ════════════ */
function Scene02() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= 18.07 && T < 20.07;
    vis(ctx, 'scene-02', ON);
    if (ON) {
      ctx.timeline()
        .fromTo('glass-frame', { opacity: 0, filter: 'blur(10px)' }, { opacity: 1, filter: 'blur(0px)', duration: 0.42, ease: 'power3.out' }, 18.09)
        .fromTo(['vs-w1', 'vs-w2', 'vs-w3', 'vs-w4', 'vs-w5'], { x: 320, opacity: 0 }, { x: 0, opacity: 1, duration: 0.5, ease: 'power3.out', stagger: 0.16 }, 18.29)
        .set('vs-w5', { transformOrigin: '0% 100%' }, 0)
        .to('vs-w5', { rotation: -4.5, y: 10, duration: 0.36, ease: 'back.out(1.6)' }, 18.97)
        .fromTo('vt-progress', { width: 0 }, { width: 476.6, duration: 1.6, ease: 'none' }, 18.27)
        .fromTo('vt-handle', { x: 0 }, { x: 476.6, duration: 1.6, ease: 'none' }, 18.27);
    }
  });
  return (
    <div id="scene-02" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-02-stage" class="absolute inset-0">
        <div class="absolute inset-0 flex items-center justify-center">
          <div id="glass-frame" class="relative w-[1680px] h-[945px] rounded-[56px] border border-[rgba(255,255,255,0.16)] bg-[linear-gradient(160deg,rgba(255,255,255,0.075)_0%,rgba(255,255,255,0.025)_45%,rgba(255,255,255,0.010)_70%,rgba(255,255,255,0.055)_100%)] shadow-[0_30px_90px_rgba(0,0,0,0.42),inset_0_1px_0_rgba(255,255,255,0.22)] opacity-0 scale-[0.8]">
            <div class="absolute left-[50px] top-[72px] w-[1580px] h-[720px] rounded-[40px] bg-[#000000] overflow-hidden flex items-center px-[90px] shadow-[0_10px_28px_rgba(0,0,0,0.35),inset_0_0_80px_rgba(0,0,0,0.65)]">
              <div class="flex items-baseline gap-[23px] text-[104px] font-[if9] leading-none tracking-[-4.68px] text-[#fff8e9] whitespace-nowrap">
                <text id="vs-w1" class="inline-block opacity-0">Sometimes</text>
                <text id="vs-w2" class="inline-block opacity-0">AI</text>
                <text id="vs-w3" class="inline-block opacity-0">gets</text>
                <text id="vs-w4" class="inline-block opacity-0">you</text>
                <text id="vs-w5" class="inline-block opacity-0">close</text>
              </div>
            </div>
            <div class="absolute left-[50px] bottom-[72px] w-[1580px] h-[50px] flex items-center gap-[22px] px-[14px] text-[rgba(255,248,233,0.78)]">
              <div class="w-[28px] h-[28px]"><path d="M8 5 L23 14 L8 23 Z" view-box="0 0 28 28" fill="rgba(255,248,233,0.86)" /></div>
              <text class="w-[56px] text-center text-[18px] font-[jb4] tracking-[0.72px]">0:00</text>
              <div class="flex-1 relative h-[4px] rounded-full bg-[rgba(255,248,233,0.18)]">
                <div id="vt-progress" class="absolute left-0 top-0 h-full w-0 bg-[#fff8e9] rounded-full"></div>
                <div id="vt-handle" class="absolute left-0 top-1/2 mt-[-7px] ml-[-7px] w-[14px] h-[14px] rounded-full bg-[#fff8e9] shadow-[0_0_14px_rgba(255,248,233,0.5)]"></div>
              </div>
              <text class="w-[56px] text-center text-[18px] font-[jb4] tracking-[0.72px]">0:42</text>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ scene 03: but close ≠ ship → ship ════════════ */
function Scene03() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= 20.07 && T < 21.77;
    vis(ctx, 'scene-03', ON);
    if (ON) {
      ctx.timeline()
        .fromTo('sc3-h1', { scale: 0.82, opacity: 0.1, filter: 'blur(20px)' }, { scale: 1, opacity: 1, filter: 'blur(0px)', duration: 0.32, ease: 'expo.out' }, 20.45)
        .fromTo(['ne-top', 'ne-bottom', 'ne-slash'], { strokeDashoffset: 100 }, { strokeDashoffset: 0, duration: 0.20, ease: 'power3.inOut', stagger: 0.06 }, 20.69)
        .to(['sl-0', 'sl-1', 'sl-2', 'sl-3'], { scale: 0, opacity: 0, width: 0, rotation: -28, duration: 0.22, stagger: 0.04, ease: 'power3.in' }, 21.05)
        .to('ship-final', { width: 149.6, opacity: 1, scale: 1, rotation: 0, duration: 0.36, ease: 'back.out(2)' }, 21.17);
    }
  });
  return (
    <div id="scene-03" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-03-stage" class="absolute inset-0 grid place-items-center p-[120px]">
        <div class="relative w-full h-full flex items-center justify-center">
          <div id="sc3-h1" class="flex items-center text-[136px] font-[ge7] leading-[0.92] tracking-[-1px] text-[#fff8e9] whitespace-nowrap [text-shadow:2px_0_rgba(25,183,213,0.36),-2px_0_rgba(217,119,87,0.28),0_30px_90px_rgba(0,0,0,0.42)]">
            <text>but close</text>
            <text class="inline-block mx-[8px] w-[106px] h-[106px]">
              <div class="w-[106px] h-[106px]">
                <path id="ne-top" d="M 16 40 L 84 40" view-box="0 0 100 100" fill="none" stroke="#fff8e9" stroke-width="14" stroke-dasharray="100" stroke-dashoffset="100" />
                <path id="ne-bottom" d="M 16 62 L 84 62" view-box="0 0 100 100" fill="none" stroke="#fff8e9" stroke-width="14" stroke-dasharray="100" stroke-dashoffset="100" />
                <path id="ne-slash" d="M 28 82 L 72 18" view-box="0 0 100 100" fill="none" stroke="#fff8e9" stroke-width="14" stroke-dasharray="100" stroke-dashoffset="100" />
              </div>
            </text>
            <text id="sl-0" class="inline-block">s</text>
            <text id="sl-1" class="inline-block">h</text>
            <text id="sl-2" class="inline-block">i</text>
            <text id="sl-3" class="inline-block">p</text>
            <text id="ship-final" class="inline-block w-0 opacity-0 overflow-hidden">
              <div class="w-[150px] h-[110px]">
                <path d="M6 46 L94 46 L80 68 L20 68 Z" view-box="0 0 100 74" fill="#3B6EA5" />
                <path d="M46 4 L46 42 L18 42 Z" view-box="0 0 100 74" fill="#E8EDF2" />
                <path d="M52 10 L52 42 L82 42 Z" view-box="0 0 100 74" fill="#C9D6E3" />
                <path d="M45 2 h4 v44 h-4 Z" fill="#8A6D4B" />
              </div>
            </text>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ Claude shell shared chrome (scenes 04/05) ════════════ */
function ClaudeTopbar({ pfx }) {
  return (
    <div class="h-[76px] px-[28px] flex items-center justify-between border-b border-[rgba(255,248,233,0.12)] bg-[rgba(7,12,9,0.72)]">
      <div class="flex items-center gap-[14px]">
        <div class="w-[38px] h-[38px] grid place-items-center">
          <div class="w-[22px] h-[22px]"><path d="M3 6h18M3 12h18M3 18h18" view-box="0 0 24 24" fill="none" stroke="rgba(255,248,233,0.58)" stroke-width="1.8" /></div>
        </div>
        <div class="w-[36px] h-[36px] grid place-items-center">
          <ClaudeMark id={pfx + 'mark'} size="w-[30px] h-[30px]" />
        </div>
        <text class="text-[22px] font-[if6] text-[#fff8e9]">HyperFrames Launch</text>
      </div>
      <div class="flex items-center gap-[14px]">
        <div class="flex items-center gap-[8px] p-[9px_16px] border border-[rgba(255,248,233,0.14)] rounded-full bg-[rgba(255,248,233,0.07)]">
          <text class="text-[18px] font-[if5] text-[rgba(255,248,233,0.68)]">Claude Opus 4.7</text>
          <div class="w-[11px] h-[11px]"><path d="M3 5l3 3 3-3" view-box="0 0 12 12" fill="none" stroke="rgba(255,248,233,0.68)" stroke-width="1.6" /></div>
        </div>
        <div class="w-[38px] h-[38px] grid place-items-center rounded-full bg-[#d97757]">
          <text class="text-[17px] font-[if7] text-white">J</text>
        </div>
      </div>
    </div>
  );
}
function ClaudeComposer({ pfx, textId, initial }) {
  return (
    <div class="absolute left-[64px] right-[64px] bottom-[36px] min-h-[100px] p-[22px_24px_18px] border border-[rgba(255,248,233,0.18)] rounded-[22px] bg-[linear-gradient(180deg,rgba(22,28,22,0.98),rgba(9,15,12,0.98)),#10140f] shadow-[0_24px_66px_rgba(0,0,0,0.3),inset_0_1px_0_rgba(255,248,233,0.06)]">
      <div class="min-h-[34px] pr-[130px] text-[24px] font-[if6] whitespace-nowrap overflow-hidden text-[#fff8e9]">
        <text id={textId}>{initial}</text>
      </div>
      <div class="absolute right-[18px] bottom-[14px] flex items-center gap-[12px]">
        <div class="w-[44px] h-[44px] grid place-items-center">
          <div class="w-[20px] h-[20px]"><path d="M21 11.5l-8.5 8.5a5 5 0 1 1-7-7l8.5-8.5a3.5 3.5 0 1 1 5 5L11 17.5a2 2 0 1 1-3-3l7-7" view-box="0 0 24 24" fill="none" stroke="rgba(255,248,233,0.54)" stroke-width="1.6" /></div>
        </div>
        <div id={pfx + 'send'} class="w-[44px] h-[44px] grid place-items-center bg-[#d97757] rounded-[12px]">
          <div class="w-[18px] h-[18px]"><path d="M12 5v14M5 12l7-7 7 7" view-box="0 0 24 24" fill="none" stroke="#FFFFFF" stroke-width="2.2" /></div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ scene 04: faithful Claude shell ════════════ */
function Scene04() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= 21.77 && T < 24.17;
    vis(ctx, 'scene-04', ON);
    if (ON) {
      ctx.timeline()
        .fromTo('cl-app', { y: 70, opacity: 0.001, filter: 'blur(10px)' }, { y: 0, opacity: 1, filter: 'blur(0px)', duration: 0.5, ease: 'expo.out' }, 21.79)
        .fromTo('cl-comp-text', { clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)', duration: 0.5, ease: 'steps(18)' }, 22.01)
        .fromTo('cl-send', { scale: 0.72 }, { scale: 1, duration: 0.18, ease: 'back.out(2)' }, 22.43)
        .fromTo('cl-bubble', { y: 36, opacity: 0 }, { y: 0, opacity: 1, duration: 0.32, ease: 'power4.out' }, 22.57)
        .fromTo('cl-reply', { y: 28, opacity: 0 }, { y: 0, opacity: 1, duration: 0.38, ease: 'power3.out' }, 23.01);
    }
  });
  return (
    <div id="scene-04" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-04-stage" class="absolute inset-0 grid place-items-center p-[120px]">
        <div class="relative w-full h-full flex items-center justify-center">
          <div id="cl-app" class="relative w-[1280px] h-[760px] overflow-hidden border border-[rgba(255,248,233,0.18)] rounded-[38px] bg-[linear-gradient(180deg,rgba(22,28,22,0.98),rgba(9,15,12,0.98)),#10140f] shadow-[0_70px_200px_rgba(0,0,0,0.52),inset_0_1px_0_rgba(255,248,233,0.08)]">
            <ClaudeTopbar pfx="cl-" />
            <div class="absolute left-[64px] right-[64px] top-[110px] bottom-[158px] overflow-hidden">
              <div class="flex justify-end">
                <div id="cl-bubble" class="max-w-[720px] p-[22px_28px] border border-[rgba(255,248,233,0.14)] rounded-[24px] bg-[rgba(255,248,233,0.08)] shadow-[0_18px_42px_rgba(0,0,0,0.26)] opacity-0">
                  <text class="text-[30px] font-[if6] leading-[1.25] text-[#fff8e9]">Can we try a different font?</text>
                </div>
              </div>
              <div id="cl-reply" class="flex items-start gap-[18px] mt-[32px] max-w-[880px] opacity-0">
                <div class="shrink-0 w-[44px] h-[44px] grid place-items-center border border-[rgba(255,248,233,0.14)] rounded-[12px] bg-[rgba(255,248,233,0.06)]">
                  <ClaudeMark id="cl-rmark" size="w-[22px] h-[22px]" />
                </div>
                <text class="text-[28px] leading-[1.4] text-[rgba(255,248,233,0.9)]">I'll update the title treatment and re-render the preview.</text>
              </div>
            </div>
            <ClaudeComposer pfx="cl-" textId="cl-comp-text" initial="Can we try a different font?" />
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ scene 05: firehose retry ════════════ */
var S5_PROMPTS = [
  'Make this faster.', 'Try a warmer color.', 'Move left 20px.',
  'Less bounce.', 'Slow the curve 20%.', 'Try another font.',
  'Caption: Ship it.', 'One frame later.', 'Actually right.',
  'Bigger title.', 'Cyan accent.', 'Trim 4 frames.',
  'Snap to grid.', 'Less blur.', 'Hold longer.',
  'Quieter exit.', 'Re-time.', 'Sync the cut.',
  'Brighter dots.', 'Just a hair more.', 'Tighter.',
  'Nudge right.', 'Still off.', 'One more frame.',
  '0.05s later.', 'Half a beat.', 'Push it.',
  'Closer.', 'Almost.', 'Try again.',
  'Once more.', 'Just so.'
];
var S5_START = 24.17;
var S5_ITEMS = (function () {
  var items = [], t = S5_START + 0.08, i;
  function gapFor(i2) { if (i2 < 3) return 0.30; if (i2 < 9) return 0.14; return 0.046; }
  for (i = 0; i < S5_PROMPTS.length; i += 1) {
    items.push({ t: t, popDur: i >= 9 ? 0.05 : (i < 3 ? 0.20 : 0.14) });
    t += gapFor(i);
  }
  return items;
})();
function Scene05() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    var ON = T >= S5_START && T < 27.17;
    vis(ctx, 'scene-05', ON);
    if (ON) {
      var tl = ctx.timeline();
      tl.set('s5-scroll', { y: 0, filter: 'blur(0px)' }, S5_START);
      S5_PROMPTS.forEach((p, i) => {
        tl.set('fx-' + i, { opacity: 0, y: 30 }, S5_START);
        tl.to('fx-' + i, { opacity: 1, y: 0, duration: S5_ITEMS[i].popDur, ease: 'power2.out' }, S5_ITEMS[i].t + 0.02);
        if (i >= 2) {
          tl.to('s5-scroll', { y: -(i - 1) * 100, duration: S5_ITEMS[i].popDur, ease: i >= 9 ? 'none' : 'power2.out' }, S5_ITEMS[i].t + 0.04);
        }
        if (i < 3) {
          tl.to('s5-send', { scale: 0.9, duration: 0.04, ease: 'power2.out' }, S5_ITEMS[i].t + 0.04);
          tl.to('s5-send', { scale: 1, duration: 0.08, ease: 'back.out(2)' }, S5_ITEMS[i].t + 0.08);
        }
      });
      tl.to('s5-scroll', { filter: 'blur(1.8px)', duration: 0.5, ease: 'power2.in' }, S5_START + 1.4);
      tl.to('s5-scroll', { filter: 'blur(0px)', duration: 0.22, ease: 'power3.out' }, S5_START + 2.65);
      /* composer flash per prompt — computed per frame (ref tl.call) */
      var compText = '', i;
      for (i = 0; i < S5_ITEMS.length; i += 1) {
        if (T >= S5_ITEMS[i].t && T < S5_ITEMS[i].t + 0.05) compText = S5_PROMPTS[i];
      }
      if (T >= S5_START + 2.65) compText = 'Keep nudging until it feels right...';
      ctx.getNode('s5-composer').text(compText);
    }
  });
  return (
    <div id="scene-05" class="absolute inset-0 z-[1] opacity-0">
      <div id="scene-05-stage" class="absolute inset-0 grid place-items-center p-[120px]">
        <div class="relative w-full h-full flex items-center justify-center">
          <div class="relative w-[1280px] h-[760px] overflow-hidden border border-[rgba(255,248,233,0.18)] rounded-[38px] bg-[linear-gradient(180deg,rgba(22,28,22,0.98),rgba(9,15,12,0.98)),#10140f] shadow-[0_70px_200px_rgba(0,0,0,0.52),inset_0_1px_0_rgba(255,248,233,0.08)]">
            <ClaudeTopbar pfx="s5-" />
            <div class="absolute left-[64px] right-[64px] top-[110px] bottom-[158px] overflow-hidden">
              <div id="s5-scroll" class="absolute left-0 right-0 top-[16px] flex flex-col gap-[18px]">
                {S5_PROMPTS.map((p, i) => (
                  <div key={i} id={'fx-' + i} class="flex flex-col gap-[10px] opacity-0">
                    <div class="flex justify-end">
                      <div class="max-w-[720px] p-[18px_24px] border border-[rgba(255,248,233,0.14)] rounded-[20px] bg-[rgba(255,248,233,0.08)]">
                        <text class="text-[26px] font-[if5] leading-[1.3] text-[#fff8e9]">{p}</text>
                      </div>
                    </div>
                    <div class="flex items-start gap-[14px]">
                      <div class="shrink-0 w-[38px] h-[38px] grid place-items-center border border-[rgba(255,248,233,0.14)] rounded-[10px] bg-[rgba(255,248,233,0.06)]">
                        <ClaudeMark id={'s5m-' + i} size="w-[20px] h-[20px]" />
                      </div>
                      <text class="pt-[4px] text-[26px] font-[if5] text-[rgba(255,248,233,0.9)]">Done.</text>
                    </div>
                  </div>
                ))}
              </div>
            </div>
            <ClaudeComposer pfx="s5-" textId="s5-composer" initial="" />
          </div>
        </div>
      </div>
    </div>
  );
}

/* ═════════════ entry — z-stacked composition (caller-owned root id) ═════════════ */

export default function InspectorLaunch() {
  return (
    <div id="root" class="relative w-[1920px] h-[1080px] overflow-hidden bg-[#0B0E14] font-[if4]">
      <Halftone />
      <BgSystem />
      <IntroBg />
      <Scene00 />
      <Scene01 />
      <Scene01b />
      <Scene01c />
      <Scene02 />
      <Scene03 />
      <Scene04 />
      <Scene05 />
      <Logo />
      <InspectorText />
      <SelectionHighlight />
      <MagGlass />
      <InspectorPanel />
      <ColorPicker />
      <FontList />
      <Cursor />
      <GreenFlash />
    </div>
  );
}

/* @@TAIL@@ */
