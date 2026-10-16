// JSX port of hyperframes-launches/frame-md-launch-storyboard — 8 scenes +
// quiet captions on one 1920x1080 stage, every scene gated to its index.html
// window (opacity) and driven by a per-frame ctx.timeline() with absolute
// seconds (scene-local reference time + scene start).
//
// Font mapping (verified against reference frames f10.5/f28.5/f42.8/f49.6/f56.5):
//   * the reference renderer falls back every `ui-monospace` stack to the sans
//     face → all md/doc text renders as Inter here;
//   * the "Cooper Black"/"Arial Rounded MT Bold" display stack renders as a
//     heavy rounded face → Nunito Black;
//   * scene-01's token is JetBrains Mono (ch = 0.6em for the typewriter widths).
// Scene-03's wipe / handoff-chip are gsap.set()-only in the reference (never
// animated in) so they stay invisible and are omitted.

import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 60.254 };

export const fonts = {
  default: 'fl-sans',
  faces: [
    { id: 'fl-sans', family: 'Inter', path: 'assets/fonts/Inter-Regular.ttf' },
    { id: 'fl-sans-med', family: 'Inter', path: 'assets/fonts/Inter-Medium.ttf' },
    { id: 'fl-sans-bold', family: 'Inter', path: 'assets/fonts/Inter-Bold.ttf' },
    { id: 'fl-sans-black', family: 'Inter', path: 'assets/fonts/Inter-Black.ttf' },
    { id: 'fl-mono', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Regular.ttf', role: 'mono' },
    { id: 'fl-mono-bold', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Bold.ttf', role: 'mono' },
    { id: 'fl-hand', family: 'Just Another Hand', path: 'assets/fonts/JustAnotherHand.ttf' },
    { id: 'fl-round', family: 'Nunito', path: 'assets/fonts/Nunito-Black.ttf' },
  ],
};

// Scene windows from index.html (start, duration).
var S1 = 0, D1 = 4;
var S2 = 4, D2 = 12.36;
var S3 = 16.36, D3 = 11;
var S4 = 26.96, D4 = 5.24;
var S5 = 29.2, D5 = 13.184;
var S6 = 41.624, D6 = 7.25;
var S7 = 48.234, D7 = 4.15;
var S8 = 52.034, D8 = 8.22;
var SC = 4, DC = 56.254;

function vis(ctx, id, on) {
  ctx.getNode(id).opacity(on ? 1 : 0);
}

// 1x1 measure surface (get-or-create, never renders pixels).
function makeMeasure(ctx) {
  var MSR = ly.surface(1, 1);
  function w(family, weight, size, text, ls) {
    MSR.font = weight + ' ' + size + 'px "' + family + '"';
    MSR.letterSpacing = (ls || 0) + 'px';
    return MSR.measureText(text).width;
  }
  return { w: w };
}

// ── shared slide (scene-02 2G furniture == scene-03 beat-a still) ─────────
// kicker / hero(2 lines, "videos" underlined) / sub / brand / page number.
function SlideFurniture(p) {
  var pre = p.pre;
  var hide = p.hide ? ' opacity-0' : '';
  return (
    <div>
      <text id={pre + 'kicker'} class="absolute left-[190px] top-[300px] font-[fl-sans-bold] font-[700] text-[28px] leading-none tracking-[6.2px] text-[#6e6e74] whitespace-nowrap">TRANSLATED FOR VIDEO</text>
      <div id={pre + 'hero'} class={'absolute left-[190px] top-[400px] font-[fl-sans-bold] font-[700] text-[116px] leading-[1.02] tracking-[-4.6px] text-[#ffffff]' + hide}>
        <text id={pre + 'hero-l1'} class="whitespace-nowrap">Interfaces,</text>
        <div class="flex flex-row items-start">
          <text class="whitespace-nowrap">{'not '}</text>
          <div class="relative">
            <text class="whitespace-nowrap">videos</text>
            <div id={pre + 'hero-u'} class="absolute left-[0px] bottom-[6px] w-full h-[9px] bg-[#ffffff]"></div>
          </div>
        </div>
      </div>
      <text id={pre + 'sub'} class={'absolute left-[190px] top-[660px] w-[1100px] font-[fl-sans] text-[38px] leading-[1.4] text-[#a0a0a6]' + hide}>Hand an agent a screen-design spec and the brand arranges itself like slides — not frames.</text>
      <text id={pre + 'brand'} class={'absolute left-[190px] top-[952px] font-[fl-sans-bold] font-[700] text-[30px] leading-none text-[#ffffff] whitespace-nowrap' + hide}>DESIGN.md</text>
      <text id={pre + 'page'} class={'absolute right-[190px] bottom-[96px] font-[fl-sans-bold] font-[700] text-[26px] leading-none text-[#7a7a80] whitespace-nowrap' + hide}>01</text>
    </div>
  );
}

// clay blob: radial highlight sphere.
function Blob(p) {
  var grads = {
    pink: '#ffc1d5,#ff4d8b_56%,#cb1c5f',
    peach: '#ffd9bf,#ffb084_58%,#db7a4e',
    ochre: '#ffe9a3,#e8b94a_58%,#a87b13',
    lav: '#e9dcff,#b8a4ed_58%,#7f62c8',
    mint: '#e4fff5,#a4d4c5_58%,#6aa896',
  };
  var base = 'absolute rounded-[999px] saturate-[1.08] shadow-[inset_-21px_-27px_38px_rgba(0,0,0,0.16),inset_21px_23px_31px_rgba(255,255,255,0.42)] bg-[radial-gradient(circle_at_32%_28%,' + grads[p.c] + ')]';
  if (p.op !== undefined) base += ' opacity-[' + p.op + ']';
  return <div id={p.id} class={base + ' ' + p.pos + ' w-[' + p.s + 'px] h-[' + p.s + 'px]'}></div>;
}

/* ════════════ SCENE 01 — typewriter hook (0 → 4s) ════════════ */
function Scene1() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S1 && ctx.global.time < S1 + D1;
    vis(ctx, 's1-root', on);
    if (!on) return;
    var tl = ctx.timeline({ delay: S1 });
    // ch = 0.6em @ 122px JetBrains Mono = 73.2px
    tl.set('#s1-fw', { width: 0 });
    tl.set('#s1-dw', { width: 0 });
    tl.set('#s1-sw', { width: 0 });
    tl.set('#s1-caret', { x: 0, y: 3, opacity: 0 });
    tl.set('#s1-cursor', { x: 210, y: 1160, scale: 1, opacity: 0, transformOrigin: '8px 5px' });
    tl.set('#s1-stage', { x: 0 });
    tl.to('#s1-cursor', { opacity: 1, duration: 0.05, ease: 'none' }, 0);
    tl.to('#s1-cursor', { x: 960, y: 540, duration: 0.26, ease: 'power3.out' }, 0);
    tl.to('#s1-cursor', { scale: 0.82, duration: 0.05, ease: 'power4.in' }, 0.24);
    tl.set('#s1-caret', { opacity: 1 }, 0.29);
    tl.to('#s1-cursor', { scale: 1, duration: 0.1, ease: 'back.out(2)' }, 0.29);
    tl.to('#s1-cursor', { x: 1130, y: 665, duration: 0.18, ease: 'power2.out' }, 0.4);
    [0.46, 0.82, 1.18, 1.54, 1.9, 2.26, 2.62, 2.98, 3.34, 3.7].forEach(function (time) {
      tl.to('#s1-caret', { opacity: 0.14, duration: 0.04, ease: 'none' }, time);
      tl.to('#s1-caret', { opacity: 1, duration: 0.04, ease: 'none' }, time + 0.18);
    });
    tl.to('#s1-dw', { width: 439.2, duration: 0.27, ease: 'steps(6)' }, 0.7);
    tl.to('#s1-sw', { width: 219.6, duration: 0.13, ease: 'steps(3)' }, 0.97);
    tl.to('#s1-cursor', { x: 1044, y: 540, duration: 0.32, ease: 'power2.inOut' }, 1.66);
    tl.to('#s1-cursor', { scale: 0.82, duration: 0.05, ease: 'power4.in' }, 1.98);
    tl.set('#s1-caret', { x: -256 }, 2.02);
    tl.to('#s1-cursor', { scale: 1, duration: 0.1, ease: 'back.out(2)' }, 2.03);
    tl.to('#s1-cursor', { opacity: 0, duration: 0.12, ease: 'none' }, 2.14);
    tl.to('#s1-dw', { width: 0, duration: 0.3, ease: 'steps(6)' }, 2.4);
    tl.set('#s1-caret', { x: -218 }, 2.7);
    tl.to('#s1-fw', { width: 347.7, duration: 0.32, ease: 'steps(5)' }, 2.7);
    tl.to('#s1-stage', { x: -1920, duration: 0.18, ease: 'power4.in' }, 3.82);
  });
  return (
    <div id="s1-root" class="absolute inset-0 overflow-hidden bg-[#f7f7f5] z-[11]">
      <div id="s1-stage" class="absolute inset-0 flex flex-row items-center justify-center bg-[#f7f7f5]">
        <div id="s1-token" class="flex flex-row items-center justify-center font-[fl-mono-bold] text-[122px] leading-none text-[#050505]">
          <div id="s1-fw" class="w-[0px] overflow-hidden whitespace-nowrap"><text>FRAME</text></div>
          <div id="s1-dw" class="w-[0px] overflow-hidden whitespace-nowrap"><text>DESIGN</text></div>
          <div id="s1-sw" class="w-[0px] overflow-hidden whitespace-nowrap"><text>.md</text></div>
          <div id="s1-caret" class="w-[8px] h-[116px] ml-[20px] rounded-[4px] bg-[#050505]"></div>
        </div>
        <div id="s1-cursor" class="absolute left-[0px] top-[0px] w-[92px] h-[112px] opacity-0">
          <path class="w-full h-full fill-[#050505] stroke-[#ffffff] stroke-2" d="M2.5 1.5L2.5 28L8.5 21.5L14 32L18.5 30L13 19.5L22 19.5L2.5 1.5Z" view-box="0 0 28 34" />
        </div>
      </div>
    </div>
  );
}

/* ════════════ SCENE 02 — diagnosis (4 → 16.36s) ════════════ */
// doc lines of the mz panel, hand-laid from the CSS block flow
// (panel content origin = panel + padding 56/52 → local (52,56)).
function MzDoc() {
  var CL = [
    { c: '#1ee6ff', hex: '#060606', d: ' deep black surfaces' },
    { c: '#ff4040', hex: '#ff3b5f', d: ' caption accent glow' },
    { c: '#61e675', hex: '#61e675', d: ' local terminal green' },
    { c: '#ff9b2f', hex: '#ff9b2f', d: ' punch/highlight red' },
  ];
  return (
    <div id="s2-mz" class="absolute left-[52px] top-[56px] w-[696px] font-[fl-sans] text-[#f1f1f1]">
      <text id="s2-mzh1" class="absolute left-[0px] top-[0px] font-[fl-sans-med] font-[500] text-[56px] leading-[1.1] whitespace-nowrap">DESIGN.md</text>
      <text id="s2-mzh2" class="absolute left-[0px] top-[85.1px] font-[fl-sans-med] font-[500] text-[34px] leading-[1.15] whitespace-nowrap">## Style Prompt</text>
      <div id="s2-scroll" class="absolute left-[0px] top-[141.2px] w-[696px]">
        <text id="s2-p1" class="absolute left-[0px] top-[0px] w-[486px] text-[27px] leading-[1.55]">Horizontal launch video built around local background removal and maximal caption energy.</text>
        <text id="s2-p2" class="absolute left-[0px] top-[152.55px] w-[486px] text-[27px] leading-[1.55]">Use a dark terminal surface, crisp white type, and confident motion cues.</text>
        <text id="s2-h2c" class="absolute left-[0px] top-[298.95px] font-[fl-sans-med] font-[500] text-[34px] leading-[1.15] whitespace-nowrap">## Colors</text>
        {CL.map(function (l, i) {
          return (
            <div id={'s2-cl' + i} class={'absolute left-[0px] top-[' + (361 + i * 53.73) + 'px] flex flex-row items-center font-[fl-sans] text-[27px] leading-[1.55]'}>
              <text class="whitespace-nowrap">{'- '}</text>
              <text class={'whitespace-nowrap text-[' + l.c + ']'}>{l.hex}</text>
              <text class="whitespace-nowrap">{' ' + l.d}</text>
            </div>
          );
        })}
        <text id="s2-h2t" class="absolute left-[0px] top-[605.66px] font-[fl-sans-med] font-[500] text-[34px] leading-[1.15] whitespace-nowrap">## Typography</text>
        <text id="s2-p3" class="absolute left-[0px] top-[661.76px] w-[486px] text-[27px] leading-[1.55]">Inter / system sans, heavy weights. SF Mono for terminal and spec surfaces.</text>
        <text id="s2-p4" class="absolute left-[0px] top-[772.46px] w-[486px] text-[27px] leading-[1.55]">Specialty caption moments may borrow self, stroke, glow, pill, and texture treatments.</text>
        <text id="s2-h2k" class="absolute left-[0px] top-[918.86px] font-[fl-sans-med] font-[500] text-[34px] leading-[1.15] whitespace-nowrap">## Components</text>
        <text id="s2-p5" class="absolute left-[0px] top-[974.96px] w-[486px] text-[27px] leading-[1.55]">Cards, badges, nav bars, feature grids, buttons, panels.</text>
        <text id="s2-p6" class="absolute left-[0px] top-[1085.66px] w-[486px] text-[27px] leading-[1.55]">Favor web-interface primitives when describing pages and product surfaces.</text>
      </div>
    </div>
  );
}

// one hook word (id + text); positions come from a centered flex row per line.
function HookWord(p) {
  return <text id={p.id} class="whitespace-nowrap opacity-0">{p.t}</text>;
}

function Scene2() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S2 && ctx.global.time < S2 + D2;
    vis(ctx, 's2-root', on);
    if (!on) return;
    var t = ctx.global.time - S2;
    var tl = ctx.timeline({ delay: S2 });
    var Z = { x: -1505, y: 162, scale: 3.5 };
    tl.set('#s2-cam', { x: Z.x, y: Z.y, scale: Z.scale, transformOrigin: '0px 0px', transformPerspective: 2400 });
    tl.set('#s2-panel', { opacity: 1, scale: 1, y: 310, rotationY: 0, transformOrigin: '50% 50%', transformPerspective: 2400 });
    tl.set('#s2-mz', { opacity: 1, blur: 0, y: 0 });
    tl.set('#s2-mzh1', { opacity: 0, y: 24 });
    tl.set('#s2-mzh2', { opacity: 0, y: 24 });
    tl.set('#s2-scroll', { opacity: 0, y: 0 });
    tl.set('#s2-anno', { opacity: 1 });
    tl.set('#s2-aw', { opacity: 0 });
    tl.set('#s2-af', { opacity: 0 });
    tl.set('#s2-ao', { opacity: 0 });
    tl.set('#s2-ar', { opacity: 0 });
    tl.set('#s2-apages', { opacity: 0 });
    ['#s2-chrome', '#s2-navmenu', '#s2-cta', '#s2-eyebrow', '#s2-subtext', '#s2-actions', '#s2-cards', '#s2-hero', '#s2-brand', '#s2g-kicker', '#s2g-sub', '#s2g-page'].forEach(function (id) {
      tl.set(id, { opacity: 0 });
    });
    tl.set('#s2-brand', { y: 16 });
    tl.set('#s2-eyebrow', { y: 16 });
    tl.set('#s2-navmenu', { y: 16 });
    tl.set('#s2-cta', { y: 16 });
    tl.set('#s2-subtext', { y: 16 });
    tl.set('#s2-actions', { y: 16 });
    tl.set('#s2-hero', { y: 40, scale: 1, transformOrigin: '0px 0px' });
    tl.set('#s2-c1', { opacity: 0, y: 24 });
    tl.set('#s2-c2', { opacity: 0, y: 24 });
    tl.set('#s2-c3', { opacity: 0, y: 24 });
    // initial hook words
    var i;
    for (i = 1; i <= 12; i++) tl.set('#s2-hw' + i, { opacity: 0, x: 980, blur: 8 });

    // 2A→2C modal build
    tl.to('#s2-panel', { y: 0, duration: 0.5, ease: 'power4.out' }, 1.5);
    var words = [
      ['s2-hw1', 0.25], ['s2-hw2', 0.31], ['s2-hw3', 0.44], ['s2-hw4', 0.63], ['s2-hw5', 0.78],
      ['s2-hw6', 1.06], ['s2-hw7', 1.18], ['s2-hw8', 1.43], ['s2-hw9', 1.84],
      ['s2-hw10', 2.33], ['s2-hw11', 2.67], ['s2-hw12', 2.71],
    ];
    words.forEach(function (wv) {
      tl.to('#' + wv[0], { opacity: 1, x: 0, blur: 0, duration: 0.3, ease: 'power4.out' }, wv[1]);
    });
    tl.to('#s2-hw1', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 1.02);
    tl.to('#s2-hw2', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 1.045);
    tl.to('#s2-hw3', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 1.07);
    tl.to('#s2-hw4', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 1.095);
    tl.to('#s2-hw5', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 1.12);
    tl.to('#s2-hw6', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 2.27);
    tl.to('#s2-hw7', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 2.295);
    tl.to('#s2-hw8', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 2.32);
    tl.to('#s2-hw9', { opacity: 0, x: -240, blur: 8, duration: 0.22, ease: 'power2.in' }, 2.345);
    tl.to('#s2-hw10', { opacity: 0, x: -240, blur: 8, duration: 0.24, ease: 'power2.in' }, 3.68);
    tl.to('#s2-hw11', { opacity: 0, x: -240, blur: 8, duration: 0.24, ease: 'power2.in' }, 3.705);
    tl.to('#s2-hw12', { opacity: 0, x: -240, blur: 8, duration: 0.24, ease: 'power2.in' }, 3.73);
    tl.to('#s2-mzh1', { opacity: 1, y: 0, duration: 0.4, ease: 'power3.out' }, 2.33);
    tl.to('#s2-mzh2', { opacity: 1, y: 0, duration: 0.4, ease: 'power3.out' }, 2.67);
    tl.to('#s2-scroll', { opacity: 1, duration: 0.4, ease: 'power2.out' }, 2.78);
    // annotation (reference draws it with a stroke-dash pass; fill fade matches
    // its post-4.34s state which is all this camera zoom ever shows)
    tl.to('#s2-aw', { opacity: 1, duration: 0.18, ease: 'power1.out' }, 4.34);
    tl.to('#s2-af', { opacity: 1, duration: 0.16, ease: 'power1.out' }, 4.52);
    tl.to('#s2-ao', { opacity: 1, duration: 0.16, ease: 'power1.out' }, 4.59);
    tl.to('#s2-ar', { opacity: 1, duration: 0.16, ease: 'power1.out' }, 4.66);
    tl.to('#s2-apages', { opacity: 1, duration: 0.2, ease: 'power1.out' }, 4.78);

    // 2C→2D zoom out
    tl.to('#s2-cam', { x: 0, y: 0, scale: 1, duration: 1.5, ease: 'power3.inOut' }, 5.69);
    tl.to('#s2-anno', { opacity: 0, duration: 0.45, ease: 'power1.in' }, 5.69);
    tl.to('#s2-scroll', { opacity: 1, duration: 0.5, ease: 'power2.out' }, 6.19);
    tl.to('#s2-scroll', { y: -310, duration: 0.65, ease: 'none' }, 7.22);

    // 2D→2F panel reshapes into the browser window
    tl.to('#s2-panel', { left: 190, top: 100, width: 1540, height: 880, borderRadius: 34, duration: 1.7, ease: 'power3.inOut' }, 7.77);
    tl.to('#s2-panel', { rotationY: -18, duration: 0.85, ease: 'power2.inOut' }, 7.77);
    tl.to('#s2-panel', { rotationY: 0, duration: 0.85, ease: 'power2.inOut' }, 8.62);
    tl.to('#s2-mz', { opacity: 0, blur: 7, duration: 0.6, ease: 'power2.in' }, 7.87);
    tl.to('#s2-chrome', { opacity: 1, duration: 0.45, ease: 'power2.out' }, 9.22);
    tl.to('#s2-brand', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.22);
    tl.to('#s2-eyebrow', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.27);
    tl.to('#s2-navmenu', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.33);
    tl.to('#s2-cta', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.39);
    tl.to('#s2-hero', { opacity: 1, y: 0, duration: 0.55, ease: 'expo.out' }, 9.32);
    tl.to('#s2-subtext', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.48);
    tl.to('#s2-actions', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.54);
    tl.to('#s2-c1', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.62);
    tl.to('#s2-c2', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.68);
    tl.to('#s2-c3', { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 9.74);

    // 2F→2G full-bleed slide
    tl.to('#s2-panel', { left: 0, top: 0, width: 1920, height: 1080, borderRadius: 0, duration: 1.0, ease: 'power3.inOut' }, 9.99);
    tl.to('#s2-chrome', { opacity: 0, duration: 0.45, ease: 'power2.in' }, 9.99);
    tl.to('#s2-navmenu', { opacity: 0, duration: 0.45, ease: 'power2.in' }, 9.99);
    tl.to('#s2-cta', { opacity: 0, duration: 0.45, ease: 'power2.in' }, 9.99);
    tl.to('#s2-subtext', { opacity: 0, duration: 0.45, ease: 'power2.in' }, 9.99);
    tl.to('#s2-actions', { opacity: 0, duration: 0.45, ease: 'power2.in' }, 9.99);
    tl.to('#s2-cards', { opacity: 0, duration: 0.45, ease: 'power2.in' }, 9.99);
    tl.to('#s2-eyebrow', { opacity: 0, duration: 0.45, ease: 'power2.in' }, 9.99);
    // fontSize 96→116 == scale 1.2083 about the box origin
    tl.to('#s2-hero', { left: 190, top: 400, scale: 1.2083333, duration: 1.0, ease: 'power3.inOut' }, 9.99);
    tl.to('#s2-brand', { left: 190, top: 952, scale: 0.9375, duration: 1.0, ease: 'power3.inOut' }, 9.99);
    tl.to('#s2g-kicker', { opacity: 1, duration: 0.5, ease: 'power2.out' }, 10.69);
    tl.to('#s2g-sub', { opacity: 1, duration: 0.5, ease: 'power2.out' }, 10.84);
    tl.to('#s2g-page', { opacity: 1, duration: 0.5, ease: 'power2.out' }, 10.94);

    // annotation geometry via measure (rotated hand-lettered margin notes)
    var m = makeMeasure(ctx);
    var ww = m.w('Just Another Hand', 400, 95, 'Written');
    tl.set('#s2-aw', { left: 525 - ww / 2, top: 180 - 47.5, rotation: -90, transformOrigin: '50% 50%' });
    tl.set('#s2-af', { left: 545 - 17, top: 69 - 28, rotation: -65, transformOrigin: '50% 50%' });
    tl.set('#s2-ao', { left: 565 - 17, top: 46 - 28, rotation: -42, transformOrigin: '50% 50%' });
    tl.set('#s2-ar', { left: 596 - 16, top: 40 - 28, rotation: 0, transformOrigin: '50% 50%' });
  });
  return (
    <div id="s2-root" class="absolute inset-0 overflow-hidden bg-[#f7f7f5] z-[1] text-[#f1f1f1]">
      {/* hook words — three stacked centered lines */}
      <div id="s2-hooks" class="absolute left-[0px] top-[190px] w-[1920px] h-[190px] z-[5] font-[fl-sans-black] font-[900] text-[143px] leading-none tracking-[-6.435px] text-[#000000]">
        <div class="absolute left-[0px] top-[0px] w-full flex flex-row justify-center gap-[37.2px]">
          <HookWord id="s2-hw1" t="When" /><HookWord id="s2-hw2" t="you" /><HookWord id="s2-hw3" t="ask" /><HookWord id="s2-hw4" t="an" /><HookWord id="s2-hw5" t="agent" />
        </div>
        <div class="absolute left-[0px] top-[0px] w-full flex flex-row justify-center gap-[37.2px]">
          <HookWord id="s2-hw6" t="to" /><HookWord id="s2-hw7" t="make" /><HookWord id="s2-hw8" t="motion" /><HookWord id="s2-hw9" t="graphics" />
        </div>
        <div class="absolute left-[0px] top-[0px] w-full flex flex-row justify-center gap-[37.2px]">
          <HookWord id="s2-hw10" t="from" /><HookWord id="s2-hw11" t="a" /><HookWord id="s2-hw12" t="design.md" />
        </div>
      </div>

      <div id="s2-cam" class="absolute inset-0 z-[1]">
        <div id="s2-panel" class="absolute left-[560px] top-[70px] w-[800px] h-[1000px] bg-[#000000] border-[1px] border-[#262629] rounded-[28px] pt-[56px] pb-[56px] pl-[52px] pr-[52px] overflow-hidden shadow-[0_40px_90px_rgba(0,0,0,0.35)]">
          <MzDoc />
        </div>
        <div id="s2-anno" class="absolute inset-0">
          <text id="s2-aw" class="absolute font-[fl-hand] text-[95px] leading-none text-[#000000] whitespace-nowrap">Written</text>
          <text id="s2-af" class="absolute font-[fl-sans-bold] font-[700] text-[56px] leading-none text-[#000000] whitespace-nowrap">f</text>
          <text id="s2-ao" class="absolute font-[fl-sans-bold] font-[700] text-[56px] leading-none text-[#000000] whitespace-nowrap">o</text>
          <text id="s2-ar" class="absolute font-[fl-sans-bold] font-[700] text-[56px] leading-none text-[#000000] whitespace-nowrap">r</text>
          <text id="s2-apages" class="absolute left-[628px] top-[13px] font-[fl-sans-bold] font-[700] text-[54px] leading-none text-[#000000] whitespace-nowrap">pages</text>
        </div>
      </div>

      {/* 2F web-UI overlays (frame coords) */}
      <div id="s2-chrome" class="absolute left-[190px] top-[100px] w-[1540px] h-[64px] bg-[#151518] border-b-[1px] border-b-[#232326] rounded-t-[34px] flex flex-row items-center px-[30px] gap-[22px] z-[2] opacity-0">
        <div class="flex flex-row gap-[13px]">
          <div class="w-[15px] h-[15px] rounded-[999px] bg-[#ff5f57]"></div>
          <div class="w-[15px] h-[15px] rounded-[999px] bg-[#ffbd2e]"></div>
          <div class="w-[15px] h-[15px] rounded-[999px] bg-[#28c840]"></div>
        </div>
        <div class="grow text-center font-[fl-mono] text-[22px] text-[#66666b]"><text class="whitespace-nowrap">brand.com</text></div>
      </div>
      <div id="s2-navmenu" class="absolute left-[1140px] top-[196px] flex flex-row gap-[38px] font-[fl-sans] text-[24px] text-[#9a9aa0] z-[2] opacity-0">
        <text class="whitespace-nowrap">Product</text><text class="whitespace-nowrap">Docs</text><text class="whitespace-nowrap">Pricing</text>
      </div>
      <div id="s2-cta" class="absolute left-[1488px] top-[178px] bg-[#ffffff] text-[#000000] rounded-[14px] px-[28px] py-[14px] font-[fl-sans-bold] font-[700] text-[24px] leading-none z-[2] opacity-0">
        <text class="whitespace-nowrap">Get started</text>
      </div>
      <text id="s2-eyebrow" class="absolute left-[270px] top-[332px] font-[fl-sans-bold] font-[700] text-[20px] leading-none tracking-[4px] text-[#6e6e74] whitespace-nowrap z-[2] opacity-0">PLATFORM</text>
      <text id="s2-subtext" class="absolute left-[270px] top-[624px] w-[600px] font-[fl-sans] text-[28px] leading-[1.45] text-[#a0a0a6] z-[2] opacity-0">Ship polished product surfaces with components your team already knows.</text>
      <div id="s2-actions" class="absolute left-[270px] top-[724px] flex flex-row gap-[22px] z-[2] opacity-0">
        <div class="bg-[#ffffff] text-[#000000] rounded-[14px] px-[34px] py-[18px] font-[fl-sans-bold] font-[700] text-[25px] leading-none"><text class="whitespace-nowrap">Get started</text></div>
        <div class="border-[1px] border-[#3a3a3f] text-[#e6e6ea] rounded-[14px] px-[34px] py-[18px] font-[fl-sans] text-[25px] leading-none"><text class="whitespace-nowrap">Read docs</text></div>
      </div>
      <div id="s2-cards" class="absolute left-[270px] top-[806px] w-[1380px] grid grid-cols-3 gap-[30px] z-[2] opacity-0">
        {[1, 2, 3].map(function (n) {
          return (
            <div id={'s2-c' + n} class="bg-[#151518] border-[1px] border-[#2a2a2e] rounded-[20px] p-[28px] h-[148px] opacity-0">
              <div class="w-[193.6px] h-[18px] bg-[#ffffff] rounded-[999px]"></div>
              <div class="w-[384px] h-[12px] bg-[#37373c] rounded-[999px] mt-[22px]"></div>
              <div class="w-[314.9px] h-[12px] bg-[#37373c] rounded-[999px] mt-[14px]"></div>
              <div class="w-[215px] h-[12px] bg-[#37373c] rounded-[999px] mt-[14px]"></div>
            </div>
          );
        })}
      </div>

      {/* persistent hero + brand */}
      <div id="s2-hero" class="absolute left-[270px] top-[380px] font-[fl-sans-bold] font-[700] text-[96px] leading-[1.02] tracking-[-2.88px] text-[#ffffff] z-[3] opacity-0">
        <text class="whitespace-nowrap">Interfaces,</text>
        <div class="flex flex-row items-start">
          <text class="whitespace-nowrap">not&nbsp;</text>
          <div class="relative">
            <text class="whitespace-nowrap">videos</text>
            <div id="s2-hero-u" class="absolute left-[0px] bottom-[5px] w-full h-[8px] bg-[#ffffff]"></div>
          </div>
        </div>
      </div>
      <text id="s2-brand" class="absolute left-[270px] top-[186px] font-[fl-sans-bold] font-[700] text-[32px] leading-none tracking-[-0.64px] text-[#ffffff] whitespace-nowrap z-[3] opacity-0">DESIGN.md</text>

      {/* 2G slide furniture */}
      <SlideFurniture pre="s2g-" hide />
    </div>
  );
}

/* ════════════ SCENE 03 — missing layer (16.36 → 27.36s) ════════════ */
function Scene3() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S3 && ctx.global.time < S3 + D3;
    vis(ctx, 's3-root', on);
    if (!on) return;
    var t = ctx.global.time - S3;
    var tl = ctx.timeline({ delay: S3 });
    tl.set('#s3-bb', { opacity: 0, scale: 0.94, blur: 18 });
    tl.set('#s3-bc', { opacity: 0, scale: 1.06, blur: 18 });
    tl.set('#s3-bd', { opacity: 0, scale: 0.94, blur: 18 });
    tl.set('#s3-be', { opacity: 0, x: 260, scale: 1, blur: 0 });
    tl.set('#s3-bf', { opacity: 0, scale: 0.92, blur: 16 });
    tl.set('#s3-bg', { opacity: 0, scale: 1.02, blur: 12 });
    tl.set('#s3-bloom', { opacity: 0, scale: 0.82 });
    tl.set('#s3-bb-line', { opacity: 0, scale: 0.74, blur: 18 });
    tl.set('#s3-bb-ghost', { opacity: 0, scale: 1.5, blur: 28 });
    tl.set('#s3-bb-lav', { opacity: 0, scale: 0.5 });
    tl.set('#s3-bb-ochre', { opacity: 0, scale: 0.5 });
    tl.set('#s3-grade', { opacity: 0 });
    tl.set('#s3-mark', { opacity: 0 });
    tl.set('#s3-giant', { opacity: 0, scale: 0.88, y: 34, rotation: -5, transformOrigin: '50% 50%' });
    tl.set('#s3-g3', { y: 14.5 });
    tl.set('#s3-dot', { opacity: 0, scale: 0.5, x: -120, y: 50 });
    tl.set('#s3-backcard', { opacity: 0, scale: 0.75, x: 130, y: 60, rotation: 14 });
    tl.set('#s3-ribl', { opacity: 0, x: -260, rotation: -16 });
    tl.set('#s3-ribr', { opacity: 0, x: 260, rotation: 18 });
    tl.set('#s3-bc-lav', { opacity: 0, scale: 0.55 });
    tl.set('#s3-bc-peach', { opacity: 0, scale: 0.55 });
    tl.set('#s3-cap', { opacity: 0, y: 34, scale: 0.92, rotation: -1 });
    tl.set('#s3-dcard', { opacity: 0, scale: 0.92, y: 30 });
    tl.set('#s3-drail', { opacity: 0, scaleX: 0.82, transformOrigin: '50% 50%' });
    tl.set('#s3-drailfill', { scaleX: 1, transformOrigin: '0px 50%' });
    tl.set('#s3-bd-peach', { opacity: 0, scale: 0.5 });
    tl.set('#s3-bd-mint', { opacity: 0, scale: 0.5 });
    tl.set('#s3-lanea', { opacity: 0, x: 520 });
    tl.set('#s3-laneb', { opacity: 0, x: 520 });
    tl.set('#s3-lanec', { opacity: 0, x: 520 });
    tl.set('#s3-pl1', { opacity: 0, x: 520, scaleX: 1, transformOrigin: '0px 50%' });
    tl.set('#s3-pl2', { opacity: 0, x: 520, scaleX: 1, transformOrigin: '0px 50%' });
    tl.set('#s3-pl3', { opacity: 0, x: 520, scaleX: 1, transformOrigin: '0px 50%' });
    tl.set('#s3-fsmall', { opacity: 0, y: 28 });
    tl.set('#s3-fword', { opacity: 0, scale: 0.58, y: 24 });
    tl.set('#s3-qframe', { opacity: 0 });
    tl.set('#s3-qwrap', { opacity: 0 });
    tl.set('#s3-bg-ochre', { opacity: 0 });
    tl.set('#s3-bg-pink', { opacity: 0 });

    var bloom = function (at, opacity, inScale, outScale, outAt) {
      tl.to('#s3-bloom', { opacity: opacity, scale: inScale, duration: 0.2, ease: 'power2.out' }, at);
      tl.to('#s3-bloom', { opacity: 0, scale: outScale, duration: 0.38, ease: 'power2.inOut' }, at + outAt);
    };

    tl.to('#s3-ba', { scale: 1.08, blur: 26, opacity: 0, duration: 0.68, ease: 'power3.inOut' }, 0.72);
    bloom(0.82, 0.98, 0.7, 1.34, 0.2);

    tl.to('#s3-bb', { opacity: 1, scale: 1, blur: 0, duration: 0.7, ease: 'power3.out' }, 0.96);
    tl.to('#s3-bb-ghost', { opacity: 1, scale: 1.22, blur: 16, duration: 0.68, ease: 'power3.out' }, 1.0);
    tl.to('#s3-bb-lav', { opacity: 1, scale: 1, duration: 0.55, ease: 'back.out(1.4)' }, 1.1);
    tl.to('#s3-bb-ochre', { opacity: 1, scale: 1, duration: 0.55, ease: 'back.out(1.4)' }, 1.18);
    tl.to('#s3-bb-line', { opacity: 1, scale: 1, blur: 0, duration: 0.78, ease: 'expo.out' }, 1.16);

    tl.to('#s3-bb-line', { scale: 1.92, opacity: 0, blur: 38, duration: 0.46, ease: 'power3.in' }, 2.72);
    tl.to('#s3-bb-ghost', { scale: 2.1, opacity: 0, blur: 42, duration: 0.44, ease: 'power3.in' }, 2.75);
    tl.to('#s3-bb-lav', { opacity: 0, scale: 1.35, duration: 0.3, ease: 'power2.in' }, 2.8);
    tl.to('#s3-bb-ochre', { opacity: 0, scale: 1.35, duration: 0.3, ease: 'power2.in' }, 2.83);
    tl.to('#s3-bb', { opacity: 0, scale: 1.1, blur: 28, duration: 0.52, ease: 'power3.inOut' }, 2.78);

    tl.to('#s3-bc', { opacity: 1, scale: 1, blur: 0, duration: 0.66, ease: 'power3.out' }, 2.9);
    tl.to('#s3-grade', { opacity: 1, duration: 0.45, ease: 'power2.out' }, 3.08);
    tl.to('#s3-mark', { opacity: 1, duration: 0.45, ease: 'power2.out' }, 3.2);
    tl.to('#s3-giant', { opacity: 1, scale: 1, y: 0, rotation: -2, duration: 0.76, ease: 'expo.out' }, 3.24);
    tl.to('#s3-dot', { opacity: 1, scale: 1, x: 0, y: 0, duration: 0.48, ease: 'back.out(1.5)' }, 3.42);
    tl.to('#s3-backcard', { opacity: 1, scale: 1, x: 0, y: 0, rotation: 6, duration: 0.5, ease: 'back.out(1.3)' }, 3.49);
    tl.to('#s3-ribl', { opacity: 1, x: 0, rotation: -8, duration: 0.54, ease: 'power3.out' }, 3.54);
    tl.to('#s3-ribr', { opacity: 1, x: 0, rotation: 9, duration: 0.54, ease: 'power3.out' }, 3.6);
    tl.to('#s3-bc-lav', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.4)' }, 3.64);
    tl.to('#s3-bc-peach', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.4)' }, 3.69);
    tl.to('#s3-cap', { opacity: 1, y: 0, scale: 1, blur: 0, duration: 0.44, ease: 'back.out(1.5)' }, 3.86);
    tl.to('#s3-giant', { scale: 1.025, duration: 1.15, ease: 'sine.inOut', yoyo: true, repeat: 1 }, 4.0);

    tl.to('#s3-cap', { opacity: 0, y: -30, scale: 0.96, blur: 10, duration: 0.26, ease: 'power2.in' }, 4.97);
    tl.to('#s3-giant', { scale: 1.14, y: -76, rotation: 0, blur: 10, duration: 0.44, ease: 'power3.inOut' }, 5.0);
    tl.to('#s3-dot', { opacity: 0, scale: 1.08, blur: 10, duration: 0.31, ease: 'power2.in' }, 5.03);
    tl.to('#s3-backcard', { opacity: 0, scale: 1.08, blur: 10, duration: 0.31, ease: 'power2.in' }, 5.044);
    tl.to('#s3-ribl', { opacity: 0, scale: 1.08, blur: 10, duration: 0.31, ease: 'power2.in' }, 5.058);
    tl.to('#s3-ribr', { opacity: 0, scale: 1.08, blur: 10, duration: 0.31, ease: 'power2.in' }, 5.072);
    tl.to('#s3-bc-lav', { opacity: 0, scale: 1.08, blur: 10, duration: 0.31, ease: 'power2.in' }, 5.086);
    tl.to('#s3-bc-peach', { opacity: 0, scale: 1.08, blur: 10, duration: 0.31, ease: 'power2.in' }, 5.1);
    tl.to('#s3-bc', { opacity: 0, scale: 1.06, blur: 24, duration: 0.51, ease: 'power3.inOut' }, 5.06);
    bloom(5.11, 0.62, 0.84, 1.28, 0.16);

    tl.to('#s3-bd', { opacity: 1, scale: 1, blur: 0, duration: 0.58, ease: 'power3.out' }, 5.56);
    tl.to('#s3-dcard', { opacity: 1, scale: 1, y: 0, duration: 0.58, ease: 'back.out(1.25)' }, 5.66);
    tl.to('#s3-drail', { opacity: 1, scaleX: 1, duration: 0.4, ease: 'power2.out' }, 5.85);
    tl.to('#s3-bd-peach', { opacity: 1, scale: 1, duration: 0.4, ease: 'back.out(1.4)' }, 5.96);
    tl.to('#s3-bd-mint', { opacity: 1, scale: 1, duration: 0.4, ease: 'back.out(1.4)' }, 6.01);
    tl.to('#s3-drailfill', { scaleX: 0.08, duration: 1.05, ease: 'none' }, 6.75);

    tl.to('#s3-dcard', { x: -2150, y: 0, scale: 1, rotation: 0, blur: 0, duration: 0.44, ease: 'power3.inOut' }, 7.4);
    tl.to('#s3-drail', { x: -2150, opacity: 1, blur: 0, duration: 0.42, ease: 'power3.inOut' }, 7.42);
    tl.to('#s3-bd-peach', { x: -2150, opacity: 1, scale: 1, duration: 0.42, ease: 'power3.inOut' }, 7.43);
    tl.to('#s3-bd-mint', { x: -2150, opacity: 1, scale: 1, duration: 0.42, ease: 'power3.inOut' }, 7.45);
    tl.to('#s3-bd', { opacity: 0, duration: 0.08, ease: 'none' }, 7.84);

    tl.to('#s3-be', { opacity: 1, x: 0, duration: 0.44, ease: 'power3.inOut' }, 7.46);
    tl.to('#s3-pl1', { opacity: 1, x: 0, duration: 0.36, ease: 'power3.out' }, 7.52);
    tl.to('#s3-pl2', { opacity: 1, x: 0, duration: 0.36, ease: 'power3.out' }, 7.555);
    tl.to('#s3-pl3', { opacity: 1, x: 0, duration: 0.36, ease: 'power3.out' }, 7.59);
    tl.to('#s3-lanea', { opacity: 1, x: 0, duration: 0.38, ease: 'power3.out' }, 7.56);
    tl.to('#s3-laneb', { opacity: 1, x: 0, duration: 0.38, ease: 'power3.out' }, 7.615);
    tl.to('#s3-lanec', { opacity: 1, x: 0, duration: 0.38, ease: 'power3.out' }, 7.67);

    tl.to('#s3-pl1', { opacity: 0, scaleX: 0.45, blur: 8, duration: 0.28, ease: 'power2.in' }, 8.46);
    tl.to('#s3-pl2', { opacity: 0, scaleX: 0.45, blur: 8, duration: 0.28, ease: 'power2.in' }, 8.49);
    tl.to('#s3-pl3', { opacity: 0, scaleX: 0.45, blur: 8, duration: 0.28, ease: 'power2.in' }, 8.52);
    tl.to('#s3-lanea', { x: 350, y: 168, scale: 0.7, blur: 12, duration: 0.42, ease: 'power3.inOut' }, 8.48);
    tl.to('#s3-laneb', { x: -300, y: 34, scale: 0.74, blur: 12, duration: 0.42, ease: 'power3.inOut' }, 8.51);
    tl.to('#s3-lanec', { x: 220, y: -134, scale: 0.86, blur: 12, duration: 0.42, ease: 'power3.inOut' }, 8.54);
    tl.to('#s3-be', { opacity: 0, scale: 0.98, blur: 18, duration: 0.4, ease: 'power3.inOut' }, 8.62);
    bloom(8.58, 0.54, 1.02, 1.18, 0.18);

    tl.to('#s3-bf', { opacity: 1, scale: 1, blur: 0, duration: 0.48, ease: 'power3.out' }, 8.62);
    tl.to('#s3-fsmall', { opacity: 1, y: 0, duration: 0.35, ease: 'power2.out' }, 8.76);
    tl.to('#s3-fword', { opacity: 1, scale: 1, y: 0, duration: 0.52, ease: 'expo.out' }, 8.74);
    tl.to('#s3-fword', { scale: 1.24, duration: 0.3, ease: 'power1.inOut' }, 9.08);

    tl.to('#s3-fsmall', { opacity: 0, y: -22, blur: 8, duration: 0.26, ease: 'power2.in' }, 9.38);
    tl.to('#s3-fword', { scale: 1.62, opacity: 0, blur: 24, duration: 0.5, ease: 'power3.inOut' }, 9.42);
    tl.to('#s3-bf', { opacity: 0, scale: 1.04, blur: 22, duration: 0.5, ease: 'power3.inOut' }, 9.46);

    tl.to('#s3-bg', { opacity: 1, scale: 1, blur: 0, duration: 0.58, ease: 'power2.out' }, 9.56);
    tl.to('#s3-qframe', { opacity: 1, duration: 0.52, ease: 'power2.out' }, 9.64);
    tl.to('#s3-qwrap', { opacity: 1, duration: 0.58, ease: 'power2.out' }, 9.74);
    tl.to('#s3-bg-ochre', { opacity: 1, duration: 0.7, ease: 'power2.out' }, 9.82);
    tl.to('#s3-bg-pink', { opacity: 1, duration: 0.7, ease: 'power2.out' }, 9.92);
  });
  return (
    <div id="s3-root" class="absolute inset-0 overflow-hidden bg-[#fffaf0] z-[2] text-[#0a0a0a]">
      {/* beat-a: the 2G slide, still */}
      <div id="s3-ba" class="absolute inset-0 overflow-hidden bg-[#000000]">
        <SlideFurniture pre="s3a-" />
      </div>

      {/* beat-b: instructions */}
      <div id="s3-bb" class="absolute inset-0 overflow-hidden bg-[#fffaf0]">
        <div id="s3-bb-ghost" class="absolute inset-0 flex flex-row items-center justify-center font-[fl-sans-black] font-[900] text-[192px] tracking-[-9.6px] text-[rgba(10,10,10,0.06)] blur-[16px] scale-[1.22]">
          <text class="whitespace-nowrap">instructions</text>
        </div>
        <Blob id="s3-bb-lav" c="lav" s={154} pos="left-[154px] top-[134px]" />
        <Blob id="s3-bb-ochre" c="ochre" s={134} pos="right-[154px] bottom-[134px]" />
        <div id="s3-bb-line" class="absolute left-[144px] right-[144px] top-[420px] text-center font-[fl-sans-black] font-[900] text-[125px] leading-[0.96] tracking-[-5.6px]">
          <text class="whitespace-nowrap">But your agent needs</text>
          <text class="whitespace-nowrap">different instructions.</text>
        </div>
      </div>

      {/* beat-c: 16x9 frame aroll composite */}
      <div id="s3-bc" class="absolute inset-0 overflow-hidden bg-[#111111]">
        <div class="absolute inset-0 overflow-hidden bg-[#111111]">
          <video class="absolute inset-0 w-full h-full object-cover" src="assets/scene-03-aroll-h264.mp4" data-start={S3 + 2.94} data-duration={2.63} />
          <div id="s3-grade" class="absolute inset-0 z-[2] [mix-blend-mode:screen] opacity-0 bg-[radial-gradient(circle_at_24%_20%,rgba(255,77,139,0.34),transparent_288px),radial-gradient(circle_at_80%_18%,rgba(184,164,237,0.35),transparent_288px),linear-gradient(180deg,rgba(255,250,240,0.1),rgba(10,10,10,0.08))]"></div>
          <div id="s3-giant" class="absolute left-[153.5px] top-[25px] w-[1613px] h-[653px] flex flex-row items-center justify-center gap-[15px] px-[35px] pb-[9px] rounded-[84px] z-[3] bg-[radial-gradient(circle_at_28%_24%,rgba(255,255,255,0.5),transparent_307px),linear-gradient(135deg,#b8a4ed,#ff4d8b)] shadow-[0_23px_0_rgba(10,10,10,0.16),inset_-23px_-29px_42px_rgba(10,10,10,0.14),inset_19px_19px_35px_rgba(255,255,255,0.34)] font-[fl-sans-black] font-[900] text-[726px] leading-[0.72] tracking-[-36.3px] text-[rgba(10,10,10,0.86)] [text-shadow:0_5px_0_rgba(255,255,255,0.22)]">
            <text id="s3-g1" class="whitespace-nowrap">1</text>
            <text id="s3-g2" class="whitespace-nowrap">6</text>
            <text id="s3-g3" class="whitespace-nowrap text-[609.84px]">x</text>
            <text id="s3-g4" class="whitespace-nowrap">9</text>
          </div>
          <div id="s3-mark" class="absolute left-[111px] top-[86px] w-[1698px] h-[890px] border-[3px] border-[rgba(10,10,10,0.16)] rounded-[38px] z-[3] opacity-0">
            <div class="absolute left-[695.5px] top-[-21px] w-[307px] h-[6px] rounded-[999px] bg-[rgba(10,10,10,0.18)]"></div>
          </div>
          <div id="s3-dot" class="absolute left-[182px] top-[267px] w-[177px] h-[177px] rounded-[999px] bg-[#ffb084] z-[3] shadow-[0_14px_0_rgba(10,10,10,0.1),inset_-15px_-19px_29px_rgba(0,0,0,0.13),inset_14px_14px_25px_rgba(255,255,255,0.36)] opacity-0"></div>
          <div id="s3-backcard" class="absolute left-[1526px] top-[371px] w-[298px] h-[169px] rounded-[42px] bg-[#a4d4c5] z-[3] rotate-[6deg] shadow-[0_17px_0_rgba(10,10,10,0.08)] border-[2px] border-[rgba(10,10,10,0.08)] opacity-0">
            <div class="absolute left-[31px] right-[31px] bottom-[29px] h-[10px] rounded-[999px] bg-[rgba(10,10,10,0.2)]"></div>
          </div>
          <div id="s3-ribl" class="absolute left-[-144px] top-[780px] w-[576px] h-[104px] rounded-[999px] bg-[#1a3a3a] z-[3] rotate-[-8deg] shadow-[inset_-15px_-19px_31px_rgba(0,0,0,0.14),inset_13px_15px_25px_rgba(255,255,255,0.36)] opacity-0"></div>
          <div id="s3-ribr" class="absolute left-[1507px] top-[726px] w-[595px] h-[104px] rounded-[999px] bg-[#e8b94a] z-[3] rotate-[9deg] shadow-[inset_-15px_-19px_31px_rgba(0,0,0,0.14),inset_13px_15px_25px_rgba(255,255,255,0.36)] opacity-0"></div>
          <Blob id="s3-bc-lav" c="lav" s={109} pos="left-[415px] top-[192px]" />
          <Blob id="s3-bc-peach" c="peach" s={92} pos="left-[1440px] top-[228px]" />
          <video class="absolute inset-0 w-full h-full object-cover z-[4]" src="assets/scene-03-aroll-cutout-alpha.mov" data-start={S3 + 2.94} data-duration={2.63} />
          <div id="s3-cap" class="absolute left-[0px] bottom-[79px] w-[1920px] flex flex-row justify-center z-[6]">
            <div class="flex flex-row items-center px-[42px] pt-[21px] pb-[24px] rounded-[27px] bg-[#ff4d8b] text-[#ffffff] font-[fl-sans-black] font-[900] text-[67px] leading-[0.96] tracking-[-3.7px] text-center shadow-[0_14px_0_rgba(10,10,10,0.14)] rotate-[-1deg]">
              <text class="whitespace-nowrap">{'use a\u00a0'}</text>
              <div class="px-[9px] pb-[2px] rounded-[13px] bg-[#e8b94a] text-[#0a0a0a]"><text class="whitespace-nowrap">16:9</text></div>
              <text class="whitespace-nowrap">{'\u00a0frame'}</text>
            </div>
          </div>
        </div>
      </div>

      {/* beat-d: duration */}
      <div id="s3-bd" class="absolute inset-0 overflow-hidden bg-[#fffaf0]">
        <div id="s3-dcard" class="absolute left-[158px] top-[292px] w-[1604px] h-[300px] rounded-[999px] bg-[#b8a4ed] px-[81px] py-[38px] flex flex-row items-center justify-center text-center shadow-[0_23px_0_rgba(10,10,10,0.08)]">
          <text class="font-[fl-sans-black] font-[900] text-[101px] leading-[0.96] tracking-[-4.5px] whitespace-nowrap">how long a message can live</text>
        </div>
        <div id="s3-drail" class="absolute left-[230px] top-[863px] w-[1460px] h-[25px] bg-[rgba(10,10,10,0.12)] rounded-[999px] overflow-hidden opacity-0">
          <div id="s3-drailfill" class="w-[525.6px] h-full bg-[#0a0a0a]"></div>
        </div>
        <Blob id="s3-bd-peach" c="peach" s={106} pos="left-[154px] top-[840px]" op={0.75} />
        <Blob id="s3-bd-mint" c="mint" s={115} pos="left-[1671px] top-[134px]" op={0.55} />
      </div>

      {/* beat-e: what should move */}
      <div id="s3-be" class="absolute inset-0 overflow-hidden bg-[#fffaf0]">
        <div id="s3-pl1" class="absolute left-[77px] top-[326px] w-[672px] h-[4px] bg-[rgba(10,10,10,0.18)]"></div>
        <div id="s3-pl2" class="absolute left-[1171px] top-[480px] w-[672px] h-[4px] bg-[rgba(10,10,10,0.18)]"></div>
        <div id="s3-pl3" class="absolute left-[77px] top-[634px] w-[806px] h-[4px] bg-[rgba(10,10,10,0.18)]"></div>
        <div id="s3-lanea" class="absolute left-[192px] top-[265px] w-[768px] h-[108px] rounded-[26px] bg-[#e8b94a] flex flex-row items-center px-[40px] font-[fl-sans-black] font-[900] text-[61px] leading-none tracking-[-2.1px] shadow-[0_15px_0_rgba(10,10,10,0.08)] opacity-0"><text class="whitespace-nowrap">what</text></div>
        <div id="s3-laneb" class="absolute left-[922px] top-[417px] w-[787px] h-[108px] rounded-[26px] bg-[#1a3a3a] text-[#ffffff] flex flex-row items-center px-[40px] font-[fl-sans-black] font-[900] text-[61px] leading-none tracking-[-2.1px] shadow-[0_15px_0_rgba(10,10,10,0.08)] opacity-0"><text class="whitespace-nowrap">should</text></div>
        <div id="s3-lanec" class="absolute left-[384px] top-[570px] w-[845px] h-[108px] rounded-[26px] bg-[#ff4d8b] text-[#ffffff] flex flex-row items-center px-[40px] font-[fl-sans-black] font-[900] text-[61px] leading-none tracking-[-2.1px] shadow-[0_15px_0_rgba(10,10,10,0.08)] opacity-0"><text class="whitespace-nowrap">move</text></div>
        <Blob id="s3-be-lav" c="lav" s={115} pos="right-[154px] bottom-[134px]" />
      </div>

      {/* beat-f: scale */}
      <div id="s3-bf" class="absolute inset-0 overflow-hidden bg-[#fffaf0]">
        <div class="absolute inset-0 flex flex-col items-center justify-center text-center">
          <text id="s3-fsmall" class="font-[fl-sans-bold] font-[700] text-[61px] leading-none tracking-[-2.1px] text-[rgba(10,10,10,0.44)]">what needs</text>
          <text id="s3-fword" class="mt-[-19px] font-[fl-sans-black] font-[900] text-[269px] leading-[0.84] tracking-[-18.8px]">scale</text>
        </div>
      </div>

      {/* beat-g: silence */}
      <div id="s3-bg" class="absolute inset-0 overflow-hidden bg-[#fffaf0]">
        <div id="s3-qframe" class="absolute left-[154px] top-[134px] w-[1612px] h-[812px] border-[2px] border-[rgba(10,10,10,0.08)] rounded-[23px] opacity-0"></div>
        <div id="s3-qwrap" class="absolute inset-0 flex flex-row items-center justify-center font-[fl-sans-bold] font-[700] text-[54px] leading-[1.1] tracking-[-1.35px] text-[rgba(10,10,10,0.58)] opacity-0">
          <text class="whitespace-nowrap">what deserves silence</text>
        </div>
        <Blob id="s3-bg-ochre" c="ochre" s={77} pos="left-[192px] top-[811px]" op={0.2} />
        <Blob id="s3-bg-pink" c="pink" s={86} pos="left-[1604px] top-[211px]" op={0.14} />
      </div>

      <div id="s3-bloom" class="absolute left-[-120px] top-[-120px] w-[2160px] h-[1320px] z-[40] opacity-0 blur-[34px] bg-[radial-gradient(circle_at_50%_48%,rgba(255,250,240,1)_0_22%,rgba(255,250,240,0.92)_38%,rgba(255,77,139,0.28)_54%,rgba(184,164,237,0.14)_68%,transparent_82%)]"></div>
    </div>
  );
}

/* ════════════ SCENE 04 — frame.md built (26.96 → 32.2s) ════════════ */
function Scene4() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S4 && ctx.global.time < S4 + D4;
    vis(ctx, 's4-root', on);
    if (!on) return;
    var tl = ctx.timeline({ delay: S4 });
    // elliptical dome approximated with a 223.2px quad radius (18% of 1240)
    tl.set('#s4-shell', { y: 1110, scaleX: 1.08, borderRadius: 223.2, transformOrigin: '50% 100%' });
    tl.set('#s4-stack', { opacity: 0, scale: 0.78, y: 108, blur: 20 });
    tl.set('#s4-bp', { opacity: 0 });
    tl.set('#s4-bb', { opacity: 0 });
    tl.set('#s4-x1', { opacity: 0, y: 165, scale: 0.72, rotation: -7, blur: 15, transformOrigin: '50% 62%' });
    tl.set('#s4-x2', { opacity: 0, y: 165, scale: 0.72, rotation: -7, blur: 15, transformOrigin: '50% 62%' });
    tl.set('#s4-x3', { opacity: 0, y: 165, scale: 0.72, rotation: -7, blur: 15, transformOrigin: '50% 62%' });
    tl.set('#s4-x4', { opacity: 0, y: 165, scale: 0.72, rotation: -7, blur: 15, transformOrigin: '50% 62%' });
    tl.set('#s4-x5', { opacity: 0, y: 165, scale: 0.72, rotation: -7, blur: 15, transformOrigin: '50% 62%' });
    tl.set('#s4-x6', { opacity: 0, y: 165, scale: 0.72, rotation: -7, blur: 15, transformOrigin: '50% 62%' });
    tl.set('#s4-x7', { opacity: 0, y: 165, scale: 0.72, rotation: -7, blur: 15, transformOrigin: '50% 62%' });
    tl.set('#s4-bp', { x: -24, y: -24 });
    tl.set('#s4-bb', { x: 24, y: 30 });

    tl.to('#s4-shell', { y: 0, scaleX: 1, borderRadius: 0, duration: 0.74, ease: 'power4.inOut' }, 0);
    tl.to('#s4-stack', { opacity: 1, scale: 1, y: 0, blur: 0, duration: 0.68, ease: 'expo.out' }, 0.72);
    tl.to('#s4-bp', { opacity: 1, duration: 0.28, ease: 'power2.out' }, 0.78);
    tl.to('#s4-bb', { opacity: 1, duration: 0.28, ease: 'power2.out' }, 0.78);
    ['#s4-x1', '#s4-x2', '#s4-x3', '#s4-x4', '#s4-x5', '#s4-x6', '#s4-x7'].forEach(function (id, i) {
      tl.to(id, { opacity: 1, y: 0, scale: 1, rotation: 0, blur: 0, duration: 0.7, ease: 'back.out(1.32)' }, 0.84 + i * 0.035);
    });
    tl.to('#s4-stack', { scale: 1.035, duration: 1.5, ease: 'sine.inOut' }, 1.88);
  });
  var WORD = [
    { t: 'f' }, { t: 'r' }, { t: 'a' }, { t: 'm' }, { t: 'e' },
    { t: '.', color: '#e8b94a' }, { t: 'md', color: '#a4d4c5' },
  ];
  return (
    <div id="s4-root" class="absolute inset-0 overflow-hidden z-[3] text-[#0a0a0a]">
      <div id="s4-shell" class="absolute left-[-170px] top-[0px] w-[2260px] h-[1240px] rounded-[56px] bg-[#fffaf0] overflow-hidden shadow-[0_-34px_80px_rgba(10,10,10,0.18)]">
        <div id="s4-stack" class="absolute left-[210px] top-[370px] w-[1840px] h-[500px] opacity-0">
          <div id="s4-bp" class="absolute inset-0 flex flex-row items-center justify-center font-[fl-round] font-[900] text-[392px] leading-[0.9] tracking-[-29.4px] text-[#ff4d8b] opacity-0">
            <text class="whitespace-nowrap">frame.md</text>
          </div>
          <div id="s4-bb" class="absolute inset-0 flex flex-row items-center justify-center font-[fl-round] font-[900] text-[392px] leading-[0.9] tracking-[-29.4px] text-[#0007cd] opacity-0">
            <text class="whitespace-nowrap">frame.md</text>
          </div>
          <div id="s4-main" class="absolute inset-0 flex flex-row items-center justify-center font-[fl-round] font-[900] text-[392px] leading-[0.9] text-[#ffffff] [-webkit-text-stroke:8px_rgba(10,10,10,0.92)] [text-shadow:0_20px_34px_rgba(10,10,10,0.24)]">
            {WORD.map(function (c, i) {
              return <text id={'s4-x' + (i + 1)} class={'tracking-[-29.4px] whitespace-nowrap' + (c.color ? ' text-[' + c.color + ']' : '')}>{c.t}</text>;
            })}
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ SCENE 05 — UI flow (29.2 → 42.384s) ════════════ */
function S5Caption(p) {
  return (
    <div id={p.id} class="absolute left-[0px] bottom-[48px] w-[1920px] flex flex-row justify-center z-[30] opacity-0">
      <div class="flex flex-row items-center px-[82px] pt-[10px] pb-[13px] rounded-[20px] bg-[#ff4d8b] text-[#ffffff] border-[1px] border-[rgba(255,255,255,0.32)] shadow-[0_7px_0_rgba(10,10,10,0.12),0_15px_28px_rgba(255,77,139,0.22)] font-[fl-sans-black] font-[900] text-[40px] leading-[0.92] tracking-[-1.4px] text-center whitespace-nowrap rotate-[-1deg]">
        {p.parts.map(function (part) {
          if (part.hl === 'em') return <div class="px-[8px] pb-[2px] rounded-[10px] bg-[#a4d4c5] text-[#0a0a0a]"><text class="whitespace-nowrap">{part.t}</text></div>;
          if (part.hl === 'strong') return <div class="px-[8px] pb-[2px] rounded-[10px] bg-[#e8b94a] text-[#0a0a0a]"><text class="whitespace-nowrap">{part.t}</text></div>;
          return <text class="whitespace-nowrap">{part.t}</text>;
        })}
      </div>
    </div>
  );
}

function Scene5() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S5 && ctx.global.time < S5 + D5;
    vis(ctx, 's5-root', on);
    if (!on) return;
    var tl = ctx.timeline({ delay: S5 });
    tl.set('#s5-cover', { y: 1110, scaleX: 1.08, borderRadius: 223.2, transformOrigin: '50% 100%' });
    tl.set('#s5-frame', { opacity: 0, y: 80, scale: 0.94, blur: 18 });
    tl.set('#s5-plight', { opacity: 0, y: 28, scale: 0.98 });
    tl.set('#s5-pdark', { opacity: 0, y: 28, scale: 0.98 });
    tl.set('#s5-bigph', { opacity: 0, y: 24, blur: 10 });
    tl.set('#s5-cursor', { opacity: 0, scale: 0.5 });
    ['#s5-cap1', '#s5-cap2', '#s5-cap3', '#s5-cap4', '#s5-cap5', '#s5-cap6'].forEach(function (id) {
      tl.set(id, { opacity: 0, y: 18, scale: 0.985, blur: 6 });
    });

    var showCap = function (id, start, end) {
      tl.to(id, { opacity: 1, y: 0, scale: 1, blur: 0, duration: 0.22, ease: 'power2.out' }, start);
      tl.to(id, { opacity: 0, y: 10, scale: 0.99, blur: 5, duration: 0.18, ease: 'power2.in' }, Math.max(start + 0.22, end - 0.18));
    };

    tl.to('#s5-cover', { y: 0, scaleX: 1, borderRadius: 0, duration: 0.64, ease: 'power4.inOut' }, 0);
    tl.to('#s5-frame', { opacity: 1, y: 0, scale: 1, blur: 0, duration: 0.68, ease: 'power3.out' }, 0.2);
    tl.to('#s5-plight', { opacity: 1, y: 0, scale: 1, duration: 0.5, ease: 'power3.out' }, 0.44);
    tl.to('#s5-pdark', { opacity: 1, y: 0, scale: 1, duration: 0.5, ease: 'power3.out' }, 0.52);
    tl.to('#s5-bigph', { opacity: 1, y: 0, blur: 0, duration: 0.46, ease: 'power2.out' }, 0.72);
    tl.to('#s5-cursor', { opacity: 1, scale: 1, duration: 0.34, ease: 'back.out(1.5)' }, 1.18);
    tl.to('#s5-cursor', { x: -520, y: -185, duration: 2.0, ease: 'power2.inOut' }, 2.0);
    tl.to('#s5-cursor', { x: 325, y: 105, duration: 2.3, ease: 'power2.inOut' }, 4.4);
    tl.to('#s5-frame', { scale: 1.018, duration: 5.2, ease: 'sine.inOut' }, 2.0);

    showCap('#s5-cap1', 0.18, 2.46);
    showCap('#s5-cap2', 2.56, 5.42);
    showCap('#s5-cap3', 5.48, 6.62);
    showCap('#s5-cap4', 6.78, 8.16);
    showCap('#s5-cap5', 8.46, 9.08);
    showCap('#s5-cap6', 9.08, 12.304);
  });
  // panel rows: [class-color, width%] on light(546.2) / dark(705.8) content widths
  var LR = [['#0007cd', 245.8], ['rgba(10,10,10,0.13)', 480.7], ['rgba(10,10,10,0.13)', 382.3], ['rgba(10,10,10,0.13)', 229.4], ['#ff4d8b', 360.5], ['rgba(10,10,10,0.13)', 480.7]];
  var DR = [['#a4d4c5', 381.1], ['rgba(255,255,255,0.18)', 621.1], ['rgba(255,255,255,0.18)', 494.1], ['#0007cd', 317.6], ['rgba(255,255,255,0.18)', 621.1], ['rgba(255,255,255,0.18)', 296.4]];
  return (
    <div id="s5-root" class="absolute inset-0 overflow-hidden z-[4] text-[#0a0a0a]">
      <div id="s5-cover" class="absolute left-[-170px] top-[0px] w-[2260px] h-[1240px] rounded-[56px] bg-[#fffaf0] shadow-[0_-34px_80px_rgba(10,10,10,0.16)]">
        <div id="s5-frame" class="absolute left-[330px] top-[74px] w-[1600px] h-[900px] rounded-[54px] p-[8px] bg-[#0007cd] shadow-[0_10px_0_rgba(10,10,10,0.1),0_24px_58px_rgba(0,7,205,0.18)] overflow-hidden opacity-0">
          <div class="relative w-full h-full rounded-[46px] overflow-hidden bg-[radial-gradient(circle_at_72%_18%,rgba(255,77,139,0.26),transparent_310px),radial-gradient(circle_at_21%_74%,rgba(0,7,205,0.24),transparent_340px),linear-gradient(135deg,#f9f5e9,#fffaf0_48%,#f1ecdf)]">
            <div class="absolute left-[64px] top-[56px] px-[18px] pt-[13px] pb-[15px] rounded-[16px] bg-[#ff4d8b] text-[#ffffff] font-[fl-sans-black] font-[900] text-[28px] leading-none rotate-[-2deg] shadow-[0_7px_0_rgba(10,10,10,0.13)]">
              <text class="whitespace-nowrap">Screen Studio placeholder</text>
            </div>
            <div id="s5-plight" class="absolute left-[85px] top-[132px] w-[614.2px] h-[674px] rounded-[28px] bg-[rgba(255,255,255,0.74)] border-[1px] border-[rgba(10,10,10,0.12)] shadow-[inset_0_0_0_1px_rgba(255,255,255,0.55)] opacity-0">
              {LR.map(function (r, i) {
                return <div class={'absolute left-[34px] top-[' + (34 + i * 40) + 'px] w-[' + r[1] + 'px] h-[20px] rounded-[999px] bg-[' + r[0] + ']'}></div>;
              })}
            </div>
            <div id="s5-pdark" class="absolute left-[725.2px] top-[132px] w-[773.8px] h-[674px] rounded-[28px] bg-[rgba(12,12,13,0.92)] border-[1px] border-[rgba(10,10,10,0.12)] shadow-[inset_0_0_0_1px_rgba(255,255,255,0.55)] opacity-0">
              {DR.map(function (r, i) {
                return <div class={'absolute left-[34px] top-[' + (34 + i * 40) + 'px] w-[' + r[1] + 'px] h-[20px] rounded-[999px] bg-[' + r[0] + ']'}></div>;
              })}
            </div>
            <div id="s5-bigph" class="absolute left-[0px] top-[359.6px] w-[1584px] flex flex-col items-center text-center font-[fl-sans-black] font-[900] text-[96px] leading-[0.95] text-[rgba(10,10,10,0.13)] opacity-0 rotate-[-2deg]">
              <text class="whitespace-nowrap">design spec</text>
              <text class="whitespace-nowrap">to frame language</text>
            </div>
            <div id="s5-cursor" class="absolute left-[1015px] top-[492px] w-[34px] h-[34px] rounded-[999px] bg-[#e8b94a] shadow-[0_0_0_12px_rgba(232,185,74,0.24)] opacity-0"></div>
            <video class="absolute inset-0 w-full h-full object-cover z-[20]" src="assets/ssframe.mp4" data-start={S5} data-duration={13.184} />
          </div>
        </div>
      </div>
      <S5Caption id="s5-cap1" parts={[{ t: 'It takes the brand language from ' }, { t: 'design.md', hl: 'em' }]} />
      <S5Caption id="s5-cap2" parts={[{ t: 'and translates it into ' }, { t: 'video-native direction.', hl: 'strong' }]} />
      <S5Caption id="s5-cap3" parts={[{ t: 'Paste a design spec.' }]} />
      <S5Caption id="s5-cap4" parts={[{ t: 'HyperFrames reads it,' }]} />
      <S5Caption id="s5-cap5" parts={[{ t: 'interprets it,' }]} />
      <S5Caption id="s5-cap6" parts={[{ t: 'and generates a frame language your agent can direct from.' }]} />
    </div>
  );
}

/* ════════════ SCENE 06 — text to frame (41.624 → 48.874s) ════════════ */
function Scene6() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S6 && ctx.global.time < S6 + D6;
    vis(ctx, 's6-root', on);
    if (!on) return;
    var tl = ctx.timeline({ delay: S6 });
    ['#s6-title', '#s6-origin', '#s6-body', '#s6-pace'].forEach(function (id) {
      tl.set(id, { opacity: 0, y: 38, blur: 10 });
    });
    tl.set('#s6-sweep', { opacity: 0, scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s6-fstage', { opacity: 0, scale: 1.04, blur: 18 });
    tl.set('#s6-spot', { opacity: 0, scale: 0.74 });
    tl.set('#s6-shapea', { opacity: 0, y: 42, scale: 0.88 });
    tl.set('#s6-shapeb', { opacity: 0, y: 42, scale: 0.88 });
    tl.set('#s6-shapec', { opacity: 0, y: 42, scale: 0.88 });
    tl.set('#s6-shaped', { opacity: 0, y: 42, scale: 0.88 });
    tl.set('#s6-herom', { opacity: 0, y: 34, blur: 12 });
    tl.set('#s6-tgrid', { opacity: 0, y: 56, scale: 0.92, blur: 16 });
    tl.set('#s6-tp1', { opacity: 0, y: 18 });
    tl.set('#s6-tp2', { opacity: 0, y: 18 });
    tl.set('#s6-tp3', { opacity: 0, y: 18 });
    tl.set('#s6-tp4', { opacity: 0, y: 18 });
    tl.set('#s6-mpath', { strokeDasharray: 1600, strokeDashoffset: 1600 });
    tl.set('#s6-pdot', { opacity: 0, x: 330, y: 708, scale: 0.55 });
    ['#s6-cap1', '#s6-cap2'].forEach(function (id) {
      tl.set(id, { opacity: 0, y: 18, scale: 0.985, blur: 6 });
    });

    tl.to('#s6-title', { opacity: 1, y: 0, blur: 0, duration: 0.58, ease: 'power3.out' }, 0.16);
    tl.to('#s6-origin', { opacity: 1, y: 0, blur: 0, duration: 0.58, ease: 'power3.out' }, 0.32);
    tl.to('#s6-body', { opacity: 1, y: 0, blur: 0, duration: 0.58, ease: 'power3.out' }, 0.48);
    tl.to('#s6-pace', { opacity: 1, y: 0, blur: 0, duration: 0.58, ease: 'power3.out' }, 0.64);
    tl.to('#s6-cap1', { opacity: 0.96, y: 0, scale: 1, blur: 0, duration: 0.22, ease: 'power2.out' }, 0.82);
    tl.to('#s6-cap1', { opacity: 0, y: -10, scale: 0.99, blur: 5, duration: 0.18, ease: 'power2.in' }, 2.35);
    tl.to('#s6-cap2', { opacity: 0.96, y: 0, scale: 1, blur: 0, duration: 0.22, ease: 'power2.out' }, 2.54);
    tl.to('#s6-cap2', { opacity: 0, y: -10, scale: 0.99, blur: 5, duration: 0.18, ease: 'power2.in' }, 4.44);
    tl.to('#s6-sweep', { opacity: 1, scaleX: 1, duration: 0.54, ease: 'power3.out' }, 2.55);
    tl.to('#s6-sweep', { opacity: 0.35, duration: 1.2, ease: 'power2.out' }, 3.1);

    tl.to('#s6-tstage', { scale: 1.42, opacity: 0, blur: 28, duration: 0.78, ease: 'power3.inOut' }, 4.82);
    tl.to('#s6-fstage', { opacity: 1, scale: 1, blur: 0, duration: 0.74, ease: 'power3.out' }, 5.0);
    tl.to('#s6-spot', { opacity: 1, scale: 1, duration: 0.85, ease: 'power2.out' }, 5.12);
    tl.to('#s6-shapea', { opacity: 1, y: 0, scale: 1, duration: 0.58, ease: 'back.out(1.35)' }, 5.22);
    tl.to('#s6-shapeb', { opacity: 1, y: 0, scale: 1, duration: 0.58, ease: 'back.out(1.35)' }, 5.275);
    tl.to('#s6-shapec', { opacity: 1, y: 0, scale: 1, duration: 0.58, ease: 'back.out(1.35)' }, 5.33);
    tl.to('#s6-shaped', { opacity: 1, y: 0, scale: 1, duration: 0.58, ease: 'back.out(1.35)' }, 5.385);
    tl.to('#s6-herom', { opacity: 1, y: 0, blur: 0, duration: 0.68, ease: 'power3.out' }, 5.48);
    tl.to('#s6-tgrid', { opacity: 1, y: 0, scale: 1, blur: 0, duration: 0.72, ease: 'power3.out' }, 5.82);
    tl.to('#s6-tp1', { opacity: 1, y: 0, duration: 0.34, ease: 'power2.out' }, 6.02);
    tl.to('#s6-tp2', { opacity: 1, y: 0, duration: 0.34, ease: 'power2.out' }, 6.07);
    tl.to('#s6-tp3', { opacity: 1, y: 0, duration: 0.34, ease: 'power2.out' }, 6.12);
    tl.to('#s6-tp4', { opacity: 1, y: 0, duration: 0.34, ease: 'power2.out' }, 6.17);
    tl.to('#s6-mpath', { strokeDashoffset: 0, duration: 1.3, ease: 'power2.inOut' }, 6.74);
    tl.to('#s6-pdot', { opacity: 1, scale: 1, duration: 0.22, ease: 'power2.out' }, 6.78);
    tl.to('#s6-pdot', { x: 1550, y: 412, duration: 1.26, ease: 'power2.inOut' }, 6.82);
    tl.to('#s6-tgrid', { scale: 1.035, duration: 1.7, ease: 'sine.inOut' }, 8.04);
    tl.to('#s6-spot', { scale: 1.12, opacity: 0.92, duration: 1.7, ease: 'sine.inOut' }, 8.04);
  });
  // terminal panes: [bar-color, w1, w2, w3] on 474px content width
  var PANES = [
    ['#ff4d8b'], ['#b8a4ed'], ['#e8b94a'], ['#a4d4c5'],
  ];
  return (
    <div id="s6-root" class="absolute inset-0 overflow-hidden bg-[#fffaf0] z-[5] text-[#0a0a0a]">
      <div id="s6-tstage" class="absolute inset-0 overflow-hidden bg-[#ffffff]">
        <div id="s6-sweep" class="absolute left-[96px] top-[84px] w-[1620px] h-[760px] border-l-[11px] border-l-[#ef00e4] opacity-0"></div>
        {/* hf-studio path offsets baked in (title -6,121 / origin -14,170 / body -11,163 / pace -4,162) */}
        <text id="s6-title" class="absolute left-[140px] top-[247px] font-[fl-sans-black] font-[900] text-[82px] leading-[1.08] text-[#ef00e4] whitespace-nowrap">## Hero Band</text>
        <text id="s6-origin" class="absolute left-[132px] top-[430.56px] font-[fl-sans-black] font-[900] text-[41px] leading-[1.86] text-[#ef00e4] whitespace-nowrap">hero band with spotlight and terminal.</text>
        <text id="s6-body" class="absolute left-[135px] top-[511.82px] w-[1560px] font-[fl-sans-black] font-[900] text-[41px] leading-[1.86] text-[#ef00e4]">Anchor scene. Centered display-mega headline with a primary final clause, badge pair above, and the 2x2 terminal grid centered beneath inside a radial primary-glow field.</text>
        <text id="s6-pace" class="absolute left-[142px] top-[751.6px] w-[1540px] font-[fl-sans-black] font-[900] text-[41px] leading-[1.86] text-[#ef00e4]">Frame-pace: low. Needs dwelling for the code panes to read.</text>
      </div>

      <div id="s6-fstage" class="absolute inset-0 overflow-hidden bg-[#fffaf0] opacity-0">
        <div id="s6-spot" class="absolute left-[300px] top-[115.2px] w-[1320px] h-[720px] rounded-[52px] bg-[radial-gradient(circle_at_36%_28%,rgba(255,176,132,0.92),transparent_170px),radial-gradient(circle_at_72%_38%,rgba(184,164,237,0.94),transparent_220px),linear-gradient(135deg,#ff4d8b,#b8a4ed_56%,#a4d4c5)] opacity-0"></div>
        <div id="s6-shapea" class="absolute left-[94px] top-[626px] w-[268px] h-[112px] border-[5px] border-[#0a0a0a] rounded-[999px] bg-[#1a3a3a] rotate-[-10deg] shadow-[14px_16px_0_rgba(10,10,10,0.18)] opacity-0"></div>
        <div id="s6-shapeb" class="absolute left-[1532px] top-[590px] w-[290px] h-[104px] border-[5px] border-[#0a0a0a] rounded-[999px] bg-[#e8b94a] rotate-[12deg] shadow-[14px_16px_0_rgba(10,10,10,0.18)] opacity-0"></div>
        <div id="s6-shapec" class="absolute left-[1582px] top-[188px] w-[150px] h-[150px] border-[5px] border-[#0a0a0a] rounded-full bg-[#ffb084] shadow-[14px_16px_0_rgba(10,10,10,0.18)] opacity-0"></div>
        <div id="s6-shaped" class="absolute left-[206px] top-[176px] w-[120px] h-[120px] border-[5px] border-[#0a0a0a] rounded-full bg-[#b8a4ed] shadow-[14px_16px_0_rgba(10,10,10,0.18)] opacity-0"></div>
        <div class="absolute left-[126px] top-[92px] w-[1668px] h-[896px]">
          {/* hero: 144px inline override, offset (3,-39) baked */}
          <div class="absolute left-[172px] top-[91px] w-[1330px] text-center">
            <div id="s6-herom" class="font-[fl-round] font-[900] text-[144px] leading-[0.96] text-[#ff4d8b] [-webkit-text-stroke:3px_#0a0a0a] opacity-0">
              <text class="whitespace-nowrap">It becomes a scene</text>
            </div>
          </div>
          <div id="s6-tgrid" class="absolute left-[279px] top-[424px] w-[1110px] h-[430px] grid grid-cols-2 grid-rows-2 gap-[16px] p-[28px] rounded-[28px] bg-[#0a0a0a] border-[5px] border-[#0a0a0a] shadow-[20px_22px_0_rgba(10,10,10,0.16)] opacity-0">
            {PANES.map(function (pn, i) {
              return (
                <div id={'s6-tp' + (i + 1)} class="relative rounded-[17px] bg-[#142727] border-[2px] border-[rgba(255,255,255,0.12)] opacity-0">
                  <div class={'absolute left-[20px] top-[20px] w-[180.1px] h-[12px] rounded-[999px] bg-[' + pn[0] + ']'}></div>
                  <div class="absolute left-[20px] top-[46px] w-[388.7px] h-[12px] rounded-[999px] bg-[rgba(255,255,255,0.32)]"></div>
                  <div class="absolute left-[20px] top-[72px] w-[293.9px] h-[12px] rounded-[999px] bg-[rgba(255,255,255,0.32)]"></div>
                  <div class="absolute left-[20px] top-[98px] w-[350.8px] h-[12px] rounded-[999px] bg-[rgba(164,212,197,0.68)]"></div>
                </div>
              );
            })}
          </div>
          <path id="s6-mpath" class="absolute left-[238px] top-[330px] w-[1410px] h-[480px] stroke-[#ff4d8b] stroke-[8] opacity-88" d="M 84 404 C 290 258, 436 188, 638 244 S 982 356, 1304 92" view-box="0 0 1410 480" />
          <div id="s6-pdot" class="absolute left-[0px] top-[0px] w-[18px] h-[18px] rounded-full bg-[#e8b94a] border-[4px] border-[#0a0a0a] shadow-[0_0_0_12px_rgba(255,77,139,0.2)] opacity-0"></div>
        </div>
      </div>

      <div id="s6-cap1" class="absolute left-[0px] bottom-[62px] w-[1920px] flex flex-row justify-center z-[8] opacity-0">
        <div class="flex flex-row items-center px-[40px] pt-[20px] pb-[23px] rounded-[27px] bg-[#ff4d8b] text-[#ffffff] shadow-[0_14px_0_rgba(10,10,10,0.14)] font-[fl-sans-black] font-[900] text-[42px] leading-[0.96] text-center whitespace-nowrap rotate-[-1deg]">
          <text class="whitespace-nowrap">{'A paragraph like\u00a0'}</text>
          <div class="px-[8px] pb-[2px] rounded-[10px] bg-[#e8b94a] text-[#0a0a0a]"><text class="whitespace-nowrap">hero band with spotlight and terminal</text></div>
        </div>
      </div>
      <div id="s6-cap2" class="absolute left-[0px] bottom-[62px] w-[1920px] flex flex-row justify-center z-[8] opacity-0">
        <div class="px-[40px] pt-[20px] pb-[23px] rounded-[27px] bg-[#ff4d8b] text-[#ffffff] shadow-[0_14px_0_rgba(10,10,10,0.14)] font-[fl-sans-black] font-[900] text-[42px] leading-[0.96] text-center whitespace-nowrap rotate-[-1deg]">
          <text class="whitespace-nowrap">stops being documentation.</text>
        </div>
      </div>
    </div>
  );
}

/* ════════════ SCENE 07 — new format (48.234 → 52.384s) ════════════ */
function Scene7() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S7 && ctx.global.time < S7 + D7;
    vis(ctx, 's7-root', on);
    if (!on) return;
    var tl = ctx.timeline({ delay: S7 });
    tl.set('#s7-wash', { opacity: 0, scale: 1.08 });
    tl.set('#s7-slab', { opacity: 0, x: -180, y: 34, rotation: -8, blur: 12 });
    tl.set('#s7-l1', { opacity: 0, scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s7-l2', { opacity: 0, scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s7-l3', { opacity: 0, scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s7-l4', { opacity: 0, scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s7-l5', { opacity: 0, scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s7-not', { opacity: 0, scale: 0.55, rotation: -25 });
    tl.set('#s7-old1', { opacity: 0, x: 110, blur: 10 });
    tl.set('#s7-old2', { opacity: 0, x: 110, blur: 10 });
    tl.set('#s7-ffield', { opacity: 0, scale: 1.08, blur: 22 });
    tl.set('#s7-nfm', { opacity: 0, scale: 0.72, y: 42, blur: 18 });
    tl.set('#s7-pill', { opacity: 0, y: 46, scale: 0.82 });
    tl.set('#s7-o1', { opacity: 0, scale: 0.55 });
    tl.set('#s7-o2', { opacity: 0, scale: 0.55 });
    tl.set('#s7-o3', { opacity: 0, scale: 0.55 });
    tl.set('#s7-o4', { opacity: 0, scale: 0.55 });
    tl.set('#s7-slash', { scaleX: 0, transformOrigin: '0px 50%' });

    tl.to('#s7-wash', { opacity: 1, scale: 1, duration: 0.55, ease: 'power2.out' }, 0.04);
    tl.to('#s7-slab', { opacity: 1, x: 0, y: 0, rotation: -2, blur: 0, duration: 0.62, ease: 'expo.out' }, 0.1);
    tl.to('#s7-l1', { opacity: 1, scaleX: 1, duration: 0.36, ease: 'power3.out' }, 0.34);
    tl.to('#s7-l2', { opacity: 1, scaleX: 1, duration: 0.36, ease: 'power3.out' }, 0.39);
    tl.to('#s7-l3', { opacity: 1, scaleX: 1, duration: 0.36, ease: 'power3.out' }, 0.44);
    tl.to('#s7-l4', { opacity: 1, scaleX: 1, duration: 0.36, ease: 'power3.out' }, 0.49);
    tl.to('#s7-l5', { opacity: 1, scaleX: 1, duration: 0.36, ease: 'power3.out' }, 0.54);
    tl.to('#s7-old1', { opacity: 1, x: 0, blur: 0, duration: 0.46, ease: 'power3.out' }, 0.5);
    tl.to('#s7-old2', { opacity: 1, x: 0, blur: 0, duration: 0.46, ease: 'power3.out' }, 0.58);
    tl.to('#s7-not', { opacity: 1, scale: 1, rotation: -12, duration: 0.42, ease: 'back.out(1.7)' }, 0.72);
    tl.to('#s7-slash', { scaleX: 1, duration: 0.34, ease: 'power3.inOut' }, 1.72);
    tl.to('#s7-fidea', { x: -140, scale: 0.82, opacity: 0, blur: 24, duration: 0.3, ease: 'power3.inOut' }, 1.92);
    tl.to('#s7-ffield', { opacity: 1, scale: 1, blur: 0, duration: 0.54, ease: 'power3.out' }, 2.14);
    tl.to('#s7-o1', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.55)' }, 2.34);
    tl.to('#s7-o2', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.55)' }, 2.395);
    tl.to('#s7-o3', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.55)' }, 2.45);
    tl.to('#s7-o4', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.55)' }, 2.505);
    tl.to('#s7-nfm', { opacity: 1, scale: 1, y: 0, blur: 0, duration: 0.56, ease: 'expo.out' }, 2.4);
    tl.to('#s7-pill', { opacity: 1, y: 0, scale: 1, duration: 0.44, ease: 'back.out(1.45)' }, 3.12);
    tl.to('#s7-nfm', { scale: 1.045, duration: 0.78, ease: 'sine.inOut' }, 3.28);
    tl.to('#s7-o1', { x: 42, y: -18, duration: 0.78, ease: 'sine.inOut' }, 3.28);
    tl.to('#s7-o2', { x: -38, y: 22, rotation: 5, duration: 0.78, ease: 'sine.inOut' }, 3.28);
    tl.to('#s7-o3', { x: 38, y: 28, rotation: -4, duration: 0.78, ease: 'sine.inOut' }, 3.28);
    tl.to('#s7-o4', { x: -30, y: -26, duration: 0.78, ease: 'sine.inOut' }, 3.28);
  });
  return (
    <div id="s7-root" class="absolute inset-0 overflow-hidden bg-[#fffaf0] z-[6] text-[#0a0a0a]">
      <div id="s7-wash" class="absolute left-[-120px] top-[-120px] w-[2160px] h-[1320px] opacity-0 bg-[radial-gradient(circle_at_24%_30%,rgba(255,77,139,0.36),transparent_320px),radial-gradient(circle_at_77%_68%,rgba(184,164,237,0.38),transparent_380px),radial-gradient(circle_at_52%_48%,rgba(164,212,197,0.3),transparent_410px)]"></div>

      <div id="s7-fidea" class="absolute inset-0">
        <div id="s7-slab" class="absolute left-[210px] top-[164px] w-[910px] h-[620px] border-[6px] border-[#0a0a0a] rounded-[32px] bg-[#ffffff] rotate-[-2deg] shadow-[24px_28px_0_rgba(10,10,10,0.14)]">
          <text class="absolute left-[46px] top-[42px] font-[fl-sans-black] font-[900] text-[34px] leading-none text-[#ef00e4] whitespace-nowrap">design.md</text>
          <div id="s7-l1" class="absolute left-[46px] top-[142px] w-[690px] h-[24px] rounded-[999px] bg-[#ff4d8b]"></div>
          <div id="s7-l2" class="absolute left-[46px] top-[204px] w-[540px] h-[24px] rounded-[999px] bg-[rgba(10,10,10,0.13)]"></div>
          <div id="s7-l3" class="absolute left-[46px] top-[266px] w-[650px] h-[24px] rounded-[999px] bg-[#b8a4ed]"></div>
          <div id="s7-l4" class="absolute left-[46px] top-[328px] w-[470px] h-[24px] rounded-[999px] bg-[rgba(10,10,10,0.13)]"></div>
          <div id="s7-l5" class="absolute left-[46px] top-[390px] w-[610px] h-[24px] rounded-[999px] bg-[#a4d4c5]"></div>
        </div>
        {/* hf-studio offsets baked: NOT (97,-33), old-line (489,-29) */}
        <div id="s7-not" class="absolute left-[917px] top-[111px] px-[42px] pt-[24px] pb-[30px] border-[7px] border-[#0a0a0a] rounded-[999px] bg-[#e8b94a] font-[fl-sans-black] font-[900] text-[108px] leading-[0.8] rotate-[-12deg] shadow-[13px_14px_0_rgba(10,10,10,0.18)] opacity-0">
          <text class="whitespace-nowrap">NOT</text>
        </div>
        <text id="s7-old1" class="absolute left-[1219px] top-[505px] font-[fl-sans-black] font-[900] text-[104px] leading-[0.9] whitespace-nowrap opacity-0">a prettier</text>
        <text id="s7-old2" class="absolute left-[1219px] top-[598.6px] font-[fl-sans-black] font-[900] text-[104px] leading-[0.9] whitespace-nowrap opacity-0">design file</text>
        <div id="s7-slash" class="absolute left-[660px] top-[538px] w-[960px] h-[26px] rounded-[999px] border-[4px] border-[#0a0a0a] bg-[#ff4d8b] rotate-[-8deg] shadow-[0_10px_0_rgba(10,10,10,0.14)]"></div>
      </div>

      <div id="s7-ffield" class="absolute inset-0 opacity-0">
        <div class="absolute left-[280px] top-[132px] w-[1360px] h-[765px] border-[8px] border-[#0a0a0a] rounded-[46px] bg-[radial-gradient(circle_at_22%_38%,rgba(255,176,132,0.78),transparent_175px),radial-gradient(circle_at_72%_36%,rgba(184,164,237,0.88),transparent_240px),linear-gradient(135deg,#ff4d8b,#f9c1ff_48%,#a4d4c5)] shadow-[22px_25px_0_rgba(10,10,10,0.14)]"></div>
        <div id="s7-o1" class="absolute left-[182px] top-[300px] w-[172px] h-[172px] border-[5px] border-[#0a0a0a] rounded-full bg-[#b8a4ed] shadow-[10px_12px_0_rgba(10,10,10,0.16)] opacity-0"></div>
        <div id="s7-o2" class="absolute left-[1538px] top-[230px] w-[210px] h-[126px] border-[5px] border-[#0a0a0a] rounded-[999px] bg-[#e8b94a] rotate-[12deg] shadow-[10px_12px_0_rgba(10,10,10,0.16)] opacity-0"></div>
        <div id="s7-o3" class="absolute left-[232px] top-[820px] w-[245px] h-[104px] border-[5px] border-[#0a0a0a] rounded-[999px] bg-[#ffb084] rotate-[-10deg] shadow-[10px_12px_0_rgba(10,10,10,0.16)] opacity-0"></div>
        <div id="s7-o4" class="absolute left-[1512px] top-[760px] w-[154px] h-[154px] border-[5px] border-[#0a0a0a] rounded-full bg-[#a4d4c5] shadow-[10px_12px_0_rgba(10,10,10,0.16)] opacity-0"></div>
        <div class="absolute left-[210px] top-[230px] w-[1500px] text-center">
          <div id="s7-nfm" class="font-[fl-round] font-[900] text-[250px] leading-[0.82] [text-shadow:8px_9px_0_rgba(255,255,255,0.46)] opacity-0">
            <text class="whitespace-nowrap">NEW FORMAT</text>
          </div>
        </div>
        <div class="absolute left-[0px] top-[690px] w-[1920px] flex flex-row justify-center">
          <div id="s7-pill" class="px-[42px] pt-[18px] pb-[24px] rounded-[30px] border-[6px] border-[#0a0a0a] bg-[#1a3a3a] text-[#ffffff] font-[fl-sans-black] font-[900] text-[66px] leading-[0.9] rotate-[-2deg] shadow-[12px_14px_0_rgba(10,10,10,0.17)] opacity-0">
            <text class="whitespace-nowrap">for agents</text>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ════════════ SCENE 08 — close (52.034 → 60.254s) ════════════ */
var LOGO_BLACK_D = 'M0 16.6738H4.96V23.3838H11.53V16.6738H16.49V35.4538H11.53V27.6738H4.96V35.4538H0V16.6738Z M17.8899 28.0137C17.8899 23.4237 21.2399 20.0237 25.7999 20.0237C30.0099 20.0237 32.9399 23.0837 32.9399 27.5637C32.9399 28.2037 32.8599 28.8237 32.8299 29.3337H22.6099C23.0399 31.1337 24.6199 32.1237 27.1399 32.1237C28.7199 32.1237 30.2199 31.7237 31.2999 31.0237V34.6737C30.1699 35.3937 28.2999 35.8537 26.3599 35.8537C21.2599 35.8537 17.8799 32.6037 17.8799 28.0237L17.8899 28.0137ZM28.5699 26.1937C28.4899 24.5337 27.3899 23.5637 25.7499 23.5637C24.1099 23.5637 22.9599 24.5337 22.6099 26.1937H28.5699Z M37.85 33.8638L32 20.4238H37.29L40.46 28.1538L43.6 20.4238H48.48L40.33 39.5738H35.26L37.86 33.8638H37.85Z M48.1797 26.1138C48.1797 20.4238 52.7097 16.2438 58.9397 16.2438C61.2997 16.2438 63.5297 16.8038 64.8197 17.7238V21.9338C63.5597 20.9138 61.6797 20.2938 59.7497 20.2938C55.7797 20.2938 53.2597 22.6238 53.2597 26.1438C53.2597 29.6638 55.5697 31.8838 58.8897 31.8838C59.6397 31.8838 60.4997 31.7538 61.1997 31.5038V27.8838H57.4697V24.2338H65.5197V33.7038C63.6697 35.0938 61.1997 35.8738 58.5997 35.8738C52.5397 35.8738 48.1897 31.7438 48.1897 26.1138H48.1797Z M66.6604 28.0137C66.6604 23.4237 70.0104 20.0237 74.5704 20.0237C78.7804 20.0237 81.7104 23.0837 81.7104 27.5637C81.7104 28.2037 81.6304 28.8237 81.6004 29.3337H71.3804C71.8104 31.1337 73.3904 32.1237 75.9104 32.1237C77.4904 32.1237 78.9904 31.7237 80.0704 31.0237V34.6737C78.9404 35.3937 77.0704 35.8537 75.1304 35.8537C70.0304 35.8537 66.6504 32.6037 66.6504 28.0237L66.6604 28.0137ZM77.3404 26.1937C77.2604 24.5337 76.1604 23.5637 74.5204 23.5637C72.8804 23.5637 71.7304 24.5337 71.3804 26.1937H77.3404Z M82.9688 20.4238H87.8488V22.4138C88.7588 20.9638 90.3488 20.1038 92.3288 20.1038C95.4988 20.1038 97.3988 22.3338 97.3988 25.8138V35.4438H92.5188V26.8638C92.5188 25.0438 91.8188 24.1838 90.4788 24.1838C88.9488 24.1838 87.8488 25.3638 87.8488 27.2638V35.4438H82.9688V20.4238Z M252.115 72.502C250.192 72.502 248.268 72.0838 246.93 71.3728V65.6853C247.808 66.3126 248.937 66.689 250.066 66.689C251.739 66.689 252.784 65.8526 252.784 64.5562C252.784 63.8452 252.492 63.0925 251.655 62.0888L249.899 59.956C248.728 58.4923 248.226 56.9031 248.226 55.2721C248.226 51.0065 251.864 48.0791 256.883 48.0791C258.723 48.0791 260.438 48.4555 261.65 49.0828V54.7703C260.898 54.2266 259.768 53.8503 258.723 53.8503C257.092 53.8503 256.046 54.6867 256.046 55.8994C256.046 56.6522 256.423 57.405 257.259 58.4086L258.974 60.4996C260.187 62.047 260.73 63.5943 260.73 65.2671C260.73 69.5746 257.05 72.502 252.115 72.502Z M236.338 72.6269C228.392 72.6269 223.123 67.5667 223.123 60.4155C223.123 53.2643 228.351 47.9531 235.46 47.9531C242.026 47.9531 246.584 52.7206 246.584 59.7045C246.584 60.7082 246.459 61.6701 246.417 62.4647H230.483C231.152 65.2666 233.62 66.8139 237.551 66.8139C240.018 66.8139 242.36 66.1866 244.033 65.0993V70.7868C242.277 71.916 239.349 72.6269 236.338 72.6269ZM230.483 57.5717H239.767C239.642 54.9789 237.927 53.4734 235.376 53.4734C232.867 53.4734 231.027 54.9789 230.483 57.5717Z M184.998 72.0001V48.5809H192.609V51.6756C193.989 49.4173 196.373 48.0791 199.426 48.0791C202.521 48.0791 204.779 49.5428 206.075 52.052C207.497 49.5846 210.132 48.0791 213.352 48.0791C218.245 48.0791 221.214 51.592 221.214 57.0286V72.0001H213.603V58.5341C213.603 55.774 212.557 54.4357 210.634 54.4357C208.459 54.4357 206.912 56.234 206.912 59.0778V72.0001H199.3V58.5341C199.3 55.774 198.255 54.4357 196.373 54.4357C194.157 54.4357 192.609 56.234 192.609 59.0778V72.0001H184.998Z M167.26 72.4602C161.154 72.4602 156.596 67.2745 156.596 60.3324C156.596 53.3066 161.196 48.0791 167.302 48.0791C170.438 48.0791 173.031 49.4173 174.536 51.8429V48.5809H182.148V72.0001H174.536V68.4873C173.031 71.0801 170.438 72.4602 167.26 72.4602ZM169.393 66.1872C172.78 66.1872 174.536 63.3434 174.536 60.876V59.6632C174.536 57.2377 172.78 54.4357 169.393 54.4357C166.298 54.4357 164.207 56.7777 164.207 60.2905C164.207 63.7616 166.298 66.1872 169.393 66.1872Z M141.258 72V48.5808H148.827V52.9719C149.831 50.0445 151.796 48.2881 154.306 48.2881C154.975 48.2881 155.602 48.4135 156.02 48.6645V55.8993C155.435 55.5648 154.724 55.4393 153.887 55.4393C150.751 55.4393 148.869 57.6558 148.869 61.2941V72H141.258Z M122.643 71.9996V42.7256H139.371V49.124H130.379V54.477H138.911V60.5827H130.379V71.9996H122.643Z M105.645 72V48.5808H113.214V52.9719C114.218 50.0445 116.183 48.2881 118.692 48.2881C119.361 48.2881 119.989 48.4135 120.407 48.6645V55.8993C119.822 55.5648 119.111 55.4393 118.274 55.4393C115.138 55.4393 113.256 57.6558 113.256 61.2941V72H105.645Z M93.4397 72.6269C85.4939 72.6269 80.2246 67.5667 80.2246 60.4155C80.2246 53.2643 85.4521 47.9531 92.5615 47.9531C99.1273 47.9531 103.686 52.7206 103.686 59.7045C103.686 60.7082 103.56 61.6701 103.518 62.4647H87.5849C88.2541 65.2666 90.7214 66.8139 94.6525 66.8139C97.1199 66.8139 99.4618 66.1866 101.135 65.0993V70.7868C99.3782 71.916 96.4508 72.6269 93.4397 72.6269ZM87.5849 57.5717H96.869C96.7435 54.9789 95.0289 53.4734 92.4779 53.4734C89.9687 53.4734 88.1286 54.9789 87.5849 57.5717Z M53.3711 78.4404V48.5809H60.9823V52.052C62.446 49.501 65.0807 48.0791 68.3008 48.0791C74.3647 48.0791 78.9649 53.3066 78.9649 60.2487C78.9649 67.2745 74.3229 72.4602 68.259 72.4602C65.0807 72.4602 62.446 71.1637 60.9823 68.78V78.4404H53.3711ZM66.1262 66.1872C69.2627 66.1872 71.3119 63.7616 71.3119 60.2905C71.3119 56.8195 69.2627 54.4357 66.1262 54.4357C62.7388 54.4357 60.9823 57.2795 60.9823 59.7051V60.9178C60.9823 63.3434 62.7388 66.1872 66.1262 66.1872Z M31.9606 78.4405L36.0171 69.5329L26.9004 48.5811H35.1389L40.0737 60.6252L44.9666 48.5811H52.5779L39.8646 78.4405H31.9606Z M0 71.9996V42.7256H7.7367V53.1806H17.9826V42.7256H25.7193V71.9996H17.9826V59.8718H7.7367V71.9996H0Z';
var LOGO_ARROW1_D = 'M195.219 26.1937L213.529 38.9937C216.009 40.7237 220.239 38.7637 221.009 35.5337L228.419 4.33374C229.189 1.10374 225.879 -0.856262 222.589 0.873738L198.199 13.6737C192.649 16.5837 191.059 23.2837 195.219 26.1937Z';
var LOGO_ARROW2_D = 'M256.97 25.9638L232.58 38.7638C229.28 40.4938 225.98 38.5338 226.75 35.3038L234.16 4.10376C234.93 0.873757 239.16 -1.08624 241.64 0.643757L259.95 13.4438C264.12 16.3538 262.52 23.0538 256.97 25.9638Z';

function Scene8() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= S8 && ctx.global.time < S8 + D8;
    vis(ctx, 's8-root', on);
    if (!on) return;
    var tl = ctx.timeline({ delay: S8 });
    tl.set('#s8-softbg', { opacity: 0, scale: 1.08 });
    tl.set('#s8-dcard', { opacity: 0, x: -170, y: 38, rotation: -4, blur: 14 });
    tl.set('#s8-fcard', { opacity: 0, x: 170, y: 38, rotation: 4, blur: 14 });
    tl.set('#s8-plus', { opacity: 0, scale: 0.5, rotation: -28 });
    tl.set('#s8-dl1', { scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s8-dl2', { scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s8-dl3', { scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s8-dl4', { scaleX: 0, transformOrigin: '0px 50%' });
    tl.set('#s8-sw1', { opacity: 0, scale: 0.45, y: 18 });
    tl.set('#s8-sw2', { opacity: 0, scale: 0.45, y: 18 });
    tl.set('#s8-sw3', { opacity: 0, scale: 0.45, y: 18 });
    tl.set('#s8-sw4', { opacity: 0, scale: 0.45, y: 18 });
    tl.set('#s8-chipa', { opacity: 0, scale: 0.6 });
    tl.set('#s8-chipb', { opacity: 0, scale: 0.6 });
    tl.set('#s8-chipc', { opacity: 0, scale: 0.6 });
    tl.set('#s8-mpath', { strokeDasharray: 900, strokeDashoffset: 900 });
    tl.set('#s8-st1', { opacity: 0, y: 36, blur: 10 });
    tl.set('#s8-st2', { opacity: 0, y: 36, blur: 10 });
    tl.set('#s8-final', { opacity: 0, scale: 0.84, blur: 18 });
    tl.set('#s8-logo', { opacity: 0, scale: 0.22, blur: 24 });

    tl.to('#s8-softbg', { opacity: 1, scale: 1, duration: 0.65, ease: 'power2.out' }, 0.04);
    tl.to('#s8-dcard', { opacity: 1, x: 0, y: 0, rotation: -2, blur: 0, duration: 0.68, ease: 'expo.out' }, 0.14);
    tl.to('#s8-dl1', { scaleX: 1, duration: 0.32, ease: 'power3.out' }, 0.46);
    tl.to('#s8-dl2', { scaleX: 1, duration: 0.32, ease: 'power3.out' }, 0.515);
    tl.to('#s8-dl3', { scaleX: 1, duration: 0.32, ease: 'power3.out' }, 0.57);
    tl.to('#s8-dl4', { scaleX: 1, duration: 0.32, ease: 'power3.out' }, 0.625);
    tl.to('#s8-sw1', { opacity: 1, scale: 1, y: 0, duration: 0.38, ease: 'back.out(1.6)' }, 0.72);
    tl.to('#s8-sw2', { opacity: 1, scale: 1, y: 0, duration: 0.38, ease: 'back.out(1.6)' }, 0.77);
    tl.to('#s8-sw3', { opacity: 1, scale: 1, y: 0, duration: 0.38, ease: 'back.out(1.6)' }, 0.82);
    tl.to('#s8-sw4', { opacity: 1, scale: 1, y: 0, duration: 0.38, ease: 'back.out(1.6)' }, 0.87);
    tl.to('#s8-st1', { opacity: 1, y: 0, blur: 0, duration: 0.44, ease: 'power3.out' }, 1.02);
    tl.to('#s8-plus', { opacity: 1, scale: 1, rotation: 0, duration: 0.46, ease: 'back.out(1.7)' }, 1.48);
    tl.to('#s8-fcard', { opacity: 1, x: 0, y: 0, rotation: 2, blur: 0, duration: 0.7, ease: 'expo.out' }, 1.72);
    tl.to('#s8-chipa', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.5)' }, 2.04);
    tl.to('#s8-chipb', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.5)' }, 2.12);
    tl.to('#s8-chipc', { opacity: 1, scale: 1, duration: 0.42, ease: 'back.out(1.5)' }, 2.2);
    tl.to('#s8-mpath', { strokeDashoffset: 0, duration: 1.05, ease: 'power2.inOut' }, 2.28);
    tl.to('#s8-st2', { opacity: 1, y: 0, blur: 0, duration: 0.5, ease: 'power3.out' }, 2.58);
    tl.to('#s8-chipa', { x: 58, y: -16, rotation: 3, duration: 1.25, ease: 'sine.inOut' }, 2.66);
    tl.to('#s8-chipb', { x: -42, y: 24, scale: 1.06, duration: 1.25, ease: 'sine.inOut' }, 2.66);
    tl.to('#s8-chipc', { x: 34, y: -28, rotation: -5, duration: 1.25, ease: 'sine.inOut' }, 2.66);
    tl.to('#s8-stage', { opacity: 0, scale: 0.94, blur: 22, duration: 0.6, ease: 'power3.inOut' }, 4.0);
    tl.to('#s8-final', { opacity: 1, scale: 1, blur: 0, duration: 0.62, ease: 'expo.out' }, 4.08);
    tl.to('#s8-final-em', { scale: 1.035, duration: 0.56, ease: 'sine.inOut' }, 4.68);
    tl.to('#s8-final', { opacity: 0, scale: 5.2, blur: 30, duration: 0.62, ease: 'power4.in' }, 6.72);
    tl.to('#s8-softbg', { opacity: 0.42, scale: 1.18, duration: 0.72, ease: 'power3.inOut' }, 6.72);
    tl.to('#s8-logo', { opacity: 1, scale: 1, blur: 0, duration: 0.68, ease: 'expo.out' }, 7.04);
    tl.to('#s8-logo', { scale: 1.018, duration: 0.78, ease: 'sine.inOut' }, 7.44);

    // statement spans: absolute placement of the two offset/rotated words
    var m = makeMeasure(ctx);
    var w1 = m.w('Nunito', 900, 86, 'what the brand is');
    var w2 = m.w('Nunito', 900, 86, 'how it moves');
    var start = 960 - (w1 + w2 + 40) / 2;
    tl.set('#s8-st1', { left: start + 20 - 166, top: 912.3 - 159, rotation: -2, transformOrigin: '50% 50%' });
    tl.set('#s8-st2', { left: start + w1 + 60 + 67, top: 912.3 - 149, rotation: 3, transformOrigin: '50% 50%' });
  });
  return (
    <div id="s8-root" class="absolute inset-0 overflow-hidden bg-[#fffaf0] z-[7] text-[#0a0a0a]">
      <div id="s8-softbg" class="absolute left-[-120px] top-[-120px] w-[2160px] h-[1320px] opacity-0 bg-[radial-gradient(circle_at_18%_32%,rgba(255,77,139,0.22),transparent_340px),radial-gradient(circle_at_78%_28%,rgba(184,164,237,0.26),transparent_360px),radial-gradient(circle_at_54%_78%,rgba(232,185,74,0.22),transparent_330px)]"></div>
      <div id="s8-stage" class="absolute inset-0">
        <div id="s8-dcard" class="absolute left-[128px] top-[150px] w-[700px] h-[540px] border-[6px] border-[#0a0a0a] rounded-[34px] bg-[#ffffff] rotate-[-2deg] shadow-[18px_21px_0_rgba(10,10,10,0.13)] overflow-hidden opacity-0">
          <text class="absolute left-[36px] top-[34px] font-[fl-sans-black] font-[900] text-[48px] leading-none text-[#ef00e4] whitespace-nowrap">design.md</text>
          <div id="s8-dl1" class="absolute left-[42px] top-[142px] w-[430px] h-[22px] rounded-[999px] bg-[#ff4d8b]"></div>
          <div id="s8-dl2" class="absolute left-[42px] top-[196px] w-[552px] h-[22px] rounded-[999px] bg-[rgba(10,10,10,0.15)]"></div>
          <div id="s8-dl3" class="absolute left-[42px] top-[250px] w-[370px] h-[22px] rounded-[999px] bg-[#b8a4ed]"></div>
          <div id="s8-dl4" class="absolute left-[42px] top-[304px] w-[500px] h-[22px] rounded-[999px] bg-[rgba(10,10,10,0.15)]"></div>
          <div class="absolute left-[42px] top-[418px] flex flex-row gap-[18px]">
            <div id="s8-sw1" class="w-[68px] h-[68px] rounded-full border-[4px] border-[#0a0a0a] bg-[#ff4d8b] opacity-0"></div>
            <div id="s8-sw2" class="w-[68px] h-[68px] rounded-full border-[4px] border-[#0a0a0a] bg-[#1a3a3a] opacity-0"></div>
            <div id="s8-sw3" class="w-[68px] h-[68px] rounded-full border-[4px] border-[#0a0a0a] bg-[#b8a4ed] opacity-0"></div>
            <div id="s8-sw4" class="w-[68px] h-[68px] rounded-full border-[4px] border-[#0a0a0a] bg-[#e8b94a] opacity-0"></div>
          </div>
        </div>
        <div id="s8-plus" class="absolute left-[905px] top-[410px] w-[110px] h-[110px] border-[6px] border-[#0a0a0a] rounded-full bg-[#e8b94a] flex flex-row items-center justify-center font-[fl-sans-black] font-[900] text-[82px] leading-none shadow-[10px_12px_0_rgba(10,10,10,0.14)] opacity-0">
          <text>+</text>
        </div>
        <div id="s8-fcard" class="absolute left-[1092px] top-[150px] w-[700px] h-[540px] border-[6px] border-[#0a0a0a] rounded-[34px] rotate-[2deg] shadow-[18px_21px_0_rgba(10,10,10,0.13)] overflow-hidden opacity-0 bg-[radial-gradient(circle_at_50%_54%,rgba(255,77,139,0.56),transparent_205px),linear-gradient(135deg,#b8a4ed,#a4d4c5)]">
          <text class="absolute left-[36px] top-[34px] font-[fl-sans-black] font-[900] text-[48px] leading-none text-[#ef00e4] whitespace-nowrap">frame.md</text>
          <div class="absolute left-[58px] top-[132px] w-[584px] h-[329px] border-[7px] border-[#0a0a0a] rounded-[24px] bg-[#fffaf0] overflow-hidden">
            <div id="s8-chipa" class="absolute left-[48px] top-[86px] w-[210px] h-[76px] border-[4px] border-[#0a0a0a] rounded-[999px] bg-[#ff4d8b] rotate-[-8deg] shadow-[8px_9px_0_rgba(10,10,10,0.16)] opacity-0"></div>
            <div id="s8-chipb" class="absolute left-[360px] top-[168px] w-[178px] h-[178px] border-[4px] border-[#0a0a0a] rounded-full bg-[#ffb084] shadow-[8px_9px_0_rgba(10,10,10,0.16)] opacity-0"></div>
            <div id="s8-chipc" class="absolute left-[170px] top-[201px] w-[260px] h-[70px] border-[4px] border-[#0a0a0a] rounded-[999px] bg-[#1a3a3a] rotate-[7deg] shadow-[8px_9px_0_rgba(10,10,10,0.16)] opacity-0"></div>
            <path id="s8-mpath" class="absolute left-[74px] top-[118px] w-[520px] h-[260px] stroke-[#ef00e4] stroke-[9]" d="M 30 198 C 142 64, 245 72, 306 138 S 418 232, 494 70" view-box="0 0 520 260" />
          </div>
        </div>
        {/* statement spans (offset/rotate baked via set in the hook) */}
        <text id="s8-st1" class="absolute left-[0px] top-[753.3px] font-[fl-round] font-[900] text-[86px] leading-[0.95] whitespace-nowrap opacity-0">what the brand is</text>
        <text id="s8-st2" class="absolute left-[0px] top-[763.3px] font-[fl-round] font-[900] text-[86px] leading-[0.95] text-[#ff4d8b] [-webkit-text-stroke:3px_#0a0a0a] whitespace-nowrap opacity-0">how it moves</text>
      </div>
      <div id="s8-final" class="absolute left-[170px] top-[414.45px] w-[1580px] text-center font-[fl-round] font-[900] text-[146px] leading-[0.86] opacity-0">
        <text class="whitespace-nowrap">Your brand,</text>
        <div id="s8-final-em" class="text-[#ff4d8b] [-webkit-text-stroke:4px_#0a0a0a]">
          <text class="whitespace-nowrap">translated for motion.</text>
        </div>
      </div>
      <div id="s8-logo" class="absolute left-[310px] top-[344.75px] w-[1300px] h-[390.5px] opacity-0">
        <path class="absolute inset-0 w-full h-full fill-[#000000]" d={LOGO_BLACK_D} view-box="0 0 263 79" />
        <path class="absolute inset-0 w-full h-full fill-[#2ADFAE]" d={LOGO_ARROW1_D} view-box="0 0 263 79" />
        <path class="absolute inset-0 w-full h-full fill-[#2ADFAE]" d={LOGO_ARROW2_D} view-box="0 0 263 79" />
      </div>
    </div>
  );
}

/* ════════════ QUIET CAPTIONS (4 → 60.254s) ════════════ */
function Cap(p) {
  var dark = p.dark ? ' bg-[rgba(12,12,13,0.72)] border-[rgba(255,255,255,0.13)] text-[#f7f7f5] shadow-[0_14px_38px_rgba(0,0,0,0.28)]'
    : ' bg-[rgba(247,247,245,0.96)] border-[rgba(0,0,0,0.11)] text-[#080808] shadow-[0_12px_34px_rgba(0,0,0,0.12)]';
  return (
    <div id={p.id} class="absolute left-[0px] bottom-[54px] w-[1920px] flex flex-row justify-center opacity-0">
      <div class={'px-[38px] pt-[23px] pb-[25px] rounded-[22px] border-[1px] font-[fl-sans-bold] font-[700] text-[43px] leading-[1.08] text-center whitespace-nowrap' + dark}>
        <text>{p.t}</text>
      </div>
    </div>
  );
}

function Caps() {
  useGSAP(function (ctx) {
    var on = ctx.global.time >= SC && ctx.global.time < SC + DC;
    vis(ctx, 'caps-root', on);
    if (!on) return;
    var tl = ctx.timeline();
    ['#cap0', '#cap1', '#cap2', '#cap3', '#cap4', '#cap5'].forEach(function (id) {
      tl.set(id, { opacity: 0, y: 18, blur: 6, scale: 0.98 });
    });
    var show = function (id, start, end) {
      tl.to(id, { opacity: 0.88, y: 0, scale: 1, blur: 0, duration: 0.22, ease: 'power2.out' }, start);
      tl.to(id, { opacity: 0, y: 10, scale: 0.99, blur: 5, duration: 0.18, ease: 'power2.in' }, Math.max(start + 0.22, end - 0.18));
    };
    // windows are quiet-captions-local in the reference; +4s scene offset
    show('#cap0', 3.84 + 4, 5.34 + 4);
    show('#cap1', 5.54 + 4, 6.58 + 4);
    show('#cap2', 6.72 + 4, 7.84 + 4);
    show('#cap3', 7.94 + 4, 9.34 + 4);
    show('#cap4', 9.76 + 4, 10.72 + 4);
    show('#cap5', 10.82 + 4, 12.08 + 4);
  });
  return (
    <div id="caps-root" class="absolute inset-0 z-[10]">
      <Cap id="cap0" t="it often builds a presentation." />
      <Cap id="cap1" t="That's not random." />
      <Cap id="cap2" t="Most design specs were written" />
      <Cap id="cap3" t="for websites and products." />
      <Cap id="cap4" t="They describe buttons," dark />
      <Cap id="cap5" t="cards, navs, grids, spacing." dark />
    </div>
  );
}

/* ════════════ ROOT ════════════ */
export default function App() {
  return (
    <div id="fmdl-root" class="relative w-[1920px] h-[1080px] overflow-hidden bg-[#f7f7f5]">
      <Scene2 />
      <Scene3 />
      <Scene4 />
      <Scene5 />
      <Scene6 />
      <Scene7 />
      <Scene8 />
      <Caps />
      <Scene1 />
    </div>
  );
}
