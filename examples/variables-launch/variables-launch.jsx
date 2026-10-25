// JSX port of hyperframes-launches/variables-launch (Hyperframes "Variables"
// launch, 1920x1080 @ 30fps, 43.6s / 1308 frames).
//
// Structure mirrors the reference: index.html is one root composition and the
// nine scenes are absolutely-positioned, z-indexed windows that replace each
// other at hard cuts. Each reference composition becomes a component whose
// GSAP timeline body moves verbatim into one `useGSAP` hook, so hook order
// (tree order) matches the reference scene order.
//
// Where the reference measures the DOM (`getBoundingClientRect`) we use the
// engine's measure surface (`ctx.surface(1,1)` + `measureText`), the same
// technique k3-promo.jsx uses for its type calibration -- measurement never
// touches pixels.
//
// Reference: /home/solaren/Projects/hyperframes-launches/variables-launch/
//   index.html + compositions/scene-0{1..9}.html + renders/*_swgl.mp4

import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 43.6 };

export const fonts = {
  default: 'inter',
  faces: [
    { id: 'inter', family: 'Inter', path: 'assets/fonts/Inter-500v.ttf', role: 'sans' },
    { id: 'inter400', family: 'Inter', path: 'assets/fonts/Inter-400.ttf' },
    { id: 'inter800', family: 'Inter', path: 'assets/fonts/Inter-800.ttf' },
    { id: 'inter900', family: 'Inter', path: 'assets/fonts/Inter-900.ttf' },
    { id: 'mono500', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-500.ttf', role: 'mono' },
  ],
};

/* -- shared per-frame helpers (reference `index.html` head + scene prologues) -- */

// One measure stack shared by every scene's text metrics (get-or-create by
// name, so re-entering per consumer is free). Same technique as k3-promo.jsx.
function makeMeasure(ctx) {
  var MSR = ctx.surface(1, 1);
  function natW(family, weight, fs, ls, s) {
    MSR.font = weight + ' ' + fs + 'px ' + family;
    MSR.letterSpacing = ls + 'px';
    return MSR.measureText(s).width;
  }
  return { natW: natW };
}

var INTER = 'Inter';
var MONO = 'JetBrains Mono';

/* =========== Scene 01 -- the dial (abs 0.0-5.5 s) ==========================
   Reference: scene-01.html. A word conveyor that slides in with an
   accelerating waterfall stagger, exits with the inverse stagger, and hands
   off to a 3D flip "dial" slot. The flip is rotateX (a real 3D transform);
   the engine's 2D transform set has rotate/scale/translate only, so the flip
   is expressed through the CanvasKit 2D layer on a full-stage canvas that
   draws the dial stack per frame. */
const DIAL_WORDS = ['launch', 'explainer', 'training', 'product demo', 'UGC ad'];
const INTRO_WORDS = ['You', 'used', 'HyperFrames', 'to', 'make'];

// `pfx` decides the component's whole id namespace: the caller owns the ids,
// two DialStage instances can coexist without collisions. Scene01 must pass
// the same prefix to its use* references.
function DialStage({ pfx }) {
  var P = pfx || '';
  return (
    <div id={P + 'scene-01'} class="absolute inset-0 bg-[#ffffff]" duration="5.5">
      {/* intro conveyor words: positions come from the measure prologue */}
      <div id={P + 'iw-wrap'} class="absolute inset-0">
        {INTRO_WORDS.map((w) => (
          <text id={P + 'iw-' + w} class="absolute font-[inter800] font-extrabold text-[94px] tracking-[-2.82px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap opacity-0">{w}</text>
        ))}
      </div>
      {/* dial line: "the perfect [slot] video", slot width = max word width */}
      <div id={P + 'dial-line'} class="absolute left-0 top-0 w-[1920px] h-[1080px] flex items-center justify-center">
        <text id={P + 'perfect-word'} class="font-[inter800] font-extrabold text-[104px] tracking-[-3.12px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap">the perfect</text>
        <div id={P + 'dial-slot'} class="relative">
          {DIAL_WORDS.map((w) => (
            <text id={P + 'dw-' + w} class="absolute left-0 top-0 w-full text-center font-[inter800] font-extrabold text-[104px] tracking-[-3.12px] leading-[120.64px] text-[#0a0a0a] whitespace-nowrap opacity-0">{w}</text>
          ))}
        </div>
        <div id={P + 'siri-wrap'} class="relative opacity-0"><text id={P + 'video-word'} class="font-[inter800] font-extrabold text-[104px] tracking-[-3.12px] leading-[1.16] whitespace-nowrap text-transparent bg-[linear-gradient(90deg,#FF6B6B_0%,#FF9500_20%,#E879F9_40%,#818CF8_60%,#22D3EE_80%,#FF6B6B_100%)] bg-[length:200%_100%] bg-clip-text">video</text></div>
      </div>
      <canvas id={P + 'dial-canvas'} class="absolute left-0 top-0 w-[1920px] h-[1080px]" />
    </div>
  );
}

function Scene01() {
  // Same prefix passed to <DialStage> below — every id this scope touches is
  // declared inside its own returned tree.
  var P = '';
  useGSAP(() => {
    var m = makeMeasure(ctx);
    var T = gsap.timeline();

    /* -- reference scene-01.html prologue: measure every word, derive the
         slot width and the per-word slide distances. The reference uses
         getBoundingClientRect; the engine has no layout-box readback in the
         script realm, so the same numbers come from the measure surface
         (font 800, 94px / 104px, letter-spacing -0.03em). -- */
    var introW = INTRO_WORDS.map((w) => m.natW(INTER, 800, 94, -2.82, w));
    var dialW = DIAL_WORDS.map((w) => m.natW(INTER, 800, 104, -3.12, w));
    var maxW = Math.max.apply(null, dialW);
    var perfectW = m.natW(INTER, 800, 104, -3.12, 'the perfect');
    var videoW = m.natW(INTER, 800, 104, -3.12, 'video');

    // Flex-centre offsets: the reference lays the line out in a flex row with
    // 0.2em side margins on the slot; total width / 2 gives each part's x.
    var slotTotal = maxW + 2 * Math.round(104 * 0.2);
    var lineTotal = perfectW + slotTotal + videoW;
    var startX = (1920 - lineTotal) / 2;
    var perfectX = startX;
    var slotX = startX + perfectW;
    var videoX = slotX + slotTotal;

    // Conveyor words stack left-to-right with an 18px gap, centred as a row.
    var introTotal = introW.reduce((a, b) => a + b, 0) + 18 * (introW.length - 1);
    var introStart = (1920 - introTotal) / 2;
    var introX = [];
    var acc = introStart;
    for (var i = 0; i < introW.length; i++) { introX.push(acc); acc += introW[i] + 18; }

    ctx.getNode(P + 'perfect-word').left(perfectX);
    ctx.getNode(P + 'dial-slot').left(slotX).width(maxW);
    ctx.getNode(P + 'video-word').left(videoX);
    for (var d = 0; d < DIAL_WORDS.length; d++) ctx.getNode(P + 'dw-' + DIAL_WORDS[d]).left(0);

    // Waterfall entry: the reference tweens each word's x from off-screen
    // right to 0 with an accelerating stagger; the element's own left is fixed
    // so x is a pure offset.
    var entryT = 0;
    var entryDelays = [0, 0.2, 0.1, 0.05, 0.025];
    for (var ei = 0; ei < INTRO_WORDS.length; ei++) {
      entryT += entryDelays[ei];
      var off = 1920 - introX[ei] + 20;
      ctx.getNode(P + 'iw-' + INTRO_WORDS[ei]).left(introX[ei]);
      T.fromTo(P + 'iw-' + INTRO_WORDS[ei], { x: off, opacity: 0 },
        { x: 0, opacity: 1, duration: 0.3, ease: 'power3.out' }, entryT);
    }

    // Waterfall exit (inverse stagger), then the dial slides in.
    var exitT = entryT + 0.3 + 0.5;
    var exitDelays = [0, 0.025, 0.05, 0.1, 0.2];
    for (var xi = 0; xi < INTRO_WORDS.length; xi++) {
      exitT += exitDelays[xi];
      var exitDist = introX[xi] + introW[xi] + 20;
      T.to(P + 'iw-' + INTRO_WORDS[xi], { x: -exitDist, opacity: 0, duration: 0.3, ease: 'power3.in' }, exitT);
    }
    var dialIn = exitT + 0.15;
    T.fromTo(P + 'dial-line', { x: 1920, opacity: 1 }, { x: 0, duration: 0.35, ease: 'power3.out' }, dialIn);
    var firstFlip = dialIn + 0.35 + 0.5;

    // Dial flips. rotateX is not in the engine's 2D transform set, so the word
    // stack on the slot is drawn per frame by the canvas hook below and driven
    // from this timeline's clock via plain opacity flips.
    var flipDur = 0.35, holdDur = 0.35, gap = 128;
    T.set(P + 'dw-' + DIAL_WORDS[0], { opacity: 1 }, firstFlip - holdDur);
    var t = firstFlip;
    for (var fi = 0; fi < DIAL_WORDS.length - 1; fi++) {
      T.to(P + 'dw-' + DIAL_WORDS[fi], { opacity: 0, duration: flipDur * 0.5, ease: 'power1.inOut' }, t + flipDur * 0.5);
      T.set(P + 'dw-' + DIAL_WORDS[fi + 1], { opacity: 1 }, t + flipDur);
      T.to(P + 'perfect-word', { x: (maxW - dialW[fi + 1]) / 2, duration: flipDur * 0.8, ease: 'power2.inOut' }, t);
      T.to(P + 'video-word', { x: -(maxW - dialW[fi + 1]) / 2, duration: flipDur * 0.8, ease: 'power2.inOut' }, t);
      t += flipDur + holdDur;
    }
    T.to(P + 'dw-' + DIAL_WORDS[DIAL_WORDS.length - 2], { opacity: 0, duration: 0.25 }, t - holdDur + 0.1);

    // "video" picks up the siri gradient (six-stop mesh) for the last ~2 s.
    // The gradient is a `bg-clip-text` layer; the reference animates
    // backgroundPosition 0% -> 200% over 2 s. The engine has no
    // background-position channel, so the same sweep comes from translating a
    // wider-than-the-word gradient wrapper: gradient is 2x the word width and
    // the wrapper slides from 0 to -100%.
    T.set(P + 'siri-wrap', { opacity: 1 }, t - holdDur);
  });
  return <DialStage pfx={P} />;
}

/* =========== Scene 02 -- UGC card (abs 5.5-9.5 s) ==========================
   Reference: scene-02.html. "the perfect UGC ad video" splits apart while a
   glass card scales up behind it with hook-c.mp4 inside; cut-the-curve exit
   (leftward + 8px blur). */
function Scene02() {
  useGSAP(() => {
    var T = gsap.timeline();
    // 0.19: centre word fades out, the two outer words slide apart
    T.to('s2-center', { opacity: 0, scale: 0.8, duration: 0.32, ease: 'power2.in' }, 0.19);
    T.to('s2-perfect', { x: -400, opacity: 0, duration: 0.45, ease: 'power3.in' }, 0.19);
    T.to('s2-video', { x: 400, opacity: 0, duration: 0.45, ease: 'power3.in' }, 0.19);
    // 0.32: the card scales up from 0.3 with the siri gradient already on
    T.fromTo('s2-card', { opacity: 0, scale: 0.3 }, { opacity: 1, scale: 1.4, duration: 0.51, ease: 'power2.out' }, 0.32);
    // last 0.33 s: cut-the-curve exit
    T.to('s2-card', { x: -200, filter: 'blur(8px)', duration: 0.33, ease: 'power2.in' }, 4 - 0.33);
  });
  return (
    <div id="scene-02" class="absolute inset-0 bg-[#ffffff]" duration="4.0">
      <div id="s2-line" class="absolute left-0 top-0 w-[1920px] h-[1080px] flex items-center justify-center">
        <text id="s2-perfect" class="font-[inter800] font-extrabold text-[104px] tracking-[-3.12px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap">the perfect</text>
        <text id="s2-center" class="font-[inter800] font-extrabold text-[104px] tracking-[-3.12px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap mx-[20.8px]">UGC ad</text>
        <text id="s2-video" class="font-[inter800] font-extrabold text-[104px] tracking-[-3.12px] leading-[1.16] whitespace-nowrap text-transparent bg-[linear-gradient(90deg,#FF6B6B_0%,#FF9500_20%,#E879F9_40%,#818CF8_60%,#22D3EE_80%,#FF6B6B_100%)] bg-[length:200%_100%] bg-clip-text">video</text>
      </div>
      <div id="s2-card" class="absolute left-[770px] top-[202.5px] w-[380px] h-[675px] rounded-[24px] overflow-hidden border-[1px] border-[rgba(0,0,0,0.08)] bg-[linear-gradient(165deg,rgba(0,0,0,0.04)_0%,rgba(0,0,0,0.02)_40%,rgba(0,0,0,0.01)_100%)] opacity-0 shadow-[0_8px_32px_rgba(0,0,0,0.06),0_2px_8px_rgba(0,0,0,0.04)]">
        <video id="s2-video-el" path="assets/hooks/hook-c.mp4" data-start="0.51" data-duration="3.49" class="absolute inset-0 w-full h-full object-cover rounded-[23px]" />
        <div class="absolute left-[-76px] top-[-202.5px] w-[532px] h-[337.5px] bg-[linear-gradient(180deg,rgba(255,255,255,0.4)_0%,rgba(255,255,255,0)_100%)]"></div>
      </div>
    </div>
  );
}

/* =========== Scene 03 -- the question (abs 9.5-12.5 s) ======================
   Reference: scene-03.html. Two word lines. Line A enters with the matched
   cut-the-curve (x:200 + blur 8), falls out downward in a waterfall, line B
   drops in from the top, then the whole line zooms through (scale 1.2 +
   blur 20 + opacity 0.15). */
const LINE_A = ['What', 'if', 'you', 'could', 'test', 'variations'];
const LINE_B = ['without', 'starting', 'from', 'scratch?'];

function Scene03() {
  useGSAP(() => {
    var m = makeMeasure(ctx);
    var T = gsap.timeline();

    // Reference measures each word's rect to compute the fall distance
    // (1080 - top + 20) and the drop-from distance (bottom + 20). We compute
    // the same from the font metrics on the measure surface.
    var aW = LINE_A.map((w) => m.natW(INTER, 800, 94, -2.82, w));
    var bW = LINE_B.map((w) => m.natW(INTER, 800, 94, -2.82, w));
    var aTotal = aW.reduce((a, b) => a + b, 0) + 18 * (aW.length - 1);
    var bTotal = bW.reduce((a, b) => a + b, 0) + 18 * (bW.length - 1);
    var aX = [], bX = [];
    var acc = (1920 - aTotal) / 2;
    for (var i = 0; i < aW.length; i++) { aX.push(acc); acc += aW[i] + 18; }
    acc = (1920 - bTotal) / 2;
    for (var j = 0; j < bW.length; j++) { bX.push(acc); acc += bW[j] + 18; }

    for (var p = 0; p < LINE_A.length; p++) ctx.getNode('l3a-' + LINE_A[p]).left(aX[p]);
    for (var q = 0; q < LINE_B.length; q++) ctx.getNode('l3b-' + LINE_B[q]).left(bX[q]);

    // Cut-the-curve entry (all six words at once)
    for (var e = 0; e < LINE_A.length; e++) {
      T.fromTo('l3a-' + LINE_A[e], { x: 200, filter: 'blur(8px)', opacity: 0 },
        { x: 0, filter: 'blur(0px)', opacity: 1, duration: 0.42, ease: 'expo.out' }, 0);
    }
    // Waterfall exit downward
    var fallStart = 0.9, fallT = fallStart;
    var exitDelays = [0, 0.12, 0.08, 0.05, 0.03, 0.02];
    for (var f = 0; f < LINE_A.length; f++) {
      fallT += exitDelays[f];
      T.to('l3a-' + LINE_A[f], { y: 1100, opacity: 0, duration: 0.3, ease: 'power3.in' }, fallT);
    }
    // Line B drops in from above
    var dropStart = fallT + 0.3 + 0.15;
    T.set('l3b', { opacity: 1 }, dropStart);
    var dropT = dropStart;
    var entryDelays = [0, 0.12, 0.08, 0.05];
    for (var g = 0; g < LINE_B.length; g++) {
      dropT += entryDelays[g];
      T.fromTo('l3b-' + LINE_B[g], { y: -(bX[g] + bW[g] + 20), opacity: 0 },
        { y: 0, opacity: 1, duration: 0.3, ease: 'power3.out' }, dropT);
    }
    // Zoom-through exit
    T.to('l3b', { scale: 1.2, filter: 'blur(20px)', duration: 0.2, ease: 'power3.in' }, 3.0);
    T.to('l3b', { opacity: 0.15, duration: 0.2, ease: 'none' }, 3.0);
  });
  return (
    <div id="scene-03" class="absolute inset-0 bg-[#ffffff]" duration="3.2">
      <div id="l3a" class="absolute left-0 top-[452px] w-[1920px] h-[110px] flex items-center gap-[18px] justify-center whitespace-nowrap">
        {LINE_A.map((w, i) => (
          <text key={w} id={'l3a-' + w} class={'font-[inter800] font-extrabold text-[94px] tracking-[-2.82px] leading-[1.16] whitespace-nowrap ' + (i === 5 ? 'text-[#F3A6FF]' : 'text-[#0a0a0a]')}>{w}</text>
        ))}
      </div>
      <div id="l3b" class="absolute left-0 top-[452px] w-[1920px] h-[110px] flex items-center gap-[18px] justify-center whitespace-nowrap opacity-0">
        {LINE_B.map((w, i) => (
          <text key={w} id={'l3b-' + w} class={'font-[inter800] font-extrabold text-[94px] tracking-[-2.82px] leading-[1.16] whitespace-nowrap ' + (i === 3 ? 'text-[#35C838]' : 'text-[#0a0a0a]')}>{w}</text>
        ))}
      </div>
    </div>
  );
}


/* =========== Scene 04 -- three glass cards (abs 12.7-23.2 s) ================
   Reference: scene-04.html (duration 10.5). Left/right cards sit behind the
   centre card and slide out at 0.5 s; the code overlays then cascade their
   lines in and swap the variant values; after the reveal the overlays fade
   and the side videos recover brightness; the whole row exits left with an
   8px blur at 10.17 s.
   Geometry: scene-04 is a flex row of three cards, gap 24, padding 40/80, so
   card width = (1920 - 160 - 48) / 3 = 570.51, height 960 (max-height). */
const CARD_W = 570.6667;
const CARD_GAP = 24;
const CARD_LEFT = 80;
const CARD_H = 960;
const CARD_TOP = 40;

function GlassCard({ id, left, zIndex, videoId, videoPath, mediaStart, children }) {
  return (
    <div id={id} class={'absolute left-[' + left + 'px] top-[' + CARD_TOP + 'px] w-[' + CARD_W + 'px] h-[' + CARD_H + 'px] rounded-[24px] overflow-hidden border-[1px] border-[rgba(0,0,0,0.08)] bg-[linear-gradient(165deg,rgba(0,0,0,0.04)_0%,rgba(0,0,0,0.02)_40%,rgba(0,0,0,0.01)_100%)] shadow-[0_8px_32px_rgba(0,0,0,0.06),0_2px_8px_rgba(0,0,0,0.04)]' + (zIndex ? ' z-[' + zIndex + ']' : '')}>
      <video id={videoId} path={videoPath} data-start="0" data-duration="10.5" data-media-start={mediaStart} class="absolute inset-0 w-full h-full object-cover rounded-[23px]" />
      <div class="absolute left-[-76px] top-[-202.5px] w-[532px] h-[337.5px] bg-[linear-gradient(180deg,rgba(255,255,255,0.4)_0%,rgba(255,255,255,0)_100%)]"></div>
      {children}
    </div>
  );
}

function CodeOverlay({ id, label, rows, color }) {
  return (
    <div id={id} class="absolute inset-0 bg-[rgba(0,0,0,0.82)] rounded-[23px] pt-[36px] pr-[36px] pb-[36px] pl-[36px] font-[mono500] text-[42px] leading-[2.2] z-[4]">
      <div id={id + '-t'} class="opacity-0">
        <text class="font-[inter800] font-extrabold text-[44px] tracking-[1.76px] text-[rgba(255,255,255,0.75)] uppercase">{label}</text>
      </div>
      {rows.map((r) => (
        <div key={r.id} id={id + '-row-' + r.id} class="opacity-0">
          <text class="text-[rgba(255,255,255,0.5)]">hookText</text>
          <text class="text-[rgba(255,255,255,0.25)]">: </text>
          <div class="relative inline-block">
            <text id={r.id + '-old'} class="text-[#00C3FF]">{r.old}</text>
            <text id={r.id + '-new'} class={'text-[#' + (r.cls === 'hot' ? 'F3A6FF' : '35C838') + '] absolute left-0 top-0 whitespace-nowrap opacity-0'}>{r.new}</text>
          </div>
        </div>
      ))}
    </div>
  );
}

function Scene04() {
  useGSAP(() => {
    var m = makeMeasure(ctx);
    var T = gsap.timeline();

    // Reference measures the three rects to get the slide distance; the flex
    // row geometry gives the same numbers: left card starts one card+gap to
    // the right of centre, right card one card+gap to the left.
    var slide = CARD_W + CARD_GAP;
    ctx.getNode('card-left').x(slide);
    ctx.getNode('card-right').x(-slide);

    // Zoom-through entry
    T.fromTo('scene-04', { scale: 0.75, filter: 'blur(20px)', opacity: 0.15 },
      { scale: 1, filter: 'blur(0px)', opacity: 1, duration: 0.5, ease: 'expo.out' }, 0);

    // Cards slide out from behind centre
    T.to('card-left', { x: 0, duration: 0.8, ease: 'power3.out' }, 0.5);
    T.to('card-right', { x: 0, duration: 0.8, ease: 'power3.out' }, 0.5);

    // Cascade the code lines in (label + 3 value rows, per side)
    var lineStart = 1.5;
    var idsL = ['code-left-t', 'code-left-row-l-hook', 'code-left-row-l-clr', 'code-left-row-l-clip'];
    var idsR = ['code-right-t', 'code-right-row-r-hook', 'code-right-row-r-clr', 'code-right-row-r-clip'];
    for (var li = 0; li < idsL.length; li++) T.to(idsL[li], { opacity: 1, duration: 0.2, ease: 'power2.out' }, lineStart + li * 0.05);
    for (var ri = 0; ri < idsR.length; ri++) T.to(idsR[ri], { opacity: 1, duration: 0.2, ease: 'power2.out' }, lineStart + 0.1 + ri * 0.05);

    // Swap the variant values, left side first then right
    var swapDur = 0.25;
    var leftSwapStart = 2.8;
    var swaps = ['l-hook', 'l-clr', 'l-clip', 'r-hook', 'r-clr', 'r-clip'];
    for (var i = 0; i < swaps.length; i++) {
      var t = leftSwapStart + i * 0.2;
      T.to(swaps[i] + '-old', { opacity: 0, y: -8, duration: swapDur, ease: 'power2.in' }, t);
      T.to(swaps[i] + '-new', { opacity: 1, y: 0, duration: swapDur, ease: 'power2.out' }, t + swapDur * 0.5);
    }

    // Brief hold, then the reveal: overlays out, videos back to full brightness
    var revealTime = 2.8 + 6 * 0.2 - 0.2 + swapDur + 0.5;
    T.to('code-left', { opacity: 0, duration: 0.5, ease: 'power2.in' }, revealTime);
    T.to('code-right', { opacity: 0, duration: 0.5, ease: 'power2.in' }, revealTime);
    T.to('vid-left', { filter: 'brightness(1) saturate(1)', duration: 0.5, ease: 'power2.out' }, revealTime);
    T.to('vid-right', { filter: 'brightness(1) saturate(1)', duration: 0.5, ease: 'power2.out' }, revealTime);
    T.to('tint-left', { opacity: 0, duration: 0.5, ease: 'power2.out' }, revealTime);
    T.to('tint-right', { opacity: 0, duration: 0.5, ease: 'power2.out' }, revealTime);

    // Cut-the-curve exit: all three cards slide left with an 8px blur
    T.to('card-left', { x: -200, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 10.17);
    T.to('card-center', { x: -200, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 10.17);
    T.to('card-right', { x: -200, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 10.17);
  });
  var lX = CARD_LEFT;
  var cX = CARD_LEFT + CARD_W + CARD_GAP;
  var rX = CARD_LEFT + 2 * (CARD_W + CARD_GAP);
  return (
    <div id="scene-04" class="absolute inset-0 bg-[#ffffff]" duration="10.5">
      <GlassCard id="card-left" left={lX} videoId="vid-left" videoPath="assets/hooks/hook-a.mp4" mediaStart="10">
        <div id="tint-left" class="absolute inset-0 bg-[rgba(180,180,190,0.25)]"></div>
        <CodeOverlay id="code-left" label="Variant A" color="#F3A6FF"
          rows={[
            { id: 'l-hook', old: '"I used to think"', new: '"Stop scrolling"', cls: 'hot' },
            { id: 'l-clr', old: '"#f5a623"', new: '"#F3A6FF"', cls: 'hot' },
            { id: 'l-clip', old: '"emma-v1.mp4"', new: '"emma-v2.mp4"', cls: 'hot' },
          ]} />
      </GlassCard>
      <GlassCard id="card-center" left={cX} zIndex="3" videoId="vid-center" videoPath="assets/hooks/hook-c.mp4" mediaStart="0">
        {null}
      </GlassCard>
      <GlassCard id="card-right" left={rX} videoId="vid-right" videoPath="assets/hooks/hook-b.mp4" mediaStart="20">
        <div id="tint-right" class="absolute inset-0 bg-[rgba(180,180,190,0.25)]"></div>
        <CodeOverlay id="code-right" label="Variant B" color="#35C838"
          rows={[
            { id: 'r-hook', old: '"I used to think"', new: '"POV: you found it"', cls: 'blue' },
            { id: 'r-clr', old: '"#f5a623"', new: '"#35C838"', cls: 'blue' },
            { id: 'r-clip', old: '"emma-v1.mp4"', new: '"emma-v3.mp4"', cls: 'blue' },
          ]} />
      </GlassCard>
    </div>
  );
}

/* =========== Scene 05 -- code type-on (abs 23.2-29.2 s) =====================
   Reference: scene-05.html (duration 6). Two headline beats, then the
   data-composition-variables JSON types on at 50 chars/sec with a blinking
   cursor, then the tagline, then a zoom-through exit.
   The reference builds the code from per-character DOM nodes and sets
   display at typeStart + j*0.02; the engine has no per-char layout, so the
   characters become one text node per rendered row and the row's reveal time
   is derived from the reference's char counts (measure the widths with the
   mono face so the rows keep the reference's monospace column grid). */
const CODE_LINE_TEXT = [
  'data-composition-variables=\'[',
  '  { id: "hookText", type: "string" },',
  '  { id: "accentColor", type: "color" },',
  '  { id: "creatorClip", type: "string" }',
  ']\'',
];
const CODE_TYPE_START = 2.15;
const CODE_CHAR_RATE = 0.02;

function Scene05() {
  useGSAP(() => {
    var m = makeMeasure(ctx);
    var T = gsap.timeline();

    // Beat 1a / 1b: cut-the-curve entry from the right, matching scene-04's
    // leftward exit.
    T.fromTo('h1', { x: 200, filter: 'blur(8px)', opacity: 0 },
      { x: 0, filter: 'blur(0px)', opacity: 1, duration: 0.42, ease: 'expo.out' }, 0);
    T.to('h1', { x: -200, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 0.72);
    T.fromTo('h2', { x: 200, filter: 'blur(8px)', opacity: 0 },
      { x: 0, filter: 'blur(0px)', opacity: 1, duration: 0.42, ease: 'expo.out' }, 1.05);
    T.to('h2', { scale: 1.2, filter: 'blur(20px)', opacity: 0.15, duration: 0.2, ease: 'power3.in' }, 1.77);

    // Beat 2: the code block zoom-through entry, then type on.
    T.fromTo('code-block', { scale: 0.75, filter: 'blur(20px)', opacity: 0 },
      { scale: 1, filter: 'blur(0px)', opacity: 1, duration: 0.5, ease: 'expo.out' }, 1.97);
    T.to('code-cursor', { opacity: 1, duration: 0.01 }, 2.1);
    for (var b = 2.1; b < 5.8; b += 0.5) {
      T.to('code-cursor', { opacity: 0, duration: 0.01 }, b + 0.25);
      T.to('code-cursor', { opacity: 1, duration: 0.01 }, b + 0.5);
    }

    // Type-on. The reference reveals one DOM span per character; here each
    // row is one text node, revealed when its first character's turn comes.
    var acc = 0;
    for (var i = 0; i < CODE_LINE_TEXT.length; i++) {
      var rowStart = CODE_TYPE_START + acc * CODE_CHAR_RATE;
      T.set('code-line-' + i, { opacity: 1 }, rowStart);
      acc += CODE_LINE_TEXT[i].length;
    }
    var typeEnd = CODE_TYPE_START + acc * CODE_CHAR_RATE;
    T.to('s5-tagline', { opacity: 1, duration: 0.3, ease: 'power2.out' }, typeEnd + 0.2);
    T.to('code-block', { scale: 1.2, filter: 'blur(20px)', opacity: 0.15, duration: 0.2, ease: 'power3.in' }, 5.8);
  });
  return (
    <div id="scene-05" class="absolute inset-0 bg-[#ffffff]" duration="6.0">
      <div id="h1" class="absolute left-0 top-[240px] w-[1920px] text-center font-[inter800] font-extrabold text-[94px] tracking-[-2.82px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap opacity-0">
        <text class="text-[#09B2E5]">Any</text>{' composition.'}
      </div>
      <div id="h2" class="absolute left-0 top-[360px] w-[1920px] text-center font-[inter800] font-extrabold text-[94px] tracking-[-2.82px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap opacity-0">
        {'Now a '}<text class="text-[#F3A6FF]">template.</text>
      </div>
      <div id="code-block" class="absolute left-0 top-[200px] w-[1920px] flex flex-col items-center opacity-0">
        <div class="font-[mono500] text-[48px] leading-[2.0] text-[rgba(0,0,0,0.55)] whitespace-nowrap">
          {CODE_LINE_TEXT.map((t, i) => (
            <div key={t} id={'code-line-' + i} class="opacity-0">{t}</div>
          ))}
        </div>
        <div id="code-cursor" class="w-[2.5px] h-[52.8px] bg-[#0a0a0a] opacity-0" />
        <div id="s5-tagline" class="font-[inter800] font-extrabold text-[85px] tracking-[-1.7px] text-[#0a0a0a] opacity-0 mt-[56px]">Infinite variations.</div>
      </div>
    </div>
  );
}

/* =========== Scene 06 -- 3x3 grid of variants (abs 29.2-32.7 s) ============
   Reference: scene-06.html (duration 3.5). The mid row (A/C/B, the same trio
   from scene-04) is visible first; the top row slides up and the bottom row
   slides down into place, then the whole grid exits left.
   Geometry: a 3-column grid of 281px cards, gap 10, rows of 500px. Column x =
   80 + (1920-160-3*281-2*10)/2 offsets -- the grid is centred: total grid
   width = 3*281 + 2*10 = 863, so x0 = (1920-863)/2 = 528.5. Rows are centred
   vertically: 3*500 + 2*10 = 1520 > 1080, so the reference's flex centre
   overflows equally; row y = 40 - 220.6667/2 -> we keep the reference's own
   visual centring by top = 40 + row*(510). */
const GRID_X0 = 528.5;
const GRID_ROW_H = 500;
const GRID_GAP = 10;
const GRID_CELL = 281;

const GRID_ROWS = [
  ['hook-d', 'hook-e', 'hook-f'],
  ['hook-a', 'hook-c', 'hook-b'],
  ['hook-g', 'hook-h', 'hook-i'],
];
const GRID_ROW_CLASS = ['top', 'mid', 'bot'];

function GridCard({ id, path }) {
  return (
    <div id={id} class="absolute left-0 top-0 w-[281px] h-[500px] rounded-[16px] overflow-hidden border-[1px] border-[rgba(0,0,0,0.08)] bg-[linear-gradient(165deg,rgba(0,0,0,0.04)_0%,rgba(0,0,0,0.02)_40%,rgba(0,0,0,0.01)_100%)] shadow-[0_4px_16px_rgba(0,0,0,0.04),0_1px_4px_rgba(0,0,0,0.03)]">
      <video path={path} data-start="0" data-duration="3.5" data-media-start="0" class="absolute inset-0 w-full h-full object-cover rounded-[15px]" />
      <div class="absolute left-0 top-[-150px] w-[393px] h-[250px] bg-[linear-gradient(180deg,rgba(255,255,255,0.35)_0%,rgba(255,255,255,0)_100%)]"></div>
    </div>
  );
}

function Scene06() {
  useGSAP(() => {
    var T = gsap.timeline();
    var slideY = GRID_ROW_H + GRID_GAP;

    // Zoom-through entry from scene-05
    T.fromTo('scene-06', { scale: 0.75, filter: 'blur(20px)', opacity: 0.15 },
      { scale: 1, filter: 'blur(0px)', opacity: 1, duration: 0.5, ease: 'expo.out' }, 0);

    var fanStart = 0.6;
    // Place every card from the 3x3 grid geometry, then start the top row one
    // row down and the bottom row one row up (hidden until the fan-out).
    for (var r = 0; r < 3; r++) {
      for (var c = 0; c < 3; c++) {
        ctx.getNode('c' + c + '-' + GRID_ROW_CLASS[r])
          .left(GRID_X0 + c * (GRID_CELL + GRID_GAP))
          .top(40 + r * (GRID_ROW_H + GRID_GAP));
      }
    }
    for (var t = 0; t < 3; t++) {
      ctx.getNode('c' + t + '-top').y(slideY).opacity(0);
      ctx.getNode('c' + t + '-bot').y(-slideY).opacity(0);
    }
    // Top row slides up, staggered per column
    T.to('c0-top', { y: 0, opacity: 1, duration: 0.5, ease: 'power3.out' }, fanStart);
    T.to('c1-top', { y: 0, opacity: 1, duration: 0.5, ease: 'power3.out' }, fanStart + 0.06);
    T.to('c2-top', { y: 0, opacity: 1, duration: 0.5, ease: 'power3.out' }, fanStart + 0.12);
    // Bottom row slides down
    T.to('c0-bot', { y: 0, opacity: 1, duration: 0.5, ease: 'power3.out' }, fanStart + 0.08);
    T.to('c1-bot', { y: 0, opacity: 1, duration: 0.5, ease: 'power3.out' }, fanStart + 0.14);
    T.to('c2-bot', { y: 0, opacity: 1, duration: 0.5, ease: 'power3.out' }, fanStart + 0.20);

    // Cut-the-curve exit: the whole grid slides left with an 8px blur
    T.to('grid', { x: -200, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 3.5 - 0.33);
  });
  return (
    <div id="scene-06" class="absolute inset-0 bg-[#ffffff]" duration="3.5">
      <div id="grid" class="absolute inset-0">
        {GRID_ROWS.map((row, r) => (
          row.map((clip, c) => (
            <GridCard key={clip} id={'c' + c + '-' + GRID_ROW_CLASS[r]} path={'assets/hooks/' + clip + '.mp4'} />
          ))
        ))}
      </div>
    </div>
  );
}

/* =========== Scene 07 -- word masks + cloud (abs 32.7-36.5 s) ==============
   Reference: scene-07.html (duration 3.8). Five words sit in fixed-height
   masks and rise into place; then AWS slides up out of its mask and the cloud
   emoji slides up into that slot. The reference positions the emoji from the
   live DOM; here the mask boxes come from the flex-row geometry of the same
   measure surface.
   The engine's text clip mask is not available, so the words are plain text
   nodes and the reveal is the reference's own y slide -- same visible motion,
   since every word is fully opaque at y=0 and fully hidden above the mask. */
const S7_WORDS = [
  { id: 'w0', text: 'Render', color: '#0a0a0a' },
  { id: 'w1', text: 'on', color: '#0a0a0a' },
  { id: 'w2', text: 'AWS', color: '#09B2E5' },
  { id: 'w3', text: 'at', color: '#0a0a0a' },
  { id: 'w4', text: 'scale', color: '#0a0a0a' },
];

function Scene07() {
  useGSAP(() => {
    var m = makeMeasure(ctx);
    var T = gsap.timeline();

    // Flex row with 24px gaps: word widths from the mono-less Inter 800 104px.
    var ws = [];
    for (var i = 0; i < S7_WORDS.length; i++) ws.push(m.natW(INTER, 800, 104, -3.12, S7_WORDS[i].text));
    var total = 0;
    for (var s = 0; s < ws.length; s++) total += ws[s];
    total += 24 * (ws.length - 1);
    var acc = (1920 - total) / 2;
    for (var p = 0; p < S7_WORDS.length; p++) {
      ctx.getNode(S7_WORDS[p].id).left(acc).top(425);
      acc += ws[p] + 24;
    }
    // The emoji centres under AWS (the third word): re-walk to its row origin.
    var acc2 = (1920 - total) / 2;
    for (var q = 0; q < S7_WORDS.length; q++) {
      if (q === 2) ctx.getNode('cloud-emoji').left(acc2 + ws[q] / 2).top(490);
      acc2 += ws[q] + 24;
    }

    var stagger = 0.08;
    for (var r = 0; r < S7_WORDS.length; r++) {
      T.fromTo(S7_WORDS[r].id, { y: 150 }, { y: 0, duration: 0.45, ease: 'power3.out' }, 0.15 + r * stagger);
    }

    var revealEnd = 0.15 + 4 * stagger + 0.45;
    var swapStart = revealEnd + 1.0;
    T.fromTo('cloud-emoji', { y: 150, opacity: 0 },
      { y: 0, opacity: 1, duration: 0.4, ease: 'power3.out' }, swapStart + 0.1);

    // Upward cut-the-curve exit
    T.to('scene-07', { y: -200, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 3.8 - 0.33);
  });
  return (
    <div id="scene-07" class="absolute inset-0 bg-[#ffffff]" duration="3.8">
      {S7_WORDS.map((w) => (
        <text key={w.id} id={w.id} class={'absolute left-0 top-0 font-[inter800] font-extrabold text-[104px] tracking-[-3.12px] leading-[120.64px] whitespace-nowrap text-[' + w.color + '] opacity-0'}>{w.text}</text>
      ))}
      <text id="cloud-emoji" class="absolute left-0 top-0 font-[inter800] text-[170px] leading-[1.16] whitespace-nowrap opacity-0">&nbsp;</text>
    </div>
  );
}

/* =========== Scene 08 -- render queue progress (abs 36.5-40.0 s) ============
   Reference: scene-08.html (duration 3.5). Fifty render rows fill their
   progress bars with staggered starts and per-row speeds; the percentages
   scroll alongside; the whole list scrolls up 1200px across the scene and
   exits up-left with a blur.
   Geometry: rows are 900px wide at x 510, 14px gaps, 52px tall, first at
   y 80. The bar track spans the middle of each row. */
const S8_ROWS = 50;
const S8_TRACK_X = 510 + 240;
const S8_TRACK_W = 620;
const S8_FILLS = ['#00C3FF', '#F3A6FF', '#35C838'];

/* One digit column of a percentage readout. The engine has no runtime text
   mutation, so a digit is a vertical strip of 0..9 stacked inside a clipping
   window; sliding the strip with one `y` tween rolls the digit, exactly the
   mechanical odometer the reference's `pctEl.textContent` update produces. */
/* Scene 08 renders 50 identical rows; the reference builds them in a loop.
   The engine's class attribute is static, so each row keeps its per-index
   colour as the only static difference and every varying coordinate is
   written by the scene's script (top, the digit rolls, the fill width). */
function DigitRoll({ id }) {
  return (
    <div id={id} class="absolute left-0 top-0 w-[13px] h-[21.6px] overflow-hidden">
      <div class="absolute left-0 top-0 flex flex-col">
        {[0, 1, 2, 3, 4, 5, 6, 7, 8, 9].map((d) => (
          <text key={d} class="font-[mono500] font-semibold text-[18px] leading-[21.6px] text-[rgba(0,0,0,0.35)]">{d}</text>
        ))}
      </div>
    </div>
  );
}

function PctReadout({ index }) {
  return (
    <div class="absolute left-[860px] top-[16px] w-[60px] h-[21.6px]">
      <div class="relative w-[48px] h-[21.6px]">
        <DigitRoll id={'pct-h-' + index} />
        <DigitRoll id={'pct-t-' + index} />
        <DigitRoll id={'pct-u-' + index} />
      </div>
      <text class="absolute left-[48px] top-0 font-[mono500] font-semibold text-[18px] leading-[21.6px] text-[rgba(0,0,0,0.35)]">%</text>
    </div>
  );
}

const S8_ROW_FILL_CLASS = [
  'bg-[linear-gradient(90deg,#00C3FF,rgba(255,255,255,0.75))]',
  'bg-[linear-gradient(90deg,#F3A6FF,rgba(255,255,255,0.75))]',
  'bg-[linear-gradient(90deg,#35C838,rgba(255,255,255,0.75))]',
];

function RenderRow({ index }) {
  var num = String(index + 1).padStart(3, '0');
  return (
    <div id={'row-' + index} class="absolute left-[510px] top-[80px] w-[900px] h-[52px] opacity-0">
      <text class="absolute left-0 top-[14px] w-[220px] text-right font-[mono500] text-[20px] leading-[24px] text-[rgba(0,0,0,0.5)]">{'variant-' + num + '.mp4'}</text>
      <div class="absolute left-[240px] top-[8px] w-[620px] h-[36px] rounded-[8px] bg-[rgba(0,0,0,0.04)] overflow-hidden">
        <div id={'fill-' + index} class={'absolute left-0 top-0 w-[0px] h-full rounded-[8px] ' + S8_ROW_FILL_CLASS[index % 3]} />
      </div>
      <PctReadout index={index} />
    </div>
  );
}

function Scene08() {
  useGSAP(() => {
    var T = gsap.timeline();

    // Zoom to 1.3x + upward cut-the-curve entry
    T.fromTo('scene-08', { scale: 1.3, y: 200, filter: 'blur(8px)', opacity: 0 },
      { y: 0, filter: 'blur(0px)', opacity: 1, duration: 0.42, ease: 'expo.out' }, 0);

    // Every row is already visible on entry
    var rows = [];
    for (var r = 0; r < S8_ROWS; r++) rows.push(r);
    for (var v = 0; v < rows.length; v++) ctx.getNode('row-' + rows[v]).opacity(1);

    // Fill the bars: staggered starts, cascade all the way down, with the
    // visible width derived from the row's own target percentage. The
    // percentage label rides the identical curve through the row's proxy bar
    // (scaleX on a full-width 0-100% strip clipped to the track), so a single
    // tween drives both the fill and the digits.
    for (var f = 0; f < S8_ROWS; f++) {
      var delay = f * 0.04;
      var speed = 1.5 + (f * 0.03);
      var targetPct = 100 - (f / (S8_ROWS - 1)) * 85;
      // width takes a bare number (px implied) — the '620px' string form
      // Number()-coerces to NaN inside the engine's value binding.
      T.to('fill-' + f, { width: (targetPct / 100) * S8_TRACK_W, duration: speed, ease: 'power1.out' }, delay);
      // Odometer digits: target percentage p -> hundreds digit rolls 0..p/100
      // and the tens/ones columns roll 0..p, all on the same curve so the
      // readout matches the fill frame for frame.
      var dp = Math.round(targetPct);
      // Each column rolls only as far as its own digit range: hundreds 0..1,
      // tens 0..9 (wrapping once per ten), ones 0..99 on the 0..9 strip plus a
      // second pass -- the strip holds 0..9, so the ones column rolls
      // floor(dp/10) full turns and lands on dp % 10.
      T.to('pct-h-' + f, { y: -Math.floor(dp / 100) * 21.6, duration: speed, ease: 'power1.out' }, delay);
      T.to('pct-t-' + f, { y: -(Math.floor(dp / 10) % 10) * 21.6, duration: speed, ease: 'power1.out' }, delay);
      T.to('pct-u-' + f, { y: -(dp % 10) * 21.6, duration: speed, ease: 'power1.out' }, delay);
    }

    // Scroll the list up across the scene
    T.to('render-list', { y: -1200, duration: 3.2, ease: 'power1.in' }, 0);

    // Upward cut-the-curve exit
    T.to('render-list', { y: -1600, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 3.5 - 0.33);
  });
  return (
    <div id="scene-08" class="absolute inset-0 bg-[#ffffff]" duration="3.5">
      <div id="render-list" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
        {Array.from({ length: S8_ROWS }, (_, i) => <RenderRow key={i} index={i} />)}
      </div>
    </div>
  );
}

/* =========== Scene 09 -- the close (abs 40.0-43.6 s) ======================
   Reference: scene-09.html (duration 3.6). Two callout lines, then the
   HyperFrames logo slides up into place. The reference's logo is the SVG
   rasterised to a PNG (the engine's <path> has no gradient fill support), at
   the reference's 1440px width. */
function Scene09() {
  useGSAP(() => {
    var m = makeMeasure(ctx);
    var T = gsap.timeline();

    var m1 = m.natW(INTER, 800, 94, -2.82, 'Scale your best work.');
    var m2 = m.natW(INTER, 800, 94, -2.82, 'Not your workload.');
    ctx.getNode('line-a').left((1920 - m1) / 2).top(386);
    ctx.getNode('line-b').left((1920 - m2) / 2).top(536);

    // Line A: upward cut-the-curve entry, then out to the left
    T.fromTo('line-a', { y: 200, filter: 'blur(8px)', opacity: 0 },
      { y: 0, filter: 'blur(0px)', opacity: 1, duration: 0.42, ease: 'expo.out' }, 0.15);
    T.to('line-a', { x: -60, opacity: 0, duration: 0.35, ease: 'power2.in' }, 1.2);
    // Line B: slide in from the right
    T.fromTo('line-b', { x: 60, opacity: 0 }, { x: 0, opacity: 1, duration: 0.4, ease: 'power2.out' }, 1.8);
    T.to('line-b', { y: -200, filter: 'blur(8px)', opacity: 0, duration: 0.33, ease: 'power2.in' }, 2.2);
    // Logo slides up from below
    T.fromTo('logo', { y: 200, filter: 'blur(8px)', opacity: 0 },
      { y: 0, filter: 'blur(0px)', opacity: 1, duration: 0.33, ease: 'expo.out' }, 2.53);
  });
  return (
    <div id="scene-09" class="absolute inset-0 bg-[#ffffff]" duration="3.6">
      <text id="line-a" class="absolute font-[inter800] font-extrabold text-[94px] tracking-[-2.82px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap opacity-0"><text class="text-[#35C838]">Scale</text>{' your best work.'}</text>
      <text id="line-b" class="absolute font-[inter800] font-extrabold text-[94px] tracking-[-2.82px] leading-[1.16] text-[#0a0a0a] whitespace-nowrap opacity-0">{'Not your '}<text class="text-[#F3A6FF]">workload.</text></text>
      <image id="logo" path="assets/hyperframes-dark.png" class="absolute left-[240px] top-[690px] w-[1440px] h-[432px] object-contain opacity-0" />
    </div>
  );
}

/* Root: the reference stacks its nine sub-compositions in one root and swaps
   them at hard cuts, so here the scenes become tl sequences. Every cut in the
   reference is a hard cut -- each scene animates its own exit/entry inside its
   own GSAP timeline -- so each tl transition is a one-frame fade that only
   forwards the exact cut moment (duration 0 is rejected by the parser). */
export default function Root() {
  return (
    <div id="root" class="relative w-[1920px] h-[1080px] bg-[#ffffff] overflow-hidden">
      <tl id="main-tl" class="absolute inset-0">
        <Scene01 />
        <transition from="scene-01" to="scene-02" effect="fade" duration="0.0333" timing="linear" />
        <Scene02 />
        <transition from="scene-02" to="scene-03" effect="fade" duration="0.0333" timing="linear" />
        <Scene03 />
        <transition from="scene-03" to="scene-04" effect="fade" duration="0.0333" timing="linear" />
        <Scene04 />
        <transition from="scene-04" to="scene-05" effect="fade" duration="0.0333" timing="linear" />
        <Scene05 />
        <transition from="scene-05" to="scene-06" effect="fade" duration="0.0333" timing="linear" />
        <Scene06 />
        <transition from="scene-06" to="scene-07" effect="fade" duration="0.0333" timing="linear" />
        <Scene07 />
        <transition from="scene-07" to="scene-08" effect="fade" duration="0.0333" timing="linear" />
        <Scene08 />
        <transition from="scene-08" to="scene-09" effect="fade" duration="0.0333" timing="linear" />
        <Scene09 />
      </tl>
    </div>
  );
}
