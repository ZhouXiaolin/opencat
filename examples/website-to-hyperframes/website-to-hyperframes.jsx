// JSX frontend port of examples/website-to-hyperframes/website-to-hyperframes.xml.
// Reference: /Users/solaren/Projects/hyperframes-launches/website-to-hyperframes — index.html master
// timeline (act windows + 11 video clips + captions) with the Act 1 / Act 4
// composition bodies from compositions/act-1-cold-open.html and
// compositions/act-4-end-card.html. One stage; the three act containers stack
// z-[1] < z-[2] < z-[3] and hard-cut at 12.0 / 33.8 (the reference's section
// replacement), so no transition operator.
//
// Per-act useGSAP hooks keep hook order (tree order) = XML script order:
// entry prologue → Act1 → Act3 → Act4. Act 3's 11 clips / corner captions /
// bottom captions come from the A3 data table. The master `.cap` bottom
// captions replay the reference's `cap-pop` CSS keyframes (0.26s,
// cubic-bezier(0.34,1.56,0.64,1)) as 0.182s + 0.078s tween pairs.
//
// Engine completion shipped with this port: the JSX/JSONL path had no way to
// express the markup `<video data-start / data-duration / data-color-matrix>`
// media window + YUV→RGB matrix (JsonLine::Video carried source+style only,
// timing always Default). JsonLine::Video now takes optional
// dataStart/dataDuration/dataMediaStart/dataColorMatrix/loop and
// jsx_runtime passes the same-named props through; absent fields keep the
// legacy behaviour, so no existing render changes.
//
// Audio is intentionally not ported: markup `<audio>` (soundtrack) has no
// per-clip start/volume model in the engine either, the reference master's
// audio sits outside the pixel-diff scope (k3diff compares frames), and the
// existing XML port ships without it. Rendering parity notes folded into
// comments at their sites.

import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 41.8 };

export const fonts = {
  default: 'inter',
  faces: [
    { id: 'inter', family: 'Inter', path: 'assets/fonts/Inter-500v.ttf', role: 'sans' },
    { id: 'inter400', family: 'Inter', path: 'assets/fonts/Inter-400.ttf' },
    { id: 'inter800', family: 'Inter', path: 'assets/fonts/Inter-800.ttf' },
    { id: 'inter900', family: 'Inter', path: 'assets/fonts/Inter-900.ttf' },
    { id: 'mono', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Regular.ttf', role: 'mono' },
    { id: 'mono500', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-500.ttf' },
    { id: 'bungee', family: 'Bungee Shade', path: 'assets/fonts/BungeeShade-Regular.ttf' },
  ],
};

/* ═══════════ shared data ═══════════ */

// Typing prompts — steps() counts derive from the strings themselves, exactly
// like the reference (`steps(target.length)` / `steps(round(n * k))`).
var CTA_CMD = 'npx skills add heygen-com/hyperframes';
var P1 = 'make a 30s promo for https://stripe.com';
var P2 = 'need a cinematic brand video from https://framer.com';
var P3 = 'build me an instagram ad from https://vibecode.app';
var P4 = 'launch video for https://heygen.com';

/* Act 3 rows: [video id, cap id, bottom-cap id, file, start, dur, light,
                corner num, corner brand, bottom caption]  */
var A3 = [
  ['v01', 'c01', 'm01', 'a3-01-stripe.mp4',          12.0, 1.500, 0, '01 · Product Launch', 'stripe.com',      'Product launches'],
  ['v02', 'c02', 'm02', 'a3-02-tailwindcss.mp4',     13.5, 1.917, 0, '02 · Feature Showcase', 'tailwindcss.com', 'Feature showcases'],
  ['v03', 'c03', 'm03', 'a3-03-framer.mp4',          15.4, 2.000, 0, '03 · Brand Reel',     'framer.com',      'Brand reels'],
  ['v04', 'c04', 'm04', 'a3-04-spotify.mp4',         17.4, 1.800, 0, '04 · Music & Audio',  'spotify.com',     'Music apps'],
  ['v05', 'c05', 'm05', 'a3-05-heygen.mp4',          19.2, 2.000, 1, '05 · 3D & Motion',    'heygen.com',      '3D scenes'],
  ['v06', 'c06', 'm06', 'a3-06-vercel.mp4',          21.2, 2.000, 0, '06 · Infrastructure', 'vercel.com',      'Infrastructure'],
  ['v07', 'c07', 'm07', 'a3-07-vibecode.mp4',        23.2, 2.000, 0, '07 · Product Demo',   'vibecode.app',    'Product demos'],
  ['v08', 'c08', 'm08', 'a3-08-linear.mp4',          25.2, 2.000, 0, '08 · Brand Opener',   'linear.app',      'Brand openers'],
  ['v09', 'c09', 'm09', 'a3-09-dribbble.mp4',        27.2, 3.000, 1, '09 · Logo Reveal',    'dribbble.com',    'Logo reveals'],
  ['v10', 'c10', 'm10', 'a3-10-tailwindcss-logo.mp4', 30.2, 1.608, 0, '10 · Drawn-in SVG',  'tailwindcss.com', 'Drawn in SVG'],
  ['v11', 'c11', 'm11', 'a3-11-github.mp4',          31.8, 2.000, 0, '11 · Logo Animation', 'github.com',      'Typography'],
];
/* Bottom `.cap` VO windows from the reference master (act3 span). */
var A3_MCAP = [
  [12.20, 1.25], [13.70, 1.35], [15.60, 1.02], [17.60, 0.98],
  [19.40, 1.35], [21.40, 1.16], [23.40, 1.21], [25.40, 1.16],
  [27.40, 1.30], [30.25, 1.50], [31.80, 0.80],
];

/* cap-pop keyframes, once per caption: 0% {0, y18, s0.82} → 70% {1, y-2,
   s1.04} @0.182 → 100% {1, y0, s1.00} @0.26, cubic-bezier per segment. */
var CAP_EASE = [0.34, 1.56, 0.64, 1];
function popCaption(T, id, at, dur) {
  T.set(id, { opacity: 0, y: 18, scale: 0.82 }, at);
  T.to(id, { opacity: 1, y: -2, scale: 1.04, duration: 0.182, ease: CAP_EASE }, at);
  T.to(id, { y: 0, scale: 1.0, duration: 0.078, ease: CAP_EASE }, at + 0.182);
  // The reference `.cap` is a timed clip: it vanishes when its window ends.
  T.set(id, { opacity: 0 }, at + dur);
}

/* ═══════════ structural templates → components ═══════════ */

// Terminal title bar shared by the four Act-1 panels (act-1 html .term-chrome).
function TermChrome({ ids, logo, label }) {
  return (
    <div id={ids.bar} class="flex items-center gap-[14px] px-[22px] h-[54px] bg-[#15161a] border-b-[1px] border-[rgba(255,255,255,0.06)]">
      <div id={ids.r} class="w-[13px] h-[13px] rounded-full bg-[#ff5f57]"></div>
      <div id={ids.y} class="w-[13px] h-[13px] rounded-full bg-[#febc2e]"></div>
      <div id={ids.g} class="w-[13px] h-[13px] rounded-full bg-[#28c840]"></div>
      <image id={ids.logo} class="w-[28px] h-[28px] rounded-[5px]" path={logo} />
      <text id={ids.label} class="font-[mono] font-medium text-[18px] text-[#8a8f98] whitespace-nowrap">{label}</text>
    </div>
  );
}

// Act-3 top-left corner caption (.a3-cap); light variant on clips 05 / 09.
function A3Cap({ id, nid, bid, num, brand, light }) {
  var boxCls = light
    ? 'absolute left-[56px] top-[54px] z-[20] border-l-[1px] border-[rgba(11,11,15,0.55)] bg-[rgba(255,255,255,0.78)] rounded-[4px] px-[18px] py-[12px] pl-[22px] shadow-[0_4px_14px_rgba(11,11,15,0.12)] backdrop-blur-[10px] leading-[1.15] opacity-0'
    : 'absolute left-[56px] top-[54px] z-[20] border-l-[1px] border-[rgba(255,255,255,0.65)] bg-[rgba(11,11,15,0.28)] rounded-[4px] px-[18px] py-[12px] pl-[22px] backdrop-blur-[10px] leading-[1.15] opacity-0 [text-shadow:0_1px_2px_rgba(0,0,0,0.35)]';
  var numCls = light
    ? 'font-[mono] font-normal text-[17px] tracking-[0.18em] uppercase text-[rgba(11,11,15,0.6)] mb-[8px]'
    : 'font-[mono] font-normal text-[17px] tracking-[0.18em] uppercase text-[rgba(255,255,255,0.7)] mb-[8px]';
  var brandCls = light
    ? 'font-[inter400] font-normal text-[42px] tracking-[-0.01em] text-[rgba(11,11,15,0.96)]'
    : 'font-[inter400] font-normal text-[42px] tracking-[-0.01em] text-[rgba(255,255,255,0.96)]';
  return (
    <div id={id} class={boxCls}>
      <text id={nid} class={numCls}>{num}</text>
      <text id={bid} class={brandCls}>{brand}</text>
    </div>
  );
}

// Master bottom-centred VO caption (.cap): dark default, light and cyan
// accent variants. The reference anchors `.cap` with `bottom: 88px` and its
// pill box is 92px tall (60px text at normal line-height + 10px padding).
// This port anchors from the top instead (pill top = 1080 - 88 - 92 = 900)
// and widens the vertical padding to 16px so the 60px leading-none line sits
// centred in the same 92px box: text centre lands at 900 + 16 + 30 = 946,
// matching the reference frame measurements exactly.
function BottomCap({ id, tid, t, accent, light }) {
  var pillCls = light
    ? 'px-[26px] py-[16px] rounded-[14px] bg-[rgba(255,255,255,0.82)] shadow-[0_6px_18px_rgba(11,11,15,0.14)]'
    : 'px-[26px] py-[16px] rounded-[14px] bg-[rgba(11,11,15,0.42)] backdrop-blur-[12px] shadow-[0_6px_18px_rgba(0,0,0,0.28)]';
  var textCls = 'font-[inter800] font-extrabold text-[60px] leading-none tracking-[-0.025em] whitespace-nowrap '
    + (light
      ? 'text-[#0b0b0f]'
      : '[text-shadow:0_2px_14px_rgba(0,0,0,0.55),0_0_2px_rgba(0,0,0,0.35)] ' + (accent ? 'text-[#00C3FF]' : 'text-[#ffffff]'));
  return (
    <div id={id} class="absolute left-0 top-[900px] w-[1920px] flex justify-center opacity-0 z-[25]">
      <div id={tid + 'p'} class={pillCls}>
        <text id={tid} class={textCls}>{t}</text>
      </div>
    </div>
  );
}

// Mouse cursor as the reference's SVG arrow (rect placeholder in the XML port).
function MouseCursor({ id, left, top, w, ht, z }) {
  return (
    <path
      id={id} view-box="0 0 28 40"
      d="M5 5 L5 30 L12 24 L16 32 L19 30 L15 22 L23 22 Z"
      class={'absolute left-[' + left + 'px] top-[' + top + 'px] w-[' + w + 'px] h-[' + ht + 'px] opacity-0 z-[' + z + '] fill-[#0b0b0f] stroke-[#f7f8f8] stroke-[1.5]'}
    />
  );
}

/* ═════════════ ACT 1 — cold open (abs 0.0–12.0) ═════════════ */

function Act1() {
  useGSAP((ctx) => {
    var T = ctx.timeline();

    // Phase 1: CTA caret blink + steps() typing (act-1 html:751-763)
    T.to('cta-caret', { opacity: 0, duration: 0.15 }, 0.20);
    T.to('cta-caret', { opacity: 1, duration: 0.15 }, 0.40);
    T.fromTo('cta-cmd', { clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)', duration: 0.90, ease: 'steps(' + CTA_CMD.length + ')' }, 0.45);
    T.to('cta-caret', { opacity: 0, duration: 0.15 }, 1.38);
    T.to('cta-caret', { opacity: 1, duration: 0.15 }, 1.53);

    // 1.65: send the CTA
    T.to('cta', { y: -50, scale: 1.02, opacity: 0, duration: 0.28, ease: 'power2.out' }, 1.65);
    T.to('morph', { scale: 0.98, duration: 0.12, ease: 'power2.in' }, 1.65);
    T.to('morph', { scale: 1.0, duration: 0.10, ease: 'power2.out' }, 1.77);

    // main cursor entrance
    T.fromTo('mc-main', { opacity: 0, x: 220, y: 140 }, { opacity: 1, x: 40, y: 30, duration: 0.55, ease: 'power2.out' }, 0.80);
    T.to('mc-main', { x: 0, y: 0, duration: 0.45, ease: 'power2.inOut' }, 1.25);
    T.to('mc-main', { scale: 0.9, duration: 0.08, ease: 'power2.in' }, 1.65);
    T.to('mc-main', { scale: 1, opacity: 0, duration: 0.12 }, 1.74);

    // Phase 2: morph to terminal. The reference `.morph-box` is centred
    // (left/top 50% + translate(-50%,-50%)) and resizes about its centre, so
    // left/top (100,450 → 260,150) move in lockstep with width/height
    // (1720x180 → 1400x780) keeping the centre pinned at (960,540).
    T.fromTo('morph', { width: 1720, height: 180, left: 100, top: 450 }, { width: 1400, height: 780, left: 260, top: 150, duration: 0.45, ease: 'power3.inOut' }, 1.75);
    T.to('term-content', { opacity: 1, duration: 0.30, ease: 'power2.out' }, 1.98);

    // Phase 3: panel typing. steps() counts are the reference formulas
    // (act-1 html:783-818): full length, or round(n * 0.45/0.5/0.55).
    T.fromTo('p1', { clipPath: 'inset(0 100% 0 0)' }, { clipPath: 'inset(0 0% 0 0)', duration: 1.05, ease: 'steps(' + P1.length + ')' }, 2.15);
    T.to('c1', { opacity: 1, duration: 0.12 }, 3.20);

    T.fromTo('t-cursor', { opacity: 0, x: 400, y: -80, scale: 0.92, rotation: -8 }, { opacity: 1, x: -55, y: -28, scale: 0.98, rotation: -4, duration: 0.55, ease: 'power3.out' }, 3.30);
    T.to('cu-hint', { opacity: 0, duration: 0.2 }, 3.45);
    T.set('p2', { clipPath: 'inset(0 55% 0 0)' }, 3.30);
    T.to('p2', { clipPath: 'inset(0 0% 0 0)', duration: 0.90, ease: 'steps(' + Math.round(P2.length * 0.45) + ')' }, 3.40);
    T.to('c2', { opacity: 1, duration: 0.12 }, 4.30);

    T.fromTo('t-codex', { opacity: 0, x: -400, y: 90, scale: 0.92, rotation: 10 }, { opacity: 1, x: 50, y: 32, scale: 0.97, rotation: 5, duration: 0.55, ease: 'power3.out' }, 4.80);
    T.set('p3', { clipPath: 'inset(0 60% 0 0)' }, 4.80);
    T.to('p3', { clipPath: 'inset(0 0% 0 0)', duration: 0.80, ease: 'steps(' + Math.round(P3.length * 0.5) + ')' }, 4.90);
    T.to('c3', { opacity: 1, duration: 0.12 }, 5.70);

    T.fromTo('t-gemini', { opacity: 0, x: 60, y: -220, scale: 0.92, rotation: -7 }, { opacity: 1, x: -28, y: 48, scale: 0.96, rotation: -3, duration: 0.55, ease: 'power3.out' }, 6.40);
    T.set('p4', { clipPath: 'inset(0 50% 0 0)' }, 6.40);
    T.to('ge-ph', { opacity: 0, duration: 0.18 }, 6.40);
    T.to('p4', { clipPath: 'inset(0 0% 0 0)', duration: 0.80, ease: 'steps(' + Math.round(P4.length * 0.55) + ')' }, 6.50);
    T.to('c4', { opacity: 1, duration: 0.12 }, 7.30);

    // Single blink per caret before the scatter
    var blinks = [[3.20, 'c1'], [4.30, 'c2'], [5.70, 'c3'], [7.30, 'c4']];
    for (var i = 0; blinks.length > i; i++) {
      T.to(blinks[i][1], { opacity: 0, duration: 0.2 }, blinks[i][0] + 0.45);
      T.to(blinks[i][1], { opacity: 1, duration: 0.2 }, blinks[i][0] + 0.65);
    }

    // Phase 4: charge (shake only)
    var TERMS = ['morph', 't-cursor', 't-codex', 't-gemini'];
    T.to(TERMS, { x: '+=3', yoyo: true, repeat: 5, duration: 0.045, ease: 'none' }, 8.00);

    // Phase 5: scatter
    T.to('morph',    { x: -480, y: -270, rotation: 0, scale: 0.52, duration: 0.20, ease: 'power3.out' }, 8.15);
    T.to('t-cursor', { x:  480, y: -270, rotation: 0, scale: 0.52, duration: 0.20, ease: 'power3.out' }, 8.15);
    T.to('t-codex',  { x: -480, y:  270, rotation: 0, scale: 0.52, duration: 0.20, ease: 'power3.out' }, 8.15);
    T.to('t-gemini', { x:  480, y:  270, rotation: 0, scale: 0.52, duration: 0.20, ease: 'power3.out' }, 8.15);
    T.to(TERMS, { scale: 0.55, duration: 0.14, ease: 'power2.out' }, 8.35);
    T.to(TERMS, { scale: 0.52, duration: 0.20, ease: 'elastic.out(1, 0.7)' }, 8.49);

    // Phase 6: phrase
    T.to(TERMS, { opacity: 0.12, duration: 0.4, ease: 'power2.out' }, 8.36);
    T.to('pw1', { opacity: 1, filter: 'blur(0px)', duration: 0.32, ease: 'power2.out' }, 8.45);
    T.to('pw2', { opacity: 1, filter: 'blur(0px)', duration: 0.35, ease: 'power2.out' }, 9.00);
    T.to('pw3', { opacity: 1, filter: 'blur(0px)', duration: 0.4, ease: 'power2.out' }, 9.85);

    // Phase 7: YEAHH. The reference `.yeahh { transform: rotate(-3deg) }` is a
    // static inner rotation that composes with the wrap tween (-8 → -3); set
    // it once on the text child so both apply.
    T.set('yeahh-t', { rotation: -3 }, 0);
    T.to(['pw1', 'pw2', 'pw3'], { opacity: 0, y: -8, duration: 0.22, ease: 'power2.in' }, 10.40);
    T.to(TERMS, { opacity: 1, duration: 0.18, ease: 'power2.out' }, 10.40);
    T.fromTo('yeahh', { opacity: 0, scale: 0, rotation: -8 }, { opacity: 1, scale: 1.18, rotation: -3, duration: 0.18, ease: 'back.out(2.5)' }, 10.55);
    // sparkle stars around the wordmark (act-1 html:851-853 — absent from the
    // XML port); positions approximate the reference's percentage offsets from
    // its shrink-to-fit wrap (centre 960,540).
    var STARS = ['st1', 'st2', 'st3', 'st4'];
    T.fromTo(STARS, { opacity: 0, scale: 0 }, { opacity: 1, scale: 1, duration: 0.2, stagger: { each: 0.04 }, ease: 'back.out(1.8)' }, 10.55);
    T.to('yeahh', { scale: 1.0, duration: 0.1, ease: 'power2.out' }, 10.73);
    T.to('yeahh', { opacity: 0, scale: 0.3, duration: 0.18, ease: 'power2.in' }, 10.95);
    T.to(STARS, { opacity: 0, duration: 0.14 }, 10.95);

    // Phase 8: sending
    T.to(['mc-tl', 'mc-bl'], { opacity: 1, duration: 0.18, ease: 'power2.out', stagger: { each: 0.03 } }, 11.05);
    T.to(['mc-tl', 'mc-bl'], { scale: 0.92, duration: 0.08, ease: 'power2.in' }, 11.35);
    T.to(['mc-tl', 'mc-bl'], { scale: 1, duration: 0.12, ease: 'power2.out' }, 11.43);
    T.to(['p1', 'p2', 'p3', 'p4'], { y: -40, scale: 1.03, opacity: 0, duration: 0.4, ease: 'power2.out' }, 11.35);
    T.to(['c1', 'c2', 'c3', 'c4'], { opacity: 0, duration: 0.1 }, 11.35);

    // Send affordances punch (act-1 html:893-897): send button / pill / prompt.
    var AFFORD = ['d013', 'd023', 'd033', 'x024'];
    T.to(AFFORD, { scale: 1.2, duration: 0.10, ease: 'power2.out' }, 11.35);
    T.to(AFFORD, { scale: 1.0, duration: 0.15, ease: 'power2.in' }, 11.46);
    // The reference also pulses each input area's boxShadow
    // (html:886-890, 0.15s in / 0.30s out inset ring) — the GSAP facade does
    // not interpolate box-shadow; skipped as a transient sub-frame nuance.

    T.fromTo('cr-tl', { scale: 0, opacity: 0 }, { scale: 1.3, opacity: 0.45, duration: 0.22, ease: 'power2.out' }, 11.35);
    T.fromTo('cr-bl', { scale: 0, opacity: 0 }, { scale: 1.3, opacity: 0.45, duration: 0.22, ease: 'power2.out' }, 11.38);
    T.to(['cr-tl', 'cr-bl'], { scale: 2.0, opacity: 0, duration: 0.30, ease: 'power2.out' }, 11.52);

    var bubbles = ['sm-claude', 'sm-cursor', 'sm-codex', 'sm-gemini'];
    for (var b = 0; bubbles.length > b; b++) {
      T.fromTo(bubbles[b], { opacity: 0, y: 14, scale: 0.94 }, { opacity: 1, y: 0, scale: 1, duration: 0.28, ease: 'back.out(1.4)' }, 11.55 + b * 0.03);
    }
    T.to(['mc-tl', 'mc-bl'], { opacity: 0, duration: 0.18, ease: 'power2.in' }, 11.70);
    T.to(bubbles, { opacity: 0, duration: 0.15, ease: 'power2.in' }, 11.90);

    // Master bottom captions (index.html data-start 1.66/3.30/4.80/6.40)
    var MC = [['mc1', 1.66, 1.30], ['mc2', 3.30, 1.02], ['mc3', 4.80, 0.98], ['mc4', 6.40, 1.02]];
    for (var k = 0; MC.length > k; k++) popCaption(T, MC[k][0], MC[k][1], MC[k][2]);

    // Phase 9: flash
    T.to('flash', { opacity: 1, duration: 0.05, ease: 'none' }, 11.92);
    T.to('flash', { opacity: 0, duration: 0.18, ease: 'power2.out' }, 11.98);
  });

  return (
    <div id="act1" class="absolute inset-0 z-[1] overflow-hidden bg-[#f5f3ee]">

      {/* act-1 html .canvas: `radial-gradient(...), #f5f3ee` — the engine's
          bg-[...] parser takes gradient layers but not a trailing bare colour,
          so the base colour rides on the container and the vignette on a child
          (same paint order as the reference's two-layer background). */}
      <div id="canvas" class="absolute inset-0 bg-[#f5f3ee]">
        <div id="canvas-vignette" class="absolute inset-0 bg-[radial-gradient(1400px_800px_at_50%_50%,rgba(0,0,0,0.025),transparent_65%)]"></div>
      </div>

      {/* act-1 html .grain — two dotted overlays, 4px / 6px tiles (absent from
          the XML port) */}
      <div id="grain-1" class="absolute inset-0 opacity-50 bg-[radial-gradient(circle_at_30%_40%,rgba(0,0,0,0.015)_1px,transparent_1px)] bg-[length:4px_4px]"></div>
      <div id="grain-2" class="absolute inset-0 opacity-50 bg-[radial-gradient(circle_at_70%_70%,rgba(0,0,0,0.015)_1px,transparent_1px)] bg-[length:6px_6px]"></div>

      {/* ═══ MORPH BOX: CTA → Claude Code ═══ */}
      <div id="morph" class="absolute left-[100px] top-[450px] w-[1720px] h-[180px] bg-[#1a1b1f] rounded-[20px] overflow-hidden shadow-[0_32px_70px_rgba(11,11,15,0.28),0_12px_22px_rgba(11,11,15,0.14)]">
        <div id="cta" class="absolute inset-0 flex items-center pl-[56px]">
          <text id="x001" class="font-[mono] font-normal text-[44px] tracking-[-0.005em] mr-[4px] text-[#c86d3f]">{"$ "}</text>
          <text id="cta-cmd" class="font-[mono] font-normal text-[44px] tracking-[-0.005em] text-[#f7f8f8]">{CTA_CMD}</text>
          <div id="cta-caret" class="w-[4px] h-[44px] bg-[#f7f8f8] ml-[2px]"></div>
        </div>

        <div id="term-content" class="absolute inset-0 opacity-0">
          <TermChrome ids={{ bar: 'd001', r: 'd002', y: 'd003', g: 'd004', logo: 'claude-logo', label: 'x002' }} logo="assets/w2h-logos/claude.png" label="Claude Code · Opus 4.7" />
          <div id="d005" class="absolute left-[24px] right-[24px] bottom-[28px] px-[16px] pt-[14px] pb-[12px] bg-[rgba(11,11,15,0.4)] border-[1px] border-[rgba(200,109,63,0.55)] rounded-[12px]">
            <div id="d006" class="flex items-center px-[6px] pt-[6px] pb-[14px] border-b-[1px] border-[rgba(255,255,255,0.06)]">
              <text id="p1" class="font-[mono] font-normal text-[26px] text-[#f0f0f0] whitespace-nowrap">{P1}</text>
              <div id="c1" class="w-[11px] h-[28px] bg-[#f0f0f0] ml-[2px] opacity-0"></div>
            </div>
            <div id="d007" class="flex justify-between items-center pt-[10px]">
              <div id="d008" class="flex gap-[8px]">
                <div id="d009" class="w-[26px] h-[26px] rounded-[6px] bg-[rgba(255,255,255,0.05)] flex items-center justify-center"><text id="x003" class="font-[mono] text-[14px] text-[#8a8f98]">+</text></div>
                <div id="d010" class="w-[26px] h-[26px] rounded-[6px] bg-[rgba(255,255,255,0.05)] flex items-center justify-center"><text id="x004" class="font-[mono] text-[14px] text-[#8a8f98]">/</text></div>
                <div id="d011" class="w-[26px] h-[26px] rounded-full border-[1.5px] border-[rgba(200,109,63,0.7)]"></div>
              </div>
              <div id="d012" class="flex items-center gap-[14px]">
                <text id="x005" class="font-[mono] text-[12px] text-[#8a8f98]">Ask before edits</text>
                <div id="d013" class="w-[32px] h-[32px] bg-[#c86d3f] rounded-[6px] flex items-center justify-center"><text id="x006" class="font-[mono] text-[14px] text-[#ffffff]">↑</text></div>
              </div>
            </div>
          </div>
        </div>
      </div>

      {/* ═══ Cursor pointer (CTA entry) — reference SVG arrow shape ═══ */}
      <MouseCursor id="mc-main" left="1120px" top="560px" w="28px" ht="40px" z="30" />

      {/* ═══ PANEL 2: Cursor ═══ */}
      <div id="t-cursor" class="absolute left-[260px] top-[150px] w-[1400px] h-[780px] bg-[#1a1b1f] rounded-[22px] overflow-hidden shadow-[0_32px_70px_rgba(11,11,15,0.32),0_12px_22px_rgba(11,11,15,0.16)] opacity-0">
        <TermChrome ids={{ bar: 'd014', r: 'd015', y: 'd016', g: 'd017', logo: 'cursor-logo', label: 'x007' }} logo="assets/w2h-logos/cursor.png" label="Cursor · Agent" />
        <div id="d018" class="absolute inset-x-0 top-[54px] bottom-0 px-[36px] pt-[32px]">
          <div id="d019" class="flex justify-between items-center mb-[18px]"><text id="x008" class="font-[inter400] text-[14px] text-[#c7c7c9]">New Chat</text><text id="x009" class="font-[inter400] text-[14px] text-[#8a8f98]">＋　···</text></div>
          <div id="d020" class="relative border-[1.5px] border-[rgba(120,120,140,0.45)] rounded-[14px] px-[18px] pt-[18px] pb-[14px] bg-[rgba(11,11,15,0.22)] h-[214px] mb-[12px]">
            <text id="cu-hint" class="absolute left-[22px] top-[20px] font-[inter400] text-[15px] text-[#6e7077]">Plan, @ for context, / for commands</text>
            <div id="d021" class="absolute left-[22px] top-[20px] flex items-center overflow-hidden">
              <text id="p2" class="font-[mono] font-normal text-[26px] text-[#f0f0f0] whitespace-nowrap">{P2}</text>
              <div id="c2" class="w-[11px] h-[28px] bg-[#f0f0f0] ml-[2px] opacity-0"></div>
            </div>
            <div id="d022" class="absolute left-[18px] right-[18px] bottom-[14px] flex items-center gap-[14px]">
              <div id="d023" class="flex items-center gap-[6px] pl-[10px] pr-[12px] py-[5px] border-[1px] border-[#3a3b3f] rounded-full"><text id="x010" class="font-[inter400] text-[13px] text-[#f0f0f0]">Agent ⌄</text></div>
              <text id="x011" class="font-[inter400] text-[13px] text-[#c7c7c9]">Opus 4.7 ⌄</text>
              {/* .cu-icon-set (ml-auto): two 14px glyphs + 26px mic circle */}
              <div id="d0-cuicons" class="flex items-center gap-[12px] ml-auto">
                <div id="d0-cui1" class="w-[14px] h-[14px]"></div>
                <div id="d0-cui2" class="w-[14px] h-[14px]"></div>
                <div id="d0-cumic" class="w-[26px] h-[26px] rounded-full bg-[#28292e]"></div>
              </div>
            </div>
          </div>
          {/* .cu-local pill below the input card (absent from the XML port);
              engine has no inline-flex display, so pin a fixed-size flex box
              at the flow position (card bottom + 12px margin) */}
          <div id="d0-culocal" class="absolute left-[36px] top-[349px] w-[78px] h-[26px] flex items-center pl-[8px] pr-[10px] py-[4px] border-[1px] border-[#3a3b3f] rounded-full"><text id="x0culocal" class="font-[inter400] text-[13px] text-[#c7c7c9] whitespace-nowrap">Local ⌄</text></div>
        </div>
      </div>

      {/* ═══ PANEL 3: Codex ═══ */}
      <div id="t-codex" class="absolute left-[260px] top-[150px] w-[1400px] h-[780px] bg-[#1a1b1f] rounded-[22px] overflow-hidden shadow-[0_32px_70px_rgba(11,11,15,0.32),0_12px_22px_rgba(11,11,15,0.16)] opacity-0">
        <TermChrome ids={{ bar: 'd024', r: 'd025', y: 'd026', g: 'd027', logo: 'codex-logo', label: 'x012' }} logo="assets/w2h-logos/codex.png" label="Codex · GPT-5.4" />
        <div id="d028" class="absolute inset-x-0 top-[54px] bottom-0 px-[36px] pt-[32px]">
          <div id="d029" class="flex items-center gap-[12px]"><text id="x013" class="font-[inter400] font-medium text-[14px] text-[#f7f8f8]">New thread</text><div id="x014" class="px-[12px] py-[4px] bg-[#7a6aff] rounded-full"><text id="x014t" class="font-[inter400] text-[11px] text-[#f0f0f0]">✦ Get Plus</text></div></div>
          <div id="d030" class="absolute left-[600px] top-[180px] w-[200px] text-center">
            <image id="codex-big" class="w-[44px] h-[44px] mb-[14px] mx-auto" path="assets/w2h-logos/codex-large.png" />
            <text id="x015" class="font-[inter400] text-[28px] font-medium text-[#f0f0f0]">Let's build</text>
            <text id="x016" class="font-[inter400] text-[15px] text-[#8a8f98] mt-[4px]">New project ⌄</text>
          </div>
          <div id="d031" class="absolute left-[24px] right-[24px] bottom-[60px] flex items-center gap-[14px] px-[14px] py-[12px] bg-[#15161a] border-[1px] border-[#2e2e32] rounded-[12px]">
            <text id="x017" class="font-[inter400] text-[18px] text-[#8a8f98]">＋</text>
            <div id="d032" class="flex-1 flex items-center overflow-hidden">
              <text id="p3" class="font-[mono] font-normal text-[26px] text-[#f0f0f0] whitespace-nowrap">{P3}</text>
              <div id="c3" class="w-[11px] h-[28px] bg-[#f0f0f0] ml-[2px] opacity-0"></div>
            </div>
            <div id="d033" class="w-[26px] h-[26px] bg-[#2e2e32] rounded-full flex items-center justify-center"><text id="x018" class="font-[inter400] text-[12px] text-[#f0f0f0]">↑</text></div>
          </div>
          <div id="d034" class="absolute left-[24px] bottom-[22px] flex gap-[8px]">
            <div id="d035" class="px-[10px] py-[4px] border-[1px] border-[#2e2e32] rounded-full"><text id="x019" class="font-[inter400] text-[11px] text-[#c7c7c9]">GPT-5.4 ⌄</text></div>
            <div id="d036" class="px-[10px] py-[4px] border-[1px] border-[#2e2e32] rounded-full"><text id="x020" class="font-[inter400] text-[11px] text-[#c7c7c9]">High ⌄</text></div>
          </div>
        </div>
      </div>

      {/* ═══ PANEL 4: Gemini CLI ═══ */}
      <div id="t-gemini" class="absolute left-[260px] top-[150px] w-[1400px] h-[780px] bg-[#1a1b1f] rounded-[22px] overflow-hidden shadow-[0_32px_70px_rgba(11,11,15,0.32),0_12px_22px_rgba(11,11,15,0.16)] opacity-0">
        <TermChrome ids={{ bar: 'd037', r: 'd038', y: 'd039', g: 'd040', logo: 'gemini-logo', label: 'x021' }} logo="assets/w2h-logos/gemini.png" label="Gemini CLI · gemini-3-flash" />
        <div id="d041" class="absolute inset-x-0 top-[54px] bottom-0 px-[28px] pt-[22px]">
          <div id="d042" class="flex justify-between pt-[18px] pb-[14px]"><text id="x022" class="font-[mono] text-[12px] text-[#8a8f98]">Shift+Tab to accept edits</text><text id="x023" class="font-[mono] text-[12px] text-[#8a8f98]">12 skills</text></div>
          <div id="d043" class="border-t-[1px] border-dashed border-[rgba(255,255,255,0.14)]"></div>
          <div id="d044" class="relative flex items-center gap-[10px] py-[80px]">
            <text id="x024" class="font-[mono] text-[18px] text-[#8a8f98]">{'>'}</text>
            <text id="p4" class="font-[mono] font-normal text-[26px] text-[#f0f0f0] whitespace-nowrap">{P4}</text>
            <div id="c4" class="w-[11px] h-[28px] bg-[#f0f0f0] ml-[2px] opacity-0"></div>
            <text id="ge-ph" class="absolute right-0 font-[mono] text-[13px] text-[#6e7077]">Type your message or @path/to/file</text>
          </div>
          <div id="d045" class="border-t-[1px] border-dashed border-[rgba(255,255,255,0.14)]"></div>
          <div id="d046" class="grid grid-cols-3 gap-[30px] pt-[18px]">
            <div id="gecol1"><text id="x025" class="font-[mono] text-[11px] text-[#8a8f98]">workspace</text><text id="x026" class="font-[mono] text-[11px] font-medium text-[#f0f0f0]">~/Desktop</text></div>
            <div id="gecol2"><text id="x027" class="font-[mono] text-[11px] text-[#8a8f98]">sandbox</text><text id="x028" class="font-[mono] text-[11px] font-medium text-[#ff5577]">no-sandbox</text></div>
            <div id="gecol3" class="text-right"><text id="x029" class="font-[mono] text-[11px] text-[#8a8f98]">/model</text><text id="x030" class="font-[mono] text-[11px] font-medium text-[#f0f0f0]">gemini-3-flash-preview</text></div>
          </div>
        </div>
      </div>

      {/* ═══ Phrase ═══ */}
      <div id="phrase" class="absolute left-0 top-[288px] w-[1920px] text-center tracking-[-0.025em] z-[18]">
        <text id="pw1" class="font-[inter800] font-extrabold text-[160px] leading-[1.05] text-[#0b0b0f] opacity-0 blur-[14px]">Now</text>
        <text id="pw2" class="font-[inter800] font-extrabold text-[160px] leading-[1.05] text-[#533afd] opacity-0 blur-[14px]">any site becomes</text>
        <text id="pw3" class="font-[inter800] font-extrabold italic text-[160px] leading-[1.05] text-[#00a7d9] opacity-0 blur-[14px]">video.</text>
      </div>

      {/* ═══ YEAHH + sparkle stars ═══ */}
      <div id="yeahh" class="absolute left-0 top-[420px] w-[1920px] text-center opacity-0 z-[20]">
        <text id="yeahh-t" class="font-[bungee] text-[240px] leading-none text-[#ffd60a] [text-shadow:3px_3px_0_#ff006e,6px_6px_0_rgba(0,0,0,0.4)]">YEAHH</text>
        {/* Reference stars are the text glyph ✦ in a 60px box positioned
            around its shrink-to-fit wrap (box 307.5..1612.5 x, 420..660 y;
            measured advance width 1305). The engine has no ✦ glyph, so each
            star is the same four-point star drawn as an SVG path, centred in
            the glyph's 60px box (52px ink + 4px inset). */}
        <path id="st1" view-box="0 0 60 60" d="M30 4 C32.5 19.5 40.5 27.5 56 30 C40.5 32.5 32.5 40.5 30 56 C27.5 40.5 19.5 32.5 4 30 C19.5 27.5 27.5 19.5 30 4 Z" class="absolute left-[192px] top-[-76px] w-[52px] h-[52px] fill-[#ff006e] opacity-0" />
        <path id="st2" view-box="0 0 60 60" d="M30 4 C32.5 19.5 40.5 27.5 56 30 C40.5 32.5 32.5 40.5 30 56 C27.5 40.5 19.5 32.5 4 30 C19.5 27.5 27.5 19.5 30 4 Z" class="absolute left-[1657px] top-[-56px] w-[52px] h-[52px] fill-[#ff006e] opacity-0" />
        <path id="st3" view-box="0 0 60 60" d="M30 4 C32.5 19.5 40.5 27.5 56 30 C40.5 32.5 32.5 40.5 30 56 C27.5 40.5 19.5 32.5 4 30 C19.5 27.5 27.5 19.5 30 4 Z" class="absolute left-[372px] top-[244px] w-[52px] h-[52px] fill-[#ff006e] opacity-0" />
        <path id="st4" view-box="0 0 60 60" d="M30 4 C32.5 19.5 40.5 27.5 56 30 C40.5 32.5 32.5 40.5 30 56 C27.5 40.5 19.5 32.5 4 30 C19.5 27.5 27.5 19.5 30 4 Z" class="absolute left-[1477px] top-[264px] w-[52px] h-[52px] fill-[#ff006e] opacity-0" />
      </div>

      {/* ═══ Click cursors + ripples (ripple boxes centred on 816,470 / 816,1009) ═══ */}
      <MouseCursor id="mc-tl" left="812px" top="450px" w="26px" ht="38px" z="32" />
      <MouseCursor id="mc-bl" left="812px" top="989px" w="26px" ht="38px" z="32" />
      <div id="cr-tl" class="absolute left-[786px] top-[440px] w-[60px] h-[60px] border-[2px] border-[rgba(255,255,255,0.8)] rounded-full opacity-0 z-[31]"></div>
      <div id="cr-bl" class="absolute left-[786px] top-[979px] w-[60px] h-[60px] border-[2px] border-[rgba(255,255,255,0.8)] rounded-full opacity-0 z-[31]"></div>

      {/* ═══ Sent bubbles (max-w 340 + ellipsis in the reference) ═══ */}
      <div id="sm-claude" class="absolute left-[350px] top-[160px] z-[24] rounded-[14px] px-[16px] py-[10px] bg-[rgba(200,109,63,0.22)] border-[1px] border-[rgba(200,109,63,0.45)] opacity-0">
        <text id="x031" class="font-[mono] text-[13px] text-[#f2e6d5] whitespace-nowrap max-w-[340px] truncate">{P1}</text>
      </div>
      <div id="sm-cursor" class="absolute left-[1310px] top-[130px] z-[24] rounded-[14px] px-[16px] py-[10px] bg-[rgba(120,110,180,0.22)] border-[1px] border-[rgba(150,140,220,0.4)] opacity-0">
        <text id="x032" class="font-[mono] text-[13px] text-[#e8e6f2] whitespace-nowrap max-w-[340px] truncate">{P2}</text>
      </div>
      <div id="sm-codex" class="absolute left-[350px] top-[700px] z-[24] rounded-[14px] px-[16px] py-[10px] bg-[rgba(110,100,180,0.22)] border-[1px] border-[rgba(140,130,210,0.4)] opacity-0">
        <text id="x033" class="font-[mono] text-[13px] text-[#e8e6f2] whitespace-nowrap max-w-[340px] truncate">{P3}</text>
      </div>
      <div id="sm-gemini" class="absolute left-[1150px] top-[780px] z-[24] flex items-center gap-[6px] py-[4px] opacity-0">
        <text id="x034a" class="font-[mono] text-[12px] text-[#28c840] whitespace-nowrap">✓</text>
        <text id="x034" class="font-[mono] text-[12px] text-[#7aaa7a] whitespace-nowrap">{" task accepted · website-to-hyperframes queued"}</text>
      </div>

      {/* Master bottom captions (VO-synced) */}
      <BottomCap id="mc1" tid="mt1" t="Promo for your app?" />
      <BottomCap id="mc2" tid="mt2" t="Brand video?" />
      <BottomCap id="mc3" tid="mt3" t="Social ad?" />
      <BottomCap id="mc4" tid="mt4" t="Launch clip?" />

      <div id="flash" class="absolute inset-0 bg-[#ffffff] opacity-0 z-[40]"></div>
    </div>
  );
}

/* ═════════════ ACT 3 — 11 hard-cut video clips (abs 12.0–33.8) ═════════════ */

// rootId is the act container's external id, owned by the caller (W2H drives
// the hard-cut windows on it); the component never hard-codes it.
function Act3({ rootId }) {
  useGSAP((ctx) => {
    var T = ctx.timeline();
    // Each clip owns [start, start+dur); the video media window itself is
    // bounded by dataStart/dataDuration on the node. The reference hard-cuts
    // (no cross-fade) and the top-left `.a3-cap` has NO entrance animation —
    // it is statically visible across the whole clip window, unlike the
    // bottom `.cap` which runs cap-pop.
    for (var i = 0; A3.length > i; i++) {
      var v = A3[i][0], c = A3[i][1], at = A3[i][4], dur = A3[i][5];
      T.set(v, { opacity: 1 }, at);
      T.set(v, { opacity: 0 }, at + dur);
      T.set(c, { opacity: 1 }, at);
      T.set(c, { opacity: 0 }, at + dur);
    }
  });

  return (
    <div id={rootId} class="absolute inset-0 z-[2] overflow-hidden bg-[#0A0A0A] opacity-0">
      {A3.map(function (row) {
        return (
          <video
            id={row[0]}
            path={'assets/w2h-a3/' + row[3]}
            dataStart={row[4]}
            dataDuration={row[5]}
            dataColorMatrix="bt709"
            class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover z-[5] opacity-0"
          />
        );
      })}
      {A3.map(function (row) {
        return <A3Cap id={row[1]} nid={row[1] + 'n'} bid={row[1] + 'b'} num={row[7]} brand={row[8]} light={row[6]} />;
      })}
    </div>
  );
}

/* ═════════════ ACT 4 — HyperFrames wordmark finale (abs 33.8–41.8) ═════════════ */

var PARTICLES = ['pt1', 'pt2', 'pt3', 'pt4', 'pt5', 'pt6', 'pt7', 'pt8', 'pt9', 'pt10', 'pt11', 'pt12'];
var PT_XY = [
  [230.4, 302.4], [422.4, 777.6], [672, 237.6], [921.6, 885.6],
  [1113.6, 194.4], [1305.6, 777.6], [1497.6, 302.4], [1689.6, 669.6],
  [345.6, 540], [806.4, 410.4], [1190.4, 561.6], [1574.4, 432],
];

function Act4({ rootId }) {
  useGSAP((ctx) => {
    var T = ctx.timeline();

    // 33.8 - 35.3 abs (local 0.0 - 1.5): Aurora + spotlight bloom in
    T.to('aurora', { opacity: 0.85, duration: 1.2, ease: 'power2.out' }, 33.8);
    T.to('spot', { opacity: 1, duration: 1.4, ease: 'power2.out' }, 34.0);

    // 34.1 - 36.8 abs (local 0.2 - 3.0): particles twinkle in. The reference
    // staggers `from: "random"` (unseeded); a deterministic front-to-back
    // stagger keeps the render reproducible (12 x 3px dots).
    T.to(PARTICLES, { opacity: 0.85, duration: 0.8, stagger: { each: 0.06 }, ease: 'power2.out' }, 34.1);

    // Each particle breathes over the remaining Act-4 duration.
    for (var i = 0; PARTICLES.length > i; i++) {
      var delay = 0.5 + (i % 5) * 0.4;
      var cycleDur = 2.0 + (i % 3) * 0.5;
      var totalLeft = 8.0 - delay;
      var repeats = Math.max(0, Math.floor(totalLeft / cycleDur) - 1);
      T.to(PARTICLES[i], {
        opacity: 0.3,
        y: '+=6',
        duration: cycleDur,
        ease: 'sine.inOut',
        repeat: repeats,
        yoyo: true,
      }, delay + 33.8);
    }

    // 34.7 / 35.0 abs (local 0.9 / 1.2): wordmark blur-to-sharp
    T.to('w-hyper', { opacity: 1, filter: 'blur(0px)', duration: 0.85, ease: 'power2.out' }, 34.7);
    T.to('w-frames', { opacity: 1, filter: 'blur(0px)', duration: 0.85, ease: 'power2.out' }, 35.0);

    // 36.2 / 37.1 abs (local 2.4 / 3.3): promise lines
    T.to('line-1', { opacity: 1, filter: 'blur(0px)', duration: 0.8, ease: 'power2.out' }, 36.2);
    T.to('line-2', { opacity: 1, filter: 'blur(0px)', duration: 0.8, ease: 'power2.out' }, 37.1);

    // 38.6 / 38.8 abs (local 4.8 / 5.0): install pill
    T.to('install', { opacity: 1, duration: 0.5, ease: 'power2.out' }, 38.6);
    T.to('install-cmd', { opacity: 1, filter: 'blur(0px)', duration: 0.8, ease: 'power2.out' }, 38.8);

    // 39.8 - 41.8 abs (local 6.0 - 8.0): breath dot
    T.fromTo('breath-dot', { opacity: 0, scale: 0 }, { opacity: 0.9, scale: 1.0, duration: 0.5, ease: 'back.out(2)' }, 39.8);
    T.to('breath-dot', { scale: 1.5, opacity: 0.5, duration: 0.8, ease: 'sine.inOut', yoyo: true, repeat: 1 }, 40.3);

    // Wordmark subtle breath through the whole composition
    T.to(['w-hyper', 'w-frames'], {
      scale: 1.01, duration: 3.5, ease: 'sine.inOut', yoyo: true, repeat: 1, transformOrigin: '50% 50%',
    }, 35.6);

    // Aurora drifts
    T.to('aurora', { x: 40, y: -20, duration: 7.5, ease: 'sine.inOut' }, 34.0);

    // Master captions: "Give your agent a URL" @35.8 (dur 2.83), "Go make
    // something" @40.1 (dur 1.21) — cyan accent variant.
    popCaption(T, 'cap1', 35.8, 2.83);
    popCaption(T, 'cap2', 40.1, 1.21);
  });

  return (
    <div id={rootId} class="absolute inset-0 z-[3] overflow-hidden bg-[#06070B] opacity-0">

      {/* bg-deep */}
      <div id="bg-deep" class="absolute inset-0 bg-[radial-gradient(1600px_1000px_at_50%_65%,#0a0c18_0%,#06070B_55%,#030408_100%)]"></div>

      {/* bg-aurora: four radial gradients in one layer stack, blur(2px) */}
      <div id="aurora" class="absolute inset-0 opacity-0 blur-[2px] bg-[radial-gradient(900px_500px_at_22%_38%,rgba(83,58,253,0.28),transparent_65%),radial-gradient(1100px_700px_at_78%_42%,rgba(0,195,255,0.22),transparent_60%),radial-gradient(700px_500px_at_50%_80%,rgba(62,207,142,0.18),transparent_65%),radial-gradient(600px_400px_at_80%_15%,rgba(244,75,204,0.15),transparent_65%)]"></div>

      {/* bg-grain: dotted overlays (one layer each so sizes can differ) */}
      <div id="grain-a" class="absolute inset-0 opacity-50 bg-[radial-gradient(circle_at_25%_35%,rgba(255,255,255,0.04)_1px,transparent_1px)] bg-[length:4px_4px]"></div>
      <div id="grain-b" class="absolute inset-0 opacity-50 bg-[radial-gradient(circle_at_75%_65%,rgba(255,255,255,0.03)_1px,transparent_1px)] bg-[length:6px_6px]"></div>

      {/* spotlight */}
      <div id="spot" class="absolute left-[260px] top-[212px] w-[1400px] h-[700px] opacity-0 blur-[1px] bg-[radial-gradient(closest-side,rgba(255,255,255,0.06),transparent)]"></div>

      {/* particles 12 (reference percentage grid → px) */}
      {PARTICLES.map(function (pid, i) {
        return (
          <div
            id={pid}
            class={'absolute w-[3px] h-[3px] rounded-full bg-[#f7f8f8] shadow-[0_0_8px_rgba(255,255,255,0.8)] opacity-0 left-[' + PT_XY[i][0] + 'px] top-[' + PT_XY[i][1] + 'px]'}
          ></div>
        );
      })}

      {/* wordmark: Hyper solid, Frames gradient (background-clip: text) */}
      <div id="wm-wrap" class="absolute left-0 top-[320px] w-[1920px] flex justify-center items-start">
        <text id="w-hyper" class="font-[inter900] font-black tracking-[-0.04em] text-[200px] leading-none text-[#f7f8f8] opacity-0 blur-[20px]">Hyper</text>
        <text id="w-frames" class="font-[inter900] font-black tracking-[-0.04em] text-[200px] leading-none text-transparent opacity-0 blur-[20px] bg-[linear-gradient(95deg,#533afd_0%,#00c3ff_45%,#3ecf8e_100%)] bg-clip-text">Frames</text>
      </div>

      {/* promise lines */}
      <text id="line-1" class="absolute left-0 top-[586px] w-[1920px] text-center font-[inter] font-medium text-[58px] tracking-[-0.01em] leading-none text-[rgba(247,248,248,0.9)] opacity-0 blur-[14px]">Give your agent a URL.</text>
      <text id="line-2" class="absolute left-0 top-[664px] w-[1920px] text-center font-[inter] font-medium text-[58px] tracking-[-0.01em] leading-none text-[rgba(247,248,248,0.9)] opacity-0 blur-[14px]">It does the rest.</text>

      {/* install pill */}
      <div id="install" class="absolute left-0 top-[815px] w-[1920px] flex justify-center opacity-0">
        <div id="install-inner" class="px-[28px] py-[14px] rounded-[12px] bg-[rgba(255,255,255,0.04)] border-[1px] border-[rgba(255,255,255,0.08)] backdrop-blur-[8px]">
          <text id="install-cmd" class="font-[mono500] font-medium text-[28px] tracking-[0.005em] leading-none text-[rgba(247,248,248,0.85)] opacity-0 blur-[8px]">npx skills add heygen-com/hyperframes</text>
        </div>
      </div>

      <div id="breath-dot" class="absolute left-[957px] top-[945px] w-[6px] h-[6px] rounded-full bg-[#00c3ff] shadow-[0_0_16px_rgba(0,195,255,0.8)] opacity-0"></div>

      {/* master-level captions (VO-synced), accent = cyan */}
      <BottomCap id="cap1" tid="cap1-t" t="Give your agent a URL" accent="1" />
      <BottomCap id="cap2" tid="cap2-t" t="Go make something" accent="1" />
    </div>
  );
}

/* ═════════════ entry ═════════════ */

export default function W2H() {
  // Caller-owned ids for the act containers; passed down as props and used
  // here for the hard-cut windows — the components stay id-agnostic.
  var ACT3 = 'act3', ACT4 = 'act4';
  useGSAP((ctx) => {
    // Act section windows — the reference master shows each act section only
    // inside its absolute window; boundaries are HARD CUTS (act1 12.0 → act3
    // 12.0, act3 33.8 → act4 33.8). The stacked opaque containers replicate
    // that: revealing the next container covers the previous one at the
    // boundary frame (act1's white-flash tail past 12.0 included).
    var T = ctx.timeline();
    T.set(ACT3, { opacity: 1 }, 12.0);
    T.set(ACT4, { opacity: 1 }, 33.8);
    // Act-3 bottom captions live in their own top-level overlay (outside the
    // act3 container): inside the container, alongside the 11 video siblings,
    // the engine mis-lays-out this caption subtree (offset ≈ +56/+63px and,
    // without GSAP transforms, drops it entirely). The captions are absolutely
    // positioned full-frame overlays whose opacity is fully owned by
    // popCaption, so hosting them at stage level is behaviour-identical.
    for (var i = 0; A3.length > i; i++) {
      popCaption(T, A3[i][2], A3_MCAP[i][0], A3_MCAP[i][1]);
    }
  });
  return (
    <div id="stage" class="relative w-[1920px] h-[1080px] overflow-hidden bg-[#0a0a0a]">
      <Act1 />
      <Act3 rootId={ACT3} />
      <Act4 rootId={ACT4} />
      <div id="a3-caps" class="absolute inset-0 z-[25]">
        {A3.map(function (row) {
          return <BottomCap id={row[2]} tid={row[2] + 't'} t={row[9]} light={row[6]} />;
        })}
      </div>
    </div>
  );
}
