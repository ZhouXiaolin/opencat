// pr-to-video-launch — JSX port of the hyperframes 30 s launch film
// (reference: /Users/solaren/Projects/hyperframes-launches/pr-to-video-launch, sibling .xml is the
// line-by-line ancestor of this file; deviations are marked FIX below).
import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 30.0 };

export const fonts = {
  default: 'hk4',
  faces: [
    { id: 'hk4', family: 'Hanken Grotesk', path: 'assets/HankenGrotesk-400.ttf', role: 'sans' },
    { id: 'hk5', family: 'Hanken Grotesk', path: 'assets/HankenGrotesk-500.ttf' },
    { id: 'hk6', family: 'Hanken Grotesk', path: 'assets/HankenGrotesk-600.ttf' },
    { id: 'hk7', family: 'Hanken Grotesk', path: 'assets/HankenGrotesk-700.ttf' },
    { id: 'gal', family: 'Galaxie Copernicus', path: 'assets/GalaxieCopernicus-Book.ttf' },
    { id: 'mono4', family: 'Spline Sans Mono', path: 'assets/SplineSansMono-400.ttf', role: 'mono' },
    { id: 'mono5', family: 'Spline Sans Mono', path: 'assets/SplineSansMono-500.ttf', role: 'mono' },
    { id: 'black', family: 'Archivo Black', path: 'assets/ArchivoBlack.ttf' },
  ],
};

// ── root cross-fade points (index.html timeline) ────────────────────────────
const O2 = 5.2, O3 = 8.4, O4 = 9.3, O5 = 12.3, O6 = 16.1, O7 = 18.5, O8 = 21.0, O9 = 23.0, O10 = 25.6;

// ── opener typing: the reference input wraps after "can" (frame-verified) ──
const TYPED1 = 'Someone on our team just opened a PR for a new pr-to-video skill — can';
const TYPED2 = 'you review it?';
const MSG = TYPED1 + ' ' + TYPED2;
const CUT = MSG.indexOf(TYPED2);                       // chars typed on row 1
// 3-frame fit ref0.9=17 ref1.5=51 ref2.0=80(='you revi') chars @56.65/s: LINEAR
// from 0.6s over 1.165s satisfies all three (ROW2_AT lands on 1.50 too)
const TYPE_T0 = 0.6, TYPE_DUR = 1.518;
const ROW2_AT = TYPE_T0 + TYPE_DUR * (CUT / MSG.length); // typing reaches row 2
const PR_URL = ' github.com/heygen-com/hyperframes/pull/1635';

// typewriter: reveal `msg` char-by-char over [t0, t0+dur]; the proxy's from
// value is applied on every earlier frame, so the node reads empty until t0.
function tw(T, ctx, id, msg, t0, dur, ease) {
  const node = ctx.getNode(id);
  const proxy = { set: (v) => node.text(msg.slice(0, Math.round(v.opacity))) };
  T.fromTo(proxy, { opacity: 0 }, { opacity: msg.length, duration: dur, ease: ease || 'none' }, t0);
}

// pr-scroll typeNum replica: the ref tweens with ease:`steps(len)`, and GSAP's
// SteppedEase is floor((len+1)*min(p,0.99999999))/len (gsap.min.js
// `config` body), so the char count = floor((len+1)*p) with p clamped just
// under 1 — NOT the linear round(len*p) the text typewriters use.
// Frame-verified on the ref render: "87,291" chars land at frames
// 216/219/222/226/228/231 (scene t 2.0/2.1/2.2/2.333/2.4/2.5).
function typeSteps(T, ctx, id, str, t0, dur) {
  const node = ctx.getNode(id);
  const proxy = { set: (v) => {
    const p = Math.min(Math.max(v.opacity, 0), 0.999999);
    node.text(str.slice(0, Math.floor((str.length + 1) * p)));
  } };
  T.fromTo(proxy, { opacity: 0 }, { opacity: 1, duration: dur, ease: 'none' }, t0);
}

function reveal(T, el, t) {
  T.fromTo(el, { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.3, ease: 'power3.out' }, t);
}

// feature-card title: typed under the card's inner timeScale; caret blinks
// (ref per-scene repeat: fc/ff 9, cr 7) then fades at `fadeAt` inner-seconds
// (per-scene: fc 4.5 / ff 6.0 / cr 6.1).
function cardTitle(T, ctx, id, caret, title, off, dur, signed, fadeAt, rep) {
  tw(T, ctx, id, title, off + 0.3 / signed, dur / signed);
  T.to(caret, { opacity: 0, duration: 0.4, ease: 'steps(1)', repeat: rep || 9, yoyo: true }, off + 0.3 / signed);
  T.to(caret, { opacity: 0, duration: 0.3, ease: 'power2.in' }, off + fadeAt / signed);
}

// ── pr-scroll data: 17 file rows, cloned x8 so the -3600px whip never runs dry
const FILES = [
  ['skills/pr-to-video/SKILL.md', '+212', '−40'],
  ['skills/pr-to-video/scripts/fetch-pr.mjs', '+148', '−0'],
  ['skills/pr-to-video/scripts/ingest.mjs', '+96', '−0'],
  ['skills/product-launch/SKILL.md', '+96', '−311'],
  ['skills/product-launch/references/story-design.md', '+40', '−642'],
  ['skills/faceless-explainer/SKILL.md', '+58', '−280'],
  ['skills/faceless-explainer/references/story.md', '+44', '−980'],
  ['skills/hyperframes-core/references/storyboard-format.md', '+61', '−120'],
  ['skills/hyperframes-media/references/tts.md', '+19', '−3,402'],
  ['skills/hyperframes-animation/references/blueprints.md', '+30', '−5,118'],
  ['skills/hyperframes-creative/frame-presets/claude/FRAME.md', '+402', '−0'],
  ['registry/blocks/code-diff/block.html', '+140', '−12'],
  ['registry/blocks/code-typing/block.html', '+96', '−8'],
  ['packages/cli/src/commands/init.ts', '+24', '−240'],
  ['packages/engine/src/capture/scroll.ts', '+12', '−1,904'],
  ['skills/hyperframes-animation/atoms/transitions.md', '+8', '−6,210'],
  ['skills/hyperframes-media/voices/registry.json', null, '−14,880'],
];
const ROW_BASES = ['auto', 'cl1', 'cl2', 'cl3', 'cl4', 'cl5', 'cl6', 'cl7'];

function FileRow({ base, i }) {
  const [name, add, del] = FILES[i];
  // leading 22.98 (not 23.04=1.2em): the whip scroll reads clone groups 2..4,
  // and group-height fitting on f228/f241 gives ref row height 59.31
  // (2×17.664 pad + 22.98 + 1px border, group = 1008.2px) vs our 1009.26 —
  // a 1px-per-group drift that reads dy -2..-3 by group 4 (f228 dy -2,
  // f241 dy -3 at leading 23.04).
  const mono = 'text-[19.2px] leading-[22.98px] font-[mono4]';
  // ref pr-scroll.html:90 .pm{margin-left:auto}: one right-aligned block of
  // add + one space (11.52px) + del, inner right edge x=1036.8 — add x varies
  // per row (row2 '+19 -3,402' starts 921.6, row4 '+44 -980' starts 944.6).
  return (
    <div id={base + '_' + String(55 + i * 5).padStart(3, '0')} class="relative flex items-center gap-[12px] px-[30.72px] py-[17.664px] border-b-[1px] border-[#f0f0f0]">
      {/* ref uses the text glyph ▾; the path's ink sits 3px right of it */}
      <path d="M4 7 L20 7 L12 17 Z" view-box="0 0 24 24" class="relative -left-[3px] w-[15px] h-[15px] fill-[#59636e] flex-none" />
      <p class={mono + ' text-[#1f2328]'}>{name}</p>
      <div class="ml-auto flex items-center">
        {add !== null && <p class={mono + ' text-[#1a7f37]'}>{add}</p>}
        {add !== null && <div class="w-[11.52px]"></div>}
        <p class={mono + ' text-[#cf222e]'}>{del}</p>
      </div>
    </div>
  );
}

// pr-scroll review rows (FIX: real avatar photos, not flat circles)
const REVIEWS = [
  ['jrusso1020', ' requested changes · 14 comments', 'assets/avatar-james-36.png'],
  ['miguel-heygen', ' left 9 comments on 6 files', 'assets/avatar-miguel-36.png'],
  ['WaterrrForever', ' pushed 4 commits', 'assets/avatar-water-36.png'],
  ['jrusso1020', ' approved · 3 threads open', 'assets/avatar-james-36.png'],
  ['miguel-heygen', ' resolved 5 conversations', 'assets/avatar-miguel-36.png'],
  ['WaterrrForever', ' re-requested review', 'assets/avatar-water-36.png'],
];

function ReviewRow({ i }) {
  const [name, rest, avatar] = REVIEWS[i];
  return (
    <div id={'prs-r' + (i + 1)} class="flex items-center gap-[17.3px] py-[20.2px] border-b-[1px] border-[#f0f0f0] opacity-0">
      <image path={avatar} class="w-[36.5px] h-[36.5px] rounded-full object-cover flex-none" />
      <div class="flex items-center gap-[8px]">
        <p class="text-[20.2px] font-[hk6] font-semibold text-[#1f2328]">{name}</p>
        <p class="text-[20.2px] text-[#3d3a34]">{rest}</p>
      </div>
    </div>
  );
}

// feature-code source lines: [line-no, [color, text] segments]
const FC_A = [
  ['5', [['#D97757', 'function'], ['#E0C9A6', ' ghTry'], ['#cfcabd', '(args) {']]],
  ['6', [['#cfcabd', '  '], ['#D97757', 'try'], ['#cfcabd', ' {']]],
  ['7', [['#cfcabd', '    '], ['#D97757', 'const'], ['#cfcabd', ' out = '], ['#E0C9A6', 'execFileSync'], ['#cfcabd', '('], ['#5DB8A6', '"gh"'], ['#cfcabd', ', args, { encoding: '], ['#5DB8A6', '"utf8"'], ['#cfcabd', ' });']]],
  ['8', [['#cfcabd', '    '], ['#D97757', 'return'], ['#cfcabd', ' { ok: '], ['#D97757', 'true'], ['#cfcabd', ', stdout: out };']]],
  ['9', [['#cfcabd', '  } '], ['#D97757', 'catch'], ['#cfcabd', ' (e) { '], ['#D97757', 'return'], ['#cfcabd', ' { ok: '], ['#D97757', 'false'], ['#cfcabd', ', stderr: e.stderr }; }']]],
  ['10', [['#cfcabd', '}']]],
  ['11', []],
  ['12', [['#D97757', 'const'], ['#cfcabd', ' view = '], ['#E0C9A6', 'ghTry'], ['#cfcabd', '(['], ['#5DB8A6', '"pr"'], ['#cfcabd', ', '], ['#5DB8A6', '"view"'], ['#cfcabd', ', prRef, '], ['#5DB8A6', '"--json"'], ['#cfcabd', ', FIELDS]);']]],
  ['13', []],
];
const FC_KEY = [
  ['14', [['#6F6E66', '  // complete the files list — the truncation fix']]],
  ['15', [['#D97757', 'const'], ['#cfcabd', ' files = '], ['#E0C9A6', 'ghTry'], ['#cfcabd', '(['], ['#5DB8A6', '"api"'], ['#cfcabd', ', '], ['#5DB8A6', '"--paginate"'], ['#cfcabd', ',']]],
  ['16', [['#cfcabd', '  '], ['#5DB8A6', '`repos/${owner}/${repo}/pulls/${number}/files`'], ['#cfcabd', ']);']]],
];
const FC_B = [
  ['17', []],
  ['18', [['#D97757', 'if'], ['#cfcabd', ' (files.ok) pr.files = '], ['#E0C9A6', 'parseNDJSON'], ['#cfcabd', '(files.stdout);']]],
];

function CodeLine({ seg }) {
  const [no, segs] = seg;
  return (
    <div class="flex whitespace-nowrap">
      <p class="w-[54px] text-right text-[#5e5d56] flex-none mr-[34px]">{no}</p>
      {segs.map((s, j) => <p key={j} class={'text-[' + s[0] + ']'}>{s[1]}</p>)}
    </div>
  );
}

// feature-flow strip cards: [label, thumb color]
const FF_CARDS = [['1', '#ECD6CB'], ['2', '#1D1F1F'], ['3', '#D2E4D6'], ['4', '#EEE0C6'], ['5', '#DBD1E8']];

function FfCard({ n, thumb }) {
  return (
    <div id={'ff-fc' + n} class="relative w-[160px] bg-[#FFFFFF] border-[1px] border-[rgba(20,20,19,0.10)] rounded-[13px] shadow-[0_10px_26px_-12px_rgba(20,20,19,0.28)] flex flex-col gap-[12px] p-[12px] opacity-0">
      <div class={'relative w-full h-[62px] rounded-[8px] bg-[' + thumb + '] inset-shadow-[0_0_0_1px_rgba(20,20,19,0.06)]'}>
        <div class="absolute left-[11px] right-[11px] top-[11px] h-[7px] rounded-[3px] bg-[rgba(217,119,87,0.85)]"></div>
        {/* ref .fc2 .fct::after — the content line on the dark thumb is white */}
        {thumb === '#1D1F1F'
          ? <div class="absolute left-[11px] right-[30px] bottom-[12px] h-[6px] rounded-[3px] bg-[rgba(255,255,255,0.26)]"></div>
          : <div class="absolute left-[11px] right-[30px] bottom-[12px] h-[6px] rounded-[3px] bg-[rgba(20,20,19,0.14)]"></div>}
      </div>
      <div class="relative w-[82%] h-[7px] rounded-[4px] bg-[rgba(20,20,19,0.17)] mb-[8px]">
        <div class="absolute left-0 top-[12px] h-[6px] w-[54%] rounded-[4px] bg-[rgba(20,20,19,0.10)]"></div>
      </div>
      <div class="absolute left-[8px] top-[8px] z-[2]"><p class="text-[14px] leading-[1.55] font-[mono5] text-[#3a392f] bg-[rgba(250,249,245,0.94)] rounded-[6px] px-[6px] shadow-[0_1px_3px_rgba(20,20,19,0.2)]">{'0' + n}</p></div>
    </div>
  );
}

// shared player-window chrome for the four feature cards
function PlayerWindow({ pwinId, sceneLabel, fillId, knobId, ptimeId, time0, stageBg, children }) {
  return (
    <div id={pwinId} class="relative w-[1228.8px] mt-[64.8px] bg-[#1D1F1F] rounded-[23.04px] overflow-hidden shadow-[0_46px_134px_-38px_rgba(20,20,18,0.5)]">
      {/* FIX: pbar is exactly 64px in ref (bands 172→236 on frame 745); engine
          text line-height 1.5 made it 71px and pushed the whole screen 3px
          down — pin the height instead of py so all four feature screens sit
          at the ref offset (pwin top then lands at 171.8 ≈ ref 172) */}
      <div class="flex items-center gap-[11.9px] bg-[#1D1F1F] px-[28.8px] h-[64px]">
        <div class="w-[19.2px] h-[19.2px] rounded-full bg-[#FF5F57]"></div>
        <div class="w-[19.2px] h-[19.2px] rounded-full bg-[#FEBC2E]"></div>
        <div class="w-[19.2px] h-[19.2px] rounded-full bg-[#28C840]"></div>
        <p class="ml-[17.3px] text-[20.2px] leading-none font-[mono4] text-[#cfcabd]">hyperframes-pr-1635.mp4</p>
        <p class="ml-auto text-[18.24px] leading-none font-[mono4] text-[#8C8B83]">{sceneLabel}</p>
      </div>
      <div class={'relative w-[1171.2px] h-[658.56px] mx-[28.8px] rounded-[7.68px] overflow-hidden bg-[' + stageBg + ']'}>{children}</div>
      <div class="flex items-center gap-[23px] px-[28.8px] pt-[23px] pb-[26.9px]">
        <path d="M8 5l11 7-11 7z" view-box="0 0 24 24" class="w-[28.8px] h-[28.8px] fill-[#D97757] flex-none" />
        <div class="relative flex-1 h-[6.53px] bg-[rgba(255,255,255,0.12)] rounded-full">
          <div id={fillId} class="absolute left-0 top-0 h-full w-full bg-[#D97757] rounded-full origin-left"></div>
          <div id={knobId} class="absolute top-[3.265px] w-[19.2px] h-[19.2px] -mt-[9.6px] -ml-[9.6px] rounded-full bg-[#F0EFE9] shadow-[0_0_0_3.84px_rgba(217,119,87,0.35)]"></div>
        </div>
        <p id={ptimeId} class="text-[19.2px] font-[mono4] text-[#9b988f] flex-none text-right min-w-[172.8px]">{time0}</p>
      </div>
    </div>
  );
}

function CardTitle({ titleId, caretId }) {
  return (
    <div class="absolute left-0 right-0 top-[54px] flex items-center justify-center gap-[8.64px] h-[105.6px] z-[5]">
      <p id={titleId} class="font-[black] text-[88.32px] tracking-[0.384px] text-[#1D1F1F] leading-none whitespace-nowrap"></p>
      <div id={caretId} class="w-[6.53px] h-[74.88px] bg-[#D97757] rounded-[1.34px]"></div>
    </div>
  );
}

// ═════════════════ SCENE 1 — opener (0–5.2 s) ═════════════════
function Opener({ rootId }) {
  useGSAP(() => {
    const T = gsap.timeline();
    // ref opener.html:108 dM1 = m1.offsetHeight(98) + GAP(30.72)
    T.set('op-stack', { y: 128.72 }, 0);
    T.set('op-sh', { left: 0, top: 0, width: 1920, height: 1080, borderRadius: 0, opacity: 0 }, 0);
    T.set('op-win', { left: 0, top: 0, width: 1920, height: 1080, borderRadius: 0 }, 0);
    T.set('op-composer', { left: 540, width: 840, bottom: 376.5, scale: 1.85, transformOrigin: '50% 100%' }, 0);
    T.to('op-sh', { left: 160, top: 102.6, width: 1599.36, height: 874.8, borderRadius: 25.92, opacity: 1, duration: 0.55, ease: 'power2.inOut' }, 2.55);
    T.to('op-win', { left: 160, top: 102.6, width: 1599.36, height: 874.8, borderRadius: 25.92, duration: 0.55, ease: 'power2.inOut' }, 2.55);
    // ref opener.html:134-138 composes the morph through one eased p:
    // s=1.85-0.85p, vw=1554-85.2p, w=vw/s, bottom=376.5-304.72p — w=vw/s is
    // nonlinear in p, so tween the proxy, not the layout props independently.
    const composer = ctx.getNode('op-composer');
    const morph = { set: (v) => {
      const p = v.opacity, s = 1.85 - 0.85 * p, vw = 1554 - 85.2 * p, w = vw / s, W = 1920 - 320.64 * p;
      composer.left(W / 2 - w / 2); composer.width(w); composer.bottom(376.5 - 304.72 * p); composer.scale(s);
    } };
    T.fromTo(morph, { opacity: 0 }, { opacity: 1, duration: 0.55, ease: 'power2.inOut' }, 2.55);
    T.fromTo('op-input', { x: 280, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power3.out' }, 0.02);
    T.fromTo('op-chips', { x: 340, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power3.out' }, 0.1);
    T.fromTo('op-meta', { x: 340, opacity: 0 }, { x: 0, opacity: 1, duration: 0.55, ease: 'power3.out' }, 0.16);
    T.to('op-orb', { rotation: 360, duration: 1.0, ease: 'none', repeat: 4 }, 0);
    T.to('op-caret1', { opacity: 0, duration: 0.4, ease: 'steps(1)', repeat: 1, yoyo: true }, 0.1);
    T.set('op-caret1', { opacity: 1 }, 0.6);
    // FIX: two manual rows (ref wraps after "can") + blue PR url at 2.1s
    const t1 = ctx.getNode('op-typed1'), t2 = ctx.getNode('op-typed2');
    const typing = { set: (v) => {
      const n = Math.round(v.opacity);
      t1.text(n <= CUT ? MSG.slice(0, n) : MSG.slice(0, CUT));
      t2.text(n > CUT ? MSG.slice(CUT) : '');
    } };
    T.fromTo(typing, { opacity: 0 }, { opacity: MSG.length, duration: TYPE_DUR, ease: 'none' }, TYPE_T0);
    T.to('op-caret1', { opacity: 0, duration: 0.05 }, ROW2_AT);
    T.set({ set: () => ctx.getNode('op-caret2').text('▏') }, { opacity: 1 }, ROW2_AT);
    T.set({ set: () => ctx.getNode('op-url').text(PR_URL) }, { opacity: 1 }, 2.1);
    // ref2.2: the input grows 92.24→100 (design px) when row 2 appears, top edge
    // rises while the bottom stays fixed (items-center re-centres the 2-row group)
    T.fromTo('op-row2', { height: 0 }, { height: 37.9, duration: 0.01 }, ROW2_AT);
    T.fromTo('op-input', { height: 92.24 }, { height: 100, duration: 0.05 }, ROW2_AT);
    // ref f0083 white cursor bbox (1681,524)-(1742,607): element top-left =
    // bbox.tl − (6,4)·(105.6/28) + stroke/2 → (1660.4,510.9); enters from +300 below
    T.fromTo('op-cursor', { x: 1660.4, y: 810.9, opacity: 0 }, { y: 510.9, opacity: 1, duration: 0.9, ease: 'power4.out' }, 1.3);
    T.to('op-cursor', { scale: 0.84, duration: 0.1, ease: 'power2.in', transformOrigin: '21% 14%' }, 2.3);
    T.to('op-cursor', { scale: 1, duration: 0.18, ease: 'power2.out', transformOrigin: '21% 14%' }, 2.4);
    // Enter: clear the composer row (registered after `typing`, so the clamp
    // of that tween is overwritten every frame from here on)
    T.set({ set: () => {
      t1.text(''); t2.text('');
      ctx.getNode('op-url').text('');
      ctx.getNode('op-caret1').opacity(0); ctx.getNode('op-caret2').text('');
    } }, { opacity: 1 }, 2.5);
    // ref f0083: after Enter the input collapses back to its single-row height
    // (137 screen px mid-morph) — registered after the ROW2_AT fromTo clamps
    T.set('op-row2', { height: 0 }, 2.5);
    T.set('op-input', { height: 87.8 }, 2.5);
    T.fromTo('op-bar', { opacity: 0, x: -14 }, { opacity: 1, x: 0, duration: 0.3, ease: 'power2.out' }, 2.85);
    // FIX: engine snapshots .to() layout from-values as 0 — collapse needs explicit
    // fromTo; the class carries the same natural height so pre-window frames match.
    T.fromTo('op-chips', { opacity: 1, height: 40.5 }, { opacity: 0, height: 0, duration: 0.3, ease: 'power2.in' }, 2.6);
    T.fromTo('op-chipgap', { height: 18.9 }, { height: 0, duration: 0.3, ease: 'power2.in' }, 2.6);
    T.fromTo('op-m1', { opacity: 0 }, { opacity: 1, duration: 0.3, ease: 'power2.out' }, 3.2);
    T.to('op-stack', { y: 0, duration: 0.45, ease: 'power3.out' }, 3.2);
    // ref4.5 frame: white cursor bbox (1322,637)-(1382,720) → top-left (1301.4,623.9)
    T.to('op-cursor', { x: 1301.4, y: 623.9, duration: 0.6, ease: 'power3.inOut' }, 3.5);
    T.to('op-cursor', { scale: 0.84, duration: 0.1, ease: 'power2.in' }, 4.2);
    T.to('op-cursor', { scale: 1, duration: 0.18, ease: 'power2.out' }, 4.3);
    T.fromTo('op-prlink', { color: '#D97757' }, { color: '#F2A98C', duration: 0.1 }, 4.22);
    T.to('op-prlink', { color: '#D97757', duration: 0.3 }, 4.42);
    T.set('op-ripple', { left: 1322 - 48, top: 637 - 48, scale: 0, opacity: 0 }, 4.22);
    T.to('op-ripple', { scale: 1.1, opacity: 0.5, duration: 0.2, ease: 'power2.out' }, 4.22);
    T.to('op-ripple', { scale: 1.9, opacity: 0, duration: 0.32, ease: 'power2.out' }, 4.42);
  });
  return (
    <div id={rootId} class="absolute inset-0 z-[1] font-[hk4]">
      {/* ref opener.html:92 starts the win shadow at alpha 0 and fades it in
          through the morph (:149). The engine has no boxShadow tween, so the
          shadow lives on a sibling layer (`op-sh`, painted below the win)
          whose opacity stands in for the alpha ramp. It must NOT wrap the win
          — hiding it would hide the whole scene. */}
      <div id="op-sh" class="absolute left-[160px] top-[102.6px] w-[1599.36px] h-[874.8px] rounded-[25.92px] shadow-[0_38px_115px_-38px_rgba(20,20,18,0.45)] opacity-0"></div>
      <div id="op-win" class="absolute left-[160px] top-[102.6px] w-[1599.36px] h-[874.8px] rounded-[25.92px] bg-[#1D1F1F] overflow-hidden">
        <div id="op-bar" class="absolute left-[30.7px] top-[26.9px] flex gap-[11.9px] opacity-0">
          <div class="w-[20.2px] h-[20.2px] rounded-full bg-[#FF5F57]"></div>
          <div class="w-[20.2px] h-[20.2px] rounded-full bg-[#FEBC2E]"></div>
          <div class="w-[20.2px] h-[20.2px] rounded-full bg-[#28C840]"></div>
        </div>
        {/* ref opener.html:47 .feed{left/right:3.4cqw, top:6cqh, bottom:16.5cqw} */}
        <div id="op-feed" class="absolute left-[65.3px] right-[65.3px] top-[115.2px] bottom-[316.8px] overflow-hidden">
          <div id="op-stack" class="absolute left-0 right-0 bottom-0 flex flex-col gap-[30.72px]">
            <div id="op-m1" class="self-end flex flex-col items-end text-right opacity-0">
              <div class="flex items-baseline">
                <p id="op-m1-a" class="text-[30.72px] text-[#F0EEE9] leading-[1.4]">{"Someone on our team just opened a PR for a new "}</p>
                <p class="text-[30.72px] font-[hk6] font-semibold text-[#F0EEE9] leading-[1.4]">pr-to-video</p>
                <p class="text-[30.72px] text-[#F0EEE9] leading-[1.4]">{" skill — can"}</p>
              </div>
              <div class="flex items-baseline mt-[16.6px]">
                <p id="op-m1-b" class="text-[30.72px] text-[#F0EEE9] leading-[1.4]">you review it?</p>
                <p id="op-prlink" class="text-[30.72px] font-[hk6] font-semibold text-[#D97757] leading-[1.4] border-b-[2.2px] border-[#D97757]">github.com/heygen-com/hyperframes/pull/1635</p>
              </div>
            </div>
          </div>
        </div>
        <div id="op-composer" class="absolute left-[65.3px] w-[1468.76px] bottom-[65.3px]">
          <div id="op-chips" class="flex items-end gap-[11.52px] h-[40.5px]">
            <div class="h-[40.5px] flex items-center gap-[9.6px] bg-[#2C2C2B] rounded-[10.56px] px-[15.7px]">
              <path d="M4 5h16v11H4z M2 19h20" view-box="0 0 24 24" class="w-[21.12px] h-[21.12px] stroke-[1.7] stroke-[#C9C6BC] fill-none" />
              <p class="text-[19.2px] font-[hk5] font-medium text-[#D7D4CA]">Local</p>
            </div>
            <div class="h-[40.5px] flex items-center gap-[9.6px] bg-[#2C2C2B] rounded-[10.56px] px-[15.7px]">
              <path d="M3 7.5A2 2 0 0 1 5 5.5h4.2l2 2.2H19a2 2 0 0 1 2 2V17a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" view-box="0 0 24 24" class="w-[21.12px] h-[21.12px] stroke-[1.7] stroke-[#C9C6BC] fill-none" />
              <p class="text-[19.2px] font-[hk5] font-medium text-[#D7D4CA]">hyperframes</p>
            </div>
          </div>
          <div id="op-chipgap" class="h-[18.9px]"></div>
          {/* ref1.5/ref2.2: input is 92.24 tall with one row, grows to 100 with two
              (script tweens height + #op-row2 at ROW2_AT); rows sit items-center */}
          <div id="op-input" class="relative flex items-center bg-[#2C2C2B] border-[1px] border-[#3a3a38] rounded-[21.6px] h-[92.24px] px-[23px]">
            {/* f69 subpixel fit: typed rows sit dy=+2 (engine baseline higher
                than Chrome in this 1.714 line box) */}
            <div class="flex-1 flex flex-col justify-center pr-[50px] relative top-[2px]">
              <div class="flex items-center">
                <p id="op-typed1" class="text-[22.08px] leading-[1.714] text-[#E6E2D6] whitespace-nowrap"></p>
                <p id="op-caret1" class="text-[27.84px] leading-[1.36] text-[#D97757]">▏</p>
              </div>
              <div id="op-row2" class="flex items-center">
                <p id="op-typed2" class="text-[22.08px] leading-[1.714] text-[#E6E2D6] whitespace-nowrap"></p>
                <p id="op-url" class="text-[22.08px] leading-[1.714] text-[#85B6E8] whitespace-nowrap"></p>
                <p id="op-caret2" class="text-[27.84px] leading-[1.36] text-[#D97757]"></p>
              </div>
            </div>
            <path d="M9 10 4 15 9 20 M20 4v7a4 4 0 0 1-4 4H4" view-box="0 0 24 24" class="absolute right-[21px] w-[15px] h-[15px] stroke-[2.6] stroke-[#6F6E66] fill-none" />
            {/* ref .pet{right:3cqw;bottom:100%} — no bottom margin (opener.html:53) */}
            <div class="absolute right-[57.6px] bottom-full w-[145.92px] h-[84.48px]">
              {/* pet2 (opener.html:24) only lives past 5.27s; the scene fades at 5.22s */}
              <video id="op-pet1" path="assets/pet-typing-2x.mp4" duration={5.27} class="absolute inset-0 w-full h-full object-contain" />
            </div>
          </div>
          <div id="op-meta" class="flex justify-between items-center mt-[16.8px]">
            <div class="flex items-center gap-[19.2px]">
              <div class="bg-[rgba(202,164,62,0.16)] rounded-[8px] w-[62px] h-[29.7px] flex items-center justify-center"><p class="text-[17.3px] font-[hk5] font-medium text-[#CAA43E]">Auto</p></div>
              <p class="text-[21px] text-[#8C8B83]">＋</p>
            </div>
            <div class="flex items-center gap-[19.2px]">
              <p class="text-[19.2px] text-[#7C7B73] font-[hk5] font-medium">Opus 4.8</p>
              <p class="text-[19.2px] text-[#9A998F] font-[hk5] font-medium">High</p>
              <div id="op-orb" class="w-[24px] h-[24px] rounded-full border-[3.26px] border-[rgba(96,148,214,0.26)]"></div>
            </div>
          </div>
        </div>
      </div>
      <path id="op-cursor" d="M6 4 L6 23 L11 18 L14.5 25 L17.5 23.6 L14 17 L21 17 Z" view-box="0 0 28 28" class="absolute w-[105.6px] h-[105.6px] z-[30] opacity-0 fill-[#1c1c1c] stroke-[1.4] stroke-[#ffffff]" />
      <div id="op-ripple" class="absolute w-[96px] h-[96px] rounded-full border-[5.4px] border-[rgba(217,119,87,0.85)] z-[29] opacity-0"></div>
    </div>
  );
}

// ═════════════════ SCENE 2 — pr-scroll (5.2–8.4 s) ═════════════════
function PrScroll({ rootId }) {
  useGSAP(() => {
    const T = gsap.timeline();
    T.fromTo('prs-browser', { scale: 1.04, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.4, ease: 'power3.out' }, O2);
    T.set('prs-inner', { filter: 'blur(0px)' }, O2);
    reveal(T, 'prs-head', O2 + 0.5);
    reveal(T, 'prs-head2', O2 + 0.5);
    // ref tabOn(i,t): .tab.on swaps the run to HK600 #1f2328; underline tracks
    for (const [k, t0] of [[0, 0.25], [1, 0.95], [2, 1.15]]) {
      T.set('prs-tab' + k + 'on', { opacity: 1 }, O2 + t0);
      T.set('prs-tab' + k + 'off', { opacity: 0 }, O2 + t0);
      T.set('prs-tabline' + k, { opacity: 1 }, O2 + t0);
      if (k > 0) {
        T.set('prs-tab' + (k - 1) + 'on', { opacity: 0 }, O2 + t0);
        T.set('prs-tab' + (k - 1) + 'off', { opacity: 1 }, O2 + t0);
        T.set('prs-tabline' + (k - 1), { opacity: 0 }, O2 + t0);
      }
    }
    for (let i = 0; i < 6; i++) {
      T.fromTo('prs-r' + (i + 1), { opacity: 0, x: 16 }, { opacity: 1, x: 0, duration: 0.18, ease: 'power2.out' }, O2 + 0.3 + i * 0.12);
    }
    T.to('prs-review', { opacity: 0, duration: 0.2 }, O2 + 1.1);
    T.to('prs-files', { opacity: 1, duration: 0.2 }, O2 + 1.15);
    T.to('prs-inner', { y: -3600, duration: 1.7, ease: 'power2.in' }, O2 + 1.2);
    T.fromTo('prs-inner', { filter: 'blur(0px)' }, { filter: 'blur(2px)', duration: 1.1, ease: 'power2.in' }, O2 + 1.5);
    reveal(T, 'prs-s1', O2 + 1.2);
    T.set('prs-cr1', { opacity: 1 }, O2 + 1.35);
    typeSteps(T, ctx, 'prs-files-n', '629', O2 + 1.35, 0.45);
    reveal(T, 'prs-s2', O2 + 1.75);
    T.set('prs-cr2', { opacity: 1 }, O2 + 1.9);
    typeSteps(T, ctx, 'prs-lines-n', '87,291', O2 + 1.9, 0.7);
    reveal(T, 'prs-s3', O2 + 2.35);
    T.set('prs-cr3', { opacity: 1 }, O2 + 2.5);
    typeSteps(T, ctx, 'prs-commits-n', '4', O2 + 2.5, 0.2);
    T.to('prs-cr1', { opacity: 0, duration: 0.25, ease: 'steps(1)', repeat: 1, yoyo: true }, O2 + 1.35 + 0.45);
    T.to('prs-cr2', { opacity: 0, duration: 0.25, ease: 'steps(1)', repeat: 1, yoyo: true }, O2 + 1.9 + 0.7);
    T.to('prs-cr3', { opacity: 0, duration: 0.25, ease: 'steps(1)', repeat: 1, yoyo: true }, O2 + 2.5 + 0.2);
  });
  return (
    <div id={rootId} class="absolute inset-0 z-[2] font-[hk4] opacity-0">
      <div id="prs-browser" class="absolute left-[49.9px] top-[86.4px] w-[1017.6px] h-[907.2px] bg-[#fff] rounded-[23.04px] overflow-hidden shadow-[0_35px_96px_-38px_rgba(20,20,18,0.4)]">
        <div class="flex items-center gap-[10.56px] h-[61.44px] px-[26.88px] bg-[#e9e6df] border-b-[1px] border-[#d8d2c6]">
          <div class="w-[18.24px] h-[18.24px] rounded-full bg-[#FF5F57]"></div>
          <div class="w-[18.24px] h-[18.24px] rounded-full bg-[#FEBC2E]"></div>
          <div class="w-[18.24px] h-[18.24px] rounded-full bg-[#28C840]"></div>
          <div class="ml-[19.2px] flex-1 flex items-center gap-[8.64px] bg-[#fff] border-[1px] border-[#d8d2c6] rounded-[9.6px] h-[37.44px] px-[24px]">
            <path d="M5 11h14v9H5z M8 11V8a4 4 0 0 1 8 0v3" view-box="0 0 24 24" class="w-[17.3px] h-[17.3px] stroke-[2.2] stroke-[#6c6a64] fill-none" />
            <p class="text-[17.66px] text-[#3d3a34] font-[mono4]">github.com/heygen-com/hyperframes/pull/1635</p>
          </div>
        </div>
        {/* ref pr-scroll.html:77-79 .tab: label HK400 #59636e + <b> HK700
            #1f2328; .tab.on swaps the whole run to HK600 #1f2328. Two stacked
            layers per tab, timeline flips opacity at the tabOn times.
            Underline y measured f246: ref 198-199, old test 212-213 → -14px. */}
        <div class="relative flex items-center gap-[30.72px] px-[30.72px] pt-[19.2px] pb-[19.2px] border-b-[1px] border-[#d0d7de]">
          {[
            ['0', 'Conversation ', '38'],
            ['1', 'Commits ', '4'],
            ['2', 'Files changed ', '629'],
          ].map(([k, label, num]) => (
            <div key={k} class="relative flex items-baseline">
              <div id={'prs-tab' + k + 'off'} class="flex items-baseline">
                <p class="text-[19.2px] text-[#59636e] font-[hk4]">{label}</p>
                <p class="text-[19.2px] text-[#1f2328] font-[hk4] font-bold">{num}</p>
              </div>
              <div id={'prs-tab' + k + 'on'} class="absolute left-0 top-0 flex items-baseline opacity-0">
                <p class="text-[19.2px] text-[#1f2328] font-[hk6] font-semibold">{label}</p>
                <p class="text-[19.2px] text-[#1f2328] font-[hk6] font-semibold">{num}</p>
              </div>
              <div id={'prs-tabline' + k} class="absolute left-0 right-0 bottom-[-5.2px] h-[2px] bg-[#fd8c73] opacity-0"></div>
            </div>
          ))}
        </div>
        <div id="prs-wbody" class="absolute left-0 right-0 top-[142.1px] bottom-0 overflow-hidden">
          <div id="prs-review" class="absolute inset-0 px-[30.72px] py-[11.52px]">
            {REVIEWS.map((r, i) => <ReviewRow key={i} i={i} />)}
          </div>
          <div id="prs-files" class="absolute inset-0 opacity-0">
            <div id="prs-inner" class="absolute left-0 right-0 top-0">
              {ROW_BASES.map((base) => FILES.map((f, i) => <FileRow key={base + i} base={base} i={i} />))}
            </div>
          </div>
        </div>
      </div>
      <div class="absolute left-[1152px] top-[540px] -mt-[183.6px] w-[691.2px]">
        <p id="prs-head" class="whitespace-nowrap font-[gal] text-[72.96px] leading-[75.9px] tracking-[-1.15px] text-[#262624] relative -top-[2px] opacity-0">A thousand</p>
        <p id="prs-head2" class="whitespace-nowrap font-[gal] text-[72.96px] leading-[75.9px] tracking-[-1.15px] text-[#262624] mb-[34.56px] relative -top-[2px] opacity-0">slices of change.</p>
        <div id="prs-s1" class="flex items-baseline gap-[11.52px] mb-[17.28px] border-t-[1px] border-[#ddd6c8] pt-[17.28px] opacity-0">
          <p id="prs-files-n" class="font-[mono5] text-[44.16px] text-[#D97757] leading-none relative -top-[3px]"></p>
          <div id="prs-cr1" class="w-[44.7px] h-[48px] mr-[-1.28px] mb-[0.9px] flex-none opacity-0"><div class="w-[4.5px] h-[48px] bg-[#D97757] relative top-[2px]"></div></div>
          <p class="text-[22.08px] font-[hk5] font-medium text-[#6c6a64]">files changed</p>
        </div>
        <div id="prs-s2" class="flex items-baseline gap-[11.52px] mb-[17.28px] border-t-[1px] border-[#ddd6c8] pt-[17.28px] opacity-0">
          <p id="prs-lines-n" class="font-[mono5] text-[44.16px] text-[#D97757] leading-none relative -top-[3px]"></p>
          <div id="prs-cr2" class="w-[44.7px] h-[48px] mr-[-1.28px] mb-[0.9px] flex-none opacity-0"><div class="w-[4.5px] h-[48px] bg-[#D97757] relative top-[2px]"></div></div>
          <p class="text-[22.08px] font-[hk5] font-medium text-[#6c6a64]">lines removed</p>
        </div>
        <div id="prs-s3" class="flex items-baseline gap-[11.52px] mb-[17.28px] border-t-[1px] border-[#ddd6c8] pt-[17.28px] opacity-0">
          <p id="prs-commits-n" class="font-[mono5] text-[44.16px] text-[#D97757] leading-none relative -top-[3px]"></p>
          <div id="prs-cr3" class="w-[44.7px] h-[48px] mr-[-1.28px] mb-[0.9px] flex-none opacity-0"><div class="w-[4.5px] h-[48px] bg-[#D97757] relative top-[2px]"></div></div>
          <p class="text-[22.08px] font-[hk5] font-medium text-[#6c6a64]">commits to read through</p>
        </div>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 3 — hard-read (8.4–9.3 s) ═════════════════
function HardRead({ rootId }) {
  useGSAP(() => {
    const T = gsap.timeline();
    T.fromTo('hr-txt', { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.28, ease: 'power3.out' }, O3);
  });
  return (
    <div id={rootId} class="absolute inset-0 z-[3] bg-[#1D1F1F] font-[gal] opacity-0">
      <div id="hr-txt" class="absolute left-0 right-0 top-[540px] -mt-[90px] text-center opacity-0">
        <p class="text-[182.4px] tracking-[-3.456px] text-[#EFEADD] leading-none">Hard to read</p>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 4 — summarize (9.3–12.3 s) ═════════════════
function Summarize({ rootId }) {
  useGSAP(() => {
    const T = gsap.timeline();
    T.set('sm-composer', { scale: 1.85, transformOrigin: '50% 100%' }, 0);
    T.to('sm-orb', { rotation: 360, duration: 1, ease: 'none', repeat: 2 }, O4);
    tw(T, ctx, 'sm-typed', 'summarize this PR', O4, 0.8);
    T.set('sm-caret', { opacity: 0 }, O4 + 0.82);
    T.to('sm-composer', { y: -44, opacity: 0, duration: 0.32, ease: 'power2.in' }, O4 + 0.85);
    T.to('sm-feed', { opacity: 1, duration: 0.25, ease: 'power1.out' }, O4 + 0.92);
    // dist = inner.offsetHeight - feed.offsetHeight + 30 (Chrome measures 590
    // here: cross-correlation dy grows ~p² from 0 at scroll start to 4px at
    // the tail, f306 dy≈0 / f332 dy≈3 / f351 dy≈4 → end value -590).
    T.fromTo('sm-inner', { y: 0 }, { y: -590, duration: 1.25, ease: 'power1.in' }, O4 + 1.0);
    T.to('sm-veil', { opacity: 1, duration: 0.3, ease: 'power2.in' }, O4 + 2.2);
    T.fromTo('sm-still', { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.32, ease: 'power3.out' }, O4 + 2.32);
  });
  const h2 = 'text-[26.88px] font-[hk6] font-semibold text-[#E8A55A] font-[mono5] leading-[1.7] mt-[23.04px] mb-[11.52px]';
  const al = 'text-[29.76px] leading-[1.7] text-[#cfcabd] mb-[11.5px]';
  return (
    <div id={rootId} class="absolute inset-0 z-[4] font-[hk4] opacity-0">
      <div class="absolute inset-0 bg-[#1D1F1F] overflow-hidden">
        <div id="sm-feed" class="absolute left-[115.2px] right-[153.6px] top-[115.2px] bottom-[115.2px] overflow-hidden opacity-0">
          {/* per-band subpixel fit on f312: every text band reads dy=-1.5 (the
              engine places the baseline ~1.5px lower than Chrome inside the
              1.7 line box for this font) — raise the whole scroll */}
          <div id="sm-inner" class="absolute left-0 right-0 -top-[1px]">
            <div class="relative -left-[4px] flex items-center gap-[13.44px] text-[31.68px] font-[hk5] font-medium text-[#D8D4C8] leading-[1.7] mb-[21.12px]">
              {/* ref ✳ shapes as Chrome's monochrome text glyph (Segoe UI Symbol
                  8-spoke); the engine's fallback pool picks Noto Color Emoji —
                  green, thicker — so draw the star as an 8-spoke path. */}
              <path d="M16 6.5 L16 25.5 M6.5 16 L25.5 16 M9.3 9.3 L22.7 22.7 M22.7 9.3 L9.3 22.7" view-box="0 0 32 32" class="w-[31.68px] h-[31.68px] flex-none stroke-[#D97757] fill-none stroke-[2.2] relative -top-[0.5px]" />
              <p class="text-[31.68px] text-[#D8D4C8]">Here's the summary:</p>
            </div>
            <p class={h2}>## TL;DR</p>
            <p class={al}>Three video skills now share one script-driven engine — less duplication, one place to fix bugs.</p>
            <p class={h2}>## What moved</p>
            <p class={al}>- product-launch, pr-to-video, faceless-explainer → one fetch / build-frame / audio / caption / assemble pipeline.</p>
            <p class={al}>- each SKILL.md slimmed; the inline step logic now lives in shared scripts.</p>
            <p class={al}>- story / tts / blueprints references trimmed where the engine owns the flow.</p>
            <p class={h2}>## By the numbers</p>
            <p class={al}>- 629 files touched, +8,322 / −87,291.</p>
            <p class={al}>- the −87,291 is mostly moved, not deleted; net behaviour unchanged.</p>
            <p class={al}>- 4 commits, each a coherent slice.</p>
            <p class={h2}>## New surface</p>
            <p class={al}>- a claude frame preset (FRAME.md + caption-skin.html).</p>
            <p class={al}>- code-diff / code-typing / code-morph registry blocks for code beats.</p>
            <p class={al}>- a shared audio engine: TTS + BGM lookup + SFX + duration sync.</p>
            <p class={h2}>## Behaviour changes</p>
            <p class={al}>- pr-to-video ingests via gh → a synthetic capture package; no website capture.</p>
            <p class={al}>- product-launch keeps capture but delegates audio + captions to the engine.</p>
            <p class={al}>- faceless-explainer drops its bespoke audio path entirely.</p>
            <p class={h2}>## Risks to check</p>
            <p class={al}>- the shared audio engine is a single point of failure for all three skills.</p>
            <p class={al}>- verify no skill lost a reference in the move.</p>
          </div>
        </div>
        {/* ref summarize.html:55 bottom:370 — but the composed frame puts the
            ring at 434..611 while the engine lands it at 426..603 (f311/f300
            scans): the scaled composer sits ~6px high, so trim bottom to 364
            (scale 1.85, origin 50% 100% pins the visual bottom to the box
            bottom; +6px down onto the ref position). */}
        <div id="sm-composer" class="absolute left-[540px] w-[840px] bottom-[364px]">
          <div class="flex gap-[11.52px] mb-[17.28px]">
            <div class="bg-[#2C2C2B] rounded-[10.56px] px-[15.7px] py-[8.8px]"><p class="text-[18.3px] font-[hk5] font-medium text-[#D7D4CA]">Local</p></div>
            <div class="bg-[#2C2C2B] rounded-[10.56px] px-[15.7px] py-[8.8px]"><p class="text-[18.3px] font-[hk5] font-medium text-[#D7D4CA]">hyperframes</p></div>
          </div>
          <div class="relative flex items-center bg-[#2C2C2B] border-[1px] border-[#3a3a38] rounded-[17.28px] min-h-[92.24px] px-[23px] shadow-[0_0_0_2.3px_rgba(217,119,87,0.5)]">
            <div class="flex-1 flex items-center pr-[50px]">
              <p id="sm-typed" class="text-[23.04px] font-[mono4] text-[#E6E2D6] whitespace-nowrap"></p>
              <p id="sm-caret" class="text-[27.84px] text-[#D97757]">▏</p>
            </div>
            <path d="M9 10 4 15 9 20 M20 4v7a4 4 0 0 1-4 4H4" view-box="0 0 24 24" class="absolute right-[21px] w-[15px] h-[15px] stroke-[2.6] stroke-[#6F6E66] fill-none" />
          </div>
          <div class="flex justify-between items-center mt-[16.3px]">
            <div class="flex items-center gap-[19.2px]">
              <div class="bg-[rgba(202,164,62,0.16)] rounded-[8.64px] px-[13.4px] py-[4.2px]"><p class="text-[17.7px] font-[hk5] font-medium text-[#CAA43E]">Auto</p></div>
              <p class="text-[22.1px] text-[#8C8B83]">＋</p>
            </div>
            <div class="flex items-center gap-[19.2px]">
              <p class="text-[19.2px] text-[#7C7B73] font-[hk5] font-medium">Opus 4.8</p>
              <p class="text-[19.2px] text-[#9A998F] font-[hk5] font-medium">High</p>
              <div id="sm-orb" class="w-[24px] h-[24px] rounded-full border-[3.26px] border-[rgba(96,148,214,0.26)]"></div>
            </div>
          </div>
        </div>
        <div id="sm-veil" class="absolute inset-0 bg-[#1D1F1F] opacity-0"></div>
        {/* FIX: ref .still is top:50%+translateY(-50%); settled ink sits at
            454..592 on f358 while test was 450..588 — drop the box 4px */}
        <div id="sm-still" class="absolute left-0 right-0 top-[540px] -mt-[82px] text-center opacity-0">
          <p class="font-[gal] text-[172.8px] tracking-[-3.456px] text-[#EFEADD] leading-none relative -top-[5px]">Still too long</p>
        </div>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 5 — use-skill (12.3–16.1 s) ═════════════════
function UseSkill({ rootId }) {
  useGSAP(() => {
    const T = gsap.timeline();
    // ref use-skill.html:66 — y0 = dCmd(36+26.88) + dOut(100+26.88) = 189.76
    T.set('us-stack', { y: 189.76 }, 0);
    T.to('us-orb', { rotation: 360, duration: 1, ease: 'none', repeat: 3 }, O5);
    T.to('us-caret', { opacity: 0, duration: 0.5, ease: 'steps(1)', repeat: 1, yoyo: true }, O5 + 0.1);
    T.set('us-ph', { opacity: 0 }, O5 + 0.4);
    T.set('us-caret', { opacity: 1 }, O5 + 0.4);
    tw(T, ctx, 'us-typed', 'use pr-to-video to make a video for this pr', O5 + 0.4, 1.4);
    T.set('us-caret', { opacity: 0 }, O5 + 2.0);
    // ref use-skill.html:80 — the placeholder returns when the command is sent
    T.set('us-ph', { opacity: 1 }, O5 + 2.0);
    T.fromTo('us-cmd', { opacity: 0 }, { opacity: 1, duration: 0.3, ease: 'power2.out' }, O5 + 2.05);
    // ref use-skill.html:73 — mid state parks the stack at dOut = 126.88
    T.to('us-stack', { y: 126.88, duration: 0.4, ease: 'power3.out' }, O5 + 2.05);
    T.fromTo('us-out', { opacity: 0 }, { opacity: 1, duration: 0.2 }, O5 + 2.45);
    T.to('us-stack', { y: 0, duration: 0.4, ease: 'power3.out' }, O5 + 2.5);
    T.fromTo('us-think', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.3, ease: 'power2.out' }, O5 + 2.5);
    T.to('us-ast', { rotation: 360, duration: 1.0, ease: 'none', repeat: 1 }, O5 + 2.5);
    T.fromTo('us-l0', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.35, ease: 'power3.out' }, O5 + 3.0);
  });
  return (
    <div id={rootId} class="absolute inset-0 z-[5] font-[hk4] opacity-0">
      <div class="absolute left-[163.2px] top-[102.6px] w-[1599.36px] h-[874.8px] bg-[#1D1F1F] rounded-[25.92px] overflow-hidden shadow-[0_38px_115px_-38px_rgba(20,20,18,0.45)]">
        <div class="absolute left-[30.7px] top-[26.9px] flex items-center gap-[11.9px] z-[5]">
          <div class="w-[20.2px] h-[20.2px] rounded-full bg-[#FF5F57]"></div>
          <div class="w-[20.2px] h-[20.2px] rounded-full bg-[#FEBC2E]"></div>
          <div class="w-[20.2px] h-[20.2px] rounded-full bg-[#28C840]"></div>
          <p class="relative -top-[3px] ml-[14.3px] text-[19.2px] font-[mono4] text-[#8C8B83]">claude — ~/hyperframes</p>
        </div>
        <div class="absolute left-[65.3px] right-[65.3px] top-[105.6px] bottom-[316.8px] overflow-hidden">
          <div id="us-stack" class="absolute left-0 right-0 bottom-0 flex flex-col gap-[26.88px]">
            <div id="us-cmd" class="self-end text-right opacity-0">
              <p class="text-[25.92px] leading-[36.288px] font-[mono4] text-[#E6E2D6]">use pr-to-video to make a video for this pr</p>
            </div>
            <div id="us-out" class="self-start opacity-0">
              {/* think/l0 row heights: ref .think normal line-height, .ol 1.45 */}
              <div id="us-think" class="flex items-center gap-[13.44px] mb-[19.2px] opacity-0">
                <path id="us-ast" d="M16 6.5 L16 25.5 M6.5 16 L25.5 16 M9.3 9.3 L22.7 22.7 M22.7 9.3 L9.3 22.7" view-box="0 0 32 32" class="w-[30.72px] h-[30.72px] flex-none stroke-[#D97757] fill-none stroke-[2.1] relative -top-[0.5px]" />
                <p class="text-[26.88px] leading-[35.025px] text-[#8C8B83]">thinking…</p>
              </div>
              <div id="us-l0" class="flex items-start gap-[13.44px] opacity-0">
                <p class="text-[#D97757] text-[27.84px] leading-[40.368px] font-[hk7] font-bold">⏺</p>
                <p class="text-[27.84px] leading-[40.368px] font-[mono4] text-[#A09D96]">Reading PR #1635 · 629 files · 4 commits</p>
              </div>
            </div>
          </div>
        </div>
        <div class="absolute left-[65.3px] right-[65.3px] bottom-[65.3px]">
          <div class="flex gap-[11.52px] mb-[17.28px]">
            <div class="bg-[#2C2C2B] rounded-[10.56px] px-[15.7px] py-[8.8px]"><p class="text-[18.3px] font-[hk5] font-medium text-[#D7D4CA]">Local</p></div>
            <div class="bg-[#2C2C2B] rounded-[10.56px] px-[15.7px] py-[8.8px]"><p class="text-[18.3px] font-[hk5] font-medium text-[#D7D4CA]">hyperframes</p></div>
          </div>
          <div class="relative flex items-center bg-[#2C2C2B] border-[1px] border-[#3a3a38] rounded-[17.28px] min-h-[92.24px] px-[23px] shadow-[0_0_0_2.3px_rgba(217,119,87,0.45)]">
            <div class="flex-1 flex items-center pr-[50px]">
              <p id="us-typed" class="text-[21.12px] font-[mono4] text-[#E6E2D6] whitespace-nowrap"></p>
              <p id="us-caret" class="text-[27.84px] text-[#D97757]">▏</p>
              <p id="us-ph" class="relative top-[3px] ml-[-16px] text-[22.08px] text-[#6F6E66]">Reply to Claude…</p>
            </div>
            <path d="M9 10 4 15 9 20 M20 4v7a4 4 0 0 1-4 4H4" view-box="0 0 24 24" class="absolute right-[21px] w-[15px] h-[15px] stroke-[2.6] stroke-[#6F6E66] fill-none" />
          </div>
          <div class="flex justify-between items-center mt-[16.3px]">
            <div class="flex items-center gap-[19.2px]">
              <div class="bg-[rgba(202,164,62,0.16)] rounded-[8.64px] px-[13.4px] py-[4.2px]"><p class="text-[17.7px] font-[hk5] font-medium text-[#CAA43E]">Auto</p></div>
              <p class="text-[22.1px] text-[#8C8B83]">＋</p>
            </div>
            <div class="flex items-center gap-[19.2px]">
              <p class="text-[19.2px] text-[#7C7B73] font-[hk5] font-medium">Opus 4.8</p>
              <p class="text-[19.2px] text-[#9A998F] font-[hk5] font-medium">High</p>
              <div id="us-orb" class="w-[24px] h-[24px] rounded-full border-[3.26px] border-[rgba(96,148,214,0.26)]"></div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 6 — feature-code (16.1–18.5 s, inner ×2.0) ═════════════════
function FeatureCode({ ids }) {
  useGSAP(() => {
    const T = gsap.timeline();
    cardTitle(T, ctx, 'fc-ttext', 'fc-tcaret', 'SHOW YOUR CODE', O6, 1.15, 2.0, 4.5);
    T.fromTo('fc-pwin', { opacity: 0, scale: 0.62, y: 10 }, { opacity: 1, scale: 1, y: 0, duration: 0.6 / 2, ease: 'power3.out' }, O6 + 0.15 / 2);
    T.to('fc-codeview', { opacity: 1, duration: 0.45 / 2, ease: 'power2.out' }, O6 + 1.0 / 2);
    T.to(ids.pfill, { scaleX: 0.11, duration: 3.2 / 2, ease: 'none' }, O6 + 1.4 / 2);
    T.to(ids.pknob, { left: (11 / 100) * 923.6, duration: 3.2 / 2, ease: 'none' }, O6 + 1.4 / 2);
    // ref feature-code.html:125: t2 2→5 dur 3.2 @1.4 (inner ×2), Math.floor
    for (let tk = 3; tk <= 5; tk++) {
      T.set({ set: () => ctx.getNode('fc-ptime').text('0:0' + tk + ' / 0:32') }, { opacity: 1 }, O6 + (1.4 + 3.2 * (tk - 2) / 3) / 2.0);
    }
    T.to('fc-nonkeyA', { opacity: 0.32, duration: 0.4 / 2, ease: 'power2.out' }, O6 + 2.45 / 2);
    T.to('fc-nonkeyB', { opacity: 0.32, duration: 0.4 / 2, ease: 'power2.out' }, O6 + 2.45 / 2);
    T.to('fc-band', { opacity: 1, duration: 0.4 / 2, ease: 'power2.out' }, O6 + 2.5 / 2);
  });
  return (
    <div id={ids.scene} class="absolute inset-0 z-[6] bg-[#F0EFE9] font-[hk4] opacity-0">
      <CardTitle titleId="fc-ttext" caretId="fc-tcaret" />
      <div class="absolute inset-0 z-[2] flex items-center justify-center">
        <PlayerWindow pwinId="fc-pwin" sceneLabel="scene 1 / 6" fillId={ids.pfill} knobId={ids.pknob} ptimeId="fc-ptime" time0="0:02 / 0:32" stageBg="#1A1B1A">
          <div id={ids.stage} class="absolute left-0 top-0 w-[1920px] h-[1080px] origin-top-left">
          <div id="fc-codeview" class="absolute inset-0 bg-[#1A1B1A] font-[mono4] opacity-0">
            <div class="relative -top-[6.5px] flex items-center gap-[14px] bg-[#252320] px-[40px] py-[26px]">
              <div class="w-[20px] h-[20px] rounded-full bg-[#FF5F57]"></div>
              <div class="w-[20px] h-[20px] rounded-full bg-[#FEBC2E]"></div>
              <div class="w-[20px] h-[20px] rounded-full bg-[#28C840]"></div>
              <p class="ml-[18px] text-[28px] text-[#9c9485]">skills/pr-to-video/scripts/</p>
              <p class="text-[28px] text-[#F5EFE6] font-[mono5]">fetch-pr.mjs</p>
              <p class="ml-auto text-[26px] text-[#5DB872]">+164 −0</p>
            </div>
            <div class="relative -top-[6.5px] pt-[34px]">
              <div class="px-[54px] text-[33px] leading-[62px]">
                <div id="fc-nonkeyA">{FC_A.map((s) => <CodeLine key={s[0]} seg={s} />)}</div>
                <div id="fc-key">{FC_KEY.map((s) => <CodeLine key={s[0]} seg={s} />)}</div>
                <div id="fc-nonkeyB">{FC_B.map((s) => <CodeLine key={s[0]} seg={s} />)}</div>
              </div>
              <div id="fc-band" class="absolute left-[24px] right-[24px] top-[622px] h-[198px] rounded-[10px] bg-[rgba(217,119,87,0.13)] border-l-[7px] border-[#D97757] opacity-0"></div>
            </div>
          </div>
          </div>
        </PlayerWindow>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 7 — feature-flow (18.5–21.0 s, inner ×2.56) ═════════════════
function FeatureFlow({ ids }) {
  useGSAP(() => {
    const T = gsap.timeline();
    cardTitle(T, ctx, 'ff-ttext', 'ff-tcaret', 'SHOW YOUR FEATURE', O7, 1.15, 2.56, 6.0);
    T.fromTo('ff-pwin', { opacity: 0, scale: 0.62, y: 10 }, { opacity: 1, scale: 1, y: 0, duration: 0.6 / 2.56, ease: 'power3.out' }, O7 + 0.15 / 2.56);
    T.to(ids.pfill, { scaleX: 0.42, duration: 3.6 / 2.56, ease: 'none' }, O7 + 1.9 / 2.56);
    T.to(ids.pknob, { left: (42 / 100) * 923.6, duration: 3.6 / 2.56, ease: 'none' }, O7 + 1.9 / 2.56);
    // ref feature-flow.html:174: t2 7→13 dur 3.6 @1.9 (inner ×2.56), Math.floor
    for (let fk = 8; fk <= 13; fk++) {
      T.set({ set: () => ctx.getNode('ff-ptime').text('0:' + fk + ' / 0:32') }, { opacity: 1 }, O7 + (1.9 + 3.6 * (fk - 7) / 6) / 2.56);
    }
    T.fromTo('ff-diff', { opacity: 0, x: -30, scale: 0.95 }, { opacity: 1, x: 0, scale: 1, duration: 0.5 / 2.56, ease: 'power3.out' }, O7 + 2.0 / 2.56);
    T.fromTo('ff-a1', { opacity: 0, x: -10 }, { opacity: 1, x: 0, duration: 0.35 / 2.56, ease: 'power2.out' }, O7 + 2.55 / 2.56);
    // FIX: fromTo — the xml .to had no y/scale seed so cards never popped
    for (let c = 1; c <= 5; c++) {
      T.fromTo('ff-fc' + c, { opacity: 0, y: 20, scale: 0.85 }, { opacity: 1, y: 0, scale: 1, duration: 0.4 / 2.56, ease: 'back.out(1.6)' }, O7 + 2.7 / 2.56 + (c - 1) * (0.1 / 2.56));
    }
    T.to('ff-striplbl', { opacity: 1, duration: 0.4 / 2.56, ease: 'power2.out' }, O7 + 3.0 / 2.56);
    T.fromTo('ff-a2', { opacity: 0, x: -10 }, { opacity: 1, x: 0, duration: 0.35 / 2.56, ease: 'power2.out' }, O7 + 3.45 / 2.56);
    T.fromTo('ff-video', { opacity: 0, scale: 0.8, x: 20 }, { opacity: 1, scale: 1, x: 0, duration: 0.55 / 2.56, ease: 'back.out(1.6)' }, O7 + 3.6 / 2.56);
    T.fromTo('ff-vring', { opacity: 0.5, scale: 1 }, { opacity: 0, scale: 1.8, duration: 1.0 / 2.56, ease: 'power2.out', repeat: 1 }, O7 + 4.1 / 2.56);
  });
  return (
    <div id={ids.scene} class="absolute inset-0 z-[7] bg-[#F0EFE9] font-[hk4] opacity-0">
      <CardTitle titleId="ff-ttext" caretId="ff-tcaret" />
      <div class="absolute inset-0 z-[2] flex items-center justify-center">
        <PlayerWindow pwinId="ff-pwin" sceneLabel="scene 2 / 6" fillId={ids.pfill} knobId={ids.pknob} ptimeId="ff-ptime" time0="0:07 / 0:32" stageBg="#FAF9F5">
          <div id={ids.stageA} class="absolute left-0 top-0 w-[1920px] h-[1080px] origin-top-left">
          {/* ref .arrow: stroke-width 3 + round cap/join */}
          <div id={ids.stageB} class="absolute inset-0 bg-[#FAF9F5] font-[mono4] text-[#141413] flex items-center justify-center gap-[38px]">
            <div id="ff-diff" class="w-[560px] bg-[#1A1B1A] rounded-[14px] overflow-hidden shadow-[0_8px_28px_-10px_rgba(20,20,19,0.4)] opacity-0">
              <div class="flex items-center gap-[9px] bg-[#252320] px-[22px] py-[16px]">
                <div class="w-[14px] h-[14px] rounded-full bg-[#cc5f57]"></div>
                <div class="w-[14px] h-[14px] rounded-full bg-[#d8a44a]"></div>
                <div class="w-[14px] h-[14px] rounded-full bg-[#5db872]"></div>
                <p class="ml-[14px] text-[24px] leading-[1.2] text-[#9c9485]">PR #1635 · the diff</p>
              </div>
              <div class="flex gap-[18px] px-[26px] py-[6px] text-[26px]"><p class="text-[#c64545]">-</p><p class="text-[#b87d77] whitespace-normal">gh pr view --json files</p></div>
              <div class="flex gap-[18px] px-[26px] py-[6px] text-[26px]"><p class="text-[#5db872]">+</p><p class="text-[#9fc9a8] whitespace-normal">gh api --paginate …/files</p></div>
              <div class="flex gap-[18px] px-[26px] py-[6px] mb-[22px] text-[26px]"><p class="text-[#5db872]">+</p><p class="text-[#9fc9a8] whitespace-normal">storyboard → frames → render</p></div>
            </div>
            <path id="ff-a1" d="M6 20 H104 M92 9 L110 20 L92 31" view-box="0 0 120 40" class="w-[120px] h-[40px] flex-none stroke-[#CC785C] stroke-[3] fill-none opacity-0" />
            <div class="relative flex gap-[18px] items-start justify-center">
              {/* ref .fc{width:134px;padding:12px;border:1px} content-box → 160 flow */}
              {FF_CARDS.map((c) => <FfCard key={c[0]} n={c[0]} thumb={c[1]} />)}
              <p id="ff-striplbl" class="absolute left-0 right-0 bottom-[-46px] text-center text-[22px] leading-[1.2] tracking-[0.66px] text-[#6b6459] whitespace-nowrap opacity-0">storyboard · scene by scene</p>
            </div>
            <path id="ff-a2" d="M6 20 H104 M92 9 L110 20 L92 31" view-box="0 0 120 40" class="w-[120px] h-[40px] flex-none stroke-[#CC785C] stroke-[3] fill-none opacity-0" />
            {/* ref .vcard{width:420px;height:260px;border:1.5px} content-box → 423×263 */}
            <div id="ff-video" class="w-[423px] h-[263px] bg-[#FAF9F5] border-[1.5px] border-[#CC785C] rounded-[14px] shadow-[0_10px_30px_-10px_rgba(204,120,92,0.35)] flex flex-col items-center justify-center gap-[22px] opacity-0">
              <div class="relative w-[104px] h-[104px] rounded-full bg-[#CC785C] flex items-center justify-center">
                <div id="ff-vring" class="absolute inset-0 rounded-full border-[3px] border-[#CC785C] opacity-0"></div>
                <path d="M8 5l11 7-11 7z" view-box="0 0 24 24" class="w-[46px] h-[46px] ml-[6px] fill-[#FAF9F5]" />
              </div>
              <p class="text-[28px] leading-[1.2] text-[#141413]">video.mp4</p>
            </div>
          </div>
          </div>
        </PlayerWindow>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 8 — feature-discussion (21.0–23.0 s, inner ×2.05, conv ×4.1) ═════════════════
function FeatureDiscussion({ ids }) {
  useGSAP(() => {
    const T = gsap.timeline();
    tw(T, ctx, 'fd-ttext', 'SHOW YOUR REVIEW', O8 + 0.2 / 2.05, 0.9 / 2.05);
    T.to('fd-tcaret', { opacity: 0, duration: 0.35, ease: 'steps(1)', repeat: 5, yoyo: true }, O8 + 0.2 / 2.05);
    T.to('fd-tcaret', { opacity: 0, duration: 0.25, ease: 'power2.in' }, O8 + 3.6 / 2.05);
    T.fromTo('fd-pwin', { opacity: 0, scale: 0.62, y: 10 }, { opacity: 1, scale: 1, y: 0, duration: 0.5 / 2.05, ease: 'power3.out' }, O8 + 0.1 / 2.05);
    T.to(ids.pfill, { scaleX: 0.80, duration: 3.2 / 2.05, ease: 'none' }, O8 + 0.6 / 2.05);
    T.to(ids.pknob, { left: (80 / 100) * 923.6, duration: 3.2 / 2.05, ease: 'none' }, O8 + 0.6 / 2.05);
    // ref feature-discussion.html:173: t2 24→27 dur 3.2 @0.6 (inner ×2.05)
    for (let dk = 25; dk <= 27; dk++) {
      T.set({ set: () => ctx.getNode('fd-ptime').text('0:' + dk + ' / 0:32') }, { opacity: 1 }, O8 + (0.6 + 3.2 * (dk - 24) / 3) / 2.05);
    }
    // conversation `disc` (authored 1×) is nested in inner (2.05×) then
    // timeScale(2) → plays at 4.1×; divide these events by 4.1.
    T.to('fd-card', { opacity: 1, duration: 0.5 / 4.1, ease: 'power2.out' }, O8 + 0.5 / 4.1);
    T.to('fd-chead', { opacity: 1, duration: 0.4 / 4.1, ease: 'power2.out' }, O8 + 0.9 / 4.1);
    for (let r = 1; r <= 4; r++) {
      T.fromTo('fd-r' + r, { opacity: 0, scale: 0.96, y: 14 }, { opacity: 1, scale: 1, y: 0, duration: 0.42 / 4.1, ease: 'back.out(1.5)' }, O8 + 1.4 / 4.1 + (r - 1) * (0.4 / 4.1));
    }
    T.fromTo('fd-r5', { opacity: 0, scale: 0.96, y: 18 }, { opacity: 1, scale: 1, y: 0, duration: 0.5 / 4.1, ease: 'back.out(1.6)' }, O8 + 3.4 / 4.1);
    const pairs = [['fd-p4o', 'fd-p4d'], ['fd-p3o', 'fd-p3d'], ['fd-p2o', 'fd-p2d'], ['fd-p1o', 'fd-p1d']];
    for (let pi = 0; pi < 4; pi++) {
      const pt = O8 + (4.2 + pi * 0.28) / 4.1;
      T.to(pairs[pi][0], { opacity: 0, duration: 0.2 / 4.1, ease: 'power2.in' }, pt);
      T.fromTo(pairs[pi][1], { opacity: 0, scale: 0.8 }, { opacity: 1, scale: 1, duration: 0.32 / 4.1, ease: 'back.out(1.7)' }, pt + 0.04 / 4.1);
    }
    T.fromTo('fd-merge', { opacity: 0, scale: 0.85, y: 10 }, { opacity: 1, scale: 1, y: 0, duration: 0.55 / 4.1, ease: 'back.out(1.6)' }, O8 + 5.9 / 4.1);
  });
  const rowCls = 'flex items-center gap-[24px] h-[104px] mb-[14px] px-[26px] bg-[#FAF8F2] border-[1px] border-[rgba(20,20,19,0.14)] rounded-[13px] opacity-0';
  const avCls = 'w-[62px] h-[62px] rounded-full border-[1px] border-[rgba(20,20,19,0.12)] object-cover flex-none';
  const badge = (cls, label) => (
    <div class={cls}><p class="text-[15px] leading-[1] font-[mono5]">{label}</p></div>
  );
  // pills must be div-carried: <p> only paints bg as a line-box strip and
  // drops border/rounded/padding entirely. Pill box = 17px + 2*7 + 2 border = 33.
  const pillBase = 'absolute right-0 top-[3.5px] h-[33px] flex items-center gap-[7px] px-[14px] rounded-[9px] border-[1px] text-[17px] leading-[1] font-[mono5]';
  const pillDone = (id) => (
    <div id={id} class={pillBase + ' bg-[rgba(79,157,91,0.16)] border-[rgba(79,157,91,0.5)] text-[#2f6b3d] opacity-0'}>
      <p class="text-[#4F9D5B]">✓</p>
      <p>resolved</p>
    </div>
  );
  const pillOpen = (id, mark, label, blk) => (
    <div id={id} class={pillBase + (blk ? ' bg-[rgba(214,69,69,0.12)] border-[rgba(214,69,69,0.35)] text-[#B23A3A]' : ' bg-[rgba(193,122,60,0.12)] border-[rgba(193,122,60,0.32)] text-[#9C5A1F]') + ' opacity-0'}>
      <p class={blk ? 'text-[15px] text-[#D64545]' : 'text-[15px] text-[#C77C3C]'}>{mark}</p>
      <p>{label}</p>
    </div>
  );
  const pillPush = () => (
    <div class={pillBase + ' bg-[rgba(217,119,87,0.14)] border-[rgba(217,119,87,0.45)] text-[#B4502F]'}>
      <p class="text-[#D97757]">✚</p>
      <p>pushed</p>
    </div>
  );
  return (
    <div id={ids.scene} class="absolute inset-0 z-[8] bg-[#F0EFE9] font-[hk4] opacity-0">
      <CardTitle titleId="fd-ttext" caretId="fd-tcaret" />
      <div class="absolute inset-0 z-[2] flex items-center justify-center">
        <PlayerWindow pwinId="fd-pwin" sceneLabel="scene 5 / 6" fillId={ids.pfill} knobId={ids.pknob} ptimeId="fd-ptime" time0="0:24 / 0:32" stageBg="#FAF9F5">
          <div id={ids.stage} class="absolute left-0 top-0 w-[1920px] h-[1080px] origin-top-left">
          <div class="absolute inset-0 bg-[#FAF9F5]">
            <div id="fd-card" class="absolute left-[60px] right-[60px] top-[70px] h-[760px] border-[1px] border-[rgba(20,20,19,0.12)] rounded-[18px] bg-[rgba(236,227,212,0.32)] overflow-hidden opacity-0">
              <div id="fd-chead" class="flex items-center justify-between h-[72px] px-[30px] border-b-[1px] border-[rgba(20,20,19,0.1)] opacity-0">
                <div class="flex items-center">
                  <p class="text-[24px] leading-[1.2] font-[mono5] text-[#D97757]">◆</p>
                  <p class="text-[24px] leading-[1.2] font-[mono5] text-[#3a392f]">{" #1635 · Conversation"}</p>
                </div>
                <p class="text-[20px] leading-[1.2] font-[mono4] text-[#8C8B83]">4 reviewers · 1 author</p>
              </div>
              <div class="relative top-[2px] px-[26px] py-[26px]">
                <div id="fd-r1" class={rowCls}>
                  <image path="assets/avatars/ghas-62.png" class={avCls} />
                  <div class="flex flex-col gap-[9px] flex-1">
                    <div class="flex items-center gap-[13px]">
                      <p class="text-[27px] leading-[1] font-[hk6] font-semibold text-[#1D1F1F]">CodeQL</p>
                      {badge('text-[#B23A3A] bg-[rgba(214,69,69,0.12)] border-[1px] border-[rgba(214,69,69,0.35)] px-[11px] py-[4px] rounded-[7px]', 'security · bot')}
                    </div>
                    <p class="text-[18px] leading-[1] font-[mono4] text-[#6b6a61] whitespace-nowrap">12 high-severity alerts · TOCTOU race + HTML injection</p>
                  </div>
                  <div class="relative w-[190px] h-[40px] flex-none">
                    {pillOpen('fd-p1o', '●', 'blocking', true)}
                    {pillDone('fd-p1d')}
                  </div>
                </div>
                <div id="fd-r2" class={rowCls}>
                  <image path="assets/avatars/jrusso1020-62.png" class={avCls} />
                  <div class="flex flex-col gap-[9px] flex-1">
                    <div class="flex items-center gap-[13px]">
                      <p class="text-[27px] leading-[1] font-[hk6] font-semibold text-[#1D1F1F]">jrusso1020</p>
                      {badge('text-[#9C5A1F] bg-[rgba(193,122,60,0.14)] border-[1px] border-[rgba(193,122,60,0.35)] px-[11px] py-[4px] rounded-[7px]', 'changes requested')}
                    </div>
                    <p class="text-[18px] leading-[1] font-[mono4] text-[#6b6a61] whitespace-nowrap">CodeQL required check failing — blocks approve</p>
                  </div>
                  <div class="relative w-[190px] h-[40px] flex-none">
                    {pillOpen('fd-p2o', '●', 'changes', false)}
                    {pillDone('fd-p2d')}
                  </div>
                </div>
                <div id="fd-r3" class={rowCls}>
                  <image path="assets/avatars/miguel-62.png" class={avCls} />
                  <div class="flex flex-col gap-[9px] flex-1">
                    <div class="flex items-center gap-[13px]">
                      <p class="text-[27px] leading-[1] font-[hk6] font-semibold text-[#1D1F1F]">miguel-heygen</p>
                      {badge('text-[#6b6a61] bg-[rgba(20,20,19,0.06)] border-[1px] border-[rgba(20,20,19,0.14)] px-[11px] py-[4px] rounded-[7px]', 're-review')}
                    </div>
                    <p class="text-[18px] leading-[1] font-[mono4] text-[#6b6a61] whitespace-nowrap">TOCTOU + sanitization in captions.mjs across 3 skills</p>
                  </div>
                  <div class="relative w-[190px] h-[40px] flex-none">
                    {pillOpen('fd-p3o', '●', 'open', false)}
                    {pillDone('fd-p3d')}
                  </div>
                </div>
                <div id="fd-r4" class={rowCls}>
                  <image path="assets/avatars/miga-62.png" class={avCls} />
                  <div class="flex flex-col gap-[9px] flex-1">
                    <div class="flex items-center gap-[13px]">
                      <p class="text-[27px] leading-[1] font-[hk6] font-semibold text-[#1D1F1F]">miga-heygen</p>
                      {badge('text-[#6b6a61] bg-[rgba(20,20,19,0.06)] border-[1px] border-[rgba(20,20,19,0.14)] px-[11px] py-[4px] rounded-[7px]', 'review')}
                    </div>
                    <p class="text-[18px] leading-[1] font-[mono4] text-[#6b6a61] whitespace-nowrap">major refactor · 630 files — verifying each commit</p>
                  </div>
                  <div class="relative w-[190px] h-[40px] flex-none">
                    {pillOpen('fd-p4o', '●', 'reviewing', false)}
                    {pillDone('fd-p4d')}
                  </div>
                </div>
                <div id="fd-r5" class="flex items-center gap-[24px] h-[104px] px-[26px] bg-[rgba(217,119,87,0.08)] border-[1px] border-[rgba(217,119,87,0.38)] rounded-[13px] opacity-0">
                  <image path="assets/avatars/waterrr-62.png" class={avCls} />
                  <div class="flex flex-col gap-[9px] flex-1">
                    <div class="flex items-center gap-[13px]">
                      <p class="text-[27px] leading-[1] font-[hk6] font-semibold text-[#1D1F1F]">WaterrrForever</p>
                      {badge('text-[#B4502F] bg-[rgba(217,119,87,0.14)] border-[1px] border-[rgba(217,119,87,0.42)] px-[11px] py-[4px] rounded-[7px]', 'author')}
                    </div>
                    <p class="text-[18px] leading-[1] font-[mono4] text-[#6b6a61] whitespace-nowrap">pushed cfdc213 · cb6de92 · e882ce9 — fixpoint sanitize + atomic writes</p>
                  </div>
                  <div class="relative w-[190px] h-[40px] flex-none">
                    {pillPush()}
                  </div>
                </div>
              </div>
            </div>
            {/* ref .merge: inline-flex shrink box centered via full-width justify-center */}
            <div class="absolute left-0 top-[876px] w-[1920px] flex justify-center">
              <div id="fd-merge" class="flex items-center gap-[11px] text-[30px] font-[mono5] text-[#2f6b3d] bg-[rgba(79,157,91,0.14)] border-[1px] border-[rgba(79,157,91,0.5)] rounded-[13px] px-[30px] py-[15px] opacity-0">
                <p class="text-[#4F9D5B] text-[31px]">✓</p>
                <p class="text-[30px] font-[mono5] text-[#2f6b3d]">all conversations resolved ·</p>
                <p class="text-[30px] font-[mono5] font-semibold text-[#2f6b3d]">ready to merge</p>
              </div>
            </div>
          </div>
          </div>
        </PlayerWindow>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 9 — feature-credits (23.0–25.6 s, inner ×2.54) ═════════════════
const CREDITS = [
  [270, 'assets/avatars/waterrr-150.png', 'WaterrrForever', 'author · 11 commits', '#8C8B83', false],
  [600, 'assets/avatars/jrusso1020-150.png', 'jrusso1020', 'reviewer', '#8C8B83', false],
  [850, 'assets/avatars/miguel-150.png', 'miguel-heygen', 'reviewer', '#8C8B83', false],
  [1100, 'assets/avatars/miga-150.png', 'miga-heygen', 'reviewer', '#8C8B83', false],
  [1450, 'assets/avatars/claude-150.png', 'Claude', 'co-authored · 11', '#bf7a5e', true],
  [1680, 'assets/avatars/codex-150.png', 'Codex', 'pair', '#bf7a5e', true],
];

function CreditCol({ c, i }) {
  const [left, avatar, name, role, roleColor, orange] = c;
  const n = i + 1;
  return (
    <div class={'absolute top-[362px] left-[' + left + 'px] w-[320px] -ml-[160px] flex flex-col items-center'}>
      <div class="relative w-[150px] h-[150px]">
        <image id={'cr-av' + n} path={avatar} class={'w-[150px] h-[150px] rounded-full object-cover opacity-0' + (orange
          ? ' shadow-[0_0_0_4px_#CC785C,0_12px_30px_-12px_rgba(204,120,92,0.4)]'
          : ' shadow-[0_0_0_3px_rgba(20,20,19,0.14),0_12px_30px_-12px_rgba(20,20,19,0.3)]')} />
        {/* FIX: entrance ring pulse (ref ::after animation) */}
        <div id={'cr-ring' + n} class="absolute left-[-7px] right-[-7px] top-[-7px] bottom-[-7px] rounded-full border-[4px] border-[#CC785C] opacity-0"></div>
      </div>
      <p id={'cr-nm' + n} class="mt-[22px] text-[24px] font-[mono5] text-[#1D1F1F] whitespace-nowrap opacity-0">{name}</p>
      <p id={'cr-rl' + n} class={'mt-[7px] text-[20px] text-[' + roleColor + '] whitespace-nowrap opacity-0'}>{role}</p>
    </div>
  );
}

function FeatureCredits({ ids }) {
  useGSAP(() => {
    const T = gsap.timeline();
    cardTitle(T, ctx, 'cr-ttext', 'cr-tcaret', 'SHOW YOUR CONTRIBUTOR', O9, 1.0, 2.54, 6.1, 7);
    T.fromTo('cr-pwin', { opacity: 0, scale: 0.62, y: 10 }, { opacity: 1, scale: 1, y: 0, duration: 0.6 / 2.54, ease: 'power3.out' }, O9 + 0.15 / 2.54);
    T.to(ids.pfill, { scaleX: 0.98, duration: 3.4 / 2.54, ease: 'none' }, O9 + 1.9 / 2.54);
    T.to(ids.pknob, { left: (98 / 100) * 923.6, duration: 3.4 / 2.54, ease: 'none' }, O9 + 1.9 / 2.54);
    // ref feature-credits.html:133: t2 30→32 dur 3.4 @1.9 (inner ×2.54)
    for (let rk = 31; rk <= 32; rk++) {
      T.set({ set: () => ctx.getNode('cr-ptime').text('0:' + rk + ' / 0:32') }, { opacity: 1 }, O9 + (1.9 + 3.4 * (rk - 30) / 2) / 2.54);
    }
    // FIX: seed the y offsets the ref gsap.set's before its .to tweens —
    // labels drop 8px, name/role lines rise 10px, sign rises 8px
    T.fromTo(['cr-g1', 'cr-g2', 'cr-g3'], { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.4 / 2.54, ease: 'power3.out', stagger: 0.12 / 2.54 }, O9 + 1.95 / 2.54);
    T.to(ids.path, { scaleX: 1, duration: 1.2 / 2.54, ease: 'power2.inOut' }, O9 + 2.0 / 2.54);
    for (let ci = 1; ci <= 6; ci++) {
      const ct = O9 + (2.05 + (ci - 1) * 0.16) / 2.54;
      // FIX: fromTo pins the pre-entrance state; ring pulses out (ref ::after)
      T.fromTo('cr-av' + ci, { scale: 0.55, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.45 / 2.54, ease: 'back.out(1.7)' }, ct);
      T.fromTo('cr-ring' + ci, { scale: 0.5, opacity: 0.6 }, { scale: 1.35, opacity: 0, duration: 0.7 / 2.54, ease: 'power2.out' }, ct);
      T.fromTo('cr-nm' + ci, { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.35 / 2.54, ease: 'power3.out' }, ct + 0.14 / 2.54);
      T.fromTo('cr-rl' + ci, { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.35 / 2.54, ease: 'power3.out' }, ct + 0.24 / 2.54);
    }
    T.fromTo('cr-sign', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.5 / 2.54, ease: 'power3.out' }, O9 + 3.9 / 2.54);
    T.fromTo('cr-sign2', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.5 / 2.54, ease: 'power3.out' }, O9 + 3.9 / 2.54);
  });
  // FIX: group labels are centered over their avatar column (ref uses
  // left:50%+translateX(-50%); mono5 22px tracking 3px → 16.2px per char)
  const grp = (id, left, color, label) => (
    <p id={id} class={'absolute left-[' + left + 'px] top-[300px] text-[22px] font-[mono5] tracking-[3px] text-[' + color + '] opacity-0'}>{label}</p>
  );
  return (
    <div id={ids.scene} class="absolute inset-0 z-[9] bg-[#F0EFE9] font-[hk4] opacity-0">
      <CardTitle titleId="cr-ttext" caretId="cr-tcaret" />
      <div class="absolute inset-0 z-[2] flex items-center justify-center">
        <PlayerWindow pwinId="cr-pwin" sceneLabel="scene 6 / 6" fillId={ids.pfill} knobId={ids.pknob} ptimeId="cr-ptime" time0="0:30 / 0:32" stageBg="#FAF9F5">
          <div id={ids.stage} class="absolute left-0 top-0 w-[1920px] h-[1080px] origin-top-left">
          <div class="absolute inset-0 bg-[#FAF9F5] font-[mono4] text-[#141413]">
            {grp('cr-g1', 197.1, '#8C8B83', 'COMMITTED')}
            {grp('cr-g2', 785.2, '#8C8B83', 'REVIEWED')}
            {grp('cr-g3', 1479, '#CC785C', 'BUILT WITH')}
            <div id={ids.path} class="absolute left-[270px] top-[438px] w-[1410px] h-0 border-t-[2px] border-dashed border-[rgba(204,120,92,0.4)] origin-left"></div>
            {CREDITS.map((c, i) => <CreditCol key={i} c={c} i={i} />)}
            {/* FIX: ref signoff is one centered row (left:50% + -50% translate):
                whole line = 476(seg1, engine-measured) + 28(rdy margin) + 233
                → seg1 starts at 960-368.5=591.5 (visual 735 = ref 735, frame 745) */}
            <p id="cr-sign" class="absolute left-[591.5px] top-[830px] text-[24px] text-[#8C8B83] whitespace-nowrap opacity-0">PR #1635 · heygen-com/hyperframes</p>
            <p id="cr-sign2" class="absolute left-[1095.5px] top-[830px] text-[24px] text-[#4F9D5B] whitespace-nowrap opacity-0">✓ ready to merge</p>
          </div>
          </div>
        </PlayerWindow>
      </div>
    </div>
  );
}

// ═════════════════ SCENE 10 — cta (25.6–30.0 s) ═════════════════
function CtaOutro({ rootId }) {
  useGSAP(() => {
    const T = gsap.timeline();
    T.set('cta-top', { scale: 1.18, y: 206, transformOrigin: '50% 50%', opacity: 0 }, 0);
    T.to('cta-orb', { rotation: 360, duration: 1, ease: 'none', repeat: 4 }, O10);
    T.fromTo('cta-gh', { opacity: 0, scale: 0.96 }, { opacity: 1, scale: 1, duration: 0.6, ease: 'power2.out' }, O10);
    T.to('cta-hcaret', { opacity: 0, duration: 0.3, ease: 'steps(1)', repeat: 1, yoyo: true }, O10 + 0.05);
    T.set('cta-top', { opacity: 1 }, O10);
    T.set('cta-hcaret', { opacity: 1 }, O10 + 0.3);
    // FIX: ref types through ONE global counter — o.n: 0→26 (TOT chars) over
    // 0.95s with Math.round, then slices three segments off it (cta.html:89-96).
    // Per-segment typewriters drift a half char against that.
    const h1n = ctx.getNode('cta-h1'), h2n = ctx.getNode('cta-h2'), h3n = ctx.getNode('cta-h3');
    const typer = { set: (v) => {
      const n = Math.round(26 * Math.min(Math.max(v.opacity, 0), 1));
      h1n.text('Create your '.slice(0, Math.min(n, 12)));
      h2n.text('PR video'.slice(0, Math.max(0, Math.min(n - 12, 8))));
      h3n.text(' today'.slice(0, Math.max(0, Math.min(n - 20, 6))));
    } };
    T.fromTo(typer, { opacity: 0 }, { opacity: 1, duration: 0.95, ease: 'none' }, O10 + 0.3);
    T.to('cta-top', { scale: 1, y: 0, duration: 0.55, ease: 'power3.inOut' }, O10 + 1.5);
    T.to('cta-hcaret', { opacity: 0, duration: 0.2, ease: 'power1.out' }, O10 + 1.5);
    // ref cta.html:84 — composer starts 44px low and rides up as it fades in
    T.set('cta-composer', { y: 44 }, 0);
    T.to('cta-composer', { opacity: 1, y: 0, duration: 0.5, ease: 'power3.out' }, O10 + 1.75);
    T.to('cta-ph', { opacity: 0, duration: 0.12 }, O10 + 2.15);
    // ref cta.html:74,104-105 — CMD includes the trailing space and the
    // counter runs 0→CMD.length (16), so the final state keeps the space and
    // the URL lands one advance (≈15px) further right (f854: ref sel-bg
    // 632..1332 vs test 618..1315 — width matched, position was 14px left)
    const cmdNode = ctx.getNode('cta-cmd');
    const cmdTyper = { set: (v) => {
      const n = Math.round(16 * Math.min(Math.max(v.opacity, 0), 1));
      cmdNode.text('use pr-to-video '.slice(0, n));
    } };
    T.fromTo(cmdTyper, { opacity: 0 }, { opacity: 1, duration: 0.55, ease: 'none' }, O10 + 2.2);
    // ref sets url.textContent instantly at 2.85 — insert AT that time so the
    // empty node keeps zero width (caret + placeholder stay left-aligned).
    T.set({ set: () => ctx.getNode('cta-url').text('https://github.com/your-org/your-repo/pull/42') }, { opacity: 1 }, O10 + 2.85);
    T.fromTo('cta-url', { backgroundColor: 'rgba(217,119,87,0.36)' }, { backgroundColor: 'rgba(217,119,87,0)', duration: 0.5, ease: 'power2.out' }, O10 + 2.85);
    T.fromTo('cta-url', { scale: 1.07 }, { scale: 1, duration: 0.3, ease: 'power3.out', transformOrigin: '0% 50%' }, O10 + 2.85);
    T.to('cta-caret', { opacity: 0, duration: 0.4, ease: 'steps(1)', repeat: 5, yoyo: true }, O10 + 3.15);
    T.to('cta-ret', { color: '#D97757', scale: 1.18, duration: 0.16, ease: 'back.out(2)', transformOrigin: '50% 50%' }, O10 + 3.25);
    T.to('cta-ret', { scale: 1, duration: 0.28, ease: 'power2.out' }, O10 + 3.42);
  });
  return (
    <div id={rootId} class="absolute inset-0 z-[10] bg-[#F0EFE9] font-[hk4] text-[#1D1F1F] opacity-0">
      <div id="cta-top" class="absolute inset-0 origin-center">
        <image id="cta-gh" path="assets/gh-mark-84.png" class="absolute left-[917.76px] top-[195.36px] w-[84.48px] h-[84.48px] opacity-0" />
        {/* ref cta.html:39,82 — headrow is left:50% + xPercent:-50, so the row
            stays CENTERED while the headline types in (and the bare caret blinks
            at x≈960); full-width justify-center reproduces that per-frame
            recentering, a fixed left anchor would pin the final x during typing */}
        <div id="cta-headrow" class="absolute left-0 right-0 top-[356.4px] flex items-baseline justify-center whitespace-nowrap">
          <p id="cta-h1" class="text-[96px] font-[hk7] font-bold tracking-[-2.88px] leading-none text-[#1D1F1F] whitespace-nowrap relative -top-[2px]"></p>
          <p id="cta-h2" class="text-[96px] font-[hk7] font-bold tracking-[-2.88px] leading-none text-[#D97757] whitespace-nowrap relative -top-[2px]"></p>
          <p id="cta-h3" class="text-[96px] font-[hk7] font-bold tracking-[-2.88px] leading-none text-[#1D1F1F] whitespace-nowrap relative -top-[2px]"></p>
          {/* ref renders ▏ through a fallback font (latin subset lacks U+258F)
              whose advance is ~84px @96 — cross-checked twice: unscaled ref
              ink starts at x=368 and the f821 mid-zoom ink at 308. Round-6
              ink scan on the composed frame: ref headline spans 369..1481
              (center 925), test 364..1476 — same 1112px ink width, sitting
              5px left, so the caret placeholder is 10px too wide: 74px puts
              the 1192px row box at x=364 and the ink at 369=ref. */}
          <p id="cta-hcaret" class="text-[96px] font-[hk4] leading-none text-[#D97757] ml-[0.77px] w-[74px] relative -top-[2px]">▏</p>
        </div>
      </div>
      {/* ref cta.html:43-26 composer spans y 646..875 (bottom 205.2). Flow
          stacking renders the inputbar ~10px low (f854: ref 706..823 vs test
          716..833) while the metarow lands true, so pin all three rows
          absolutely: chips top-0 (646), inputbar top-61 (707), metarow
          bottom-0 (841.24..875) — ref cta.html:44-54 geometry. */}
      <div id="cta-composer" class="absolute left-[364.8px] bottom-[205.2px] w-[1190.4px] h-[229px] opacity-0">
        <div class="absolute left-0 top-0 flex gap-[11.52px]">
          <div class="bg-[#2C2C2B] rounded-[11.52px] px-[17.3px] h-[44px] flex items-center"><p class="text-[19.2px] font-[hk5] font-medium text-[#D7D4CA] leading-none">Local</p></div>
          <div class="bg-[#2C2C2B] rounded-[11.52px] px-[17.3px] h-[44px] flex items-center"><p class="text-[19.2px] font-[hk5] font-medium text-[#D7D4CA] leading-none">hyperframes</p></div>
        </div>
        <div class="absolute left-0 right-0 top-[61px]">
        <div class="relative flex items-center bg-[#2C2C2B] border-[1px] border-[#3a3a38] rounded-[19.2px] min-h-[115.28px] px-[26.9px] shadow-[0_0_0_2.5px_rgba(217,119,87,0.55)]">
          {/* 1-D cross-correlation f856/f880: test text band sits 6px HIGH at
              -3px offset → net +3px from unshifted; ref .url pads 0 .12cqw
              (2.304px) so the box and trailing caret sit 2.3px right. */}
          <div class="relative top-[3px] flex-1 flex items-center pr-[53.8px]">
            <p id="cta-cmd" class="text-[24.96px] font-[mono4] text-[#E6E2D6] whitespace-nowrap"></p>
            <p id="cta-url" class="text-[24.96px] font-[mono4] text-[#9CC4E6] whitespace-nowrap px-[2.3px] py-[5.6px]"></p>
            <p id="cta-caret" class="text-[32.64px] text-[#D97757]">▏</p>
            <p id="cta-ph" class="ml-[-16px] text-[25.92px] text-[#6F6E66]">Reply to Claude…</p>
          </div>
          <path id="cta-ret" d="M9 10 4 15 9 20 M20 4v7a4 4 0 0 1-4 4H4" view-box="0 0 24 24" class="absolute right-[23px] w-[17px] h-[17px] stroke-[2.6] stroke-[#6F6E66] fill-none" />
        </div>
        </div>
        {/* ref metarow is 33.76px tall (875-18.24-823); absolute bottom-0 pins
            it against composer bottom — flow margin-top rendered ~7px gap
            (f854 diff band 824-835) */}
        <div class="absolute left-0 right-0 bottom-0 flex justify-between items-center h-[33.76px]">
          <div class="flex items-center gap-[21.12px]">
            <div class="bg-[rgba(202,164,62,0.16)] rounded-[9.6px] px-[14.4px] py-[4.6px]"><p class="text-[18.4px] font-[hk5] font-medium text-[#CAA43E]">Auto</p></div>
            <p class="text-[22.1px] text-[#8C8B83]">＋</p>
          </div>
          <div class="flex items-center gap-[21.12px]">
            <p class="text-[20.16px] text-[#7C7B73] font-[hk5] font-medium">Opus 4.8</p>
            <p class="text-[20.16px] text-[#9A998F] font-[hk5] font-medium">High</p>
            <div id="cta-orb" class="w-[25px] h-[25px] rounded-full border-[3.46px] border-[rgba(96,148,214,0.26)]"></div>
          </div>
        </div>
      </div>
    </div>
  );
}

// ═════════════════ ROOT — ten z-stacked scenes, cross-faded by the root tl ═════════════════
export default function PrToVideoLaunch() {
  // Caller-owned ids: the scene components mount them via props, the root tl
  // orchestrates the cross-fade windows through this table.
  var OPENER = 's-opener', PRSCROLL = 's-prscroll', HARD = 's-hard', SUMMARIZE = 's-summarize',
      USESKILL = 's-useskill', CTA = 's-cta';
  var FC = { scene: 's-fcode', stage: 'auto220', pfill: 'fc-pfill', pknob: 'fc-pknob' };
  var FF = { scene: 's-fflow', stageA: 'auto327', stageB: 'auto328', pfill: 'ff-pfill', pknob: 'ff-pknob' };
  var FD = { scene: 's-fdisc', stage: 'auto374', pfill: 'fd-pfill', pknob: 'fd-pknob' };
  var CR = { scene: 's-fcredits', stage: 'auto421', path: 'cr-path', pfill: 'cr-pfill', pknob: 'cr-pknob' };
  useGSAP(() => {
    const T = gsap.timeline();
    T.set([PRSCROLL, HARD, SUMMARIZE, USESKILL, FC.scene, FF.scene, FD.scene, CR.scene, CTA], { opacity: 0 }, 0);
    // Reference unmounts the outgoing scene instantly at its window end, so
    // the light #F0EFE9 root shows through during the incoming fade-in.
    const xfade = (a, b, t, d) => {
      T.set(a, { opacity: 0 }, t);
      T.fromTo(b, { opacity: 0 }, { opacity: 1, duration: d, ease: 'power1.inOut' }, t);
    };
    T.to(OPENER, { scale: 1.6, opacity: 0, filter: 'blur(8px)', duration: 0.4, ease: 'power2.in', transformOrigin: '66% 59%' }, 4.8);
    T.set(OPENER, { opacity: 0 }, 5.22);
    T.set(PRSCROLL, { opacity: 1 }, 5.2);
    T.fromTo(PRSCROLL, { opacity: 0 }, { opacity: 1, duration: 0.3, ease: 'power2.out' }, 5.15);
    xfade(PRSCROLL, HARD, 8.4, 0.28);
    xfade(HARD, SUMMARIZE, 9.3, 0.28);
    xfade(SUMMARIZE, USESKILL, 12.3, 0.28);
    xfade(USESKILL, FC.scene, 16.1, 0.35);
    xfade(FC.scene, FF.scene, 18.5, 0.3);
    xfade(FF.scene, FD.scene, 21.0, 0.3);
    xfade(FD.scene, CR.scene, 23.0, 0.3);
    xfade(CR.scene, CTA, 25.6, 0.35);
    // pknob left: ref is % of ptrack; ptrack = 1228.8 − 2×28.8 − 28.8 − 2×23 −
    // 172.8 = 923.6px. Engine inset animation is px-only → % × 923.6.
    T.set(FC.pfill, { scaleX: 0.05 }, 0); T.set(FC.pknob, { left: (5 / 100) * 923.6 }, 0);
    T.set(FF.pfill, { scaleX: 0.22 }, 0); T.set(FF.pknob, { left: (22 / 100) * 923.6 }, 0);
    T.set(FD.pfill, { scaleX: 0.66 }, 0); T.set(FD.pknob, { left: (66 / 100) * 923.6 }, 0);
    T.set(CR.pfill, { scaleX: 0.86 }, 0); T.set(CR.pknob, { left: (86 / 100) * 923.6 }, 0);
    // FIX: seed the credits path draw-on (the xml .to had no scaleX seed)
    T.set(CR.path, { scaleX: 0 }, 0);
    // inner 1920×1080 stages → 1171.2×658.56 stage = scale .61; flow row .84
    T.set(FC.stage, { scale: 0.61 }, 0);
    T.set(FF.stageA, { scale: 0.61 }, 0);
    T.set(FF.stageB, { scale: 0.84 }, 0);
    T.set(FD.stage, { scale: 0.61 }, 0);
    T.set(CR.stage, { scale: 0.61 }, 0);
  });
  return (
    <div id="root" class="relative w-[1920px] h-[1080px] bg-[#F0EFE9] overflow-hidden font-[hk4]">
      <Opener rootId={OPENER} />
      <PrScroll rootId={PRSCROLL} />
      <HardRead rootId={HARD} />
      <Summarize rootId={SUMMARIZE} />
      <UseSkill rootId={USESKILL} />
      <FeatureCode ids={FC} />
      <FeatureFlow ids={FF} />
      <FeatureDiscussion ids={FD} />
      <FeatureCredits ids={CR} />
      <CtaOutro rootId={CTA} />
    </div>
  );
}
