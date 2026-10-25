// JSX port of the figma-launch reference film (index.html + compositions/act1..11).
//
// Shape: one 1080x1080 stage; the six per-act brand-pattern bg videos plus the
// two output-card videos sit at their reference z-order, and each act is a
// component whose GSAP hook keeps the reference timeline verbatim, shifted to
// global time (local T = ctx.global.time - ACT start). Media windows ride the video
// node's data-start/data-duration/data-media-start fields (this lane added
// those to the JSONL video line — markup parity with hyperframes). The
// engine's Transform::Rotate3 carries the act-1/2 dimensional pops
// (transformPerspective + rotationX/rotationY fold into one projective
// concat). Geometry the reference measures from the live DOM (act-1 cold-open
// dock, act-2 zoom ladder, every body-text wrap) is re-measured here from the
// same font files through a 1x1 measure surface plus a greedy line-break
// model, cached on globalThis. Act-2's blurred backdrops are pre-baked PNGs
// (gaussian 34px + brightness/saturate + flips) because the engine has no
// element-level CSS blur. Audio: the reference schedules 7 click SFX; the
// audio plan has no per-clip absolute start, so they are dropped — k3diff is
// pixel-only and every visual element is ported.

import { gsap } from 'gsap';
import { CustomEase } from 'gsap/CustomEase';
import { useGSAP } from 'opencat/runtime';

/* Act-11 flight curves (reference cubic-beziers) as official CustomEases,
 * registered once at module load; names resolve from the resident gsap
 * module on every per-frame timeline rebuild. */
gsap.registerPlugin(CustomEase);
CustomEase.create('markFlightX', 'M0,0 C0.6,-0.28 0.85,0.62 1,1');
CustomEase.create('markFlightY', 'M0,0 C0.2,0.6 0.4,1.1 1,1');
CustomEase.create('markShrink', 'M0,0 C0.7,0.05 0.85,1 1,1');

export const meta = { width: 1080, height: 1080, fps: 30, duration: 64.0666 };

export const fonts = {
  default: 'fl-sans',
  faces: [
    { id: 'fl-sans', family: 'TT Norms Pro', path: 'assets/fonts/TTNormsPro-Regular.ttf', role: 'sans' },
    { id: 'fl-sans-medium', family: 'TT Norms Pro', path: 'assets/fonts/TTNormsPro-Medium.ttf' },
    { id: 'fl-sans-bold', family: 'TT Norms Pro', path: 'assets/fonts/TTNormsPro-Bold.ttf' },
    { id: 'fl-mono', family: 'TT Norms Mono', path: 'assets/fonts/TTNormsMono-Regular.ttf', role: 'mono' },
    { id: 'fl-display', family: 'ABC Solar Display', path: 'assets/fonts/ABCSolarDisplay-Bold.ttf' },
    { id: 'fl-inter', family: 'Inter', path: 'assets/fonts/Inter-Regular.ttf' },
    { id: 'fl-inter-medium', family: 'Inter', path: 'assets/fonts/Inter-Medium.ttf' },
    { id: 'fl-segoe-bold', family: 'Segoe UI', path: 'assets/fonts/SegoeUI-Bold.ttf' },
  ],
};

/* ════════════ act windows (master time) ════════════ */
var A1 = { s: 0.0 };
var A2 = { s: 11.8 };
var A3 = { s: 14.7 };
var A4 = { s: 15.8 };
var A5 = { s: 18.0 };
var A6 = { s: 35.95 };
var A7 = { s: 41.15 };
var A8 = { s: 44.15 };
var A9 = { s: 46.15 };
var A10 = { s: 55.85 };
var A11 = { s: 59.15 };

/* video visibility windows — the engine parks a finished video on its last
   frame, so each clip is gated off where the reference composition ends */
var VID_WIN = {
  'bg-act1': [3.4, 11.8],
  'bg-act34': [14.7, 18.0],
  'bg-act5': [21.5, 35.95],
  'output-vid': [25.95, 35.95],
  'bg-act78': [41.15, 46.15],
  'bg-act9': [46.15, 55.85],
  'tail-vid': [52.95, 55.85],
  'bg-act1011': [55.85, 64.05],
};

var TTN = '"TT Norms Pro"';
var TTNM = '"TT Norms Mono"';
var ABCD = '"ABC Solar Display"';

function vis(ctx, id, on) { ctx.getNode(id).opacity(on ? 1 : 0); }
function clamp(v, lo, hi) { return hi < lo ? (lo + hi) / 2 : Math.min(hi, Math.max(lo, v)); }
function easeInOutCubic(p) { return p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2; } // power2.inOut

/* advance width through the shared 1x1 measure surface (never touches pixels) */
function makeMeasure(ctx) {
  var MSR = ctx.surface(1, 1);
  return function adv(family, weight, fs, ls, s) {
    MSR.font = weight + ' ' + fs + 'px ' + family;
    MSR.letterSpacing = (ls || 0) + 'px';
    return MSR.measureText(s).width;
  };
}

/* greedy wrap over pre-measured atoms (widths exclude the joining space).
   Returns rows of {i, x}; a highlight atom carries its own padded width and
   its box is drawn at x-10 (reference .hlwrap margin-left:-10px). */
function wrapAtoms(atoms, lineW, spaceW) {
  var rows = [], row = null, x = 0;
  for (var i = 0; i < atoms.length; i++) {
    var w = atoms[i].w;
    if (row === null) { row = [{ i: i, x: 0 }]; x = w; }
    else if (x + spaceW + w <= lineW + 0.01) { row.push({ i: i, x: x + spaceW }); x = x + spaceW + w; }
    else { rows.push(row); row = [{ i: i, x: 0 }]; x = w; }
  }
  if (row !== null) rows.push(row);
  return rows;
}
function rowString(atoms, row) {
  var s = '';
  for (var k = 0; k < row.length; k++) s += (k > 0 ? ' ' : '') + atoms[row[k].i].t;
  return s;
}

/* ════════════ shared tweet-card furniture (acts 1 & 2) ════════════
   act1.html CSS: left 30, w 1020, padding 44 50 36, radius 30; head h 74;
   body top 142, 38px, line-height 1.38 = 52.44; meta 24px (TT Norms Pro
   normal line-height 1.28 = 30.72); row mt 21 pt 19, icons 29. */
var LINE = 52.44;
var BODY_TOP = 142;
var META_H = 30.72;
var CARD_INNER = 920;
var HL_H = 59.44; // line 52.44 + padding 2/5

function metaTop(lines) { return 44 + 74 + 24 + lines * LINE + 21; }
/* rows containing a .hlwrap grow the reference's line box by 7px each */
function hlRowCount(rows, atoms) {
  var n = 0;
  for (var i = 0; i < rows.length; i++) {
    for (var k = 0; k < rows[i].length; k++) {
      if (atoms[rows[i][k].i].hl) { n++; break; }
    }
  }
  return n;
}

var X_PATH = 'M18.244 2.25h3.308l-7.227 8.26 8.502 11.24H16.17l-5.214-6.817L4.99 21.75H1.68l7.73-8.835L1.254 2.25H8.08l4.713 6.231zm-1.161 17.52h1.833L7.084 4.126H5.117z';
var ICON_REPLY = 'M8 12h.01M12 12h.01M16 12h.01M21 12c0 4.4-4 8-9 8-1.5 0-2.9-.3-4.1-.9L3 20l1.1-3.6C3.4 15.1 3 13.6 3 12c0-4.4 4-8 9-8s9 3.6 9 8z';
var ICON_RT = 'M17 2l4 4-4 4M21 6H8a4 4 0 0 0-4 4v1M7 22l-4-4 4-4M3 18h13a4 4 0 0 0 4-4v-1';
var ICON_LIKE = 'M12 21s-7.5-4.6-9.7-9C.8 8.9 2.4 5.6 5.5 5c1.9-.4 3.9.4 5 2 .6.9.9 1.6 1.5 1.6s.9-.7 1.5-1.6c1.1-1.6 3.1-2.4 5-2 3.1.6 4.7 3.9 3.2 7-2.2 4.4-9.7 9-9.7 9z';
var CURSOR_D = 'M6 4 L6 23 L11 18 L14.5 25 L17.5 23.6 L14 17 L21 17 Z';

function GlassCard({ glassId, blurClass, satClass, bgClass }) {
  return (
    <div id={glassId} class={'absolute left-0 top-0 w-full h-full rounded-[30px] border-[1px] border-[rgba(190,255,205,0.32)] ' + (bgClass || 'bg-[linear-gradient(120deg,rgba(255,255,255,0.10),rgba(255,255,255,0.045)_45%,rgba(180,240,200,0.06))]') + ' ' + (satClass || 'saturate-[1.15]') + ' ' + (blurClass || 'backdrop-blur-[13px]') + ' shadow-[0_30px_70px_rgba(0,0,0,0.45)]'}></div>
  );
}

function CardHead({ av, name, handle }) {
  return (
    <div class="absolute left-[50px] top-[44px] w-[920px] h-[74px] flex items-center gap-[20px]">
      <div class={'w-[74px] h-[74px] rounded-full flex-none bg-[linear-gradient(135deg,' + av + ')]'}></div>
      <div class="flex-1">
        <text class="font-[fl-sans-bold] font-[700] text-[#ffffff] text-[30px] leading-[34.5px]">{name}</text>
        <text class="font-[fl-sans] text-[rgba(245,246,244,0.5)] text-[26px] leading-[29.9px]">{handle}</text>
      </div>
      <path view-box="0 0 24 24" class="w-[38px] h-[38px] flex-none fill-[#ffffff] opacity-90" d={X_PATH} />
    </div>
  );
}

function CardFoot({ footId, m0, m1, views, s0, s1, s2 }) {
  return (
    <div id={footId} class="absolute left-[50px] w-[920px]">
      <div class="flex whitespace-nowrap font-[fl-sans] text-[rgba(245,246,244,0.5)] text-[24px] leading-[30.72px]">
        <text>{m0}</text>
        <text>{' · '}</text>
        <text>{m1}</text>
        <text>{' · '}</text>
        <text class="font-[fl-sans-bold] font-[700] text-[rgba(245,246,244,0.72)]">{views}</text>
        <text>{' Views'}</text>
      </div>
      <div class="flex gap-[90px] mt-[21px] pt-[19px] border-t-[1px] border-[rgba(255,255,255,0.12)] whitespace-nowrap font-[fl-sans] text-[rgba(245,246,244,0.5)] text-[24px] leading-[30.72px]">
        <div class="flex items-center gap-[12px]">
          <path view-box="0 0 24 24" class="w-[29px] h-[29px] stroke-[rgba(245,246,244,0.5)] stroke-[1.8]" d={ICON_REPLY} />
          <text>{s0}</text>
        </div>
        <div class="flex items-center gap-[12px]">
          <path view-box="0 0 24 24" class="w-[29px] h-[29px] stroke-[rgba(245,246,244,0.5)] stroke-[1.8]" d={ICON_RT} />
          <text>{s1}</text>
        </div>
        <div class="flex items-center gap-[12px]">
          <path view-box="0 0 24 24" class="w-[29px] h-[29px] stroke-[rgba(245,246,244,0.5)] stroke-[1.8]" d={ICON_LIKE} />
          <text>{s2}</text>
        </div>
      </div>
    </div>
  );
}

/* body block: 3 absolute nowrap rows + highlight boxes; texts filled by hooks */
function CardBody({ id, hlCount, top }) {
  return (
    <div id={id} class={'absolute left-[50px] top-[' + (top || 142) + 'px] w-[920px] h-[157.32px]'}>
      {hlCount >= 1 ? <div id={id + '-hl1'} class="absolute rounded-[8px] bg-[linear-gradient(90deg,rgba(94,241,124,0.42),rgba(79,214,255,0.30))] [transform-origin:0_50%] opacity-0"></div> : null}
      {hlCount >= 2 ? <div id={id + '-hl2'} class="absolute rounded-[8px] bg-[linear-gradient(90deg,rgba(94,241,124,0.42),rgba(79,214,255,0.30))] [transform-origin:0_50%] opacity-0"></div> : null}
      <text id={id + '-r0'} class="absolute left-0 top-0 whitespace-nowrap font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px]"></text>
      <text id={id + '-r1'} class="absolute left-0 top-[52.44px] whitespace-nowrap font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px]"></text>
      <text id={id + '-r2'} class="absolute left-0 top-[104.88px] whitespace-nowrap font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px]"></text>
    </div>
  );
}

/* write measured rows + highlight boxes into a CardBody; highlight atoms
   (atoms[i].hl) render at x-10 with their padded width.
   The reference's .hlwrap is an inline-block with padding 2px 10px 5px: its
   line box grows to 59.44px (bottom+5 wins over the baseline fit), so every
   row AFTER a highlighted row sits 7px lower — meta/foot follow via margins. */
function writeRows(ctx, id, atoms, rows, hlNames) {
  var hlIndex = 0;
  var shift = 0;
  for (var r = 0; r < 3; r++) {
    var tnode = ctx.getNode(id + '-r' + r);
    if (r < rows.length) {
      tnode.top(r * LINE + shift);
      tnode.text(rowString(atoms, rows[r]));
      tnode.opacity(1);
      var rowHasHl = false;
      for (var k = 0; k < rows[r].length; k++) {
        var a = atoms[rows[r][k].i];
        if (a.hl && hlIndex < hlNames.length) {
          var hl = ctx.getNode(id + '-' + hlNames[hlIndex]);
          hl.left(rows[r][k].x - 10); hl.top(r * LINE + shift - 2); hl.width(a.w); hl.height(HL_H);
          hl.opacity(1);
          hlIndex++;
          rowHasHl = true;
        }
      }
      if (rowHasHl) shift += 7;
    } else {
      tnode.text(''); tnode.opacity(0);
    }
  }
  for (; hlIndex < hlNames.length; hlIndex++) ctx.getNode(id + '-' + hlNames[hlIndex]).opacity(0);
}

function words(adv, s) {
  var out = [];
  var parts = s.split(' ');
  for (var i = 0; i < parts.length; i++) out.push({ t: parts[i], w: adv(TTN, '400', 38, -0.19, parts[i]) });
  return out;
}

/* ════════════ ACT 1 · cold open + hero tweets (0 → 11.8, z1) ════════════ */
function Act1() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A1.s && T < A2.s;
    vis(ctx, 'sec-act1', ON);
    if (!ON) return;
    var t = T - A1.s;

    /* ── measured geometry — the reference's function-based values ── */
    var G = globalThis.__flA1 || (globalThis.__flA1 = (function () {
      var adv = makeMeasure(ctx);
      var sq = adv(TTN, '400', 38, -0.19, ' ');
      var m1 = adv(TTN, '400', 38, -0.19, 'hyperframes is genuinely great');
      var butW = adv(TTN, '400', 38, -0.19, 'but') + 9;
      var K = 1.5;
      // page rect of line 1: card(30,301) + pad(50,142); line box centre
      var pcx = 30 + 50 + m1 / 2, pcy = 301 + BODY_TOP + LINE / 2;
      var dx1 = 540 - 30 - K * (pcx - 30);
      var dy1 = 500 - 301 - K * (pcy - 301);
      var dx2 = dx1 - (K * butW) / 2;
      // card 1 rest line: [highlight, why would I ship …]
      var hl1 = { t: "it can't use my actual designs.", w: adv(TTN, '400', 38, -0.19, "it can't use my actual designs.") + 20, hl: true };
      var rest = [hl1].concat(words(adv, "why would I ship a launch video that doesn't look like my product?"));
      var restRows = wrapAtoms(rest, CARD_INNER, sq);
      // card 2: cool tool. still | HL_A | HL_B | without babysitting every frame.
      var c2 = words(adv, 'cool tool. still')
        .concat([
          { t: "couldn't get it to", w: adv(TTN, '400', 38, -0.19, "couldn't get it to") + 20, hl: true },
          { t: 'match what we built in Figma', w: adv(TTN, '400', 38, -0.19, 'match what we built in Figma') + 20, hl: true },
        ], words(adv, 'without babysitting every frame.'));
      var c2Rows = wrapAtoms(c2, CARD_INNER, sq);
      var c3hlW = adv(TTN, '400', 38, -0.19, "inventing a UI we don't have.") + 20;
      var lines1 = 1 + restRows.length;
      var nHl1 = hlRowCount(restRows, rest), nHl2 = hlRowCount(c2Rows, c2);
      return {
        m1: m1, dx1: dx1, dy1: dy1, dx2: dx2,
        rest: rest, restRows: restRows, c2: c2, c2Rows: c2Rows, c3hlW: c3hlW,
        h1: 44 + 74 + 24 + lines1 * LINE + 7 * nHl1 + 21 + META_H + 21 + 19 + META_H + 36,
        h2: 44 + 74 + 24 + c2Rows.length * LINE + 7 * nHl2 + 21 + META_H + 21 + 19 + META_H + 36,
        h3: 44 + 74 + 24 + 2 * LINE + 7 + 21 + META_H + 21 + 19 + META_H + 36,
        mt1: metaTop(lines1) + 7 * nHl1, mt2: metaTop(c2Rows.length) + 7 * nHl2, mt3: metaTop(2) + 7,
      };
    })());

    writeRows(ctx, 'act1-rest', G.rest, G.restRows, ['hl1']);
    writeRows(ctx, 'act1-c2body', G.c2, G.c2Rows, ['hl1', 'hl2']);
    var c3hl = ctx.getNode('act1-c3body-hl1');
    c3hl.left(-10); c3hl.top(LINE - 2); c3hl.width(G.c3hlW); c3hl.height(HL_H);
    c3hl.opacity(1);
    // card chrome heights follow the measured line counts
    ctx.getNode('act1-card1').height(G.h1);
    ctx.getNode('act1-card2').height(G.h2);
    ctx.getNode('act1-card3').height(G.h3);
    ctx.getNode('act1-c1foot').top(G.mt1);
    ctx.getNode('act1-c2foot').top(G.mt2);
    ctx.getNode('act1-c3foot').top(G.mt3);

    var tl = gsap.timeline();
    // ── 0.0 cold pose (black void, chrome hidden) ──
    tl.set('act1-card1', { x: G.dx1, y: G.dy1, scale: 1.5, transformOrigin: '0px 0px' }, A1.s);
    // ── 0.15–1.05 line slides in R→L inside its mask ──
    tl.fromTo('act1-l1slide', { x: G.m1 * 1.05 }, { x: 0, duration: 0.9, ease: 'power4.out' }, A1.s + 0.15);
    // ── 1.8–2.6 line nudges left as "but" cascades in, letter by letter ──
    tl.to('act1-card1', { x: G.dx2, duration: 0.7, ease: 'power2.inOut' }, A1.s + 1.8);
    tl.fromTo('act1-b1', { x: 130, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power4.out' }, A1.s + 1.85);
    tl.fromTo('act1-b2', { x: 130, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power4.out' }, A1.s + 1.99);
    tl.fromTo('act1-b3', { x: 130, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power4.out' }, A1.s + 2.13);
    // ── 3.5–4.2 THE DOCK ──
    tl.to('act1-card1', { x: 0, y: 0, scale: 1, duration: 0.7, ease: 'power3.inOut' }, A1.s + 3.5);
    tl.to('act1-ground', { opacity: 0, duration: 0.65, ease: 'power2.out' }, A1.s + 3.5);
    tl.fromTo('act1-bgwrap', { opacity: 0 }, { opacity: 1, duration: 0.65, ease: 'power2.out' }, A1.s + 3.55);
    tl.fromTo('act1-c1glass', { opacity: 0 }, { opacity: 1, duration: 0.55, ease: 'power2.out' }, A1.s + 3.6);
    tl.fromTo('act1-c1head', { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.5, ease: 'power3.out' }, A1.s + 3.78);
    tl.fromTo('act1-rest', { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.5, ease: 'power3.out' }, A1.s + 3.88);
    tl.fromTo('act1-c1foot', { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power3.out' }, A1.s + 4.05);
    // ── dimensional pop (perspective + rotateY/X → engine Rotate3 concat) ──
    tl.set('act1-card1', { transformPerspective: 1100, transformOrigin: '50% 50%' }, A1.s + 4.29);
    tl.to('act1-card1', { scale: 1.045, duration: 0.9, ease: 'expo.out' }, A1.s + 4.3);
    tl.to('act1-card1', { rotationY: -7, rotationX: 6, duration: 1.35, ease: 'power2.out' }, A1.s + 4.3);
    // no shine on card 1 — the reference's #act1-shine1 target doesn't exist in its DOM
    // ── 6.5 card1 → card2 (cut-the-curve LEFT, zero overlap) ──
    tl.to('act1-card1', { x: -1400, duration: 0.42, ease: 'power4.in' }, A1.s + 6.5);
    tl.set('act1-card1', { opacity: 0 }, A1.s + 6.93);
    tl.fromTo('act1-card2', { x: 1400 }, { x: 0, duration: 0.5, ease: 'power4.out' }, A1.s + 6.94);
    tl.set('act1-card2', { transformPerspective: 1100, transformOrigin: '50% 50%' }, A1.s + 7.53);
    tl.to('act1-card2', { scale: 1.045, duration: 0.9, ease: 'expo.out' }, A1.s + 7.54);
    tl.to('act1-card2', { rotationY: -7, rotationX: 6, duration: 1.35, ease: 'power2.out' }, A1.s + 7.54);
    tl.fromTo('act1-shine2', { x: -320 }, { x: 1340, duration: 0.85, ease: 'power2.out' }, A1.s + 7.56);
    // ── 9.3 card2 → card3 ──
    tl.to('act1-card2', { x: -1400, duration: 0.42, ease: 'power4.in' }, A1.s + 9.3);
    tl.set('act1-card2', { opacity: 0 }, A1.s + 9.73);
    tl.fromTo('act1-card3', { x: 1400 }, { x: 0, duration: 0.5, ease: 'power4.out' }, A1.s + 9.74);
    // ── highlight sweeps (origin 0 50% via class on the boxes) ──
    tl.fromTo('act1-rest-hl1', { scaleX: 0 }, { scaleX: 1, duration: 0.7, ease: 'power2.inOut' }, A1.s + 4.6);
    tl.fromTo('act1-c2body-hl1', { scaleX: 0 }, { scaleX: 1, duration: 0.55, ease: 'power2.inOut' }, A1.s + 7.7);
    tl.fromTo('act1-c2body-hl2', { scaleX: 0 }, { scaleX: 1, duration: 0.6, ease: 'power2.inOut' }, A1.s + 8.35);
    tl.fromTo('act1-c3body-hl1', { scaleX: 0 }, { scaleX: 1, duration: 0.8, ease: 'power2.inOut' }, A1.s + 10.35);
    // ── 11.35 push into card 3; act 2 picks up mid-zoom ──
    tl.set('act1-card3', { transformOrigin: '50% 50%' }, A1.s + 11.3);
    tl.to('act1-card3', { scale: 0.78, opacity: 0, duration: 0.42, ease: 'power3.in' }, A1.s + 11.35);
  });

  return (
    <div id="sec-act1" class="absolute inset-0 z-[1] opacity-0">
      <div id="act1-ground" class="absolute inset-0 bg-[#000]"></div>
      <div class="absolute inset-0">
        <div id="act1-bgwrap" class="absolute inset-0 opacity-0">
          <div class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.45),rgba(0,0,0,0.15)_40%,rgba(0,0,0,0.5))]"></div>
        </div>

        {/* CARD 1 — the cold-open carrier */}
        <div id="act1-card1" class="absolute left-[30px] top-[301px] w-[1020px] h-[510px] [transform-origin:0_0]">
          <GlassCard glassId="act1-c1glass" blurClass="backdrop-blur-[0.01px]" />
          <div id="act1-c1head" class="absolute left-[50px] top-[44px] w-[920px] h-[74px] flex items-center gap-[20px] opacity-0">
            <div class="w-[74px] h-[74px] rounded-full flex-none bg-[linear-gradient(135deg,#B8A08A,#7A6A58)]"></div>
            <div class="flex-1">
              <text class="font-[fl-sans-bold] font-[700] text-[#ffffff] text-[30px] leading-[34.5px]">Frustrated Founder</text>
              <text class="font-[fl-sans] text-[rgba(245,246,244,0.5)] text-[26px] leading-[29.9px]">@frustrated_founder</text>
            </div>
            <path view-box="0 0 24 24" class="w-[38px] h-[38px] flex-none fill-[#ffffff] opacity-90" d={X_PATH} />
          </div>
          {/* body line 1: mask + slider + "but" cascade */}
          <div class="absolute left-[50px] top-[142px] w-[920px] h-[52.44px] flex">
            <div id="act1-l1mask" class="h-[52.44px] overflow-hidden flex-none">
              <text id="act1-l1slide" class="whitespace-nowrap font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px]">hyperframes is genuinely great</text>
            </div>
            <div class="flex flex-none ml-[9px]">
              <text id="act1-b1" class="font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px] opacity-0">b</text>
              <text id="act1-b2" class="font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px] opacity-0">u</text>
              <text id="act1-b3" class="font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px] opacity-0">t</text>
            </div>
          </div>
          <CardBody id="act1-rest" hlCount={1} top={194.44} />
          <CardFoot footId="act1-c1foot" m0="9:41 AM" m1="Jun 30, 2026" views="214K" s0="482" s1="1.1K" s2="6.4K" />
        </div>

        {/* CARD 2 */}
        <div id="act1-card2" class="absolute left-[30px] top-[327px] w-[1020px] h-[458px]">
          <GlassCard glassId="act1-c2glass" blurClass="backdrop-blur-[0.01px]" />
          <div class="absolute left-0 top-0 w-full h-full rounded-[30px] overflow-hidden">
            <div id="act1-shine2" class="absolute top-[-25%] bottom-[-25%] left-0 w-[240px] bg-[linear-gradient(105deg,transparent,rgba(255,255,255,0.15),transparent)]"></div>
          </div>
          <CardHead av="#6C8EA4,#3D5666" name="Design Perfectionist" handle="@pixel_perfectionist" />
          <CardBody id="act1-c2body" hlCount={2} />
          <CardFoot footId="act1-c2foot" m0="2:17 PM" m1="Jul 1, 2026" views="88K" s0="167" s1="640" s2="2.9K" />
        </div>

        {/* CARD 3 — the push-in target */}
        <div id="act1-card3" class="absolute left-[30px] top-[327px] w-[1020px] h-[405px]">
          <GlassCard glassId="act1-c3glass" blurClass="backdrop-blur-[0.01px]" />
          <CardHead av="#A47C6C,#66463D" name="Motion Enjoyer" handle="@motion_enjoyer" />
          <div id="act1-c3body" class="absolute left-[50px] top-[142px] w-[920px] h-[104.88px]">
            <div id="act1-c3body-hl1" class="absolute rounded-[8px] bg-[linear-gradient(90deg,rgba(94,241,124,0.42),rgba(79,214,255,0.30))] [transform-origin:0_50%] opacity-0"></div>
            <text class="absolute left-0 top-0 whitespace-nowrap font-[fl-sans] text-[#f5f6f4] text-[38px] leading-[52.44px] tracking-[-0.19px]">love the motion. hate that it keeps</text>
            <text class="absolute left-0 top-[52.44px] whitespace-nowrap font-[fl-sans] text-[#ffffff] text-[38px] leading-[52.44px] tracking-[-0.19px]">inventing a UI we don't have.</text>
          </div>
          <CardFoot footId="act1-c3foot" m0="11:03 PM" m1="Jul 2, 2026" views="301K" s0="93" s1="512" s2="2.2K" />
        </div>
      </div>
    </div>
  );
}

/* ════════════ ACT 2 · zoom-flash montage (11.8 → 14.7, z2) ════════════
   One flash = a real card zoomed so the highlight (or stem) centre lands on
   frame centre; echoes re-crop the same three cards tighter on the stem. */
var A2_SPECS = [
  {
    key: 'f1', bg: 'assets/a2-b264.png',
    av: '#8AA4B8,#4E6478', name: 'Brand Guardian', handle: '@brand_guardian',
    pre: 'third render in a row came back off. it', stem: "Couldn't capture", tail: ' my brand.',
    post: 'honestly gave up after that.',
    m0: '4:52 PM', m1: 'Jun 28, 2026', views: '118K', s0: '212', s1: '890', s2: '4.1K',
  },
  {
    key: 'f2', bg: 'assets/a2-b278f.png',
    av: '#B89A8A,#786052', name: 'UI Stickler', handle: '@ui_stickler',
    pre: 'the layout was close-ish but it just', stem: "Couldn't capture", tail: ' our actual UI.',
    post: 'the buttons weren\u2019t even our buttons lol',
    m0: '9:12 AM', m1: 'Jun 29, 2026', views: '54K', s0: '88', s1: '326', s2: '2.2K',
  },
  {
    key: 'f3', bg: 'assets/a2-b264f.png',
    av: '#9A8AB8,#5E5278', name: 'Screenshot Sender', handle: '@sends_screenshots',
    pre: 'what was even the point. it', stem: "Couldn't capture", tail: ' the screens I gave it.',
    post: 'like I attached them for a reason',
    m0: '11:03 PM', m1: 'Jul 2, 2026', views: '301K', s0: '93', s1: '512', s2: '2.9K',
  },
];
var A2_ECHO = { e1: 'f3', e2: 'f2', e3: 'f1', e4: 'f3', e5: 'f2', e6: 'f1' };
/* per-layer bg windows: uniques carry their own fitted window; the reference's
   echo clones re-window the same source centred (measured off the render) */
var A2_BG = {
  f1: 'assets/a2-b264.png',  f2: 'assets/a2-b278f.png', f3: 'assets/a2-b264f.png',
  e1: 'assets/a2-b264f.png',  e2: 'assets/a2-b278f.png', e3: 'assets/a2w-e6.png',
  e4: 'assets/a2-b264f.png',  e5: 'assets/a2-b278f.png', e6: 'assets/a2w-e6.png',
};
var A2_WIDE = { f1: true, f2: true, f3: true, e1: true, e2: true, e4: true, e5: true };
function specIndex(k) { return k.charAt(0) === 'f' ? Number(k.charAt(1)) - 1 : Number(A2_ECHO[k].charAt(1)) - 1; }

function Act2() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A2.s && T < A3.s;
    vis(ctx, 'sec-act2', ON);
    if (!ON) return;

    var G = globalThis.__flA2 || (globalThis.__flA2 = (function () {
      var adv = makeMeasure(ctx);
      var sq = adv(TTN, '400', 38, -0.19, ' ');
      var F = [];
      for (var d = 0; d < A2_SPECS.length; d++) {
        var sp = A2_SPECS[d];
        var stemW = adv(TTN, '400', 38, -0.19, sp.stem);
        var hlW = stemW + adv(TTN, '400', 38, -0.19, sp.tail) + 20;
        var atoms = words(adv, sp.pre)
          .concat([{ t: sp.stem + sp.tail, w: hlW, hl: true }], words(adv, sp.post));
        var rows = wrapAtoms(atoms, CARD_INNER, sq);
        var pos = null;
        for (var r = 0; r < rows.length && !pos; r++) {
          for (var k = 0; k < rows[r].length; k++) {
            if (atoms[rows[r][k].i].hl) { pos = { row: r, x: rows[r][k].x }; break; }
          }
        }
        F.push({
          atoms: atoms, rows: rows, hlW: hlW, stemW: stemW,
          // cardpos-local (body sits at 50,142 inside the card)
          hlCX: 50 + pos.x - 10 + hlW / 2,
          hlCY: 142 + pos.row * LINE - 2 + HL_H / 2,
          stemCX: 50 + pos.x + stemW / 2,
          stemCY: 142 + pos.row * LINE + 28.22,
          // the hlwrap row's line box is 7px taller — card grows, meta/foot follow
          h: 44 + 74 + 24 + rows.length * LINE + 7 + 21 + META_H + 21 + 19 + META_H + 36,
          mt: metaTop(rows.length) + 7,
        });
      }
      return { F: F };
    })());

    // fill the three unique card bodies; echoes re-render the same content
    for (var d2 = 0; d2 < 3; d2++) {
      writeRows(ctx, 'act2-f' + (d2 + 1) + '-body', G.F[d2].atoms, G.F[d2].rows, ['hl1']);
      ctx.getNode('act2-f' + (d2 + 1) + '-pos').height(G.F[d2].h);
      ctx.getNode('act2-f' + (d2 + 1) + '-foot').top(G.F[d2].mt);
    }
    var order = ['e1', 'e2', 'e3', 'e4', 'e5', 'e6'];
    for (var e = 0; e < order.length; e++) {
      var key = order[e];
      var F0 = G.F[specIndex(key)];
      writeRows(ctx, 'act2-' + key + '-body', F0.atoms, F0.rows, ['hl1']);
      ctx.getNode('act2-' + key + '-pos').height(F0.h);
      ctx.getNode('act2-' + key + '-foot').top(F0.mt);
    }

    // one-way zoom ladder: [layer, target, zoom, t0, t1]
    // f3 starts one frame late: the reference renders a black first frame at
    // the cut (its layer fails to paint once), so 354 shows only the black
    // ground here too.
    var cuts = [
      ['f3', 'hl', 1.32, 0.0333, 0.80],
      ['f2', 'hl', 1.60, 0.80, 1.30],
      ['f1', 'hl', 1.95, 1.30, 1.70],
      ['e1', 'stem', 2.30, 1.70, 2.00],
      ['e2', 'stem', 2.70, 2.00, 2.20],
      ['e3', 'stem', 3.10, 2.20, 2.35],
      ['e4', 'stem', 3.50, 2.35, 2.47],
      ['e5', 'stem', 3.95, 2.47, 2.57],
      ['e6', 'stem', 4.40, 2.57, 2.90],
    ];
    var tl = gsap.timeline();
    for (var i = 0; i < cuts.length; i++) {
      var c = cuts[i];
      var key2 = c[0];
      var F2 = G.F[specIndex(key2)];
      var Z = c[2], t0 = c[3], t1 = c[4];
      var target = c[1] === 'hl' ? { x: F2.hlCX, y: F2.hlCY } : { x: F2.stemCX, y: F2.stemCY };
      var wx = 540 / Z, wy = 540 / Z;
      var cx = clamp(target.x, wx, 1020 - wx);
      var cy = clamp(target.y, wy, F2.h - wy);
      var layer = 'act2-' + key2;
      tl.set(layer + '-pos', { x: 510 - Z * cx, y: 220 - Z * cy, scale: Z, transformOrigin: '0px 0px' }, A2.s);
      tl.set(layer, { opacity: 1 }, A2.s + t0);
      if (i < cuts.length - 1) tl.set(layer, { opacity: 0 }, A2.s + t1);
      // continuous Z+ drift while visible (origin = frame centre)
      if (i === 0) {
        tl.fromTo(layer, { scale: 1.5 }, { scale: 1.04, duration: t1 - t0, ease: 'expo.out' }, A2.s + t0);
      } else {
        tl.fromTo(layer, { scale: 1.0 }, { scale: 1.04, duration: t1 - t0, ease: 'none' }, A2.s + t0);
      }
    }
    // exit: last flash cuts-the-curve LEFT into act 3's "Until now"
    tl.to('act2-e6', { x: -1250, duration: 0.3, ease: 'power4.in' }, A2.s + 2.6);
  });

  function FlashCard({ k, spec }) {
    return (
      <div id={'act2-' + k} class="absolute inset-0 overflow-hidden opacity-0 [transform-origin:50%_50%]">
        <image path={A2_BG[k]} class={A2_WIDE[k] ? 'absolute left-[-86.4px] top-[-86.4px] w-[1252.8px] h-[1252.8px] object-cover' : 'absolute left-0 top-0 w-[1080px] h-[1080px]'} />
        <div id={'act2-' + k + '-pos'} class="absolute left-[30px] top-[320px] w-[1020px] h-[458px] [transform-origin:0_0]">
          <GlassCard glassId={'act2-' + k + '-glass'} blurClass={A2_WIDE[k] ? null : 'backdrop-blur-[26px]'} />
          <CardHead av={spec.av} name={spec.name} handle={spec.handle} />
          <CardBody id={'act2-' + k + '-body'} hlCount={1} />
          <CardFoot footId={'act2-' + k + '-foot'} m0={spec.m0} m1={spec.m1} views={spec.views} s0={spec.s0} s1={spec.s1} s2={spec.s2} />
        </div>
      </div>
    );
  }
  function specOf(k) { return A2_SPECS[specIndex(k)]; }
  return (
    <div id="sec-act2" class="absolute inset-0 z-[2] opacity-0">
      <div class="absolute inset-0 bg-[#000]"></div>
      <FlashCard k="f3" spec={A2_SPECS[2]} />
      <FlashCard k="f2" spec={A2_SPECS[1]} />
      <FlashCard k="f1" spec={A2_SPECS[0]} />
      <FlashCard k="e1" spec={specOf('e1')} />
      <FlashCard k="e2" spec={specOf('e2')} />
      <FlashCard k="e3" spec={specOf('e3')} />
      <FlashCard k="e4" spec={specOf('e4')} />
      <FlashCard k="e5" spec={specOf('e5')} />
      <FlashCard k="e6" spec={specOf('e6')} />
    </div>
  );
}

/* ════════════ ACT 3 · "Until now" (14.7 → 15.8, z3) ════════════ */
function Act3() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A3.s && T < A4.s;
    vis(ctx, 'sec-act3', ON);
    if (!ON) return;
    var tl = gsap.timeline();
    tl.to('act3-ground', { opacity: 0, duration: 0.45, ease: 'power2.out' }, A3.s + 0.1);
    tl.fromTo('act3-shade', { opacity: 0 }, { opacity: 1, duration: 0.45, ease: 'power2.out' }, A3.s + 0.1);
    tl.fromTo('act3-line', { x: 1150 }, { x: 0, duration: 0.5, ease: 'power4.out' }, A3.s);
    tl.to('act3-line', { x: -1250, duration: 0.3, ease: 'power4.in' }, A3.s + 0.8);
  });
  return (
    <div id="sec-act3" class="absolute inset-0 z-[3] opacity-0">
      <div id="act3-ground" class="absolute inset-0 bg-[#000]"></div>
      <div id="act3-shade" class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))] opacity-0"></div>
      <text id="act3-line" class="absolute left-0 top-[455px] w-full text-center whitespace-nowrap font-[fl-display] text-[#ffffff] text-[150px] leading-[165px] tracking-[-2.25px]">Until now</text>
    </div>
  );
}

/* ════════════ ACT 4 · Introducing /figma (15.8 → 18.0, z4) ════════════ */
function Act4() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A4.s && T < A5.s;
    vis(ctx, 'sec-act4', ON);
    if (!ON) return;
    var tl = gsap.timeline();
    tl.fromTo('act4-intro', { x: 1150 }, { x: 0, duration: 0.5, ease: 'power4.out' }, A4.s);
    tl.to('act4-intro', { y: -350, scale: 0.42, duration: 0.6, ease: 'power3.inOut' }, A4.s + 0.8);
    tl.fromTo('act4-skill', { y: -320, opacity: 0, scale: 0.92 }, { y: -95, opacity: 1, scale: 1, duration: 0.55, ease: 'power3.out' }, A4.s + 1.05);
    tl.to('act4-skill', { x: -1250, duration: 0.3, ease: 'power4.in' }, A4.s + 1.9);
    tl.to('act4-intro', { x: -1250, duration: 0.3, ease: 'power4.in' }, A4.s + 1.94);
  });
  return (
    <div id="sec-act4" class="absolute inset-0 z-[4] opacity-0">
      <div class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))]"></div>
      <text id="act4-intro" class="absolute left-0 top-[466px] w-full text-center whitespace-nowrap font-[fl-display] text-[#ffffff] text-[132px] leading-[145.2px] tracking-[-1.98px]">Introducing</text>
      <text id="act4-skill" class="absolute left-0 top-[540px] w-full text-center whitespace-nowrap font-[fl-mono] text-[#5ef17c] text-[190px] leading-[190px] opacity-0">/figma</text>
    </div>
  );
}

/* ════════════ ACT 5 · Claude /figma run (18.0 → 35.95, z5) ════════════
   cqw units at 1cqw = 10.8px. The win morph + composer dock recompute the
   reference's derived values per frame (a power2.inOut object tween there). */
var A5_SEG1 = 'Use /figma to rebuild these frames ';
var A5_LINK = 'figma.com/xK9…';
var A5_SEG2 = ' and then invoke the /product-launch-video to make a launch video for our new feature';

/* token model shared by the typed line and the sent message */
function tokenModel(adv, family, weight, fs, seg1, link, seg2) {
  var full = seg1 + link + seg2;
  var tokens = full.split(' ');
  var widths = [];
  for (var i = 0; i < tokens.length; i++) widths.push(adv(family, weight, fs, 0, tokens[i] + ' '));
  var linkIdx = -1;
  for (var j = 0; j < tokens.length; j++) if (tokens[j] === link) { linkIdx = j; break; }
  var offsets = [], o = 0;
  for (var k = 0; k < tokens.length; k++) { offsets.push(o); o += tokens[k].length + 1; }
  return {
    tokens: tokens, widths: widths, offsets: offsets, full: full,
    adv: adv, family: family, weight: weight, fs: fs,
    space: widths[0] - adv(family, weight, fs, 0, tokens[0]),
    linkIdx: linkIdx, l1: seg1.length, l2: seg1.length + link.length,
  };
}

/* greedy wrap over token indices → rows of token lists (lastW: measured
   width of a partially-typed last token, so mid-type rows break like the
   browser does) */
function wrapTokens(M, count, lineW, maxRows, lastW) {
  var rows = [], cur = [], curW = 0;
  for (var i = 0; i < count; i++) {
    var w = (lastW != null && i === count - 1) ? lastW : M.widths[i] - M.space;
    if (cur.length && curW + M.space + w > lineW + 0.01 && rows.length < maxRows - 1) {
      rows.push(cur); cur = [i]; curW = w;
    } else {
      cur.push(i);
      curW = cur.length > 1 ? curW + M.space + w : w;
    }
  }
  if (cur.length || rows.length === 0) rows.push(cur);
  return rows;
}

/* row strings cut from the source text (keeps inter-token spaces, clamps a
   partially-typed last token) by the plain/link/plain char boundaries */
function segStrings(M, indices, endChar) {
  var a = '', l = '', b = '';
  if (indices.length) {
    var start = M.offsets[indices[0]];
    var s = M.full.slice(start, endChar).replace(/ +$/, '');
    var c1 = Math.max(0, Math.min(s.length, M.l1 - start));
    var c2 = Math.max(c1, Math.min(s.length, M.l2 - start));
    a = s.slice(0, c1); l = s.slice(c1, c2); b = s.slice(c2);
  }
  return [a, l, b];
}

/* ref caret: solid on until the blink tween starts; steps(1)+yoyo flicker while
   it runs (visible = first half of even cycles / second half of odd); solid on
   after the tween ends — 10 yoyo cycles terminate on the reversed half at
   opacity 1, and the later tl.set(opacity 0) is overwritten every tick the
   blink is still active, so the caret NEVER disappears after send */
function caretBlink(t, base, repeats) {
  if (t < base) return true;
  if (t >= base + 0.5 * (repeats + 1)) return true;
  var h = Math.floor((t - base) / 0.5);
  var phase = (t - base) - 0.5 * h;
  return (phase < 0.25) === (h % 2 === 0);
}

/* per-frame typed prompt line: 3 nowrap rows, caret on the active row
   (row 0 once sent — the ref caret keeps blinking in the emptied bar) */
function typedPromptFrame(ctx, pfx, M, typedAll, caretOn, lineW) {
  var rows = [[]];
  var count = 0;
  if (typedAll) {
    count = (typedAll.match(/[^ ]+/g) || []).length;
    var partial = typedAll.slice(M.offsets[count - 1]).replace(/ +$/, '');
    var lastW = M.adv(M.family, M.weight, M.fs, 0, partial);
    rows = wrapTokens(M, count, lineW, 3, lastW);
  }
  var lastRow = 0;
  for (var r = 0; r < 3; r++) {
    var has = r < rows.length && rows[r].length > 0;
    if (has) lastRow = r;
    var seg = has ? segStrings(M, rows[r], r < rows.length - 1 ? M.offsets[rows[r + 1][0]] : typedAll.length) : ['', '', ''];
    ctx.getNode(pfx + '-r' + r + 'a').text(seg[0]);
    ctx.getNode(pfx + '-r' + r + 'l').text(seg[1]);
    ctx.getNode(pfx + '-r' + r + 'b').text(seg[2]);
    ctx.getNode(pfx + '-c' + r).opacity((has ? r === lastRow : r === 0) && caretOn ? 1 : 0);
    var ul = ctx.getNode(pfx + '-ul' + r);
    if (has && seg[1]) {
      ul.left(M.adv(M.family, M.weight, M.fs, 0, seg[0]));
      ul.width(M.adv(M.family, M.weight, M.fs, 0, seg[1]));
      ul.opacity(1);
    } else {
      ul.opacity(0);
    }
    // CSS line-box model: the caret's 23.76px inline box grows the active
    // line to 23.76×1.45 = 34.45; finished lines stay 27.4; future lines 0
    var rh = r > lastRow ? 0 : (r === lastRow ? 34.45 : 27.4);
    ctx.getNode(pfx + '-row' + r).height(rh);
  }
  // the engine's flex-col auto height doesn't accumulate fixed-height row
  // children — drive the inputbar's line-box height directly (ref: py 23.76 +
  // border 2 + active line 34.45, plus 27.4 per completed line)
  var nlines = typedAll ? rows.length : 0;
  ctx.getNode(pfx + '-inputbar').height(nlines >= 3 ? 115.01 : nlines === 2 ? 87.61 : 60.21);
}

/* right-aligned sent message rows (link colored + underlined), ids {pfx}-u{r}{a|l|b} */
function writeSentRows(ctx, pfx, M) {
  var rows = wrapTokens(M, M.tokens.length, 756, 3, null);
  for (var r = 0; r < 3; r++) {
    var box = ctx.getNode(pfx + '-u' + r);
    if (r < rows.length && rows[r].length) {
      var seg = segStrings(M, rows[r], r < rows.length - 1 ? M.offsets[rows[r + 1][0]] : M.full.length);
      ctx.getNode(pfx + '-u' + r + 'a').text(seg[0]);
      ctx.getNode(pfx + '-u' + r + 'l').text(seg[1]);
      ctx.getNode(pfx + '-u' + r + 'b').text(seg[2]);
      var ul = ctx.getNode(pfx + '-u' + r + '-ul');
      if (seg[1]) {
        ul.left(M.adv(M.family, M.weight, M.fs, 0, seg[0]));
        ul.width(M.adv(M.family, M.weight, M.fs, 0, seg[1]));
        ul.opacity(1);
      } else {
        ul.opacity(0);
      }
      box.top(r * 31.8);
      box.opacity(1);
    } else {
      box.opacity(0);
    }
  }
}

function ChatWindow({ pfx }) {
  var rowClass = 'relative flex items-center h-[27.4px] whitespace-nowrap';
  var typedClass = 'font-[fl-sans] text-[#E6E2D6] text-[18.9px]';
  var caretClass = 'font-[fl-sans] text-[#D97757] text-[23.76px] leading-[23.76px]';
  return (
    <div id={pfx + '-inputbar'} class="relative flex items-center bg-[#2C2C2B] border-[1px] border-[#3a3a38] rounded-[15.12px] min-h-[56.16px] px-[20.52px] py-[11.88px]">
      <div class="flex-1 flex flex-col pr-[43.2px]">
        <div id={pfx + '-row0'} class={rowClass}>
          <text id={pfx + '-r0a'} class={typedClass}></text>
          <text id={pfx + '-r0l'} class="font-[fl-sans] text-[#7DB8E8] text-[18.9px]"></text>
          <text id={pfx + '-r0b'} class={typedClass}></text>
          <text id={pfx + '-c0'} class={caretClass}>{'▏'}</text>
          <div id={pfx + '-ul0'} class="absolute left-0 top-[21px] h-[1.9px] w-[0px] bg-[#7DB8E8] opacity-0"></div>
        </div>
        <div id={pfx + '-row1'} class={rowClass}>
          <text id={pfx + '-r1a'} class={typedClass}></text>
          <text id={pfx + '-r1l'} class="font-[fl-sans] text-[#7DB8E8] text-[18.9px]"></text>
          <text id={pfx + '-r1b'} class={typedClass}></text>
          <text id={pfx + '-c1'} class={caretClass}>{'▏'}</text>
          <div id={pfx + '-ul1'} class="absolute left-0 top-[21px] h-[1.9px] w-[0px] bg-[#7DB8E8] opacity-0"></div>
        </div>
        <div id={pfx + '-row2'} class={rowClass}>
          <text id={pfx + '-r2a'} class={typedClass}></text>
          <text id={pfx + '-r2l'} class="font-[fl-sans] text-[#7DB8E8] text-[18.9px]"></text>
          <text id={pfx + '-r2b'} class={typedClass}></text>
          <text id={pfx + '-c2'} class={caretClass}>{'▏'}</text>
          <div id={pfx + '-ul2'} class="absolute left-0 top-[21px] h-[1.9px] w-[0px] bg-[#7DB8E8] opacity-0"></div>
        </div>
      </div>
      <text id={pfx + '-ret'} class="flex-none font-[fl-sans] text-[#6F6E66] text-[20.52px] leading-[24.63px]">{'↵'}</text>
    </div>
  );
}

function ComposerChips() {
  return (
    <div class="flex gap-[10.8px] mb-[15.12px]">
      <div class="flex items-center gap-[8.64px] bg-[#2C2C2B] px-[14.04px] py-[8.1px] rounded-[9.72px]">
        <path view-box="0 0 24 24" class="w-[18.9px] h-[18.9px] stroke-[#C9C6BC] stroke-[1.8]" d="M4 5h16v11H4z M2 19h20" />
        <text class="font-[fl-sans-medium] font-[500] text-[#D7D4CA] text-[16.74px] leading-[20px]">Local</text>
      </div>
      <div class="flex items-center gap-[8.64px] bg-[#2C2C2B] px-[14.04px] py-[8.1px] rounded-[9.72px]">
        <path view-box="0 0 24 24" class="w-[18.9px] h-[18.9px] stroke-[#C9C6BC] stroke-[1.8]" d="M3 7.5A2 2 0 0 1 5 5.5h4.2l2 2.2H19a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        <text class="font-[fl-sans-medium] font-[500] text-[#D7D4CA] text-[16.74px] leading-[20px]">CC</text>
      </div>
    </div>
  );
}

function ComposerMeta({ orbId }) {
  return (
    <div class="flex justify-between items-center mt-[14.04px] font-[fl-sans-medium] font-[500] text-[#7C7B73] text-[16.74px]">
      <div class="flex items-center gap-[17.28px]">
        <div class="bg-[rgba(202,164,62,0.16)] px-[11.88px] py-[3.78px] rounded-[7.56px]">
          <text class="font-[fl-sans-medium] font-[500] text-[#CAA43E] text-[15.66px] leading-[18.8px]">Auto</text>
        </div>
        <text class="text-[#8C8B83] text-[19.44px] leading-[23px]">{'+'}</text>
        <path view-box="0 0 24 24" class="w-[21.06px] h-[21.06px] stroke-[#8C8B83] stroke-[2]" d="M12 5.5a3 3 0 0 1 3 3v3a3 3 0 0 1-6 0v-3a3 3 0 0 1 3-3z M5 11a7 7 0 0 0 14 0 M12 18v3" />
        <text class="text-[#8C8B83] text-[14.58px] leading-[20px]">{'⌄'}</text>
      </div>
      <div class="flex items-center gap-[17.28px]">
        <text class="leading-[20px]">Opus 4.8</text>
        <text class="text-[#9A998F] leading-[20px]">High</text>
        <div class="relative w-[21.06px] h-[21.06px]">
          <path view-box="0 0 24 24" class="absolute left-0 top-0 w-[21.06px] h-[21.06px] stroke-[rgba(96,148,214,0.26)] stroke-[2.916]" d="M12 2.4a9.6 9.6 0 1 1 0 19.2a9.6 9.6 0 1 1 0-19.2" />
          <path id={orbId} view-box="0 0 24 24" class="absolute left-0 top-0 w-[21.06px] h-[21.06px] stroke-[#5A9BE6] stroke-[2.916] [transform-origin:50%_50%]" d="M12 2.4a9.6 9.6 0 0 1 9.6 9.6" />
        </div>
      </div>
    </div>
  );
}

function SentMessage({ pfx, cls }) {
  var rows = [];
  for (var r = 0; r < 3; r++) {
    rows.push(
      <div id={pfx + '-u' + r} class="absolute right-0 top-0 h-[31.8px] flex justify-end whitespace-nowrap opacity-0">
        <text id={pfx + '-u' + r + 'a'} class="font-[fl-sans-medium] font-[500] text-[#EFEADD] text-[24.84px] leading-[31.8px]"></text>
        <text id={pfx + '-u' + r + 'l'} class="font-[fl-sans-medium] font-[500] text-[#7DB8E8] text-[24.84px] leading-[31.8px]"></text>
        <text id={pfx + '-u' + r + 'b'} class="font-[fl-sans-medium] font-[500] text-[#EFEADD] text-[24.84px] leading-[31.8px]"></text>
        <div id={pfx + '-u' + r + '-ul'} class="absolute left-0 top-[25px] h-[2.2px] w-[0px] bg-[#7DB8E8] opacity-0"></div>
      </div>
    );
  }
  return (
    <div id={pfx + '-usermsg'} class={cls || 'absolute right-0 top-0 w-[756px] h-[95.4px] opacity-0'}>
      {rows[0]}
      {rows[1]}
      {rows[2]}
    </div>
  );
}

function Act5() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A5.s && T < A6.s;
    vis(ctx, 'sec-act5', ON);
    if (!ON) return;
    var t = T - A5.s;

    var M5 = globalThis.__flM5 || (globalThis.__flM5 = tokenModel(
      makeMeasure(ctx), TTN, '400', 18.9, A5_SEG1, A5_LINK, A5_SEG2));
    var MU5 = globalThis.__flMU5 || (globalThis.__flMU5 = tokenModel(
      makeMeasure(ctx), TTN, '500', 24.84, A5_SEG1, A5_LINK, A5_SEG2));

    /* typing progress (linear 0.7 → 3.15) */
    var PLEN = A5_SEG1.length + A5_LINK.length + A5_SEG2.length;
    var sent = t >= 3.55;
    var cleared = t >= 3.65;
    var n = cleared ? PLEN : Math.round(PLEN * clamp((t - 0.7) / 2.45, 0, 1));
    var typedAll = cleared ? '' : (A5_SEG1 + A5_LINK + A5_SEG2).slice(0, n);
    var blink = caretBlink(t, 0.6, 9);
    // typed line wrap width: geometric 603.76 tuned to 586 so the greedy
    // break points match the reference renderer's (its advances run ~2% wider):
    // line1 ends "invoke", line2 ends "new", line3 "feature"
    typedPromptFrame(ctx, 'act5-tl', M5, typedAll, blink, 586);
    if (sent) writeSentRows(ctx, 'act5', MU5);

    /* win morph + composer dock (reference-derived per-frame values) */
    var pe = easeInOutCubic(clamp((t - 3.65) / 0.7, 0, 1));
    var W = 1080 - 108 * pe;
    var H = 1080 - 194.4 * pe;
    var win = ctx.getNode('act5-win');
    win.width(W); win.height(H); win.borderRadius(25.9 * pe);
    win.x(-W / 2); win.y(-H / 2);
    var sc = 1.4 - 0.4 * pe;
    var cw = (966 - 91.2 * pe) / sc;
    var comp = ctx.getNode('act5-composer');
    comp.width(cw); comp.left(W / 2 - cw / 2); comp.bottom(360 - 311.4 * pe); comp.scale(sc);

    var tl = gsap.timeline();
    // entry: composer elements cut in from the right
    tl.fromTo('act5-tl-inputbar', { x: 280, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power3.out' }, A5.s + 0.02);
    tl.fromTo('act5-chips', { x: 340, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power3.out' }, A5.s + 0.10);
    tl.fromTo('act5-metarow', { x: 340, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power3.out' }, A5.s + 0.16);
    // orb spinner (360° × 1s × 7)
    tl.fromTo('act5-orb', { rotation: 0 }, { rotation: 2520, duration: 7, ease: 'none' }, A5.s);
    // cursor rises to ↵ as typing finishes
    tl.fromTo('act5-cursor', { x: 907, y: 1210, opacity: 0 }, { y: 610, opacity: 1, duration: 0.85, ease: 'power3.out' }, A5.s + 2.55);
    tl.to('act5-cursor', { scale: 0.84, duration: 0.1, ease: 'power2.in' }, A5.s + 3.42);
    tl.to('act5-cursor', { scale: 1, duration: 0.2, ease: 'power2.out' }, A5.s + 3.52);
    tl.fromTo('act5-tl-ret', { textColor: '#6F6E66' }, { textColor: '#D97757', duration: 0.08, ease: 'power1.in' }, A5.s + 3.5);
    tl.to('act5-tl-ret', { textColor: '#6F6E66', duration: 0.25 }, A5.s + 3.7);
    // SEND
    tl.fromTo('act5-usermsg', { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power3.out' }, A5.s + 3.6);
    tl.to('act5-cursor', { y: 691, opacity: 0, duration: 0.35, ease: 'power2.in' }, A5.s + 3.7);
    // morph: bar slides in
    tl.fromTo('act5-bar', { opacity: 0, x: -14 }, { opacity: 1, x: 0, duration: 0.35, ease: 'power2.out' }, A5.s + 3.95);
    // thinking + reply
    tl.fromTo('act5-agent', { opacity: 0 }, { opacity: 1, duration: 0.3 }, A5.s + 4.0);
    tl.fromTo('act5-think', { opacity: 0 }, { opacity: 1, duration: 0.3 }, A5.s + 4.0);
    tl.fromTo('act5-ast', { rotation: 0 }, { rotation: 360, duration: 1.0, ease: 'none' }, A5.s + 4.0);
    for (var i = 0; i < 4; i++) {
      tl.fromTo('act5-rl' + i, { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.45, ease: 'power3.out' }, A5.s + 4.85 + i * 0.42);
    }
    tl.fromTo('act5-closer', { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.45, ease: 'power3.out' }, A5.s + 6.95);
    // EXIT zoom-through
    tl.to('act5-win', { scale: 1.22, duration: 0.35, ease: 'power3.in' }, A5.s + 7.6);
    tl.to('act5-win', { opacity: 0.15, duration: 0.35, ease: 'none' }, A5.s + 7.6);
    tl.set('act5-win', { opacity: 0 }, A5.s + 7.95);
  });

  return (
    <div id="sec-act5" class="absolute inset-0 z-[5] opacity-0">
      <div class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))]"></div>
      <div id="act5-win" class="absolute left-[540px] top-[540px] w-[1080px] h-[1080px] bg-[#1D1F1F] overflow-hidden shadow-[0_32.4px_97.2px_-32.4px_rgba(0,0,0,0.6)] [transform-origin:50%_50%]">
        <div id="act5-bar" class="absolute top-[23.76px] left-[28.08px] flex gap-[10.8px] opacity-0">
          <div class="w-[17.28px] h-[17.28px] rounded-full bg-[#FF5F57]"></div>
          <div class="w-[17.28px] h-[17.28px] rounded-full bg-[#FEBC2E]"></div>
          <div class="w-[17.28px] h-[17.28px] rounded-full bg-[#28C840]"></div>
        </div>
        <div class="absolute left-[54px] right-[54px] top-[97.2px] bottom-[280.8px]">
          <SentMessage pfx="act5" />
          <div id="act5-agent" class="absolute left-0 top-[129.6px] opacity-0">
            <div id="act5-rl0" class="flex mb-[21.6px] whitespace-nowrap opacity-0">
              <text class="font-[fl-mono] text-[#8C8B83] text-[20.52px] leading-[28.73px]">Reading your Figma file…</text>
              <text class="font-[fl-mono] text-[#A9C4A0] text-[20.52px] leading-[28.73px]">{' 4 frames found.'}</text>
            </div>
            <div id="act5-rl1" class="flex mb-[21.6px] whitespace-nowrap opacity-0">
              <text class="font-[fl-mono] text-[#8C8B83] text-[20.52px] leading-[28.73px]">Brand tokens →</text>
              <text class="font-[fl-mono] text-[#A9C4A0] text-[20.52px] leading-[28.73px]">{' '}</text>
              <text class="font-[fl-mono] text-[#4fd6ff] text-[20.52px] leading-[28.73px]">#232833</text>
              <text class="font-[fl-mono] text-[#A9C4A0] text-[20.52px] leading-[28.73px]">{' · '}</text>
              <text class="font-[fl-mono] text-[#4fd6ff] text-[20.52px] leading-[28.73px]">#FBFAF9</text>
              <text class="font-[fl-mono] text-[#A9C4A0] text-[20.52px] leading-[28.73px]">{' · '}</text>
              <text class="font-[fl-mono] text-[#4fd6ff] text-[20.52px] leading-[28.73px]">#DEF0F8</text>
            </div>
            <div id="act5-rl2" class="flex mb-[21.6px] whitespace-nowrap opacity-0">
              <text class="font-[fl-mono] text-[#8C8B83] text-[20.52px] leading-[28.73px]">Type →</text>
              <text class="font-[fl-mono] text-[#A9C4A0] text-[20.52px] leading-[28.73px]">{' ABC Solar Display / TT Norms Pro'}</text>
            </div>
            <div id="act5-rl3" class="flex mb-[21.6px] whitespace-nowrap opacity-0">
              <text class="font-[fl-mono] text-[#8C8B83] text-[20.52px] leading-[28.73px]">Frames read as states →</text>
              <text class="font-[fl-mono] text-[#A9C4A0] text-[20.52px] leading-[28.73px]">{' rebuilding as live DOM…'}</text>
            </div>
            <text id="act5-closer" class="mt-[28.08px] whitespace-nowrap font-[fl-sans] text-[#EFEADD] text-[24.84px] leading-[34.78px] opacity-0">Done. Every hex, every radius — straight from your file.</text>
          </div>
          <div id="act5-think" class="absolute left-0 bottom-[6.48px] flex items-center gap-[11.88px] opacity-0">
            <path id="act5-ast" view-box="0 0 24 24" class="w-[24.84px] h-[24.84px] flex-none stroke-[#D97757] stroke-[1.7] [transform-origin:50%_50%]" d="M12 2.2v19.6 M2.2 12h19.6 M5.2 5.2l13.6 13.6 M18.8 5.2L5.2 18.8" />
            <text class="font-[fl-sans-medium] font-[500] text-[#8C8B83] text-[19.44px]">3s · thinking…</text>
          </div>
        </div>
        <div id="act5-composer" class="absolute w-[690px] bottom-[360px] [transform-origin:50%_100%]">
          <div id="act5-chips" class="opacity-0"><ComposerChips /></div>
          <ChatWindow pfx="act5-tl" />
          <div id="act5-metarow" class="opacity-0"><ComposerMeta orbId="act5-orb" /></div>
        </div>
      </div>
      <path id="act5-cursor" view-box="0 0 28 28" class="absolute left-0 top-0 w-[91.8px] h-[91.8px] fill-[#1c1c1c] stroke-[#ffffff] stroke-[1.4] drop-shadow-[0_3.24px_5.94px_rgba(0,0,0,0.35)] opacity-0 [transform-origin:21%_14%]" d={CURSOR_D} />
    </div>
  );
}

/* ════════════ ACT 6 · Figma — select frames, copy link (35.95 → 41.15, z8) ════════════ */
function Act6() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A6.s && T < A7.s;
    vis(ctx, 'sec-act6', ON);
    if (!ON) return;
    var tl = gsap.timeline();
    // INVERSE ZOOM arrival (Z−): the UI lands from large, decelerating
    tl.fromTo('act6-stage', { scale: 1.28 }, { scale: 1, duration: 0.6, ease: 'expo.out' }, A6.s);
    // cursor enters bottom-right
    tl.fromTo('act6-cursor', { x: 980, y: 1140, opacity: 0 }, { x: 300, y: 428, opacity: 1, duration: 0.75, ease: 'power3.out' }, A6.s + 0.5);
    // CLICK 1 · select the parent frame (1.35)
    tl.to('act6-cursor', { scale: 0.84, duration: 0.1, ease: 'power2.in' }, A6.s + 1.35);
    tl.to('act6-cursor', { scale: 1, duration: 0.18, ease: 'power2.out' }, A6.s + 1.46);
    tl.fromTo('act6-sel', { opacity: 0, scale: 1.015 }, { opacity: 1, scale: 1, duration: 0.18, ease: 'power2.out' }, A6.s + 1.42);
    tl.fromTo('act6-lrow', { backgroundColor: 'rgba(13,153,255,0)' }, { backgroundColor: 'rgba(13,153,255,0.28)', duration: 0.15 }, A6.s + 1.42);
    tl.fromTo('act6-flabel', { textColor: '#9a9a9a' }, { textColor: '#4FB8FF', duration: 0.15 }, A6.s + 1.42);
    // travel to Share
    tl.to('act6-cursor', { x: 1010, y: 19, duration: 0.7, ease: 'power3.inOut' }, A6.s + 2.0);
    // CLICK 2 · Share (2.8): press + dropdown
    tl.to('act6-cursor', { scale: 0.84, duration: 0.1, ease: 'power2.in' }, A6.s + 2.8);
    tl.to('act6-cursor', { scale: 1, duration: 0.18, ease: 'power2.out' }, A6.s + 2.91);
    tl.fromTo('act6-share', { scale: 1 }, { scale: 0.93, duration: 0.1, yoyo: true, repeat: 1, ease: 'power2.inOut' }, A6.s + 2.8);
    tl.fromTo('act6-drop', { opacity: 0, scaleY: 0.7 }, { opacity: 1, scaleY: 1, duration: 0.22, ease: 'power3.out' }, A6.s + 2.95);
    // travel to Copy link, hover lights
    tl.to('act6-cursor', { x: 907, y: 77, duration: 0.4, ease: 'power2.inOut' }, A6.s + 3.3);
    tl.fromTo('act6-copyrow', { backgroundColor: 'rgba(13,153,255,0)' }, { backgroundColor: 'rgba(13,153,255,1)', duration: 0.12 }, A6.s + 3.55);
    // CLICK 3 · Copy link (3.8): dropdown closes, toast rises
    tl.to('act6-cursor', { scale: 0.84, duration: 0.1, ease: 'power2.in' }, A6.s + 3.8);
    tl.to('act6-cursor', { scale: 1, duration: 0.18, ease: 'power2.out' }, A6.s + 3.91);
    tl.to('act6-drop', { opacity: 0, scaleY: 0.85, duration: 0.16, ease: 'power2.in' }, A6.s + 3.95);
    tl.fromTo('act6-toast', { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.3, ease: 'power3.out' }, A6.s + 4.1);
    // cursor exits off the top
    tl.to('act6-cursor', { y: -170, duration: 0.45, ease: 'power2.in' }, A6.s + 4.3);
    // EXIT: upward cut-the-curve
    tl.to('act6-stage', { y: -1250, duration: 0.38, ease: 'power4.in' }, A6.s + 4.82);
  });

  var LROW_ICON = 'M2 2h12v12H2z';
  var LROW_CHILD = 'M2 3h12v10H2z';
  return (
    <div id="sec-act6" class="absolute inset-0 z-[9] opacity-0 font-[fl-inter]">
      <div id="act6-ground" class="absolute inset-0 bg-[#1E1E1E]"></div>
      <div id="act6-stage" class="absolute inset-0 [transform-origin:50%_46%]">
        {/* canvas */}
        <div class="absolute left-[224px] right-0 top-[56px] bottom-0 overflow-hidden">
          <text id="act6-flabel" class="absolute left-[56px] top-[378px] font-[fl-inter] text-[#9a9a9a] text-[13px]">app-flow</text>
          <div class="absolute left-[56px] top-[400px] w-[744px] h-[170px] bg-[#0f0f0f] rounded-[4px] pl-[20px] pr-[20px] pt-[22px] pb-[18px]">
            <div class="flex gap-[16px]">
              <div>
                <image path="assets/fig-f1.png" class="w-[166px] h-[94px] rounded-[3px] object-cover" />
                <text class="mt-[8px] font-[fl-inter] text-[#7a7a7a] text-[11px]">1 · Home</text>
              </div>
              <div>
                <image path="assets/fig-f2.png" class="w-[166px] h-[94px] rounded-[3px] object-cover" />
                <text class="mt-[8px] font-[fl-inter] text-[#7a7a7a] text-[11px]">2 · Describe</text>
              </div>
              <div>
                <image path="assets/fig-f3.png" class="w-[166px] h-[94px] rounded-[3px] object-cover" />
                <text class="mt-[8px] font-[fl-inter] text-[#7a7a7a] text-[11px]">3 · Generate</text>
              </div>
              <div>
                <image path="assets/fig-f4.png" class="w-[166px] h-[94px] rounded-[3px] object-cover" />
                <text class="mt-[8px] font-[fl-inter] text-[#7a7a7a] text-[11px]">4 · Result</text>
              </div>
            </div>
          </div>
          {/* selection chrome */}
          <div id="act6-sel" class="absolute left-[52px] top-[396px] w-[752px] h-[178px] border-[2px] border-[#0D99FF] opacity-0 [transform-origin:50%_50%]">
            <div class="absolute left-[-5px] top-[-5px] w-[8px] h-[8px] bg-[#ffffff] border-[1.5px] border-[#0D99FF]"></div>
            <div class="absolute right-[-5px] top-[-5px] w-[8px] h-[8px] bg-[#ffffff] border-[1.5px] border-[#0D99FF]"></div>
            <div class="absolute left-[-5px] bottom-[-5px] w-[8px] h-[8px] bg-[#ffffff] border-[1.5px] border-[#0D99FF]"></div>
            <div class="absolute right-[-5px] bottom-[-5px] w-[8px] h-[8px] bg-[#ffffff] border-[1.5px] border-[#0D99FF]"></div>
            <div class="absolute left-0 right-0 bottom-[-30px] flex justify-center">
              <div class="bg-[#0D99FF] px-[8px] py-[3px] rounded-[3px]">
                <text class="font-[fl-inter-medium] font-[500] text-[#ffffff] text-[11.5px]">2976 × 680</text>
              </div>
            </div>
          </div>
        </div>
        {/* layers panel */}
        <div class="absolute left-0 top-[56px] bottom-0 w-[224px] bg-[#2C2C2C] border-r-[1px] border-[#1a1a1a] pt-[14px]">
          <div class="pl-[16px] pb-[10px]"><text class="font-[fl-inter-medium] font-[500] text-[#888] text-[12px]">Layers</text></div>
          <div id="act6-lrow" class="flex items-center gap-[8px] pl-[16px] pr-[16px] pt-[7px] pb-[7px]">
            <path view-box="0 0 16 16" class="w-[13px] h-[13px] stroke-[#999] stroke-[1.5]" d={LROW_ICON} />
            <text class="font-[fl-inter] text-[#ffffff] text-[13.5px]">app-flow</text>
          </div>
          <div class="flex items-center gap-[8px] pl-[38px] pr-[16px] pt-[7px] pb-[7px]">
            <path view-box="0 0 16 16" class="w-[13px] h-[13px] stroke-[#999] stroke-[1.5]" d={LROW_CHILD} />
            <text class="font-[fl-inter] text-[#cccccc] text-[13.5px]">1 · Home</text>
          </div>
          <div class="flex items-center gap-[8px] pl-[38px] pr-[16px] pt-[7px] pb-[7px]">
            <path view-box="0 0 16 16" class="w-[13px] h-[13px] stroke-[#999] stroke-[1.5]" d={LROW_CHILD} />
            <text class="font-[fl-inter] text-[#cccccc] text-[13.5px]">2 · Describe</text>
          </div>
          <div class="flex items-center gap-[8px] pl-[38px] pr-[16px] pt-[7px] pb-[7px]">
            <path view-box="0 0 16 16" class="w-[13px] h-[13px] stroke-[#999] stroke-[1.5]" d={LROW_CHILD} />
            <text class="font-[fl-inter] text-[#cccccc] text-[13.5px]">3 · Generate</text>
          </div>
          <div class="flex items-center gap-[8px] pl-[38px] pr-[16px] pt-[7px] pb-[7px]">
            <path view-box="0 0 16 16" class="w-[13px] h-[13px] stroke-[#999] stroke-[1.5]" d={LROW_CHILD} />
            <text class="font-[fl-inter] text-[#cccccc] text-[13.5px]">4 · Result</text>
          </div>
        </div>
        {/* top toolbar */}
        <div class="absolute left-0 right-0 top-0 h-[56px] bg-[#2C2C2C] border-b-[1px] border-[#1a1a1a] flex items-center pl-[16px] pr-[16px]">
          <div class="w-[34px] h-[34px] rounded-[6px] flex items-center justify-center">
            <path view-box="0 0 24 24" class="w-[20px] h-[20px] fill-[#ffffff]" d="M12 2 3 7v10l9 5 9-5V7z" />
          </div>
          <div class="flex items-center gap-[4px] ml-[8px]">
            <div class="w-[34px] h-[34px] rounded-[6px] flex items-center justify-center bg-[#0D99FF]">
              <path view-box="0 0 24 24" class="w-[17px] h-[17px] fill-[#ffffff]" d="M6 3l12 9-6 1 3 6-3 1.4L9 14l-3 4z" />
            </div>
            <div class="w-[34px] h-[34px] rounded-[6px] flex items-center justify-center">
              <path view-box="0 0 24 24" class="w-[17px] h-[17px] fill-[#cccccc]" d="M4 4h16v16H4z" />
            </div>
            <div class="w-[34px] h-[34px] rounded-[6px] flex items-center justify-center">
              <path view-box="0 0 24 24" class="w-[17px] h-[17px] stroke-[#cccccc] stroke-[1.8]" d="M4 20L20 4M14 4h6v6" />
            </div>
          </div>
          <div class="absolute left-0 right-0 top-0 h-[56px] flex items-center justify-center">
            <div class="flex items-center gap-[8px]">
              <text class="font-[fl-inter-medium] font-[500] text-[#dddddd] text-[15px]">AI Video Generator — Frames</text>
              <path view-box="0 0 10 6" class="w-[10px] h-[6px] fill-[#888888]" d="M0 0h10L5 6z" />
            </div>
          </div>
          <div class="flex items-center gap-[14px] ml-auto">
            <div class="w-[30px] h-[30px] rounded-full bg-[linear-gradient(135deg,#8AA4B8,#4E6478)] border-[2px] border-[#2C2C2C]"></div>
            <div id="act6-share" class="bg-[#0D99FF] px-[18px] py-[9px] rounded-[7px] [transform-origin:50%_50%]">
              <text class="font-[fl-inter-medium] font-[500] text-[#ffffff] text-[14px]">Share</text>
            </div>
          </div>
        </div>
        {/* share dropdown */}
        <div id="act6-drop" class="absolute right-[14px] top-[60px] w-[290px] bg-[#2C2C2C] border-[1px] border-[#444444] rounded-[10px] p-[8px] opacity-0 shadow-[0_16px_40px_rgba(0,0,0,0.5)] [transform-origin:88%_0%]">
          <div id="act6-copyrow" class="flex items-center gap-[10px] p-[10px] pl-[12px] pr-[12px] rounded-[6px]">
            <path view-box="0 0 24 24" class="w-[16px] h-[16px] stroke-[#cccccc] stroke-[1.6]" d="M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7" />
            <text class="font-[fl-inter] text-[#eeeeee] text-[14px]">Copy link</text>
            <text class="ml-auto font-[fl-inter] text-[#999999] text-[12px]">{'⌘L'}</text>
          </div>
          <div class="flex items-center gap-[10px] p-[10px] pl-[12px] pr-[12px] rounded-[6px]">
            <path view-box="0 0 24 24" class="w-[16px] h-[16px] stroke-[#cccccc] stroke-[1.6]" d="M3 5h18v14H3z M3 9h18" />
            <text class="font-[fl-inter] text-[#eeeeee] text-[14px]">Publish to Community</text>
          </div>
        </div>
        {/* toast */}
        <div class="absolute left-0 right-0 bottom-[56px] flex justify-center">
          <div id="act6-toast" class="flex items-center bg-[#1a1a1a] border-[1px] border-[#3a3a3a] px-[20px] py-[12px] rounded-[8px] shadow-[0_12px_30px_rgba(0,0,0,0.5)] opacity-0">
            <text class="font-[fl-inter] text-[#ffffff] text-[14.5px]">Link copied — </text>
            <text class="font-[fl-inter] text-[#4FB8FF] text-[14.5px]">figma.com/xK9…</text>
          </div>
        </div>
      </div>
      <path id="act6-cursor" view-box="0 0 28 28" class="absolute left-0 top-0 w-[66px] h-[66px] fill-[#1c1c1c] stroke-[#ffffff] stroke-[1.4] drop-shadow-[0_3px_6px_rgba(0,0,0,0.45)] opacity-0 [transform-origin:21%_14%]" d={CURSOR_D} />
    </div>
  );
}

/* ════════════ ACT 7 · Never been easier (41.15 → 44.15, z9) ════════════ */
function Act7() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A7.s && T < A8.s;
    vis(ctx, 'sec-act7', ON);
    if (!ON) return;
    var G7 = globalThis.__flA7 || (globalThis.__flA7 = (function () {
      var adv = makeMeasure(ctx);
      return { sp: adv(ABCD, '700', 88, -1.32, ' ') };
    })());
    ctx.getNode('act7-sp3').width(G7.sp);
    var tl = gsap.timeline();
    // upward mask reveal
    tl.fromTo('act7-in1', { y: 110.3872 }, { y: 0, duration: 0.55, ease: 'power4.out' }, A7.s);
    tl.fromTo('act7-in2', { y: 110.3872 }, { y: 0, duration: 0.6, ease: 'power4.out' }, A7.s + 0.1);
    tl.fromTo('act7-in3', { y: 110.3872 }, { y: 0, duration: 0.65, ease: 'power4.out' }, A7.s + 0.2);
    // exit left, mid-motion at the cut
    tl.to('act7-l1', { x: -1250, duration: 0.34, ease: 'power4.in' }, A7.s + 2.66);
    tl.to('act7-l2', { x: -1250, duration: 0.32, ease: 'power4.in' }, A7.s + 2.68);
    tl.to('act7-l3', { x: -1250, duration: 0.3, ease: 'power4.in' }, A7.s + 2.7);
  });
  var LN = 'font-[fl-display] text-[#ffffff] text-[88px] leading-[98.56px] tracking-[-1.32px] whitespace-nowrap';
  return (
    <div id="sec-act7" class="absolute inset-0 z-[10] opacity-0">
      <div class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))]"></div>
      <div id="act7-l1" class="absolute left-0 top-[390px] w-full h-[104.56px] overflow-hidden flex justify-center">
        <div id="act7-in1" class="flex-none h-[98.56px]"><text class={LN}>Making videos</text></div>
      </div>
      <div id="act7-l2" class="absolute left-0 top-[489px] w-full h-[104.56px] overflow-hidden flex justify-center">
        <div id="act7-in2" class="flex-none h-[98.56px]"><text class={LN}>from your designs</text></div>
      </div>
      <div id="act7-l3" class="absolute left-0 top-[588px] w-full h-[104.56px] overflow-hidden">
        <div id="act7-in3" class="absolute left-0 top-0 w-full h-[98.56px] flex justify-center">
          <text class={LN}>has</text>
          <div id="act7-sp3" class="flex-none"></div>
          <text class={LN + ' text-[#5ef17c]'}>never been easier.</text>
        </div>
      </div>
    </div>
  );
}

/* ════════════ ACT 8 · Even motion works (44.15 → 46.15, z10) ════════════
   "motion" carries a light band sweeping across (bg-position tween in the
   reference) — modeled as three stacked motion texts with per-frame clip
   windows: green-left / light band / green-right. */
function Act8() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A8.s && T < A9.s;
    vis(ctx, 'sec-act8', ON);
    if (!ON) return;
    var t = T - A8.s;

    var G8 = globalThis.__flA8 || (globalThis.__flA8 = (function () {
      var adv = makeMeasure(ctx);
      var sp = adv(ABCD, '700', 84, -1.26, ' ');
      var W = adv(ABCD, '700', 84, -1.26, 'figma motion works');
      var Wp = adv(ABCD, '700', 84, -1.26, 'figma') + sp;
      var Wm = adv(ABCD, '700', 84, -1.26, 'motion');
      return { sp: sp, x0: (1080 - W) / 2, Wp: Wp, Wm: Wm };
    })());
    ctx.getNode('act8-w1').left(G8.x0);
    ctx.getNode('act8-w2').left(G8.x0 + G8.Wp + G8.Wm + G8.sp);
    for (var m = 0; m < 3; m++) ctx.getNode('act8-m' + m).left(G8.x0 + G8.Wp);
    ctx.getNode('act8-sp').left(G8.x0 + G8.Wp + G8.Wm);
    ctx.getNode('act8-sp').width(G8.sp);

    // shimmer sweep: band centre c runs -0.22Wm → 1.22Wm, power2.inOut
    var q = t < 0.95 ? 0 : t > 1.6 ? 1 : easeInOutCubic((t - 0.95) / 0.65);
    var Wm = G8.Wm;
    var c = (-0.22 + 1.44 * q) * Wm;
    var half = 0.132 * Wm;
    var Lb = clamp(c - half, 0, Wm);
    var Rb = clamp(c + half, 0, Wm);
    ctx.getNode('act8-m0').clipPath('inset(0px ' + (Wm - Lb) + 'px 0px 0px)');
    ctx.getNode('act8-m2').clipPath('inset(0px 0px 0px ' + Rb + 'px)');
    ctx.getNode('act8-m1').clipPath('inset(0px ' + (Wm - Rb) + 'px 0px ' + Lb + 'px)');

    var tl = gsap.timeline();
    tl.fromTo('act8-l1', { x: 1150 }, { x: 0, duration: 0.5, ease: 'power4.out' }, A8.s);
    tl.fromTo('act8-l2', { x: 1250 }, { x: 0, duration: 0.55, ease: 'power4.out' }, A8.s + 0.3);
    tl.to('act8-l1', { x: -1250, duration: 0.32, ease: 'power4.in' }, A8.s + 1.68);
    tl.to('act8-l2', { x: -1250, duration: 0.3, ease: 'power4.in' }, A8.s + 1.7);
  });
  var L1 = 'font-[fl-display] text-[#ffffff] text-[100px] leading-[110px] tracking-[-1.5px] whitespace-nowrap';
  var L2 = 'absolute top-0 font-[fl-display] text-[84px] leading-[92.4px] tracking-[-1.26px] whitespace-nowrap';
  return (
    <div id="sec-act8" class="absolute inset-0 z-[11] opacity-0">
      <div class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))]"></div>
      <div id="act8-l1" class="absolute left-0 top-[410px] w-full h-[110px] flex justify-center">
        <text class={L1}>And yes,</text>
      </div>
      <div id="act8-l2" class="absolute left-0 top-[532px] w-full h-[92.4px]">
        <text id="act8-w1" class={L2 + ' text-[#ffffff]'}>figma</text>
        <text id="act8-m0" class={L2 + ' text-[#5ef17c]'}>motion</text>
        <text id="act8-m1" class={L2 + ' text-[#eafff0]'}>motion</text>
        <text id="act8-m2" class={L2 + ' text-[#5ef17c]'}>motion</text>
        <div id="act8-sp" class="absolute top-0 h-[92.4px]"></div>
        <text id="act8-w2" class={L2 + ' text-[#ffffff]'}>works</text>
      </div>
    </div>
  );
}

/* ════════════ ACT 10 · Try it yourself (55.85 → 59.15, z13) ════════════ */
function Act10() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A10.s && T < A11.s;
    vis(ctx, 'sec-act10', ON);
    if (!ON) return;
    var tl = gsap.timeline();
    tl.fromTo('act10-l1', { x: 1150 }, { x: 0, duration: 0.5, ease: 'power4.out' }, A10.s);
    tl.fromTo('act10-l2', { x: 1250 }, { x: 0, duration: 0.55, ease: 'power4.out' }, A10.s + 0.12);
    tl.to('act10-l1', { x: -1250, duration: 0.32, ease: 'power4.in' }, A10.s + 2.98);
    tl.to('act10-l2', { x: -1250, duration: 0.3, ease: 'power4.in' }, A10.s + 3.0);
  });
  return (
    <div id="sec-act10" class="absolute inset-0 z-[20] opacity-0">
      <div class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))]"></div>
      <text id="act10-l1" class="absolute left-0 top-[436px] w-full text-center whitespace-nowrap font-[fl-display] text-[#ffffff] text-[72px] leading-[79.2px] tracking-[-1.08px]">Try it yourself</text>
      <text id="act10-l2" class="absolute left-0 top-[551px] w-full text-center whitespace-nowrap font-[fl-mono] text-[#5ef17c] text-[50px] leading-[60px]">npx hyperframes@latest skills</text>
    </div>
  );
}

/* ════════════ ACT 9 · The motion prompt (46.15 → 55.85, z11) ════════════ */
var A9_SEG1 = 'Use /figma to grab the Motion animation on our outro frame ';
var A9_LINK = 'figma.com/xK9…?node-id=12-4';
var A9_SEG2 = ' and add it to the end of the video';
var A9_REPLY = 'Done — grabbed the Motion animation from your outro frame. Appending it to the end…';

function Act9() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A9.s && T < A10.s;
    vis(ctx, 'sec-act9', ON);
    if (!ON) return;
    var t = T - A9.s;

    var M9 = globalThis.__flM9 || (globalThis.__flM9 = tokenModel(
      makeMeasure(ctx), TTN, '400', 18.9, A9_SEG1, A9_LINK, A9_SEG2));
    var MU9 = globalThis.__flMU9 || (globalThis.__flMU9 = tokenModel(
      makeMeasure(ctx), TTN, '500', 24.84, A9_SEG1, A9_LINK, A9_SEG2));
    // agent2 reply wrap (leading ● atom rides row 0)
    var AG = globalThis.__flAG9 || (globalThis.__flAG9 = (function () {
      var adv = makeMeasure(ctx);
      var sp = adv(TTN, '400', 24.84, 0, ' ');
      var full = [{ t: '●', w: adv(TTN, '400', 24.84, 0, '●'), acc: true }];
      var toks = A9_REPLY.split(' ');
      for (var i = 0; i < toks.length; i++) full.push({ t: toks[i], w: adv(TTN, '400', 24.84, 0, toks[i]) });
      return { atoms: full, rows: wrapAtoms(full, 777.6, sp), sp: sp };
    })());
    ctx.getNode('act9-ag-sp').width(AG.sp);

    var PLEN = A9_SEG1.length + A9_LINK.length + A9_SEG2.length;
    var sent = t >= 3.65;
    var cleared = t >= 3.75;
    var n = cleared ? PLEN : Math.round(PLEN * clamp((t - 0.65) / 2.4, 0, 1));
    var typedAll = cleared ? '' : (A9_SEG1 + A9_LINK + A9_SEG2).slice(0, n);
    var blink = caretBlink(t, 0.4, 7);
    // typed line width: 972 − padding 41.04 − border 2 − pr 43.2
    typedPromptFrame(ctx, 'act9-tl', M9, typedAll, blink, 885.76);
    if (sent) writeSentRows(ctx, 'act9', MU9);
    // agent2 rows
    for (var r = 0; r < 3; r++) {
      var node = ctx.getNode('act9-ag-r' + r);
      if (r < AG.rows.length) {
        var s = '';
        for (var k = 0; k < AG.rows[r].length; k++) {
          var a = AG.atoms[AG.rows[r][k].i];
          if (a.acc) continue;
          s += (s ? ' ' : '') + a.t;
        }
        node.text(s);
        node.opacity(1);
      } else node.text(''), node.opacity(0);
    }

    var tl = gsap.timeline();
    // window enters on the leftward current, mid-flight
    tl.fromTo('act9-win', { x: 1250 }, { x: 0, duration: 0.55, ease: 'power4.out' }, A9.s);
    tl.fromTo('act9-shade', { opacity: 0 }, { opacity: 1, duration: 0.4, ease: 'power2.out' }, A9.s);
    // orb spinner (360° × 1s × 5)
    tl.fromTo('act9-orb', { rotation: 0 }, { rotation: 1800, duration: 5, ease: 'none' }, A9.s);
    // cursor rises to ↵, taps
    tl.fromTo('act9-cursor', { x: 880, y: 1210, opacity: 0 }, { y: 800, opacity: 1, duration: 0.8, ease: 'power3.out' }, A9.s + 2.6);
    tl.to('act9-cursor', { scale: 0.84, duration: 0.1, ease: 'power2.in' }, A9.s + 3.5);
    tl.to('act9-cursor', { scale: 1, duration: 0.2, ease: 'power2.out' }, A9.s + 3.6);
    tl.fromTo('act9-tl-ret', { textColor: '#6F6E66' }, { textColor: '#D97757', duration: 0.08, ease: 'power1.in' }, A9.s + 3.58);
    tl.to('act9-tl-ret', { textColor: '#6F6E66', duration: 0.25 }, A9.s + 3.78);
    // send
    tl.fromTo('act9-usermsg', { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power3.out' }, A9.s + 3.7);
    tl.to('act9-cursor', { y: 880, opacity: 0, duration: 0.35, ease: 'power2.in' }, A9.s + 3.75);
    tl.fromTo('act9-think', { opacity: 0 }, { opacity: 1, duration: 0.3 }, A9.s + 4.1);
    tl.fromTo('act9-ast', { rotation: 0 }, { rotation: 360, duration: 1.0, ease: 'none' }, A9.s + 4.1);
    tl.to('act9-think', { opacity: 0, duration: 0.25 }, A9.s + 5.0);
    tl.fromTo('act9-agent2', { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.45, ease: 'power3.out' }, A9.s + 5.1);
    // EXIT zoom-through
    tl.to('act9-win', { scale: 1.22, duration: 0.35, ease: 'power3.in' }, A9.s + 6.45);
    tl.to('act9-win', { opacity: 0.15, duration: 0.35, ease: 'none' }, A9.s + 6.45);
    tl.set('act9-win', { opacity: 0 }, A9.s + 6.8);
  });

  return (
    <div id="sec-act9" class="absolute inset-0 z-[12] opacity-0">
      <div id="act9-shade" class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))] opacity-0"></div>
      <div id="act9-win" class="absolute left-[54px] top-[97px] w-[972px] h-[885.6px] rounded-[25.9px] bg-[#1D1F1F] overflow-hidden shadow-[0_32.4px_97.2px_-32.4px_rgba(0,0,0,0.6)] [transform-origin:50%_50%]">
        <div class="absolute top-[23.76px] left-[28.08px] flex gap-[10.8px]">
          <div class="w-[17.28px] h-[17.28px] rounded-full bg-[#FF5F57]"></div>
          <div class="w-[17.28px] h-[17.28px] rounded-full bg-[#FEBC2E]"></div>
          <div class="w-[17.28px] h-[17.28px] rounded-full bg-[#28C840]"></div>
        </div>
        <div class="absolute left-[54px] right-[54px] top-[97.2px] bottom-[280.8px]">
          <text class="absolute left-0 top-0 whitespace-nowrap font-[fl-sans] text-[rgba(216,212,200,0.4)] text-[22.68px]">Done. Every hex, every radius — straight from your file.</text>
          <SentMessage pfx="act9" cls="absolute right-0 top-[86.4px] w-[756px] h-[95.4px] opacity-0" />
          <div id="act9-agent2" class="absolute left-0 top-[183.6px] w-[777.6px] opacity-0">
            <div class="absolute left-0 top-0 h-[34.78px] flex items-center whitespace-nowrap">
              <div class="w-[12.4px] h-[12.4px] rounded-full bg-[#D97757] flex-none"></div>
              <div id="act9-ag-sp" class="flex-none"></div>
              <text id="act9-ag-r0" class="font-[fl-sans] text-[#D8D4C8] text-[24.84px] leading-[34.78px]"></text>
            </div>
            <text id="act9-ag-r1" class="absolute left-0 top-[34.78px] whitespace-nowrap font-[fl-sans] text-[#D8D4C8] text-[24.84px] leading-[34.78px] opacity-0"></text>
            <text id="act9-ag-r2" class="absolute left-0 top-[69.56px] whitespace-nowrap font-[fl-sans] text-[#D8D4C8] text-[24.84px] leading-[34.78px] opacity-0"></text>
          </div>
          <div id="act9-think" class="absolute left-0 bottom-[6.48px] flex items-center gap-[11.88px] opacity-0">
            <path id="act9-ast" view-box="0 0 24 24" class="w-[24.84px] h-[24.84px] flex-none stroke-[#D97757] stroke-[1.7] [transform-origin:50%_50%]" d="M12 2.2v19.6 M2.2 12h19.6 M5.2 5.2l13.6 13.6 M18.8 5.2L5.2 18.8" />
            <text class="font-[fl-sans-medium] font-[500] text-[#8C8B83] text-[19.44px]">2s · thinking…</text>
          </div>
        </div>
        <div class="absolute left-[48.6px] right-[48.6px] bottom-[48.6px]">
          <ComposerChips />
          <ChatWindow pfx="act9-tl" />
          <ComposerMeta orbId="act9-orb" />
        </div>
      </div>
      <path id="act9-cursor" view-box="0 0 28 28" class="absolute left-0 top-0 w-[91.8px] h-[91.8px] fill-[#1c1c1c] stroke-[#ffffff] stroke-[1.4] drop-shadow-[0_3.24px_5.94px_rgba(0,0,0,0.35)] opacity-0 [transform-origin:21%_14%]" d={CURSOR_D} />
    </div>
  );
}

/* ════════════ ACT 11 · Logo outro (59.15 → 64.05, z14) ════════════
   Wings fly in → liquid gradient fill (clip reveal; the reference's two
   userSpace linearGradients become three horizontal color bands, sampled
   cyan→green top-down on the left wing and reversed on the right) → cursor
   click → the mark flies to its lockup slot → wordmark mask-reveals →
   GitHub star pill gets clicked. */
var A11_WING_L = 'M195.219 26.1937L213.529 38.9937C216.009 40.7237 220.239 38.7637 221.009 35.5337L228.419 4.33374C229.189 1.10374 225.879 -0.856262 222.589 0.873738L198.199 13.6737C192.649 16.5837 191.059 23.2837 195.219 26.1937Z';
var A11_WING_R = 'M256.97 25.9638L232.58 38.7638C229.28 40.4938 225.98 38.5338 226.75 35.3038L234.16 4.10376C234.93 0.873757 239.16 -1.08624 241.64 0.643757L259.95 13.4438C264.12 16.3538 262.52 23.0538 256.97 25.9638Z';
var A11_VB = '190 -2 74 43';
/* liquid fill: the reference paints each wing with its own near-vertical
   userSpaceOnUse linearGradient (cyan #06E3FA → green #4FDB5E). Path fills
   here take a flat colour, so the gradient is band-sampled: 16 horizontal
   strips, each wing coloured at its gradient's t(strip centre). */
var A11_BANDS = [
  { l: '#06E2F3', r: '#4EC35B' },
  { l: '#0DD6E3', r: '#4BD161' },
  { l: '#11DADC', r: '#4AD463' },
  { l: '#15DCD4', r: '#49D666' },
  { l: '#1BDCC9', r: '#45D86D' },
  { l: '#1FDCBD', r: '#42D874' },
  { l: '#26DBB2', r: '#3EDA7E' },
  { l: '#2ADBA7', r: '#39DA8A' },
  { l: '#30DB9C', r: '#33DA95' },
  { l: '#35DA91', r: '#2DDAA2' },
  { l: '#3BD985', r: '#27DBB0' },
  { l: '#3FD87B', r: '#20DBBE' },
  { l: '#44D871', r: '#19DDCE' },
  { l: '#49D666', r: '#12DCDA' },
  { l: '#4ED55E', r: '#0BDDEA' },
  { l: '#4FDB5E', r: '#08DFF0' },
];
var A11_LOCK_PATHS = [
  'M0 16.6737H4.96V23.3837H11.53V16.6737H16.49V35.4537H11.53V27.6737H4.96V35.4537H0V16.6737Z',
  'M17.89 28.0137C17.89 23.4237 21.24 20.0237 25.8 20.0237C30.01 20.0237 32.94 23.0837 32.94 27.5637C32.94 28.2037 32.86 28.8237 32.83 29.3337H22.61C23.04 31.1337 24.62 32.1237 27.14 32.1237C28.72 32.1237 30.22 31.7237 31.3 31.0237V34.6737C30.17 35.3937 28.3 35.8537 26.36 35.8537C21.26 35.8537 17.88 32.6037 17.88 28.0237L17.89 28.0137ZM28.57 26.1937C28.49 24.5337 27.39 23.5637 25.75 23.5637C24.11 23.5637 22.96 24.5337 22.61 26.1937H28.57Z',
  'M37.85 33.8637L32 20.4237H37.29L40.46 28.1537L43.6 20.4237H48.48L40.33 39.5737H35.26L37.86 33.8637H37.85Z',
  'M48.1797 26.1138C48.1797 20.4238 52.7097 16.2438 58.9397 16.2438C61.2997 16.2438 63.5297 16.8038 64.8197 17.7238V21.9338C63.5597 20.9138 61.6797 20.2938 59.7497 20.2938C55.7797 20.2938 53.2597 22.6238 53.2597 26.1438C53.2597 29.6638 55.5697 31.8838 58.8897 31.8838C59.6397 31.8838 60.4997 31.7538 61.1997 31.5038V27.8838H57.4697V24.2338H65.5197V33.7038C63.6697 35.0938 61.1997 35.8738 58.5997 35.8738C52.5397 35.8738 48.1897 31.7438 48.1897 26.1138H48.1797Z',
  'M66.6604 28.0137C66.6604 23.4237 70.0104 20.0237 74.5704 20.0237C78.7804 20.0237 81.7104 23.0837 81.7104 27.5637C81.7104 28.2037 81.6304 28.8237 81.6004 29.3337H71.3804C71.8104 31.1337 73.3904 32.1237 75.9104 32.1237C77.4904 32.1237 78.9904 31.7237 80.0704 31.0237V34.6737C78.9404 35.3937 77.0704 35.8537 75.1304 35.8537C70.0304 35.8537 66.6504 32.6037 66.6504 28.0237L66.6604 28.0137ZM77.3404 26.1937C77.2604 24.5337 76.1604 23.5637 74.5204 23.5637C72.8804 23.5637 71.7304 24.5337 71.3804 26.1937H77.3404Z',
  'M82.9697 20.4238H87.8497V22.4138C88.7597 20.9638 90.3497 20.1038 92.3297 20.1038C95.4997 20.1038 97.3997 22.3338 97.3997 25.8138V35.4438H92.5197V26.8638C92.5197 25.0438 91.8197 24.1838 90.4797 24.1838C88.9497 24.1838 87.8497 25.3638 87.8497 27.2638V35.4438H82.9697V20.4238Z',
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
var A11_GH_STAR = 'M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 .416 1.279l-3.046 2.97.719 4.192a.75.75 0 0 1-1.088.791L8 12.347l-3.766 1.98a.75.75 0 0 1-1.088-.79l.72-4.194L.818 6.374a.75.75 0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25Zm0 2.445L6.615 5.5a.75.75 0 0 1-.564.41l-3.097.45 2.24 2.184a.75.75 0 0 1 .216.664l-.528 3.084 2.769-1.456a.75.75 0 0 1 .698 0l2.77 1.456-.53-3.084a.75.75 0 0 1 .216-.664l2.24-2.183-3.096-.45a.75.75 0 0 1-.564-.41L8 2.694Z';
var A11_GH_STAR_ON = 'M8 .25a.75.75 0 0 1 .673.418l1.882 3.815 4.21.612a.75.75 0 0 1 .416 1.279l-3.046 2.97.719 4.192a.75.75 0 0 1-1.088.791L8 12.347l-3.766 1.98a.75.75 0 0 1-1.088-.79l.72-4.194L.818 6.374a.75.75 0 0 1 .416-1.28l4.21-.611L7.327.668A.75.75 0 0 1 8 .25Z';

function FillBand(props) {
  return (
    <div id={props.id} class="absolute left-0 top-0 w-[491.4px] h-[285.55px]">
      <path id={'a11-fwL' + props.n} view-box={A11_VB} class={'absolute left-0 top-0 w-[491.4px] h-[285.55px] fill-[' + props.lColor + ']'} d={A11_WING_L} />
      <path id={'a11-fwR' + props.n} view-box={A11_VB} class={'absolute left-0 top-0 w-[491.4px] h-[285.55px] fill-[' + props.rColor + ']'} d={A11_WING_R} />
    </div>
  );
}

function LockPath(props) {
  return <path view-box="0 0 263 79" class="absolute left-0 top-0 w-[907.2px] h-[272.5px] fill-[#ffffff]" d={A11_LOCK_PATHS[props.idx]} />;
}

function Act11() {
  useGSAP(function () {
    var T = ctx.global.time;
    var ON = T >= A11.s;
    vis(ctx, 'sec-act11', ON);
    if (!ON) return;

    // wing tweens land on both the white base path and the three fill-band
    // paths of the same wing (SVG px = user units × 491.4/74)
    var XPX = 491.4 / 74;
    var wingL = ['a11-bwL'];
    var wingR = ['a11-bwR'];
    for (i = 0; i < A11_BANDS.length; i++) {
      wingL.push('a11-fwL' + (i + 1));
      wingR.push('a11-fwR' + (i + 1));
    }
    var allWings = wingL.concat(wingR);
    var i;
    var F = 0.85, FD = 0.9;

    var tl = gsap.timeline();
    // initial states
    for (i = 0; i < allWings.length; i++) tl.set(allWings[i], { x: 34 * XPX, opacity: 0 }, A11.s);
    tl.set('a11-fill', { clipPath: 'inset(100% 0% 0% 0%)' }, A11.s);
    for (i = 0; i < A11_BANDS.length; i++) {
      // bottom edge overlaps the next band by 0.4% so rounding gaps never let
      // the white base wing show through as hairline seams
      var bottom = Math.max(0, (A11_BANDS.length - 1 - i) * 100 / A11_BANDS.length - 0.4);
      tl.set('a11-b' + (i + 1), { clipPath: 'inset(' + (i * 100 / A11_BANDS.length) + '% 0% ' + bottom + '% 0%)' }, A11.s);
    }
    tl.set('a11-lock', { opacity: 0, y: -62, clipPath: 'inset(0% 0% 0% 100%)' }, A11.s);
    tl.set('a11-cursor', { x: 532, y: 1210 }, A11.s);
    tl.set('a11-pill', { y: 40 }, A11.s);

    // ENTRY — cut-the-curve: wings answer from the right
    for (i = 0; i < wingL.length; i++) {
      tl.set(wingL[i], { opacity: 0.35 }, A11.s);
      tl.to(wingL[i], { x: 0, opacity: 1, duration: 0.3, ease: 'power4.out' }, A11.s);
    }
    for (i = 0; i < wingR.length; i++) {
      tl.set(wingR[i], { opacity: 0.35 }, A11.s + 0.05);
      tl.to(wingR[i], { x: -5.2 * XPX, opacity: 1, duration: 0.22, ease: 'power3.out' }, A11.s + 0.05);
      tl.to(wingR[i], { x: 0, duration: 0.18, ease: 'back.out(2.5)' }, A11.s + 0.27);
    }
    // CURSOR CLICK 1 — press the mark, launch the flight
    tl.to('a11-cursor', { y: 574, duration: 0.45, ease: 'power3.out' }, A11.s + 0.25);
    tl.to('a11-cursor', { scale: 0.84, duration: 0.08, ease: 'power2.in' }, A11.s + 0.67);
    tl.to('a11-cursor', { scale: 1, duration: 0.18, ease: 'power2.out' }, A11.s + 0.75);
    tl.to('act11-mark', { y: 16, scale: 0.90, duration: 0.12, ease: 'power3.out' }, A11.s + 0.73);
    tl.to('a11-cursor', { x: 640, y: 780, duration: 0.5, ease: 'power2.inOut' }, A11.s + 0.92);
    // F = press release: liquid fill + flight ignite together
    tl.fromTo('a11-fill', { clipPath: 'inset(100% 0% 0% 0%)' }, { clipPath: 'inset(0% 0% 0% 0%)', duration: FD, ease: 'none' }, A11.s + F);
    tl.to('act11-mark', { x: 329.4, duration: FD, ease: 'markFlightX' }, A11.s + F);
    tl.to('act11-mark', { y: -131.0, duration: FD, ease: 'markFlightY' }, A11.s + F);
    tl.to('act11-mark', { scale: 1.05, duration: 0.16, ease: 'back.out(2)' }, A11.s + F);
    tl.to('act11-mark', { scale: 0.5194, duration: FD - 0.16, ease: 'markShrink' }, A11.s + F + 0.16);
    tl.to('act11-mark', { rotation: 6, duration: FD / 2, ease: 'sine.inOut' }, A11.s + F);
    tl.to('act11-mark', { rotation: 0, duration: FD / 2, ease: 'sine.inOut' }, A11.s + F + FD / 2);
    // IMPACT RECOIL — trailing wing compresses, snaps out, rides back
    for (i = 0; i < wingL.length; i++) {
      tl.to(wingL[i], { x: 3.2 * XPX, duration: 0.07, ease: 'power3.out' }, A11.s + F + FD);
      tl.to(wingL[i], { x: -1.8 * XPX, duration: 0.09, ease: 'power2.out' }, A11.s + F + FD + 0.07);
      tl.to(wingL[i], { x: 0, duration: 0.55, ease: 'power1.inOut' }, A11.s + F + FD + 0.16);
    }
    // MASK REVEAL — the wordmark sweeps open on the rebound frame
    tl.set('a11-lock', { opacity: 1 }, A11.s + F + FD + 0.07);
    tl.to('a11-lock', { clipPath: 'inset(0% 0% 0% 0%)', duration: 0.09, ease: 'power2.out' }, A11.s + F + FD + 0.07);
    // STAR BEAT
    tl.set('a11-pill', { opacity: 1 }, A11.s + 2.05);
    tl.to('a11-pill', { y: 0, duration: 0.35, ease: 'power4.out' }, A11.s + 2.05);
    tl.to('a11-cursor', { x: 517, y: 685, duration: 0.5, ease: 'power3.inOut' }, A11.s + 2.2);
    tl.to('a11-cursor', { scale: 0.84, duration: 0.08, ease: 'power2.in' }, A11.s + 2.85);
    tl.to('a11-cursor', { scale: 1, duration: 0.18, ease: 'power2.out' }, A11.s + 2.93);
    tl.fromTo('a11-pill', { scale: 1 }, { scale: 0.96, duration: 0.09, yoyo: true, repeat: 1, ease: 'power2.inOut' }, A11.s + 2.85);
    tl.to('a11-staroff', { opacity: 0, duration: 0.08 }, A11.s + 2.95);
    tl.fromTo('a11-staron', { opacity: 0, scale: 0.4 }, { opacity: 1, scale: 1, duration: 0.4, ease: 'back.out(2.4)' }, A11.s + 2.95);
    tl.to('a11-label', { width: 104, duration: 0.28, ease: 'power2.inOut' }, A11.s + 2.98);
    tl.to('a11-lcol', { y: -34, duration: 0.28, ease: 'power3.inOut' }, A11.s + 2.98);
    tl.to('a11-ccount', { y: -36, duration: 0.3, ease: 'power3.inOut' }, A11.s + 3.05);
    tl.to('a11-cursor', { y: 1160, duration: 0.5, ease: 'power2.in' }, A11.s + 3.45);
  });

  function fillBand(id, lColor, rColor) {
    return (
      <div id={id} class="absolute left-0 top-0 w-[491.4px] h-[285.55px]">
        <path view-box={A11_VB} class="absolute left-0 top-0 w-[491.4px] h-[285.55px]" d={A11_WING_L} fill={lColor} />
        <path view-box={A11_VB} class="absolute left-0 top-0 w-[491.4px] h-[285.55px]" d={A11_WING_R} fill={rColor} />
      </div>
    );
  }
  return (
    <div id="sec-act11" class="absolute inset-0 z-[21] opacity-0">
      <div class="absolute inset-0 bg-[linear-gradient(180deg,rgba(0,0,0,0.5),rgba(0,0,0,0.25)_40%,rgba(0,0,0,0.55))]"></div>
      {/* liquid-fill mark: white base + gradient band overlay */}
      <div id="act11-mark" class="absolute left-[294.3px] top-[397.225px] w-[491.4px] h-[285.55px] [transform-origin:50%_50%]">
        <path id="a11-bwL" view-box={A11_VB} class="absolute left-0 top-0 w-[491.4px] h-[285.55px] fill-[#ffffff]" d={A11_WING_L} />
        <path id="a11-bwR" view-box={A11_VB} class="absolute left-0 top-0 w-[491.4px] h-[285.55px] fill-[#ffffff]" d={A11_WING_R} />
        <div id="a11-fill" class="absolute left-0 top-0 w-[491.4px] h-[285.55px]">
          {A11_BANDS.map(function (c, i) {
            return <FillBand key={'b' + i} id={'a11-b' + (i + 1)} n={i + 1} lColor={c.l} rColor={c.r} />;
          })}
        </div>
      </div>
      {/* final wordmark (no symbol, white) */}
      <div id="a11-lock" class="absolute left-[86.4px] top-[403.75px] w-[907.2px] h-[272.5px] [transform-origin:50%_50%]">
        <LockPath idx={0} /><LockPath idx={1} /><LockPath idx={2} /><LockPath idx={3} /><LockPath idx={4} /><LockPath idx={5} /><LockPath idx={6} /><LockPath idx={7} /><LockPath idx={8} /><LockPath idx={9} /><LockPath idx={10} /><LockPath idx={11} /><LockPath idx={12} /><LockPath idx={13} /><LockPath idx={14} /><LockPath idx={15} /><LockPath idx={16} />
      </div>
      {/* GitHub star pill */}
      <div class="absolute left-0 right-0 top-[668px] h-[64px] flex justify-center">
        <div id="a11-pill" class="flex items-center h-[64px] bg-[#21262D] border-[1.5px] border-[#363B42] rounded-[14px] opacity-0 [transform-origin:50%_50%]">
          <div class="flex items-center gap-[13px] pl-[20px] pr-[20px] flex-none">
            <div class="relative w-[32px] h-[32px] flex-none">
              <path id="a11-staroff" view-box="0 0 16 16" class="absolute left-0 top-0 w-[32px] h-[32px] fill-[#9198A1]" d={A11_GH_STAR} />
              <path id="a11-staron" view-box="0 0 16 16" class="absolute left-0 top-0 w-[32px] h-[32px] opacity-0 [transform-origin:50%_50%] fill-[#E3B341]" d={A11_GH_STAR_ON} />
            </div>
            <div id="a11-label" class="relative w-[58px] h-[34px] overflow-hidden flex-none">
              <div id="a11-lcol" class="absolute left-0 top-0">
                <div class="h-[34px] flex items-center"><text class="font-[fl-segoe-bold] font-[600] text-[#E6EDF3] text-[26px] leading-[34px]">Star</text></div>
                <div class="h-[34px] flex items-center"><text class="font-[fl-segoe-bold] font-[600] text-[#E6EDF3] text-[26px] leading-[34px]">Starred</text></div>
              </div>
            </div>
            <div class="h-[36px] rounded-full bg-[#14181F] pl-[16px] pr-[16px] overflow-hidden flex-none">
              <div id="a11-ccount" class="relative left-0 top-0">
                <div class="h-[36px] flex items-center"><text class="font-[fl-segoe-bold] font-[600] text-[#E6EDF3] text-[23px] leading-[36px]">34.1k</text></div>
                <div class="h-[36px] flex items-center"><text class="font-[fl-segoe-bold] font-[600] text-[#E6EDF3] text-[23px] leading-[36px]">34.2k</text></div>
              </div>
            </div>
          </div>
          <div class="w-[1.5px] h-[64px] bg-[#363B42] flex-none"></div>
          <div class="flex items-center pl-[18px] pr-[18px] flex-none">
            <path view-box="0 0 16 16" class="w-[18px] h-[18px] fill-[#9198A1]" d="M4.2 5.8h7.6L8 10.4z" />
          </div>
        </div>
      </div>
      <path id="a11-cursor" view-box="0 0 28 28" class="absolute left-0 top-0 w-[105.84px] h-[105.84px] fill-[#1c1c1c] stroke-[#ffffff] stroke-[1.4] drop-shadow-[0_2.16px_3.78px_rgba(0,0,0,0.5)] [transform-origin:21%_14%]" d={CURSOR_D} />
    </div>
  );
}

/* ════════════ MASTER · media seams (video windows + card zooms) ════════════ */
export default function FigmaLaunch() {
  useGSAP(function () {
    var T = ctx.global.time;
    for (var id in VID_WIN) vis(ctx, id, T >= VID_WIN[id][0] && T < VID_WIN[id][1]);
    var tl = gsap.timeline();
    tl.set('output-vid', { transformOrigin: '50% 50%' }, 0);
    tl.fromTo('output-vid', { scale: 0.75, opacity: 0.15 }, { scale: 1, opacity: 1, duration: 0.5, ease: 'expo.out' }, 25.95);
    tl.set('tail-vid', { transformOrigin: '50% 50%' }, 0);
    tl.fromTo('tail-vid', { scale: 0.75, opacity: 0.15 }, { scale: 1, opacity: 1, duration: 0.5, ease: 'expo.out' }, 52.95);
    tl.to('tail-vid', { x: -1300, duration: 0.35, ease: 'power4.in' }, 55.5);
  });

  var fullvid = 'absolute inset-0 w-full h-full object-cover';
  return (
    <div class="relative w-[1080px] h-[1080px] bg-[#000000] overflow-hidden">
      <video id="bg-act1" path="assets/bg-act1.mp4" data-start="3.4" data-duration="8.38" class={fullvid}  data-hide-after-end="true" />
      <Act1 />
      <Act2 />
      <video id="bg-act34" path="assets/bg-act34.mp4" data-start="14.7" data-duration="3.3" class={fullvid}  data-hide-after-end="true" />
      <Act3 />
      <Act4 />
      <video id="bg-act5" path="assets/bg-act5.mp4" data-start="21.5" data-duration="14.45" class={fullvid}  data-hide-after-end="true" />
      <Act5 />
      {/* output card is track 7 — ABOVE act5's shade (track 5) */}
      <video id="output-vid" path="assets/first-output.mp4" data-start="25.95" data-duration="10" class="absolute z-[7] left-[60px] top-[270px] w-[960px] h-[540px] object-cover rounded-[22px]"  data-hide-after-end="true" />
      <Act6 />
      <video id="bg-act78" path="assets/bg-act78.mp4" data-start="41.15" data-duration="5" class={fullvid}  data-hide-after-end="true" />
      <Act7 />
      <Act8 />
      <video id="bg-act9" path="assets/bg-act9.mp4" data-start="46.15" data-duration="9.7" class={fullvid}  data-hide-after-end="true" />
      <Act9 />
      <video id="tail-vid" path="assets/motion-output.mp4" data-start="52.95" data-duration="2.9" class="absolute z-[14] left-[60px] top-[270px] w-[960px] h-[540px] object-cover rounded-[22px]"  data-hide-after-end="true" />
      <video id="bg-act1011" path="assets/bg-act1011.mp4" data-start="55.85" data-duration="8.2" class={fullvid}  data-hide-after-end="true" />
      <Act10 />
      <Act11 />
    </div>
  );
}
