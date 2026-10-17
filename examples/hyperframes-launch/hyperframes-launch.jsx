/* JSX frontend port of /Users/solaren/Projects/hyperframes-launches/hyperframes-launch/index.html
   (16 sub-compositions on a 49.77s master) — one continuous 1920x1080 stage.
   Structure mirrors the reference: per-composition components, each with one
   useGSAP hook (hook order = tree order = master timeline order). Every scene
   sits in a `<tl>`, so a scene's times are its OWN: `ctx.current.time` starts
   at 0 where the scene becomes visible and the scene tree is culled outside
   its window. Element reveals the scene window does not cover (the glass-intro
   typewriter, the caption chips) stay per-frame `vis()` gates.
   Engine substitutions (web fonts -> faces, CSS 3D -> projected canvas,
   WebGL shader -> ly.pixel lambda, Canvas2D ASCII -> canvas 2d, conic
   gradients -> canvas, backgroundPosition sweeps -> crossfading layers) are
   annotated inline with the reference line they replace. */

import { useGSAP, useThree, useRef, useCanvas } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 49.8 };

/* Reference loads Google web fonts (css2 API). Engine fonts are ttf/otf, so
   each face maps to the closest-metric system font:
   Instrument Serif is bundled (assets/fonts/InstrumentSerif-*.ttf);
   EB Garamond/Cormorant -> Times New Roman (+Italic),
   Geist/Space Grotesk -> Segoe UI (+Bold), Azeret Mono -> Consolas,
   Bebas Neue -> Impact, Inter 900 -> Arial Black; Inter/JetBrains Mono are real.
   `family` MUST be the ttf's own family name (fontdb reads the file, so a
   mismatched CSS name silently falls back to the default sans). */
export const fonts = {
  default: 'hf-inter',
  faces: [
    { id: 'hf-inter', family: 'Inter', path: 'assets/fonts/Inter-Regular.ttf' },
    { id: 'hf-inter-med', family: 'Inter', path: 'assets/fonts/Inter-Medium.ttf' },
    { id: 'hf-inter-black', family: 'Arial Black', path: 'assets/fonts/ArialBlack.ttf' },
    { id: 'hf-jbm', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Regular.ttf' },
    { id: 'hf-jbm-bold', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Bold.ttf' },
    { id: 'hf-serif', family: 'Instrument Serif', path: 'assets/fonts/InstrumentSerif-Regular.ttf' },
    { id: 'hf-serif-it', family: 'Instrument Serif', path: 'assets/fonts/InstrumentSerif-Italic.ttf' },
    { id: 'hf-eb-it', family: 'Times New Roman', path: 'assets/fonts/TimesNewRoman-Italic.ttf' },
    { id: 'hf-cg-it', family: 'Times New Roman', path: 'assets/fonts/TimesNewRoman-Italic.ttf' },
    { id: 'hf-geist', family: 'Segoe UI', path: 'assets/fonts/SegoeUI.ttf' },
    { id: 'hf-geist-semi', family: 'Segoe UI', path: 'assets/fonts/SegoeUI-Bold.ttf' },
    { id: 'hf-grotesk', family: 'Segoe UI', path: 'assets/fonts/SegoeUI.ttf' },
    { id: 'hf-grotesk-bold', family: 'Segoe UI', path: 'assets/fonts/SegoeUI-Bold.ttf' },
    { id: 'hf-azeret', family: 'Consolas', path: 'assets/fonts/Consolas.ttf' },
    { id: 'hf-bebas', family: 'Impact', path: 'assets/fonts/Impact.ttf' },
  ],
};

/* ══ shared per-frame helpers ═══════════════════════════════════════════ */
function vis(ctx, id, on) { ctx.getNode(id).opacity(on ? 1 : 0); }
function clamp(v, a, b) { return Math.max(a, Math.min(b, v)); }
/* Zoom containers: the reference scales each beat's card/wrap/player about
   the canvas centre (glass-intro.html:100,122-124,159 transform-origins all
   resolve to (960,540)). A full-stage wrapper keeps that origin while inner
   elements stay static — identical geometry, one tween instead of three. */

/* glass-intro.html:1011-1025 -> one bar per beat (the reference re-stages a
   single #player-bar; three copies keep the readouts/fills independent).
   Beat-2/3: left 210 w 1500 (track 1332px); beat-4: left 326.5 w 1267
   (track 1099px, glass-intro.html:722-728). */
function PlayerBar({ id, left, w, track, time0 }) {
  return (
    <div id={id} class="absolute top-[930px] h-[28px] flex items-center gap-[18px] opacity-0" style={'left:' + left + 'px;width:' + w + 'px'}>
      <div id={id + '-icon'} class="relative w-[22px] h-[22px] shrink-0">
        <path view-box="0 0 22 22" d="M5 3.5 L18 11 L5 18.5 Z" class="absolute left-0 top-0 w-[22px] h-[22px] fill-[rgba(255,255,255,0.92)]" />
      </div>
      <div id={id + '-track'} class="relative h-[3px] rounded-[2px] bg-[rgba(255,255,255,0.28)]" style={'width:' + track + 'px'}>
        <div id={id + '-fill'} class="absolute left-0 top-0 w-0 h-full rounded-[2px] bg-white"></div>
        <div id={id + '-scrub'} class="absolute left-0 top-[-4.5px] w-[12px] h-[12px] rounded-full bg-white shadow-[0_2px_6px_rgba(0,0,0,0.25)]"></div>
      </div>
      <text id={id + '-time'} class="w-[110px] text-right text-[20px] leading-none tracking-[0.02em] opacity-[0.85] font-[hf-inter-med] text-[rgba(255,255,255,0.92)]">{time0}</text>
    </div>
  );
}

/* entryEase (glass-intro.html:464) — quadratic matched-velocity ease with a
   non-standard shape, so the reference hands GSAP the function itself. */
function entryEase(t) { return -0.806 * t * t + 1.806 * t; }

/* Blur+scale stay on the beat wrapper, matching what the shipped r5 render
   measured best. Splitting the blur onto the card (#frame) and the video wrap
   separately (the reference's own structure) regressed beat 2-3 by ~0.09
   avg_p8: the card's own backdrop-filter already blurs the stage behind it,
   so an extra ancestor blur double-counts. zoomBlur keeps the call sites
   readable. */
function zoomBlur(tl, ids, from, to, duration, ease, at) {
  for (var i = 0; i < ids.length; i++) {
    tl.fromTo(ids[i], { filter: from }, { filter: to, duration: duration, ease: ease }, at);
  }
}

/* ══ glass-intro (master 0-18.24, glass-intro.html) ═════════════════════

   The reference is one flat 18.24s stage: 4 beats that tile end to end, plus
   4 `.clip` overlay layers.

     beat         master window     length   reference layer
     text-intro   0.00 -  2.10       2.10   .clip data-start=0     duration=2.1
     figma card   2.10 -  3.85       1.75   GSAP opacity on #frame
     spotify      3.85 -  5.60       1.75   GSAP opacity on #frame-2
     beat4        5.60 - 18.24      12.64   GSAP opacity on #frame-3
     cursor       6.40 - 17.30              .clip data-start=6.4   duration=10.9
     ripple       6.80 - 15.70              .clip data-start=6.8   duration=8.9
     drag-overlay 16.14 - 18.24             .clip data-start=16.14 duration=2.1
     text-ask     16.74 - 18.24             .clip data-start=16.74 duration=1.5

   Every overlay window starts at or after 5.60, so all four live INSIDE beat4.
   The whole composition is therefore exactly 4 scenes and no transitions — the
   reference hard-cuts at 2.10 / 3.85 / 5.60, and adjacent scenes without a
   `<transition>` cut straight from one to the next.

   What this buys: a beat is no longer parsed, laid out and then hidden with a
   per-frame `vis()` gate. Only the beat whose window holds the playhead is
   resolved at all. The `vis()` calls that remain are the beat-5 typewriter,
   which is an intra-scene reveal, not a scene window.

   NOTE on `data-start`: inside a scene a video's `data-start` is SCENE-RELATIVE
   (the resolver adds the scene's own start to it), so every video reads 0 — and
   the times below are beat-local, not master-absolute. */
function Beat1() {
  useGSAP((ctx) => {
    var tl = ctx.timeline();
    /* BEAT 1 (:505-526): per-word slide-from-right, expo.out 0.33s */
    var WS = [0.09, 0.42, 0.55, 0.85, 1.09, 1.40, 1.64];
    var WSL = [360, 180, 120, 80, 50, 30, 20];
    for (var w = 0; w < 7; w++) {
      tl.fromTo('gi-w' + w, { x: WSL[w], y: 14, opacity: 0 }, { x: 0, y: 0, opacity: 1, duration: 0.33, ease: 'expo.out' }, WS[w]);
    }
    tl.fromTo('gi-intro-text', { scale: 1, filter: 'blur(0px)' }, { scale: 0.6, filter: 'blur(16px)', duration: 0.2, ease: 'expo.in' }, 1.9);
  });
  return (
    <div id="gi-text-intro" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[radial-gradient(ellipse_at_50%_45%,#fcf6e7_0%,#f6edd4_100%)]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_55%,transparent_45%,rgba(110,70,15,0.08)_100%)]"></div>
      <div id="gi-intro-mask" class="absolute left-0 top-0 w-[1920px] h-[1080px] overflow-hidden">
        <div class="absolute left-0 top-0 w-[1720px] h-full overflow-hidden">
          <div id="gi-intro-text" class="absolute left-0 top-0 w-[1920px] h-full flex items-center justify-center px-[40px]">
            <text id="gi-w0" class="text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">Imagine</text>
            <text id="gi-w1" class="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">you</text>
            <text id="gi-w2" class="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">could</text>
            <text id="gi-w3" class="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">make</text>
            <text id="gi-w4" class="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">videos</text>
            <text id="gi-w5" class="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">like</text>
            <text id="gi-w6" class="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">this</text>
          </div>
        </div>
      </div>
    </div>
  );
}

/* BEAT 2 (:430,530-622): entry 1.45->0.88 entryEase 0.9s, hold ->0.80 blur2
   linear 0.6333s, exit ->0.40 blur16 expo.in 0.2s. Master 2.10-3.85. */
function Beat2() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline();
    var B = ['gi-zoom-b2'];

    tl.set('gi-zoom-b2', { scale: 1.45 }, 0);
    tl.set('gi-bar2', { opacity: 0, y: 12 }, 0);

    tl.fromTo('gi-zoom-b2', { scale: 1.45 }, { scale: 0.88, duration: 0.9, ease: entryEase }, 0);
    zoomBlur(tl, B, 'blur(14px)', 'blur(0px)', 0.9, entryEase, 0);
    tl.fromTo('gi-zoom-b2', { scale: 0.88 }, { scale: 0.80, duration: 0.6333, ease: 'none' }, 0.9);
    zoomBlur(tl, B, 'blur(0px)', 'blur(2px)', 0.6333, 'none', 0.9);
    zoomBlur(tl, ['gi-bar2'], 'blur(0px)', 'blur(2px)', 0.6333, 'none', 0.9);
    tl.fromTo('gi-bar2', { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 0.5);
    tl.to('gi-bar2-fill', { width: 1332, duration: 1.75, ease: 'none' }, 0);
    tl.to('gi-bar2-scrub', { x: 1332, duration: 1.75, ease: 'none' }, 0);
    tl.fromTo('gi-zoom-b2', { scale: 0.80 }, { scale: 0.40, duration: 0.2, ease: 'expo.in' }, 1.5333);
    zoomBlur(tl, B, 'blur(2px)', 'blur(16px)', 0.2, 'expo.in', 1.5333);
    zoomBlur(tl, ['gi-bar2'], 'blur(2px)', 'blur(16px)', 0.2, 'expo.in', 1.5333);

    /* time readout — reference drives it with an onUpdate proxy
       (:583-598): sec = floor(progress). */
    if (T >= 0 && T < 1.75) ctx.getNode('gi-bar2-time').text('0:0' + Math.min(2, Math.floor(T)) + ' / 0:02');
  });
  return (
    <div id="gi-beat2" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
      <video id="gi-bg-video" path="assets/bg.mp4" data-start="0" data-duration="1.75" class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover" />
      <div id="gi-zoom-b2" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
        <div id="gi-frame" class="absolute left-[120px] top-[67.5px] w-[1680px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.16)] backdrop-blur-[14px] shadow-[0_30px_90px_rgba(0,0,0,0.42)] inset-shadow-[0_1px_0_rgba(255,255,255,0.22)] bg-[linear-gradient(160deg,rgba(255,255,255,0.075)_0%,rgba(255,255,255,0.025)_45%,rgba(255,255,255,0.010)_70%,rgba(255,255,255,0.055)_100%)]"></div>
        <div id="gi-figma-wrap" class="absolute left-[170px] top-[117.5px] w-[1580px] h-[758px] rounded-[40px] overflow-hidden shadow-[0_10px_28px_rgba(0,0,0,0.35)]">
          <video id="gi-figma-video" path="assets/figma-promo.mp4" data-start="0" data-duration="1.75" data-media-start="0" class="w-full h-full object-cover" />
        </div>
        <PlayerBar id="gi-bar2" left={210} w={1500} track={1332} time0="0:00 / 0:02" />
      </div>
    </div>
  );
}

/* BEAT 3 (:432,632-716): same shape @master 3.85, hold 0.6157s, bar @0.5001. */
function Beat3() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline();
    var B = ['gi-zoom-b3'];

    tl.set('gi-zoom-b3', { scale: 1.45 }, 0);
    tl.set('gi-bar3', { opacity: 0, y: 12 }, 0);

    tl.fromTo('gi-zoom-b3', { scale: 1.45 }, { scale: 0.88, duration: 0.9, ease: entryEase }, 0);
    zoomBlur(tl, B, 'blur(14px)', 'blur(0px)', 0.9, entryEase, 0);
    tl.fromTo('gi-zoom-b3', { scale: 0.88 }, { scale: 0.80, duration: 0.6157, ease: 'none' }, 0.9001);
    zoomBlur(tl, B, 'blur(0px)', 'blur(2px)', 0.6157, 'none', 0.9001);
    zoomBlur(tl, ['gi-bar3'], 'blur(0px)', 'blur(2px)', 0.6157, 'none', 0.9001);
    tl.fromTo('gi-bar3', { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 0.5001);
    tl.to('gi-bar3-fill', { width: 1332, duration: 1.75, ease: 'none' }, 0);
    tl.to('gi-bar3-scrub', { x: 1332, duration: 1.75, ease: 'none' }, 0);
    tl.fromTo('gi-zoom-b3', { scale: 0.80 }, { scale: 0.40, duration: 0.2, ease: 'expo.in' }, 1.5167);
    zoomBlur(tl, B, 'blur(2px)', 'blur(16px)', 0.2, 'expo.in', 1.5167);
    zoomBlur(tl, ['gi-bar3'], 'blur(2px)', 'blur(16px)', 0.2, 'expo.in', 1.5167);

    if (T >= 0 && T < 1.75) ctx.getNode('gi-bar3-time').text('0:0' + Math.min(2, Math.floor(T)) + ' / 0:02');
  });
  return (
    <div id="gi-beat3" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
      <video id="gi-bg-video2" path="assets/bg-2.mp4" data-start="0" data-duration="1.75" class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover" />
      <div id="gi-zoom-b3" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
        <div id="gi-frame2" class="absolute left-[120px] top-[67.5px] w-[1680px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.16)] backdrop-blur-[14px] shadow-[0_30px_90px_rgba(0,0,0,0.42)] inset-shadow-[0_1px_0_rgba(255,255,255,0.22)] bg-[linear-gradient(160deg,rgba(255,255,255,0.075)_0%,rgba(255,255,255,0.025)_45%,rgba(255,255,255,0.010)_70%,rgba(255,255,255,0.055)_100%)]"></div>
        <div id="gi-spotify-wrap" class="absolute left-[170px] top-[117.5px] w-[1580px] h-[758px] rounded-[40px] overflow-hidden shadow-[0_10px_28px_rgba(0,0,0,0.35)]">
          <video id="gi-spotify-video" path="assets/spotify-bento.mp4" data-start="0" data-duration="1.75" data-media-start="0" class="w-full h-full object-cover" />
        </div>
        <PlayerBar id="gi-bar3" left={210} w={1500} track={1332} time0="0:00 / 0:02" />
      </div>
    </div>
  );
}

/* BEAT 4 (:434,731-877,913-942): master 5.60-18.24. Poster -> clip ->
   lastframe -> logo, the cursor-click full-frame scale-up, the drag wipe and
   the typewriter. All four reference overlay layers open inside this window,
   so they are siblings of the zoom wrapper — NOT inside it: the wrapper scales
   1.45->1 and 1->1.4248, and the reference moves the cursor in canvas space. */
function Beat4() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline();
    var B = ['gi-zoom-b4'];

    /* ── master 5.60-7.10: card entry, logo, cursor slide, click ripple ── */
    tl.set('gi-zoom-b4', { scale: 1.45 }, 0);
    tl.set('gi-bar4', { opacity: 0, y: 12 }, 0);
    tl.set('gi-cursor', { x: 100, y: 1180, scale: 1, opacity: 0 }, 0);
    tl.set('gi-cursor', { opacity: 1 }, 0.8);
    tl.set('gi-ripple', { x: 307.5, y: 914, scale: 0, opacity: 0 }, 0);
    tl.set('gi-drag-overlay', { x: 1920, opacity: 0 }, 0);
    tl.set('gi-text-ask', { opacity: 0 }, 0);
    tl.set('gi-beat4-logo', { opacity: 1 }, 0);

    tl.fromTo('gi-zoom-b4', { scale: 1.45 }, { scale: 1, duration: 0.9, ease: 'expo.out' }, 0);
    zoomBlur(tl, B, 'blur(14px)', 'blur(0px)', 0.9, 'expo.out', 0);
    tl.fromTo('gi-bar4', { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 0.5);
    tl.set('gi-beat4-poster', { opacity: 0 }, 1.5);
    tl.fromTo('gi-beat4-logo', { opacity: 1 }, { opacity: 0, duration: 0.15, ease: 'power2.in' }, 1.4);

    tl.fromTo('gi-cursor', { x: 100, y: 1180 }, { x: 337.5, y: 944, duration: 0.7, ease: 'power3.out' }, 0.5);
    tl.fromTo('gi-cursor', { scale: 1 }, { scale: 0.82, duration: 0.07, ease: 'power4.in' }, 1.2);
    tl.fromTo('gi-bar4-icon', { scale: 1 }, { scale: 0.96, duration: 0.07, ease: 'power4.in' }, 1.2);
    tl.fromTo('gi-ripple', { scale: 0, opacity: 0.9 }, { scale: 2.5, opacity: 0, duration: 0.2, ease: 'power2.out' }, 1.2);
    tl.fromTo('gi-cursor', { scale: 0.82 }, { scale: 1, duration: 0.2999, ease: 'back.out(3)' }, 1.2701);
    tl.fromTo('gi-bar4-icon', { scale: 0.96 }, { scale: 1.02, duration: 0.1999, ease: 'elastic.out(1, 0.4)' }, 1.2701);
    tl.fromTo('gi-bar4-icon', { scale: 1.02 }, { scale: 1, duration: 0.2, ease: 'power2.out' }, 1.47);
    tl.fromTo('gi-cursor', { opacity: 1 }, { opacity: 0, duration: 0.05, ease: 'none' }, 1.55);

    /* ── master 7.20: full-frame scale-up (wrapper origin IS the centre) ── */
    tl.fromTo('gi-zoom-b4', { scale: 1, y: 0 }, { scale: 1.4248, y: 62, duration: 0.4, ease: 'power3.out' }, 1.6);
    tl.fromTo('gi-beat4-wrap', { borderRadius: 40 }, { borderRadius: 0, duration: 0.4, ease: 'power3.out' }, 1.6);
    tl.set('gi-bar4', { opacity: 0 }, 1.6);
    tl.set('gi-beat4-lastframe', { opacity: 1 }, 10.44);

    /* ── master 15.74-17.30: cursor returns, click, drag wipe, typewriter ── */
    tl.set('gi-cursor', { x: 100, y: 1180, scale: 1, opacity: 1, rotation: 90 }, 10.14);
    tl.fromTo('gi-cursor', { x: 100, y: 1180 }, { x: 1920, y: 540, duration: 0.4, ease: 'power3.out' }, 10.14);
    tl.fromTo('gi-cursor', { scale: 1 }, { scale: 0.82, duration: 0.07, ease: 'power4.in' }, 10.54);
    tl.fromTo('gi-ripple', { scale: 0, opacity: 0.9 }, { scale: 2.5, opacity: 0, duration: 0.2, ease: 'power2.out' }, 10.54);
    tl.fromTo('gi-cursor', { scale: 0.82 }, { scale: 1, duration: 0.1, ease: 'back.out(2)' }, 10.547);
    tl.fromTo('gi-cursor', { x: 1920, y: 540 }, { x: -50, y: 540, duration: 0.6, ease: 'power2.inOut' }, 10.54);
    tl.fromTo('gi-cursor', { opacity: 1 }, { opacity: 0, duration: 0.05, ease: 'none' }, 11.09);
    tl.set('gi-drag-overlay', { opacity: 1 }, 10.54);
    tl.fromTo('gi-drag-overlay', { x: 1920 }, { x: 0, duration: 0.6, ease: 'power2.inOut' }, 10.54);
    tl.set('gi-text-ask', { opacity: 1 }, 11.14);

    /* z-collapse exit of overlay + ask (:861-866) */
    tl.fromTo('gi-drag-overlay', { scale: 1, filter: 'blur(0px)' }, { scale: 0.6, filter: 'blur(16px)', duration: 0.3, ease: 'expo.in' }, 12.34);
    tl.fromTo('gi-text-ask', { scale: 1, filter: 'blur(0px)' }, { scale: 0.6, filter: 'blur(16px)', duration: 0.3, ease: 'expo.in' }, 12.34);

    /* root zoom-through as flex-css emerges (:997-1004) */
    tl.fromTo('gi-root', { scale: 1, opacity: 1, filter: 'blur(0px)' }, { scale: 1.15, opacity: 0, filter: 'blur(4px)', duration: 0.6, ease: 'power2.in' }, 11.7);

    /* ── beat-5 typewriter (:964-993): "Ask your agent to add", 21 chars,
       JetBrains Mono 72px -> advance 0.6em = 43.2px/char, line box centred
       at y=500.4 (79.2px). Cursor leads the next char by 15px. An intra-scene
       reveal, so it stays a per-frame gate — the window does not cover it. ── */
    var T0 = 11.04, CD = 0.028, CW = 43.2;
    var typed = clamp(Math.floor((T - T0) / CD) + 1, 0, 21);
    for (var ci = 0; ci < 21; ci++) vis(ctx, 'gi-ask-ch' + ci, T >= T0 + ci * CD);
    var cOn;
    if (T < 10.94 || (T >= 10.99 && T < 11.04)) cOn = false;
    else {
      var TEND = T0 + 21 * CD; /* 11.628 */
      cOn = T >= TEND ? ((T - TEND) % 0.8) < 0.4 : true;
    }
    var ac = ctx.getNode('gi-ask-cursor');
    ac.opacity(cOn ? 1 : 0);
    ac.left(typed * CW + 15);
  });

  var ASK = 'Ask your agent to add';
  return (
    <div id="gi-beat4" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
      {/* anatomy cream bg (:883-889) */}
      <div id="gi-bg-anat" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#fbf4e3]">
        <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_45%,#fcf6e7_0%,#f6edd4_100%)]"></div>
        <div class="absolute left-[-20px] top-[-20px] w-[1960px] h-[1120px] bg-[linear-gradient(to_right,rgba(90,60,10,0.07)_1px,transparent_1px),linear-gradient(to_bottom,rgba(90,60,10,0.07)_1px,transparent_1px)] bg-[length:40px_40px]"></div>
        <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_55%,transparent_40%,rgba(110,70,15,0.09)_100%)]"></div>
      </div>

      <div id="gi-zoom-b4" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
        <div id="gi-frame3" class="absolute left-[236.5px] top-[67.5px] w-[1447px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.16)] backdrop-blur-[14px] shadow-[0_30px_90px_rgba(0,0,0,0.42)] inset-shadow-[0_1px_0_rgba(255,255,255,0.22)] bg-[linear-gradient(160deg,rgba(255,255,255,0.075)_0%,rgba(255,255,255,0.025)_45%,rgba(255,255,255,0.010)_70%,rgba(255,255,255,0.055)_100%)]"></div>
        <div id="gi-beat4-wrap" class="absolute left-[286.5px] top-[117.5px] w-[1347px] h-[758px] rounded-[40px] overflow-hidden shadow-[0_10px_28px_rgba(0,0,0,0.35)]">
          <image id="gi-beat4-poster" path="assets/beat4-poster.jpg" class="absolute left-0 top-0 w-full h-full object-cover" />
          <video id="gi-beat4-video" path="assets/beat4-clip.mp4" data-start="1.5" data-duration="11.14" data-media-start="0" class="w-full h-full object-cover" />
          <image id="gi-beat4-lastframe" path="assets/beat4-lastframe.jpg" class="absolute left-0 top-0 w-full h-full object-cover opacity-0" />
          {/* hf-logo.svg (viewBox 600x400) at width 80% -> hf-logo-light.png
             (263x79) at the same box: left (1347-1077.6)/2, h = 1077.6*79/263 */}
          <image id="gi-beat4-logo" path="assets/hf-logo-light.png" class="absolute left-[134.7px] top-[217.2px] w-[1077.6px] h-[323.5px] object-contain opacity-0" />
        </div>
        <PlayerBar id="gi-bar4" left={326.5} w={1267} track={1099} time0="0:00 / 0:08" />
      </div>

      {/* overlay layers (:913,:920,:923,:926) — canvas space, outside the zoom */}
      <div id="gi-cursor" class="absolute left-0 top-0 w-[48px] h-[58px] [transform-origin:4.3px_2.6px] opacity-0">
        <path view-box="0 0 28 34" d="M2.5 1.5L2.5 28L8.5 21.5L14 32L18.5 30L13 19.5L22 19.5L2.5 1.5Z" class="absolute left-0 top-0 w-[48px] h-[58px] fill-[#000000] stroke-[#ffffff] stroke-[2]" />
      </div>
      <div id="gi-ripple" class="absolute left-0 top-0 w-[60px] h-[60px] rounded-full bg-[rgba(255,255,255,0.9)] opacity-0"></div>
      <div id="gi-drag-overlay" class="absolute left-0 top-0 w-[1920px] h-[1080px] opacity-0">
        <div class="w-full h-full bg-[#faf5ea]"></div>
      </div>
      <div id="gi-text-ask" class="absolute left-0 top-0 w-[1920px] h-[1080px] opacity-0">
        <div class="absolute left-0 top-0 w-[1920px] h-[1080px] flex items-center justify-center">
          <div class="relative h-[79.2px]">
            {ASK.split('').map((ch, i) => (
              <text id={"gi-ask-ch" + i} class="absolute top-0 text-[72px] leading-[79.2px] tracking-[-0.01em] font-[hf-jbm] text-[#14110a] opacity-0" style={"left:" + (i * 43.2) + "px"}>{ch === ' ' ? ' ' : ch}</text>
            ))}
            <div id="gi-ask-cursor" class="absolute top-[5.4px] w-[4px] h-[61.2px] bg-[#14110a] opacity-0"></div>
          </div>
        </div>
      </div>
    </div>
  );
}

function GlassIntro() {
  return (
    <div id="gi-root" class="absolute inset-0 w-[1920px] h-[1080px]">
      <tl id="gi-tl" class="absolute inset-0">
        <Beat1 duration={2.1} />
        <Beat2 duration={1.75} />
        <Beat3 duration={1.75} />
        <Beat4 duration={12.64} />
      </tl>
    </div>
  );
}

/* ══ captions (index.html:242-275) — 14 inline body-level overlays ══════ */
var CAPS = [
  ['cap0', 7.29, 9.15, 'HyperFrames by HeyGen.'],
  ['cap1', 9.10, 12.54, 'Agents can now write HTML and render MP4s.'],
  ['cap2', 12.73, 15.21, 'Claude Code made this launch video.'],
  ['cap3', 15.28, 16.58, 'Your agent can too.'],
  ['cap4', 18.20, 19.30, 'CSS animations'],
  ['cap5', 19.41, 20.34, 'GSAP'],
  ['cap6', 20.44, 21.34, 'Shaders'],
  ['cap7', 29.42, 30.60, 'composes together'],
  ['cap8', 32.74, 34.00, 'Your agent writes code to compose'],
  ['cap9', 34.14, 38.00, 'Hyperframes renders every frame to MP4.'],
  ['cap10', 38.16, 40.46, "It's deterministic pixel-perfect every time"],
  ['cap11', 40.60, 42.31, 'Give your agent the skill.'],
  ['cap12', 42.31, 43.91, 'Tell it what to make,'],
  ['cap13', 43.91, 44.98, 'watch it build'],
];

function CaptionLayer() {
  useGSAP((ctx) => {
    var T = ctx.global.time;
    for (var i = 0; i < CAPS.length; i++) vis(ctx, CAPS[i][0], T >= CAPS[i][1] && T < CAPS[i][2]);
  });
  return (
    <div class="absolute left-0 top-0 w-[1920px] h-[1080px] z-[50]">
      {CAPS.map((c) => (
        <div id={c[0]} class="absolute left-0 bottom-[80px] w-full flex justify-center opacity-0">
          <div class="px-[28px] py-[12px] rounded-[10px] bg-[rgba(10,8,5,0.55)] backdrop-blur-[8px] whitespace-nowrap">
            <text class="text-[40px] leading-[48px] tracking-[-0.005em] font-[500] font-[hf-inter-med] text-[rgba(240,235,220,0.92)]">{c[3]}</text>
          </div>
        </div>
      ))}
    </div>
  );
}

/* ══ flex-css (master 17.30-19.01, compositions/flex-css.html) ══════════ */
var CSS_START = 17.30, CSS_DUR = 1.71;
var CSS_ORDER = [0, 1, 2, 3, 4, 5, 11, 17, 16, 15, 14, 13, 12, 6, 7, 8, 10, 9];
var CSS_CELL = 'rounded-[16px] bg-white shadow-[0_2px_12px_rgba(0,0,0,0.05)] border-[1px] border-[rgba(0,0,0,0.04)] overflow-hidden flex items-center justify-center';
var CSS_P = { red: '#e63946', blue: '#457b9d', orange: '#f4a261', teal: '#2a9d8f', navy: '#1d3557', rose: '#d4738a', lime: '#84a98c', purple: '#6b4c9a' };

function FlexCss() {
  const spin1 = useRef(), spin2 = useRef(), orb1 = useRef(), orb2 = useRef();
  const grad1 = useRef(), grad2 = useRef();

  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };

    tl.fromTo('fc-wrap', { scale: 0.9, opacity: 0, filter: 'blur(4px)' }, { scale: 1, opacity: 1, filter: 'blur(0px)', duration: 0.5, ease: 'power2.out' }, A(0));
    for (var i = 0; i < CSS_ORDER.length; i++) {
      tl.fromTo('fc-c' + CSS_ORDER[i], { scale: 0.7, opacity: 0, y: 20 }, { scale: 1, opacity: 1, y: 0, duration: 0.28, ease: 'back.out(1.2)' }, A(0.02 + i * 0.02));
    }
    tl.fromTo('fc-title', { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.22, ease: 'power2.out' }, A(0.38));
    tl.fromTo('fc-sub', { opacity: 0 }, { opacity: 1, duration: 0.18, ease: 'power2.out' }, A(0.46));

    /* pulse (flex-css.html:181-187) — 3 bursts, dot squash + expanding rings */
    var pA = [0.10, 0.60, 1.10];
    ['fc-pulse-dot', 'fc-pulse2-dot'].forEach(function (dot, pi) {
      var r0 = pi === 0 ? 'fc-pulse-r0' : 'fc-pulse2-r0';
      var r1 = pi === 0 ? 'fc-pulse-r1' : 'fc-pulse2-r1';
      for (var p = 0; p < 3; p++) {
        var pt = pA[p];
        tl.fromTo(dot, { scale: 1 }, { scale: 1.3, duration: 0.12, ease: 'power2.out' }, A(pt));
        tl.fromTo(dot, { scale: 1.3 }, { scale: 1, duration: 0.15, ease: 'power2.in' }, A(pt + 0.12));
        tl.fromTo(r0, { scale: 1, opacity: 0.7 }, { scale: 3.5, opacity: 0, duration: 0.40, ease: 'power2.out' }, A(pt));
        tl.fromTo(r1, { scale: 1, opacity: 0.4 }, { scale: 5, opacity: 0, duration: 0.50, ease: 'power2.out' }, A(pt + 0.06));
      }
    });

    /* bars (:201-208) — cycles 0/2 move (cycle 1 targets h1 which is already
       the resting height, so it is a visual no-op in the reference too) */
    var barsDef = [
      { ids: ['fc-bars0', 'fc-bars1', 'fc-bars2', 'fc-bars3', 'fc-bars4'], h1: [30, 50, 70, 40, 60], h2: [80, 35, 90, 65, 45] },
      { ids: ['fc-bars2-0', 'fc-bars2-1', 'fc-bars2-2', 'fc-bars2-3', 'fc-bars2-4'], h1: [50, 30, 60, 80, 40], h2: [35, 75, 45, 30, 70] },
    ];
    barsDef.forEach(function (b) {
      [0.10, 1.10].forEach(function (bt) {
        b.ids.forEach(function (id, bi) {
          tl.fromTo(id, { height: b.h1[bi] }, { height: b.h2[bi], duration: 0.18, ease: 'power2.out' }, A(bt + bi * 0.03));
          tl.fromTo(id, { height: b.h2[bi] }, { height: b.h1[bi], duration: 0.18, ease: 'power2.in' }, A(bt + 0.22 + bi * 0.03));
        });
      });
    });

    /* morph (:215-218) — borderRadius % -> px on the fixed 80px box */
    var morphs = [
      { id: 'fc-morph', cols: [CSS_P.red, CSS_P.blue, CSS_P.orange, CSS_P.teal] },
      { id: 'fc-morph2', cols: [CSS_P.purple, CSS_P.lime, CSS_P.rose, CSS_P.navy] },
    ];
    morphs.forEach(function (m) {
      tl.fromTo(m.id, { borderRadius: 8, backgroundColor: m.cols[0], rotation: 0, scale: 1 }, { borderRadius: 40, backgroundColor: m.cols[1], duration: 0.30, ease: 'power2.inOut' }, A(0.15));
      tl.fromTo(m.id, { borderRadius: 40, backgroundColor: m.cols[1], rotation: 0, scale: 1 }, { borderRadius: 8, rotation: 45, scale: 0.75, backgroundColor: m.cols[2], duration: 0.30, ease: 'power2.inOut' }, A(0.50));
      tl.fromTo(m.id, { borderRadius: 8, backgroundColor: m.cols[2], rotation: 45, scale: 0.75 }, { borderRadius: 24, rotation: 0, scale: 1, backgroundColor: m.cols[3], duration: 0.30, ease: 'power2.inOut' }, A(0.85));
      tl.fromTo(m.id, { borderRadius: 24, backgroundColor: m.cols[3], scale: 1 }, { borderRadius: 40, backgroundColor: m.cols[0], scale: 0.9, duration: 0.30, ease: 'power2.inOut' }, A(1.20));
    });

    /* wave (:268-273) — widths in px on the 160px wrap */
    var waves = [
      { ids: ['fc-wave0', 'fc-wave1', 'fc-wave2'], c: CSS_P.blue },
      { ids: ['fc-wave2-0', 'fc-wave2-1', 'fc-wave2-2'], c: CSS_P.teal },
    ];
    var w0 = [160, 104, 136], w1 = [88, 160, 72], w2 = [144, 80, 160];
    waves.forEach(function (wv) {
      for (var li = 0; li < 3; li++) {
        tl.fromTo(wv.ids[li], { width: w0[li] }, { width: w1[li], duration: 0.38, ease: 'sine.inOut', yoyo: true, repeat: 1 }, A(0.10 + li * 0.05));
        tl.fromTo(wv.ids[li], { width: w0[li] }, { width: w2[li], duration: 0.32, ease: 'sine.inOut' }, A(1.10 + li * 0.05));
      }
    });

    /* bounce (:297-301) — 4 hops */
    for (var bi = 0; bi < 4; bi++) {
      var bt = 0.08 + bi * 0.40;
      tl.fromTo('fc-bounce', { y: 0 }, { y: -60, duration: 0.18, ease: 'power2.out' }, A(bt));
      tl.fromTo('fc-bounce', { y: -60 }, { y: 0, duration: 0.18, ease: 'bounce.out' }, A(bt + 0.18));
    }

    /* scale (:316-322) — 3x3 ripple pulse */
    var rippleOrder = [4, 1, 3, 5, 7, 0, 2, 6, 8];
    for (var c = 0; c < 3; c++) {
      rippleOrder.forEach(function (idx, ri) {
        var st = 0.08 + c * 0.55 + ri * 0.025;
        tl.fromTo('fc-sc' + idx, { scale: 1, opacity: 0.3 }, { scale: 1.4, opacity: 1, duration: 0.10, ease: 'power2.out' }, A(st));
        tl.fromTo('fc-sc' + idx, { scale: 1.4, opacity: 1 }, { scale: 1, opacity: 0.3, duration: 0.15, ease: 'power2.in' }, A(st + 0.10));
      });
    }

    /* gradient cells: conic ramp is a ly.pixel lambda (engine has no conic
       gradient); the rotation/breathing stay DOM tweens on the canvas node */
    tl.fromTo('fc-cv-grad1', { rotation: 0 }, { rotation: 360, duration: CSS_DUR, ease: 'none' }, A(0));
    tl.fromTo('fc-cv-grad2', { rotation: 0 }, { rotation: 360, duration: CSS_DUR, ease: 'none' }, A(0));
    tl.fromTo('fc-cv-grad1', { scale: 1 }, { scale: 1.12, duration: 0.28, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0.10));
    tl.fromTo('fc-cv-grad2', { scale: 1 }, { scale: 1.12, duration: 0.28, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0.10));

    /* flip (:288-289) — rotationY card, faces swapped per-frame (no
       backface-visibility in the engine; cos>0 picks the visible face) */
    var r1s = 0.28, r1e = 0.66, r2s = 0.88, r2e = 1.26;
    var rot = 0;
    if (T >= r1s && T < r1e) { var p1 = (T - r1s) / 0.38; rot = 180 * (p1 < 0.5 ? 2 * p1 * p1 : 1 - 2 * (1 - p1) * (1 - p1)); }
    else if (T >= r1e && T < r2s) rot = 180;
    else if (T >= r2s) { var p2 = clamp((T - r2s) / 0.38, 0, 1); rot = 180 + 180 * (p2 < 0.5 ? 2 * p2 * p2 : 1 - 2 * (1 - p2) * (1 - p2)); }
    var ff = ctx.getNode('fc-flip-front'), fb = ctx.getNode('fc-flip-back');
    ff.transformPerspective(600); fb.transformPerspective(600);
    ff.rotationY(rot); fb.rotationY(rot + 180);
    var frontOn = Math.cos(rot * Math.PI / 180) >= 0;
    ff.opacity(frontOn ? 1 : 0); fb.opacity(frontOn ? 0 : 1);

  });

  /* CanvasKit 绘制走 useCanvas；useGSAP 只留 tween */
  useCanvas(spin1, (c, t) => drawSpin(c, t, 1, CSS_P.red, CSS_P.blue));
  useCanvas(spin2, (c, t) => drawSpin(c, t, 0.8, CSS_P.teal, CSS_P.orange));
  useCanvas(orb1, (c, t) => drawOrbit(c, t, 1, [CSS_P.red, CSS_P.blue, CSS_P.orange]));
  useCanvas(orb2, (c, t) => drawOrbit(c, t, -1, [CSS_P.teal, CSS_P.purple, CSS_P.lime]));
  useCanvas(grad1, (c) => drawConic(c, [[230, 57, 70], [244, 162, 97], [42, 157, 143], [69, 123, 157]]));
  useCanvas(grad2, (c) => drawConic(c, [[107, 76, 154], [212, 115, 138], [132, 169, 140], [29, 53, 87]]));

  return (
    <div id="fc-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#fafaf2]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#fafaf2_0%,#f0efe6_100%)]"></div>
      <div id="fc-wrap" class="absolute inset-0">
        <div class="absolute left-[12px] top-[12px] w-[1896px] h-[1056px] grid grid-cols-6 grid-rows-3 gap-[12px]">
          <div id="fc-c0" class={CSS_CELL}><canvas ref={spin1} id="fc-cv-spin1" class="w-[80px] h-[80px]" /></div>
          <div id="fc-c1" class={CSS_CELL}>
            <div class="relative w-[80px] h-[80px]">
              <div id="fc-pulse-dot" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full bg-[#2a9d8f]"></div>
              <div id="fc-pulse-r0" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#2a9d8f] opacity-0"></div>
              <div id="fc-pulse-r1" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#2a9d8f] opacity-0"></div>
            </div>
          </div>
          <div id="fc-c2" class={CSS_CELL}>
            <div class="flex items-end gap-[6px] h-[90px]">
              <div id="fc-bars0" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[30px]"></div>
              <div id="fc-bars1" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[50px]"></div>
              <div id="fc-bars2" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[70px]"></div>
              <div id="fc-bars3" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[40px]"></div>
              <div id="fc-bars4" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[60px]"></div>
            </div>
          </div>
          <div id="fc-c3" class={CSS_CELL}><div id="fc-morph" class="w-[80px] h-[80px] rounded-[8px] bg-[#e63946]"></div></div>
          <div id="fc-c4" class={CSS_CELL}><canvas ref={orb1} id="fc-cv-orb1" class="w-[100px] h-[100px]" /></div>
          <div id="fc-c5" class={CSS_CELL}><canvas ref={grad1} id="fc-cv-grad1" class="w-[110px] h-[110px]" /></div>

          <div id="fc-c6" class={CSS_CELL}>
            <div class="flex flex-col gap-[10px] w-[160px]">
              <div id="fc-wave0" class="h-[5px] rounded-[3px] bg-[#457b9d] w-[160px]"></div>
              <div id="fc-wave1" class="h-[5px] rounded-[3px] bg-[#457b9d] w-[104px]"></div>
              <div id="fc-wave2" class="h-[5px] rounded-[3px] bg-[#457b9d] w-[136px]"></div>
            </div>
          </div>
          <div id="fc-c7" class={CSS_CELL}>
            <div class="relative w-[100px] h-[70px]">
              <div id="fc-flip-front" class="absolute inset-0 rounded-[8px] bg-[#2a9d8f]"></div>
              <div id="fc-flip-back" class="absolute inset-0 rounded-[8px] bg-[#e63946]"></div>
            </div>
          </div>
          <div id="fc-c8" class={CSS_CELL}><div id="fc-bounce" class="w-[36px] h-[36px] rounded-full bg-[#f4a261]"></div></div>
          <div id="fc-c9" class="rounded-[16px] bg-[#1a1a1a] overflow-hidden flex flex-col items-center justify-center gap-[6px]">
            <text id="fc-title" class="text-[80px] leading-none tracking-[-0.04em] font-[hf-grotesk-bold] text-[#fafaf2]">CSS</text>
            <text id="fc-sub" class="text-[20px] leading-none tracking-[0.16em] font-[hf-grotesk] text-[#777] uppercase">animations</text>
          </div>
          <div id="fc-c10" class={CSS_CELL}>
            <div class="grid grid-cols-3 gap-[8px]">
              {[0, 1, 2, 3, 4, 5, 6, 7, 8].map((i) => (
                <div id={'fc-sc' + i} class="w-[28px] h-[28px] rounded-[6px] bg-[#457b9d] opacity-[0.3]"></div>
              ))}
            </div>
          </div>
          <div id="fc-c11" class={CSS_CELL}><canvas ref={spin2} id="fc-cv-spin2" class="w-[80px] h-[80px]" /></div>

          <div id="fc-c12" class={CSS_CELL}><canvas ref={grad2} id="fc-cv-grad2" class="w-[110px] h-[110px]" /></div>
          <div id="fc-c13" class={CSS_CELL}><canvas ref={orb2} id="fc-cv-orb2" class="w-[100px] h-[100px]" /></div>
          <div id="fc-c14" class={CSS_CELL}>
            <div class="flex items-end gap-[6px] h-[90px]">
              <div id="fc-bars2-0" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[50px]"></div>
              <div id="fc-bars2-1" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[30px]"></div>
              <div id="fc-bars2-2" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[60px]"></div>
              <div id="fc-bars2-3" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[80px]"></div>
              <div id="fc-bars2-4" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[40px]"></div>
            </div>
          </div>
          <div id="fc-c15" class={CSS_CELL}><div id="fc-morph2" class="w-[80px] h-[80px] rounded-[8px] bg-[#6b4c9a]"></div></div>
          <div id="fc-c16" class={CSS_CELL}>
            <div class="relative w-[80px] h-[80px]">
              <div id="fc-pulse2-dot" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full bg-[#d4738a]"></div>
              <div id="fc-pulse2-r0" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#d4738a] opacity-0"></div>
              <div id="fc-pulse2-r1" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#d4738a] opacity-0"></div>
            </div>
          </div>
          <div id="fc-c17" class={CSS_CELL}>
            <div class="flex flex-col gap-[10px] w-[160px]">
              <div id="fc-wave2-0" class="h-[5px] rounded-[3px] bg-[#2a9d8f] w-[160px]"></div>
              <div id="fc-wave2-1" class="h-[5px] rounded-[3px] bg-[#2a9d8f] w-[104px]"></div>
              <div id="fc-wave2-2" class="h-[5px] rounded-[3px] bg-[#2a9d8f] w-[136px]"></div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* hex '#rrggbb' -> [r,g,b] (canvas helpers) */
function hexRgb(hx) {
  return [parseInt(hx.slice(1, 3), 16), parseInt(hx.slice(3, 5), 16), parseInt(hx.slice(5, 7), 16)];
}

/* spinner cell: two offset ring arcs, 720/-540 deg per beat. `c` is the
   ref-resolved canvas handle from useCanvas. */
function drawSpin(c, t, sp, c1, c2) {
  var ck = c.CK;
  c.clear();
  var paint = new ck.Paint();
  paint.setStyle(ck.PaintStyle.Stroke);
  paint.setStrokeWidth(3);
  paint.setAntiAlias(true);
  var c1rgb = hexRgb(c1);
  paint.setColor(ck.Color(c1rgb[0], c1rgb[1], c1rgb[2], 1));
  c.save();
  c.rotate(720 * sp * (t / CSS_DUR), 40, 40);
  c.drawArc([3, 3, 77, 77], -90, 180, false, paint);
  c.restore();
  var c2rgb = hexRgb(c2);
  paint.setColor(ck.Color(c2rgb[0], c2rgb[1], c2rgb[2], 1));
  c.save();
  c.rotate(-540 * sp * (t / CSS_DUR), 40, 40);
  c.drawArc([17, 17, 63, 63], -135, 90, false, paint);
  c.drawArc([17, 17, 63, 63], 45, 90, false, paint);
  c.restore();
}

/* orbit cell: 3 dots on R=36, proxy angle -> per-frame maths. */
function drawOrbit(c, t, dir, cols) {
  var ck = c.CK;
  c.clear();
  var paint = new ck.Paint();
  paint.setStyle(ck.PaintStyle.Fill);
  paint.setAntiAlias(true);
  var ang = dir * Math.PI * 2 * (t / CSS_DUR);
  for (var di = 0; di < 3; di++) {
    var a = ang + di * (2 * Math.PI / 3);
    var rgb = hexRgb(cols[di]);
    paint.setColor(ck.Color(rgb[0], rgb[1], rgb[2], 1));
    c.drawCircle(50 + Math.cos(a) * 36, 50 + Math.sin(a) * 36, 8, paint);
  }
}

/* conic-gradient ramp as a ly.pixel lambda (flex-css.html:344-345). Stops
   are uniform over the circle, starting at 12 o'clock, clockwise. The kernel
   is uniform-driven, so one module-level instance serves both cells. */
function drawConic(c, stops) {
  if (!drawConic._kern) {
    drawConic._kern = ly.pixel((uv, u) => {
      const dx = uv.x - 55;
      const dy = uv.y - 55;
      const d = sqrt(dx * dx + dy * dy);
      let a = atan2(dx, -dy);
      if (a < 0) a = a + 6.283185307179586;
      const t = a / 6.283185307179586;
      let col = u.c0;
      if (t < 0.25) col = u.c0 + (u.c1 - u.c0) * (t * 4);
      else if (t < 0.5) col = u.c1 + (u.c2 - u.c1) * (t * 4 - 1);
      else if (t < 0.75) col = u.c2 + (u.c3 - u.c2) * (t * 4 - 2);
      else col = u.c3 + (u.c0 - u.c3) * (t * 4 - 3);
      const al = d > 54.5 ? 0 : 1;
      return [col.x / 255, col.y / 255, col.z / 255, al];
    }, { uniforms: { c0: 'float3', c1: 'float3', c2: 'float3', c3: 'float3' } });
  }
  ly.draw(c, drawConic._kern, {
    rect: [0, 0, 110, 110],
    uniforms: { c0: stops[0], c1: stops[1], c2: stops[2], c3: stops[3] },
  });
}

/* ══ flex-gsap (master 19.01-19.68, compositions/flex-gsap.html) ════════
   15x25 dot matrix. Phase 1: per-column sweep with elastic snap-back.
   Phase 2: radial explosion (from 0/0 nominal — later items win the flush,
   matching GSAP's simultaneous-write behaviour). Master clips at 0.67s. */
var GS_START = 19.01, GS_DUR = 0.67, GS_ROWS = 15, GS_COLS = 25;

function FlexGsap() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    var centerR = (GS_ROWS - 1) / 2, centerC = (GS_COLS - 1) / 2;
    var maxDist = Math.sqrt(centerR * centerR + centerC * centerC);

    for (var c = 0; c < GS_COLS; c++) {
      var colIds = [];
      for (var rr = 0; rr < GS_ROWS; rr++) colIds.push('fg-d' + rr + '-' + c);
      var colStart = c * 0.019;
      tl.fromTo(colIds, { x: 0, y: 0 }, { x: 8, y: -22, duration: 0.08, ease: 'power2.out', stagger: 0.004 }, A(colStart));
      tl.fromTo(colIds, { x: 8, y: -22 }, { x: 0, y: 0, duration: 0.28, ease: 'elastic.out(1, 0.3)', stagger: 0.004 }, A(colStart + 0.08));
    }
    for (var r = 0; r < GS_ROWS; r++) {
      for (var c2 = 0; c2 < GS_COLS; c2++) {
        var dr = r - centerR, dc = c2 - centerC;
        var dist = Math.sqrt(dr * dr + dc * dc);
        var normDist = dist / maxDist;
        var flyDist = 160 * (1 - normDist * 0.5);
        var angle = Math.atan2(dr, dc);
        tl.fromTo('fg-d' + r + '-' + c2, { x: 0, y: 0, scale: 1, opacity: 1 }, {
          x: Math.cos(angle) * flyDist,
          y: Math.sin(angle) * flyDist - 100,
          scale: 0.3, opacity: 0.2,
          duration: 0.18, ease: 'power3.out',
        }, A(0.48 + normDist * 0.08));
      }
    }
    tl.fromTo('fg-grid', { filter: 'blur(0px)' }, { filter: 'blur(20px)', duration: 0.18, ease: 'power2.in' }, A(0.52));
  });

  var rows = [];
  for (var r = 0; r < GS_ROWS; r++) rows.push(r);
  var cols = [];
  for (var c3 = 0; c3 < GS_COLS; c3++) cols.push(c3);
  return (
    <div id="fg-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#0e100f]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#161816_0%,#0e100f_100%)]"></div>
      <div id="fg-grid" class="absolute left-[960px] top-[540px] ml-[-700px] mt-[-420px] flex flex-col">
        {rows.map((r) => (
          <div id={'fg-row' + r} class="flex">
            {cols.map((c) => (
              <div id={'fg-d' + r + '-' + c} class="w-[40px] h-[40px] m-[8px] rounded-full bg-[#fffce1]"></div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

/* ══ flex-shader (master 20.81-22.01, compositions/flex-shader.html) ════
   WebGL domain-warp fbm shader -> ly.pixel lambda, 1:1 with the reference's
   four functions (hash/noise/fbm/palette). The DSL supports top-level
   `const f = (p) => ...` user functions and bounded `for` loops, so the
   octave loop is no longer unrolled by hand. R is GLSL's column-major
   mat2(0.8,0.6,-0.6,0.8). u_time ramps 0.5->1.44 over the 0.94s beat, then
   the frame holds (master slot is 1.20s). */
var FS_START = 20.81, FS_DUR = 1.20, FS_ANIM = 0.94;

function FlexShader() {
  const canvas = useRef();

  useGSAP((ctx) => {
    var tl = ctx.timeline();
    tl.fromTo('fs-label', { opacity: 0 }, { opacity: 0.35, duration: 0.3, ease: 'power2.out' }, 0.15);
  });

  /* module-level kernel: source compiled once by Rust, sampled per frame.
     The canvas draw goes through useCanvas (ref-resolved, augmented handle),
     not useGSAP + getCanvasById. */
  useCanvas(canvas, (c, t, ctx) => {
    if (!FlexShader._kern) {
      FlexShader._kern = ly.pixel((uv, u) => {
        const ux = uv.x / 1080;
        /* GLSL v_uv.y=1 is the CANVAS TOP; ly.pixel uv.y grows downward,
           so the reference's v_uv.y is 1 - uv.y/1080. */
        const uy = 1 - uv.y / 1080;
        const t = u.time * 0.4;
        const hash = (p) => fract(sin(dot(p, [127.1, 311.7])) * 43758.5453);
        const noise = (p) => {
          const i = floor(p);
          const f = fract(p);
          const w = f * f * (3 - 2 * f);
          return mix(mix(hash(i), hash(i + [1, 0]), w.x),
                     mix(hash(i + [0, 1]), hash(i + [1, 1]), w.x), w.y);
        };
        /* reference fbm is fixed at 5 octaves */
        const fbm = (p) => {
          let v = 0;
          let a = 0.5;
          for (let o = 0; o < 5; o++) {
            v = v + a * noise(p);
            p = [0.8 * p.x - 0.6 * p.y, 0.6 * p.x + 0.8 * p.y] * 2.02;
            a = a * 0.5;
          }
          return v;
        };
        const palette = (s) => [0.5 + 0.5 * cos(6.28318 * s),
                                0.5 + 0.5 * cos(6.28318 * (s + 0.33)),
                                0.5 + 0.5 * cos(6.28318 * (s + 0.67))];
        const qx = fbm([ux * 3 + t * 0.3, uy * 3 + t * 0.3]);
        const qy = fbm([ux * 3 + 5.2 + t * 0.2, uy * 3 + 1.3 + t * 0.2]);
        const rx = fbm([ux * 3 + qx * 4 + 1.7 + t * 0.15, uy * 3 + qy * 4 + 9.2 + t * 0.15]);
        const ry = fbm([ux * 3 + qx * 4 + 8.3 + t * 0.1, uy * 3 + qy * 4 + 2.8 + t * 0.1]);
        const n = fbm([ux * 3 + rx * 2, uy * 3 + ry * 2]);
        /* palette(n*2 + t*0.2), mixed 0.4 with palette(|q|*3 + t*0.1) */
        const col = palette(n * 2 + t * 0.2);
        const q2 = palette(sqrt(qx * qx + qy * qy) * 3 + t * 0.1);
        let cr = col.x + (q2.x - col.x) * 0.4;
        let cg = col.y + (q2.y - col.y) * 0.4;
        let cb = col.z + (q2.z - col.z) * 0.4;
        const m = 0.7 + 0.3 * n;
        cr = cr + (0.92 - cr) * 0.08; cg = cg + (0.58 - cg) * 0.08; cb = cb + (0.35 - cb) * 0.08;
        const vdx = uv.x / 1920 - 0.5; const vdy = uv.y / 1080 - 0.5;
        const vig = 1 - 0.4 * sqrt(vdx * vdx + vdy * vdy);
        return [cr * m * vig, cg * m * vig, cb * m * vig, 1];
      }, { uniforms: { time: 'float' } });
    }

    c.clear();
    ly.draw(c, FlexShader._kern, {
      rect: [0, 0, 1920, 1080],
      uniforms: { time: 0.5 + clamp(t / FS_ANIM, 0, 1) * 0.94 },
    });
  });

  return (
    <div id="fs-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#0a0a12]">
      <canvas ref={canvas} id="fs-canvas" class="absolute left-0 top-0 w-[1920px] h-[1080px]" />
      <text id="fs-label" class="absolute right-[80px] bottom-[60px] text-[28px] tracking-[0.08em] font-[hf-azeret] text-[rgba(255,255,255,0.35)] opacity-0">Shaders</text>
    </div>
  );
}

/* ══ flex-threejs (master 22.01-23.22, compositions/flex-threejs.html) ══
   3D torus via ctx.three (opencat-three / wgpu). The reference is pure CSS 3D
   (no three.js library): .three-stage has perspective:900px +
   perspective-origin:50% 50%, so CSS (x,y,z) -> (960 + x*900/(900-z),
   540 + y*900/(900-z)). A three.js PerspectiveCamera at z=near+900 with
   fov=2*atan(540/900) inverts that exactly. Segment bases: centre
   (R cos a,0,R sin a), local +x = inward radial, +y = up. form: rotY
   -30->310 sine.inOut, rotX 25->10 yoyo x2. Shadow + label stay DOM. */
var FT_START = 22.01, FT_DUR = 1.21;
var FT_COLORS = [
  ['#dbb0b0', '#c49090'], ['#c8b8a8', '#b0a090'], ['#a8c0a0', '#8ea888'], ['#c4b098', '#ae9a82'],
  ['#d4a0a0', '#be8a8a'], ['#b8c8b0', '#98b088'], ['#d0c0b0', '#baa898'], ['#c0a8b8', '#a890a0'],
  ['#dbb0b0', '#c49090'], ['#c8b8a8', '#b0a090'], ['#a8c0a0', '#8ea888'], ['#c4b098', '#ae9a82'],
  ['#d4a0a0', '#be8a8a'], ['#b8c8b0', '#98b088'], ['#d0c0b0', '#baa898'], ['#c0a8b8', '#a890a0'],
];

function FlexThreejs() {
  const canvas = useRef();

  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('ft-shadow', { scaleX: 1, scaleY: 1 }, { scaleX: 1.3, scaleY: 0.7, duration: 0.4, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0));
    tl.fromTo('ft-label', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, A(0.2));
  });

  /* Real 3D via ctx.three (opencat-three / wgpu). The reference is pure CSS 3D
     (no three.js library): .three-stage has perspective:900px with
     perspective-origin:50% 50%, so a CSS point (x,y,z) projects to
     (960 + x*900/(900-z), 540 + y*900/(900-z)). A three.js PerspectiveCamera
     at z = near + 900 with fov = 2*atan(540/900) inverts that exactly
     (screen_x = 960 + x, screen_y = 540 - y on the z=0 plane). The world Y
     axis points up, so every CSS y offset is negated and the form's CSS
     rotateX negates too; the form's rotateY is unchanged. Each torus segment
     is a PlaneGeometry (segment-local +x = inward radial, +y = up) whose
     ShaderMaterial draws the rounded-rect mask + 135deg two-stop ramp. */
  useThree(canvas, (T3, node, t, ctx) => {
    var T = t;

    var scene = new T3.Scene();
    var camera = new T3.PerspectiveCamera(61.9275, 1920 / 1080, 0.1, 5000);
    camera.position.set(0, 0, 900.1);
    camera.lookAt(0, 0, 0);
    scene.add(new T3.AmbientLight(0xffffff, 1.0));
    scene.add(new T3.HemisphereLight(0xffffff, 0xffffff, 0.5));

    function face(col, half, rad, op) {
      var c1 = hexRgb(col[0]), c2 = hexRgb(col[1]);
      var mat = new T3.ShaderMaterial({
        transparent: op < 1,
        uniforms: {
          c1: { value: [c1[0] / 255, c1[1] / 255, c1[2] / 255] },
          c2: { value: [c2[0] / 255, c2[1] / 255, c2[2] / 255] },
          half: { value: half },
          rad: { value: rad },
          op: { value: op },
        },
        vertexShader: [
          'varying vec2 vUv;',
          'void main() {',
          '  vUv = uv;',
          '  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);',
          '}',
        ].join('\n'),
        fragmentShader: [
          'precision highp float;',
          'varying vec2 vUv;',
          'uniform vec3 c1;',
          'uniform vec3 c2;',
          'uniform float half;',
          'uniform float rad;',
          'uniform float op;',
          'void main() {',
          '  vec2 p = vUv * (2.0 * half);',
          '  vec3 col = mix(c1, c2, (p.x + p.y) / (4.0 * half));',
          '  vec2 q = abs(p - vec2(half)) - (half - rad);',
          '  float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - rad;',
          '  gl_FragColor = d < 0.0 ? vec4(col, op) : vec4(0.0);',
          '}',
        ].join('\n'),
      });
      return new T3.Mesh(new T3.PlaneGeometry(2 * half, 2 * half), mat);
    }

    var SEGS = 16, RING_R = 400;
    var formX = new T3.Group();
    for (var i = 0; i < SEGS; i++) {
      var a = (i / SEGS) * Math.PI * 2;
      var m = face(FT_COLORS[i], 87.5, 40, 1);
      m.position.set(Math.cos(a) * RING_R, 0, Math.sin(a) * RING_R);
      m.rotation.y = -a + Math.PI;
      formX.add(m);
    }
    for (var j = 0; j < SEGS; j++) {
      var a2 = ((j + 0.5) / SEGS) * Math.PI * 2;
      var m2 = face(FT_COLORS[(j + 4) % SEGS], 62.5, 30, 0.85);
      m2.position.set(Math.cos(a2) * RING_R * 0.7, -62, Math.sin(a2) * RING_R * 0.7);
      m2.rotation.y = -a2 + Math.PI;
      formX.add(m2);
    }
    var formY = new T3.Group();
    formY.add(formX);
    /* The CSS stage sits at top:42% -> screen y 453.6; world y is up, so the
       form's pivot (the stage point) is at world y = 540 - 453.6 = 86.4. */
    formY.position.set(0, 86.4, 0);
    scene.add(formY);

    /* GSAP form rotation: Ry(-30 -> 310) sine.inOut over 1.2s, Rx(25 -> 10)
       sine.inOut yoyo x2 over 0.4s each. */
    var ph = T / 0.4, k = Math.floor(ph), fr = ph - k;
    var sio = 0.5 - 0.5 * Math.cos(Math.PI * fr);
    var s = (k % 2 === 0) ? sio : 1 - sio;
    formY.rotation.y = (-30 + 340 * (0.5 - 0.5 * Math.cos(Math.PI * clamp(T / 1.2, 0, 1)))) * Math.PI / 180;
    formX.rotation.x = -(25 + (10 - 25) * s) * Math.PI / 180;

    new T3.Renderer('ft-canvas', 1920, 1080).render(scene, camera);
  });

  return (
    <div id="ft-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f0ebe3]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_40%_35%,rgba(255,245,230,0.6)_0%,transparent_50%),radial-gradient(ellipse_at_50%_50%,#f0ebe3_0%,#e4ddd2_100%)]"></div>
      <div class="absolute left-[672px] top-[162px] w-[600px] h-[600px] bg-[radial-gradient(circle,rgba(255,235,200,0.25)_0%,transparent_60%)]"></div>
      <div id="ft-shadow" class="absolute left-[780px] top-[680px] w-[360px] h-[50px] rounded-full bg-[radial-gradient(ellipse,rgba(80,60,40,0.18)_0%,transparent_70%)]"></div>
      <canvas id="ft-canvas" ref={canvas} class="absolute left-0 top-0 w-[1920px] h-[1080px]" />
      <div class="absolute left-0 bottom-[140px] w-full flex justify-center">
        <text id="ft-label" class="text-[64px] leading-none tracking-[0.02em] italic font-[hf-cg-it] text-[#5a4a3a] opacity-0">Three.js</text>
      </div>
    </div>
  );
}

/* ══ thesis (master 23.22-27.04 slot 3.82s; template runs 4.52s so the exit
   tween is clipped exactly as the master does, thesis.html) ═══════════════
   The gradient shimmer (add .is-gradient + backgroundPosition sweep at
   2.44/2.49) has no engine equivalent — replaced by base/gradient word
   layers crossfading with the same 0.6s power2.out stagger 0.04. */
var TH_START = 23.22, TH_DUR = 3.82;
var TH_L1CUES = [0.00, 0.23, 0.28, 0.63, 0.78];
var TH_L1SLIDES = [360, 120, 60, 25, 12];
var TH_L2CUES = [1.38, 1.60, 1.70, 1.76, 2.01, 2.11, 2.25];
var TH_L1 = ['Anything', 'a', 'browser', 'can', 'render'];
var TH_L2 = ['can', 'be', 'a', 'frame', 'in', 'your', 'video.'];
var TH_GRAD = 'bg-[linear-gradient(90deg,#14110a_0%,#14110a_15%,#5a3215_25%,#c84f1c_40%,#e2b53f_55%,#2a8a7c_70%,#14110a_85%,#14110a_100%)] bg-clip-text text-transparent bg-[length:300%_100%]';

function Thesis() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };

    tl.fromTo('th-wrap', { filter: 'blur(22px)' }, { filter: 'blur(0px)', duration: 0.33, ease: 'power2.out' }, A(0));
    tl.fromTo('th-wrap', { opacity: 1 }, { opacity: 0, duration: 0.3, ease: 'power2.in' }, A(4.52));

    for (var i = 0; i < 5; i++) {
      tl.fromTo('th-w' + i, { x: TH_L1SLIDES[i], y: 14, opacity: 0 }, { x: 0, y: 0, opacity: 1, duration: 0.33, ease: 'expo.out' }, A(TH_L1CUES[i]));
    }
    for (var j = 0; j < 7; j++) {
      tl.fromTo('th-w' + (5 + j), { y: 14, opacity: 0 }, { y: 0, opacity: 1, duration: 0.20, ease: 'power2.out' }, A(TH_L2CUES[j]));
    }

    /* shimmer crossfade (replaces backgroundPosition sweep) */
    for (var k = 0; k < 12; k++) {
      tl.fromTo('th-g' + k, { opacity: 0 }, { opacity: 1, duration: 0.6, ease: 'power2.out' }, A(2.44 + k * 0.04));
      tl.fromTo('th-w' + k, { opacity: 1 }, { opacity: 0, duration: 0.6, ease: 'power2.out' }, A(2.44 + k * 0.04));
    }

    tl.fromTo('th-scene', { scale: 1 }, { scale: 1.012, duration: 0.09, ease: 'power2.out' }, A(2.50));
    tl.fromTo('th-scene', { scale: 1.012 }, { scale: 1, duration: 0.28, ease: 'power2.inOut' }, A(2.59));
    tl.fromTo('th-l2wrap', { scaleX: 1 }, { scaleX: 1.0123, duration: 0.5, ease: 'sine.inOut' }, A(4.10));
  });

  var L1CLS = 'text-[132px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]';
  var L2CLS = 'text-[132px] leading-[1.08] tracking-[-0.02em] italic font-[hf-serif-it] text-[#2a1f0a]';
  return (
    <div id="th-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#fbf4e3]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_45%,#fcf6e7_0%,#f6edd4_100%)]"></div>
      <div id="th-wrap" class="absolute inset-0">
        <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_55%,transparent_45%,rgba(110,70,15,0.08)_100%)]"></div>
        <div id="th-scene" class="absolute left-[100px] top-[388.4px] w-[1720px]">
          <div class="relative px-[21.12px]">
            <div class="flex justify-center gap-[62.2px]">
              {TH_L1.map((w, i) => (
                <text id={'th-w' + i} class={L1CLS + ' opacity-0'}>{w}</text>
              ))}
            </div>
            <div class="absolute inset-0 flex justify-center gap-[62.2px]">
              {TH_L1.map((w, i) => (
                <text id={'th-g' + i} class={L1CLS + ' ' + TH_GRAD + ' opacity-0'}>{w}</text>
              ))}
            </div>
          </div>
          <div id="th-l2wrap" class="relative mt-[18px] px-[21.12px]">
            <div class="flex justify-center gap-[62.2px]">
              {TH_L2.map((w, j) => (
                <text id={'th-w' + (5 + j)} class={L2CLS + ' opacity-0'}>{w}</text>
              ))}
            </div>
            <div class="absolute inset-0 flex justify-center gap-[62.2px]">
              {TH_L2.map((w, j) => (
                <text id={'th-g' + (5 + j)} class={L2CLS + ' ' + TH_GRAD + ' opacity-0'}>{w}</text>
              ))}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ══ flex-drop (master 27.04-27.40, compositions/flex-drop.html) ════════ */
var FD_START = 27.04, FD_DUR = 0.36;

function FlexDrop() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('fd-text', { y: -300, opacity: 0 }, { y: 0, opacity: 1, duration: 0.25, ease: 'back.out(1.2)' }, A(0));
    tl.fromTo('fd-line', { scaleX: 0, opacity: 1 }, { scaleX: 1, duration: 0.12, ease: 'power3.out' }, A(0.15));
    tl.fromTo('fd-line', { scaleX: 1, opacity: 1 }, { scaleX: 1.3, opacity: 0, duration: 0.1, ease: 'power2.out' }, A(0.27));
  });
  return (
    <div id="fd-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#0d1b2a]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_60%,#152238_0%,#0d1b2a_100%)]"></div>
      <div class="absolute left-0 top-[450px] w-full flex justify-center">
        <text id="fd-text" class="text-[180px] leading-none tracking-[0.04em] font-[hf-bebas] text-white opacity-0 whitespace-nowrap">drop in</text>
      </div>
      <div class="absolute left-0 top-[618.5px] w-full flex justify-center">
        <div id="fd-line" class="w-[300px] h-[3px] rounded-[2px] bg-[rgba(255,255,255,0.25)]"></div>
      </div>
    </div>
  );
}

/* ══ flex-music (master 27.40-27.98, compositions/flex-music.html) ══════
   The vinyl/tone-arm SVGs become canvas draws (jsx svg supports <path>
   only). The reference's back.out rotation tween is fully overwritten by
   the later linear spin (same start, same property) — only the linear one
   is emitted. Record label text stays DOM. */
var FM_START = 27.40, FM_DUR = 0.58;

function FlexMusic() {
  const record = useRef(), arm = useRef();

  useGSAP((ctx) => {
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('fm-record', { scale: 0.4, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.3, ease: 'back.out(1.4)' }, A(0));
    tl.fromTo('fm-record', { rotation: -90 }, { rotation: 15, duration: FM_DUR, ease: 'none' }, A(0));
    tl.set('fm-arm', { opacity: 1 }, A(0.08));
    tl.fromTo('fm-arm', { rotation: 35 }, { rotation: 18, duration: 0.25, ease: 'power2.out' }, A(0.1));
    tl.fromTo('fm-text', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.15, ease: 'power2.out' }, A(0.12));
  });

  /* vinyl disc (svg 600 -> 520px canvas, k=0.8667). Static art: the spin is a
     DOM tween on #fm-record, so the canvas only needs its resting pixels. */
  useCanvas(record, (c) => {
    var ck = c.CK;
    c.clear();
    var p = new ck.Paint();
    p.setStyle(ck.PaintStyle.Fill);
    p.setAntiAlias(true);
    p.setColor(ck.Color(26, 18, 8, 1));
    c.drawCircle(260, 260, 280 * 0.8667, p);
    p.setStyle(ck.PaintStyle.Stroke);
    var grooves = [[260, 0.3, 1], [240, 0.25, 0.8], [220, 0.3, 1], [200, 0.2, 0.8], [180, 0.3, 1], [160, 0.25, 0.8], [140, 0.3, 1]];
    grooves.forEach(function (g) {
      p.setStrokeWidth(g[2] * 0.8667);
      p.setColor(ck.Color(80, 60, 20, g[1]));
      c.drawCircle(260, 260, g[0] * 0.8667, p);
    });
    p.setStyle(ck.PaintStyle.Fill);
    p.setColor(ck.Color(255, 220, 140, 0.06));
    c.save();
    c.translate(208, 208);
    c.rotate(-30);
    c.drawOval([-104, -69.3, 104, 69.3], p);
    c.restore();
    p.setColor(ck.Color(200, 132, 10, 1));
    c.drawCircle(260, 260, 90 * 0.8667, p);
    p.setStyle(ck.PaintStyle.Stroke);
    p.setStrokeWidth(0.87);
    p.setColor(ck.Color(255, 255, 255, 0.12));
    c.drawCircle(260, 260, 88 * 0.8667, p);
    p.setStyle(ck.PaintStyle.Fill);
    p.setColor(ck.Color(26, 18, 8, 1));
    c.drawCircle(260, 260, 8 * 0.8667, p);
  });

  /* tone arm (svg 200x400 -> 120x280 canvas, meet: k=0.6, y+20) */
  useCanvas(arm, (c) => {
    var ck = c.CK;
    c.clear();
    var q = new ck.Paint();
    q.setStyle(ck.PaintStyle.Fill);
    q.setAntiAlias(true);
    function X(v) { return v * 0.6; }
    function Y(v) { return v * 0.6 + 20; }
    q.setColor(ck.Color(42, 31, 16, 1));
    c.drawCircle(X(100), Y(30), X(18), q);
    q.setColor(ck.Color(61, 46, 24, 1));
    c.drawCircle(X(100), Y(30), X(12), q);
    q.setColor(ck.Color(42, 31, 16, 1));
    c.drawRRect(ck.RRectXY([X(97), Y(30), X(103), Y(310)], 1.8, 1.8), q);
    q.setColor(ck.Color(61, 46, 24, 1));
    c.drawRRect(ck.RRectXY([X(88), Y(300), X(112), Y(340)], 2.4, 2.4), q);
    q.setColor(ck.Color(26, 18, 8, 1));
    c.drawRRect(ck.RRectXY([X(94), Y(336), X(106), Y(352)], 1.2, 1.2), q);
    q.setColor(ck.Color(200, 132, 10, 1));
    c.drawCircle(X(100), Y(356), X(2), q);
  });

  return (
    <div id="fm-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f5a623]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_45%_48%,#f7c04a_0%,#e8960a_100%)]"></div>
      <div id="fm-record" class="absolute left-[700px] top-[250px] w-[520px] h-[520px] opacity-0 drop-shadow-[0_20px_60px_rgba(20,12,0,0.4)]">
        <canvas ref={record} id="fm-record-canvas" class="w-[520px] h-[520px]" />
        <text class="absolute left-0 top-[243px] w-full text-center text-[13.9px] tracking-[0.1em] font-[hf-inter-med] text-[#1a1208]">HYPERFRAMES</text>
        <text class="absolute left-0 top-[264px] w-full text-center text-[9.5px] font-[hf-inter] text-[rgba(26,18,8,0.6)]">A-SIDE</text>
      </div>
      <div id="fm-arm" class="absolute left-[1140px] top-[250px] w-[120px] h-[280px] [transform-origin:50%_15px] opacity-0 drop-shadow-[0_4px_12px_rgba(20,12,0,0.3)]">
        <canvas ref={arm} id="fm-arm-canvas" class="w-[120px] h-[280px]" />
      </div>
      <div class="absolute left-0 bottom-[120px] w-full flex justify-center">
        <text id="fm-text" class="text-[64px] leading-none tracking-[0.08em] font-[hf-inter-med] text-[#2a1808] uppercase opacity-0">music</text>
      </div>
    </div>
  );
}

/* ══ flex-sfx (master 27.98-28.91, compositions/flex-sfx.html) ══════════ */
var FX_START = 27.98, FX_DUR = 0.93;
var FX_HEIGHTS = [];
for (var _fxi = 0; _fxi < 48; _fxi++) {
  FX_HEIGHTS.push(Math.round(80 + 220 * Math.abs(Math.sin(_fxi * 0.7 + 0.3) * Math.cos(_fxi * 1.3 + 0.7)) + 60 * Math.abs(Math.sin(_fxi * 2.1 + 1.5))));
}

function FlexSfx() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('fx-glow', { scale: 0.3, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.2, ease: 'power2.out' }, A(0));
    tl.fromTo('fx-glow', { scale: 1, opacity: 1 }, { scale: 1.4, opacity: 0.6, duration: 0.5, ease: 'sine.inOut' }, A(0.2));
    for (var i = 0; i < 48; i++) {
      var delay = (i % 12) * 0.008;
      var id = 'fx-b' + i;
      /* the reference's static [transform:rotate(a)_translateY(b)] has no
         engine parse rule — decomposed into GSAP slots: rotate(a) translate
         (b along the rotated frame) === rotation:a, x:b*sin(a), y:-b*cos(a) */
      var th = i / 48 * 360 * Math.PI / 180;
      tl.set(id, { rotation: i / 48 * 360, x: 60 * Math.sin(th), y: -60 * Math.cos(th) }, A(0));
      tl.fromTo(id, { height: 4, opacity: 0 }, { height: FX_HEIGHTS[i], opacity: 0.9, duration: 0.12, ease: 'power4.out' }, A(0.02 + delay));
      tl.fromTo(id, { height: FX_HEIGHTS[i] }, { height: FX_HEIGHTS[i] * 0.5, duration: 0.35, ease: 'power2.inOut' }, A(0.14 + delay));
      tl.fromTo(id, { height: FX_HEIGHTS[i] * 0.5 }, { height: FX_HEIGHTS[i] * 0.7, duration: 0.1, ease: 'power2.out' }, A(0.45 + delay));
      tl.fromTo(id, { height: FX_HEIGHTS[i] * 0.7 }, { height: FX_HEIGHTS[i] * 0.35, duration: 0.3, ease: 'sine.out' }, A(0.55 + delay));
    }
    tl.fromTo('fx-text', { opacity: 0, scale: 0.85 }, { opacity: 1, scale: 1, duration: 0.18, ease: 'power2.out' }, A(0.08));
  });

  var bars = [];
  for (var b = 0; b < 48; b++) bars.push(b);
  return (
    <div id="fx-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#1a1208]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#2a1f10_0%,#0e0a04_100%)]"></div>
      <div id="fx-glow" class="absolute left-[560px] top-[140px] w-[800px] h-[800px] bg-[radial-gradient(circle,rgba(200,132,10,0.25)_0%,transparent_70%)] opacity-0"></div>
      <div class="absolute left-[960px] top-[540px] w-0 h-0">
        {bars.map((i) => (
          <div id={'fx-b' + i} class="absolute bottom-0 left-[-3px] w-[6px] rounded-[3px] bg-[linear-gradient(to_top,#c8840a,#f5c842)] h-[4px] opacity-0 [transform-origin:50%_100%]"></div>
        ))}
      </div>
      <div class="absolute left-0 top-[540px] w-full flex justify-center">
        <text id="fx-text" class="mt-[-26px] text-[52px] leading-none tracking-[0.1em] font-[hf-inter-med] text-[#f5e8c8] uppercase [text-shadow:0_0_40px_rgba(200,132,10,0.5)] opacity-0">sound effects</text>
      </div>
    </div>
  );
}

/* ══ flex-footage (master 28.91-29.74, compositions/flex-footage.html) ══ */
var FF_START = 28.91, FF_DUR = 0.83;
var FF_VIDEOS = [
  ['ff-v1', 'assets/Frame 1.mp4'],
  ['ff-v2', 'assets/option2.mp4'],
  ['ff-v3', 'assets/Frame 3.mp4'],
];

function FlexFootage() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('ff-text', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power2.out' }, A(0));
    tl.fromTo('ff-strip-top', { x: 0 }, { x: -100, duration: FF_DUR, ease: 'none' }, A(0));
    tl.fromTo('ff-strip-bottom', { x: -50 }, { x: -150, duration: FF_DUR, ease: 'none' }, A(0));
    tl.fromTo('ff-track', { x: 0 }, { x: -100, duration: FF_DUR, ease: 'none' }, A(0));
  });
  var STRIP = 'bg-[repeating-linear-gradient(90deg,transparent_0px,transparent_40px,#0d0d0d_40px,#0d0d0d_44px,transparent_44px,transparent_60px,rgba(255,255,255,0.06)_60px,rgba(255,255,255,0.06)_80px,transparent_80px,transparent_100px)]';
  return (
    <div id="ff-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#1a1a1a]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#262626_0%,#1a1a1a_100%)]"></div>
      <div id="ff-track" class="absolute left-[-100px] top-[120px] flex gap-[40px]">
        {FF_VIDEOS.map((v) => (
          <div class="w-[680px] h-[840px] rounded-[16px] overflow-hidden shrink-0 shadow-[0_4px_20px_rgba(0,0,0,0.5)]">
            <video id={v[0]} path={v[1]} data-start={0} data-duration={FF_DUR} data-media-start="0" class="w-full h-full object-cover" />
          </div>
        ))}
      </div>
      <div id="ff-strip-top" class={'absolute left-0 top-0 w-[2200px] h-[80px] opacity-[0.7] ' + STRIP}></div>
      <div id="ff-strip-bottom" class={'absolute left-0 bottom-0 w-[2200px] h-[80px] opacity-[0.7] ' + STRIP}></div>
      <div class="absolute left-0 top-[540px] w-full flex justify-center">
        <text id="ff-text" class="mt-[-80px] text-[160px] leading-none tracking-[0.01em] italic font-[hf-eb-it] text-[#f0e6d2] [text-shadow:0_6px_24px_rgba(0,0,0,0.8),0_2px_8px_rgba(0,0,0,0.6)] opacity-0">footage</text>
      </div>
      {/* reference grain = one node with 3 radial-gradient layers at 3/5/7px
          tiles; engine bg-[length:...] takes a single layer, so 3 stacked
          nodes with identical visual result */}
      <div class="absolute inset-0 opacity-[0.6] bg-[radial-gradient(circle_at_20%_30%,rgba(255,255,255,0.03)_1px,transparent_1px)] bg-[length:3px_3px]"></div>
      <div class="absolute inset-0 opacity-[0.6] bg-[radial-gradient(circle_at_80%_70%,rgba(255,255,255,0.02)_1px,transparent_1px)] bg-[length:5px_5px]"></div>
      <div class="absolute inset-0 opacity-[0.6] bg-[radial-gradient(circle_at_50%_50%,rgba(255,255,255,0.015)_1px,transparent_1px)] bg-[length:7px_7px]"></div>
    </div>
  );
}

/* ══ flex-it (master 29.74-29.89, compositions/flex-it.html) ════════════ */
var FI_START = 29.74, FI_DUR = 0.15;

function FlexIt() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    ctx.timeline().fromTo('fi-text', { scale: 1.06 }, { scale: 1, duration: 0.12, ease: 'power2.out' }, 0);
  });
  return (
    <div id="fi-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-black">
      <div class="absolute left-0 top-[540px] w-full flex justify-center">
        <text id="fi-text" class="mt-[-160px] text-[320px] leading-none tracking-[-0.04em] font-[hf-inter-black] text-white">it</text>
      </div>
    </div>
  );
}

/* ══ flex-all (master 29.89-30.22, compositions/flex-all.html) ══════════
   backgroundPosition 0%->100% on a 400% bg == sliding a 4x layer under an
   overflow-hidden box: (4-1)*2320 = 6960px x, (4-1)*1480 = 4440px y. */
var FA_START = 29.89, FA_DUR = 0.33;

function FlexAll() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('fa-slide', { x: 0, y: 0 }, { x: -6960, y: -4440, duration: 0.3, ease: 'none' }, A(0));
    tl.fromTo('fa-text', { scale: 1.08 }, { scale: 1, duration: 0.15, ease: 'power2.out' }, A(0));
  });
  return (
    <div id="fa-root" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
      <div class="absolute left-[-200px] top-[-200px] w-[2320px] h-[1480px] overflow-hidden">
        <div id="fa-slide" class="absolute left-0 top-0 w-[9280px] h-[5920px] bg-[linear-gradient(135deg,#e63946_0%,#88ce02_14%,#4ecdc4_28%,#d4586a_42%,#d4a0a0_56%,#f5a623_70%,#ffe14d_84%,#457b9d_100%)]"></div>
      </div>
      <div class="absolute left-0 top-[540px] w-full flex justify-center">
        <text id="fa-text" class="mt-[-150px] text-[300px] leading-none tracking-[-0.04em] font-[hf-grotesk-bold] text-white [text-shadow:0_8px_30px_rgba(0,0,0,0.4)]">all</text>
      </div>
    </div>
  );
}

/* ══ flex-compose (master 30.22-32.44, compositions/flex-compose.html) ══
   Stage 840x540 centred at (960,510) (y -30 baked into top). The iris
   clip-circle is static and the glow never animates — both omitted. */
var FC_START = 30.22, FC_DUR = 2.22;
var FC_WAVE_H = [8, 18, 12, 22, 10, 20, 14, 24, 8, 16, 22, 10, 18, 14, 20, 12, 24, 8, 16, 22, 10, 18, 14, 20, 8];

function FlexCompose() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('fc2-video', { scale: 0.9, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.3, ease: 'power2.out' }, A(0));
    tl.fromTo('fc2-lower', { y: 30, opacity: 0 }, { y: 0, opacity: 1, duration: 0.25, ease: 'power2.out' }, A(0.3));
    tl.fromTo('fc2-caption', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' }, A(0.6));
    tl.fromTo('fc2-frame', { opacity: 0, scale: 1.05 }, { opacity: 1, scale: 1, duration: 0.3, ease: 'power2.out' }, A(0.9));
    tl.fromTo('fc2-wave', { opacity: 0 }, { opacity: 1, duration: 0.15, ease: 'power2.out' }, A(1.4));
    for (var i = 0; i < 25; i++) {
      tl.fromTo('fc2-wb' + i, { height: 2 }, { height: FC_WAVE_H[i], duration: 0.15, ease: 'power2.out', yoyo: true, repeat: 1 }, A(1.42 + i * 0.008));
    }
    tl.fromTo('fc2-label', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' }, A(1.70));
    tl.fromTo('fc2-iris', { y: 0, filter: 'blur(0px)' }, { y: -150, filter: 'blur(30px)', duration: 0.33, ease: 'power2.in' }, A(1.89));
  });

  return (
    <div id="fc2-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f8f4ed]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_45%,#faf7f0_0%,#f0ebe2_100%)]"></div>
      <div id="fc2-iris" class="absolute inset-0">
        <div id="fc2-stage" class="absolute left-[540px] top-[240px] w-[840px] h-[540px]">
          <div id="fc2-video" class="absolute inset-0 rounded-[16px] overflow-hidden shadow-[0_20px_50px_rgba(40,30,10,0.18),0_4px_12px_rgba(40,30,10,0.1)] opacity-0">
            <video id="fc2-aroll" path="assets/pill.mp4" data-start={0} data-duration={FC_DUR} data-media-start="0" class="w-full h-full object-cover" />
          </div>
          <div id="fc2-frame" class="absolute left-[-6px] top-[-6px] w-[852px] h-[552px] border-[3px] border-[#c84f1c] rounded-[22px] opacity-0"></div>
          <div id="fc2-lower" class="absolute left-[30px] bottom-[40px] px-[18px] py-[12px] bg-[rgba(248,244,237,0.94)] rounded-[6px] border-l-[4px] border-[#c84f1c] opacity-0">
            <text class="text-[20px] leading-none tracking-[0.01em] font-[hf-geist-semi] text-[#1e1e1e]">Sarah Chen</text>
            <text class="mt-[2px] text-[14px] leading-none font-[hf-geist] text-[#6a5a40]">Product Designer</text>
          </div>
          <div class="absolute left-0 bottom-[120px] w-full flex justify-center">
            <text id="fc2-caption" class="px-[20px] py-[8px] rounded-[6px] bg-[rgba(26,20,10,0.75)] text-[22px] leading-none font-[hf-geist] text-[#f8f4ed] whitespace-nowrap opacity-0">building something people actually want to use</text>
          </div>
          <div id="fc2-wave" class="absolute left-0 bottom-[-50px] w-full h-[30px] flex items-center justify-center gap-[3px] opacity-0">
            {FC_WAVE_H.map(function (_, i) {
              return (
                <div id={'fc2-wb' + i} class="w-[4px] rounded-[2px] bg-[#c84f1c] h-[2px]"></div>
              );
            })}
          </div>
        </div>
        <div class="absolute left-0 bottom-[80px] w-full flex justify-center">
          <text id="fc2-label" class="text-[28px] leading-none tracking-[0.06em] font-[hf-jbm] text-[#8a6a2a] opacity-0">composes together</text>
        </div>
      </div>
    </div>
  );
}

/* ══ engine (master 32.44-41.48 slot 9.04s; template 10.68s — exit tween
   clipped like the master, engine.html) ══════════════════════════════════
   6000px vertical canvas, camera pans 0 -> -1460 -> -4310. SVG icons are
   flattened to <path> (jsx svg supports paths only). Flow lines draw via
   strokeDashoffset 700 -> 0. */
var EN_START = 32.44, EN_DUR = 9.04;

function Engine() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };

    tl.fromTo('en-wrap', { y: 150, filter: 'blur(30px)' }, { y: 0, filter: 'blur(0px)', duration: 1.0, ease: 'power2.out' }, A(0));
    ['en-r1o', 'en-r2o', 'en-r3o'].forEach(function (id) {
      tl.fromTo(id, { rotation: 0 }, { rotation: 360, duration: 10.68, ease: 'none' }, A(0));
    });
    ['en-r1i', 'en-r2i', 'en-r3i'].forEach(function (id) {
      tl.fromTo(id, { rotation: 0 }, { rotation: -360, duration: 10.68, ease: 'none' }, A(0));
    });

    tl.fromTo('en-step1', { opacity: 0, y: 80, scale: 0.88 }, { opacity: 1, y: 0, scale: 1, duration: 0.6, ease: 'expo.out' }, A(0.3));
    tl.fromTo('en-flow1', { strokeDashoffset: 700 }, { strokeDashoffset: 0, duration: 0.7, ease: 'power1.inOut' }, A(1.5));
    tl.fromTo('en-pip1', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power2.out' }, A(2.0));
    tl.fromTo('en-canvas', { y: 0 }, { y: -1460, duration: 1.2, ease: 'power2.inOut' }, A(1.8));
    tl.fromTo('en-step2', { opacity: 0, y: 80, scale: 0.88 }, { opacity: 1, y: 0, scale: 1, duration: 0.6, ease: 'expo.out' }, A(2.5));
    tl.fromTo('en-flow2', { strokeDashoffset: 700 }, { strokeDashoffset: 0, duration: 0.7, ease: 'power1.inOut' }, A(3.2));
    tl.fromTo('en-pip2', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power2.out' }, A(3.7));
    tl.fromTo('en-step3', { opacity: 0, y: 80, scale: 0.88 }, { opacity: 1, y: 0, scale: 1, duration: 0.5, ease: 'expo.out' }, A(3.8));
    tl.fromTo('en-canvas', { y: -1460 }, { y: -4310, duration: 2.3, ease: 'power2.inOut' }, A(3.3));

    tl.fromTo('en-word', { opacity: 0, y: 40, scale: 0.9 }, { opacity: 1, y: 0, scale: 1, duration: 0.5, ease: 'expo.out' }, A(5.2));
    tl.fromTo('en-card-f', { x: 0, rotation: 0 }, { x: 180, rotation: 3, duration: 0.7, ease: 'power2.out' }, A(6.0));
    tl.fromTo('en-card-b', { x: 0, rotation: 0 }, { x: -180, rotation: -3, duration: 0.7, ease: 'power2.out' }, A(6.0));
    tl.fromTo('en-proof', { opacity: 0, y: 20 }, { opacity: 1, y: 0, duration: 0.4, ease: 'back.out(1.4)' }, A(7.0));
    tl.fromTo('en-card-f', { y: 0 }, { y: -6, duration: 0.6, ease: 'sine.inOut', yoyo: true, repeat: 1 }, A(9.2));
    tl.fromTo('en-card-b', { y: 0 }, { y: 6, duration: 0.6, ease: 'sine.inOut', yoyo: true, repeat: 1 }, A(9.2));
    tl.fromTo('en-wrap', { y: 0, filter: 'blur(0px)' }, { y: -150, filter: 'blur(30px)', duration: 0.33, ease: 'power2.in' }, A(10.35));
  });

  /* duplicated det-card content (front/back) */
  function DetCard(id) {
    return (
      <div id={id} class="absolute left-0 top-0 w-[640px] h-[420px] rounded-[20px] overflow-hidden shadow-[0_24px_64px_rgba(0,0,0,0.15)] border-[2px] border-[rgba(61,64,91,0.1)]">
        <div class="w-full h-full bg-[#2a2a30] px-[32px] py-[32px]">
          <div class="w-[288px] h-[14px] rounded-[7px] bg-[#E07A5F] mb-[28px]"></div>
          <div class="flex items-center gap-[16px] mb-[20px]">
            <div class="w-[32px] h-[32px] rounded-[6px] bg-[#81B29A] shrink-0"></div>
            <div class="h-[10px] rounded-[5px] bg-[rgba(255,255,255,0.12)] w-[411px]"></div>
          </div>
          <div class="flex items-center gap-[16px] mb-[20px]">
            <div class="w-[32px] h-[32px] rounded-full bg-[#F2CC8F] shrink-0"></div>
            <div class="h-[10px] rounded-[5px] bg-[rgba(255,255,255,0.12)] w-[301.6px]"></div>
          </div>
          <div class="flex items-center gap-[16px] mb-[20px]">
            <div class="relative w-[32px] h-[28px] shrink-0">
              <path view-box="0 0 32 28" d="M16 0L32 28L0 28Z" class="absolute left-0 top-0 w-[32px] h-[28px] fill-[#E07A5F]" />
            </div>
            <div class="h-[10px] rounded-[5px] bg-[rgba(255,255,255,0.12)] w-[356.4px]"></div>
          </div>
          <div class="mt-[20px] h-[24px] rounded-[4px] bg-[repeating-linear-gradient(90deg,rgba(255,255,255,0.08)_0px,rgba(255,255,255,0.08)_6px,transparent_6px,transparent_10px)]"></div>
        </div>
      </div>
    );
  }

  var RING_I = 'absolute left-[36px] top-[36px] w-[288px] h-[288px] rounded-full border-[2px] border-dashed opacity-[0.2]';
  var RING_O = 'absolute inset-0 rounded-full border-[2.5px] opacity-[0.12]';
  var TITLE = 'text-[96px] leading-none tracking-[-0.02em] font-[hf-serif] text-[#3D405B]';
  var SUB = 'text-[28px] leading-none tracking-[0.06em] font-[hf-jbm] text-[#E07A5F]';
  return (
    <div id="en-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#F4F1DE] overflow-hidden">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_30%,rgba(255,252,240,0.6)_0%,transparent_50%),linear-gradient(180deg,#F4F1DE_0%,#ece8d2_100%)]"></div>
      <div id="en-wrap" class="absolute inset-0">
        <div id="en-canvas" class="absolute left-0 top-0 w-[1920px] h-[6000px]">
          {/* STEP 1 — Agent */}
          <div id="en-step1" class="absolute left-[510px] top-[360px] w-[900px] flex flex-col items-center gap-[32px] opacity-0">
            <div class="relative w-[360px] h-[360px]">
              <div id="en-r1o" class={RING_O + ' border-[#3D405B]'}></div>
              <div id="en-r1i" class={RING_I + ' border-[#3D405B]'}></div>
              <div class="absolute left-[72px] top-[72px] w-[216px] h-[216px] rounded-full bg-[#eaeaf2] shadow-[0_16px_48px_rgba(0,0,0,0.1)] flex items-center justify-center">
                <div class="relative w-[100px] h-[100px]">
                  <path view-box="0 0 80 80" d="M30 26a10 10 0 1 0 20 0a10 10 0 1 0 -20 0Z" class="absolute left-0 top-0 w-[100px] h-[100px] fill-[#3D405B]" />
                  <path view-box="0 0 80 80" d="M18 60c0-12 10-22 22-22s22 10 22 22" class="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#3D405B] stroke-[4]" />
                  <path view-box="0 0 80 80" d="M52 16L58 10M56 24L62 24M54 32L60 35" class="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#3D405B] stroke-[3] opacity-[0.6]" />
                </div>
              </div>
            </div>
            <text class={TITLE}>Agent</text>
            <text class={SUB}>writes HTML</text>
          </div>
          <path id="en-flow1" view-box="0 0 4 700" d="M2 0L2 700" class="absolute left-[958px] top-[920px] w-[4px] h-[700px] fill-none stroke-[#c8c0a8] stroke-[3] [stroke-dasharray:700px] [stroke-dashoffset:700px]" />
          <div id="en-pip1" class="absolute left-[950px] top-[1610px] w-[20px] h-[20px] rounded-full bg-[#E07A5F] opacity-0"></div>

          {/* STEP 2 — Renderer */}
          <div id="en-step2" class="absolute left-[510px] top-[1820px] w-[900px] flex flex-col items-center gap-[32px] opacity-0">
            <div class="relative w-[360px] h-[360px]">
              <div id="en-r2o" class={RING_O + ' border-[#E07A5F]'}></div>
              <div id="en-r2i" class={RING_I + ' border-[#E07A5F]'}></div>
              <div class="absolute left-[72px] top-[72px] w-[216px] h-[216px] rounded-full bg-[#fef0eb] shadow-[0_16px_48px_rgba(0,0,0,0.1)] flex items-center justify-center">
                <div class="relative w-[100px] h-[100px]">
                  <path view-box="0 0 80 80" d="M14 40a26 26 0 1 0 52 0a26 26 0 1 0 -52 0" class="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#E07A5F] stroke-[4]" />
                  <path view-box="0 0 80 80" d="M26 40a14 14 0 1 0 28 0a14 14 0 1 0 -28 0" class="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#E07A5F] stroke-[2] opacity-[0.4]" />
                  <path view-box="0 0 80 80" d="M35 40a5 5 0 1 0 10 0a5 5 0 1 0 -10 0Z" class="absolute left-0 top-0 w-[100px] h-[100px] fill-[#E07A5F]" />
                  <path view-box="0 0 80 80" d="M40 6L40 16M40 64L40 74M6 40L16 40M64 40L74 40" class="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#E07A5F] stroke-[3]" />
                </div>
              </div>
            </div>
            <text class={TITLE}>Renderer</text>
            <text class={SUB}>captures every frame</text>
          </div>
          <path id="en-flow2" view-box="0 0 4 700" d="M2 0L2 700" class="absolute left-[958px] top-[2380px] w-[4px] h-[700px] fill-none stroke-[#c8c0a8] stroke-[3] [stroke-dasharray:700px] [stroke-dashoffset:700px]" />
          <div id="en-pip2" class="absolute left-[950px] top-[3070px] w-[20px] h-[20px] rounded-full bg-[#E07A5F] opacity-0"></div>

          {/* STEP 3 — MP4 */}
          <div id="en-step3" class="absolute left-[510px] top-[3280px] w-[900px] flex flex-col items-center gap-[32px] opacity-0">
            <div class="relative w-[360px] h-[360px]">
              <div id="en-r3o" class={RING_O + ' border-[#81B29A]'}></div>
              <div id="en-r3i" class={RING_I + ' border-[#81B29A]'}></div>
              <div class="absolute left-[72px] top-[72px] w-[216px] h-[216px] rounded-full bg-[#eaf5ef] shadow-[0_16px_48px_rgba(0,0,0,0.1)] flex items-center justify-center">
                <div class="relative w-[100px] h-[100px]">
                  <path view-box="0 0 80 80" d="M22 10H58A12 12 0 0 1 70 22V58A12 12 0 0 1 58 70H22A12 12 0 0 1 10 58V22A12 12 0 0 1 22 10Z" class="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#81B29A] stroke-[4]" />
                  <path view-box="0 0 80 80" d="M32 24L56 40L32 56Z" class="absolute left-0 top-0 w-[100px] h-[100px] fill-[#81B29A]" />
                </div>
              </div>
            </div>
            <text class={TITLE}>MP4</text>
            <text class={SUB}>ready to ship</text>
          </div>

          {/* DETERMINISTIC */}
          <div class="absolute left-[260px] top-[4400px] w-[1400px] flex flex-col items-center gap-[48px]">
            <div class="relative w-[640px] h-[420px]">
              {DetCard('en-card-b')}
              {DetCard('en-card-f')}
            </div>
            <text id="en-word" class="text-[120px] leading-none tracking-[-0.03em] font-[hf-serif] text-[#3D405B] opacity-0">Deterministic.</text>
            <div id="en-proof" class="flex items-center gap-[16px] px-[32px] py-[14px] rounded-[14px] bg-[rgba(129,178,154,0.1)] border-[2px] border-[rgba(129,178,154,0.25)] opacity-0">
              <div class="relative w-[40px] h-[40px] shrink-0">
                <path view-box="0 0 40 40" d="M2 20a18 18 0 1 0 36 0a18 18 0 1 0 -36 0Z" class="absolute left-0 top-0 w-[40px] h-[40px] fill-[#81B29A]" />
                <path view-box="0 0 40 40" d="M12 20l6 6 10-10" class="absolute left-0 top-0 w-[40px] h-[40px] fill-none stroke-[#F4F1DE] stroke-[3]" />
              </div>
              <text class="text-[28px] italic font-[hf-serif-it] text-[#3D405B]">Identical output, every time.</text>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ══ cta (master 41.48-46.02, compositions/cta.html) ═══════════════════
   Workspace window: left terminal types two commands; right preview panel
   assembles a mini composition + streaming code panel. Template runs 5.28s
   but the master slot is 4.54s — the zoom-out exit (@4.95) and the mini-b1
   settle (@4.6) are cut by the master and therefore omitted. Typing and the
   code stream are tl.call(textContent) in the reference → per-frame .text()
   here; cursor blinks are steps(1) yoyo tweens → per-frame square wave. */
var CT_START = 41.48, CT_DUR = 4.54;
var CT_CMD1 = 'npx hyperframes init';
var CT_CMD2 = 'a product intro with kinetic typography';
var CT_CODE_LINES = [
  '<div data-composition-id="intro"',
  '  data-width="1920"',
  '  data-height="1080"',
  '  data-duration="10">',
  '',
  '  <div class="title">',
  '    Kinetic Typography',
  '  </div>',
  '',
  '  <script>',
  '    const tl = gsap.timeline();',
  '    tl.from(".title", {',
  '      opacity: 0, y: 40,',
  '      duration: 0.8',
  '    });',
  '  </script>',
  '</div>'
];

function Cta() {
  useGSAP((ctx) => {
    var T = ctx.current.time;

    /* typed 1 — one char per 0.9/20s from 0.3 (index.html cta.html:458-464) */
    var n1 = T < 0.3 ? 0 : Math.min(CT_CMD1.length, Math.floor((T - 0.3) / (0.9 / CT_CMD1.length)) + 1);
    ctx.getNode('cta-typed-1').text(CT_CMD1.slice(0, n1));
    /* cursor 1 — blink 6x0.12s steps(1) @0.3, fade @1.25 (456, 467) */
    var c1;
    if (T < 0.3 || (T >= 1.02 && T < 1.25)) c1 = 1;
    else if (T < 1.02) c1 = (Math.floor((T - 0.3) / 0.12) % 2 === 0) ? 1 : 0;
    else if (T < 1.35) c1 = 1 - (T - 1.25) / 0.1;
    else c1 = 0;
    ctx.getNode('cta-cursor-1').opacity(c1);

    /* typed 2 — one char per 1.1/39s from 2.15 (488-494) */
    var n2 = T < 2.15 ? 0 : Math.min(CT_CMD2.length, Math.floor((T - 2.15) / (1.1 / CT_CMD2.length)) + 1);
    ctx.getNode('cta-typed-2').text(CT_CMD2.slice(0, n2));
    /* cursor 2 — fade in @2.05, blink 8x0.12s @2.15, stays on (479, 486) */
    var c2;
    if (T < 2.05) c2 = 0;
    else if (T < 2.15) c2 = (T - 2.05) / 0.1;
    else if (T < 3.11) c2 = (Math.floor((T - 2.15) / 0.12) % 2 === 0) ? 1 : 0;
    else c2 = 1;
    ctx.getNode('cta-cursor-2').opacity(c2);

    /* code stream — one CODE_LINES entry per 0.06s from 3.55 (509-518) */
    var nL = T < 3.55 ? 0 : Math.min(CT_CODE_LINES.length, Math.floor((T - 3.55) / 0.06) + 1);
    ctx.getNode('cta-code-text').text(nL > 0 ? CT_CODE_LINES.slice(0, nL).join('\n') : '');

    /* status swap at 4.5 (556-559) — two stacked spans, no color mutation */
    var building = T < 4.5;
    ctx.getNode('cta-status-build').opacity(building ? 1 : 0);
    ctx.getNode('cta-status-ready').opacity(building ? 0 : 1);

    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('cta-wrap', { y: 150, filter: 'blur(30px)' }, { y: 0, filter: 'blur(0px)', duration: 1.0, ease: 'power2.out' }, A(0));
    tl.fromTo('sl-1', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.15, ease: 'power2.out' }, A(1.3));
    tl.fromTo('sl-2', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.12, ease: 'power2.out' }, A(1.42));
    tl.fromTo('sl-3', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.12, ease: 'power2.out' }, A(1.5));
    tl.fromTo('sl-4', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.12, ease: 'power2.out' }, A(1.58));
    tl.fromTo('sl-5', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.2, ease: 'back.out(1.4)' }, A(1.72));
    tl.fromTo('cta-line-2', { opacity: 0 }, { opacity: 1, duration: 0.15, ease: 'power2.out' }, A(2.05));
    tl.fromTo('cta-right', { x: 400, opacity: 0 }, { x: 0, opacity: 1, duration: 0.5, ease: 'power2.out' }, A(3.3));
    tl.fromTo('cta-code', { opacity: 0 }, { opacity: 1, duration: 0.3, ease: 'power2.out' }, A(3.5));
    tl.fromTo('cta-code-text', { y: 0 }, { y: -40, duration: 0.8, ease: 'none' }, A(4.0));
    tl.fromTo('mini-title', { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' }, A(3.6));
    tl.fromTo('mini-b1', { opacity: 0, scale: 0.6 }, { opacity: 1, scale: 1, duration: 0.2, ease: 'back.out(1.5)' }, A(3.75));
    tl.fromTo('mini-b2', { opacity: 0, x: 60 }, { opacity: 1, x: 0, duration: 0.2, ease: 'power2.out' }, A(3.85));
    tl.fromTo('mini-l1', { scaleX: 0 }, { scaleX: 1, duration: 0.2, ease: 'power2.out' }, A(3.9));
    tl.fromTo('mini-l2', { scaleX: 0 }, { scaleX: 1, duration: 0.2, ease: 'power2.out' }, A(3.95));
    tl.fromTo('mini-b3', { opacity: 0, y: 20 }, { opacity: 1, y: 0, duration: 0.2, ease: 'power2.out' }, A(4.0));
    tl.fromTo('mini-circ', { opacity: 0, scale: 0.3 }, { opacity: 1, scale: 1, duration: 0.25, ease: 'back.out(1.5)' }, A(4.05));
    tl.fromTo('mini-tl-fill', { scaleX: 0 }, { scaleX: 1, duration: 0.6, ease: 'power1.out' }, A(3.8));
    tl.fromTo('cta-ready', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.25, ease: 'back.out(1.5)' }, A(4.5));
  });

  return (
    <div id="cta-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f7f3eb]">
      <div class="absolute inset-0 bg-[linear-gradient(160deg,#f7f3eb_0%,#efe9dd_100%)]"></div>
      <div id="cta-wrap" class="absolute left-[120px] top-[60px] w-[1680px] h-[960px] rounded-[20px] bg-white shadow-[0_24px_80px_rgba(60,50,30,0.12),0_4px_16px_rgba(60,50,30,0.06)] overflow-hidden flex flex-col">
        {/* toolbar */}
        <div class="h-[52px] shrink-0 bg-[#faf8f4] border-b border-[#e8e2d6] flex items-center gap-[8px] px-[20px]">
          <div class="w-[12px] h-[12px] rounded-full bg-[#ff5f57] shrink-0"></div>
          <div class="w-[12px] h-[12px] rounded-full bg-[#febc2e] shrink-0"></div>
          <div class="w-[12px] h-[12px] rounded-full bg-[#28c840] shrink-0"></div>
          <text class="ml-[16px] text-[15px] font-[hf-geist-semi] text-[#8a7e6a] tracking-[0.02em]">Terminal — zsh</text>
        </div>
        <div class="flex-1 flex overflow-hidden">
          {/* left terminal */}
          <div class="w-[600px] shrink-0 border-r border-[#e8e2d6] pt-[40px] pb-[40px] pl-[36px] pr-[36px] flex flex-col overflow-hidden">
            <div class="flex items-center gap-[10px] h-[40px] shrink-0">
              <div class="relative w-[10px] h-[18px] mr-[8px] shrink-0">
                <path view-box="0 0 10 18" d="M1 1l8 8-8 8" class="absolute left-0 top-0 w-[10px] h-[18px] fill-none stroke-[#81B29A] stroke-[2.5]" />
              </div>
              <text id="cta-typed-1" class="text-[20px] font-[hf-jbm] text-[#2a2418]"></text>
              <div id="cta-cursor-1" class="w-[11px] h-[22px] bg-[#2a2418] ml-[1px] shrink-0"></div>
            </div>
            <div class="flex flex-col gap-[4px] pt-[12px] pb-[16px] pl-[28px]">
              <text id="sl-1" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">Creating project structure...</text>
              <text id="sl-2" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">{"  index.html"}</text>
              <text id="sl-3" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">{"  meta.json"}</text>
              <text id="sl-4" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">{"  compositions/"}</text>
              <div id="sl-5" class="flex items-center text-[15px] leading-[1.6]">
                <div class="relative w-[11px] h-[11px] mr-[6px] shrink-0">
                  <path view-box="0 0 12 12" d="M1.5 6.5l3 3 6-7" class="absolute left-0 top-0 w-[11px] h-[11px] fill-none stroke-[#81B29A] stroke-[2]" />
                </div>
                <text class="font-[hf-jbm] text-[#8a7e6a]">Project initialized</text>
              </div>
            </div>
            <div id="cta-line-2" class="mt-[8px] flex items-center gap-[10px] h-[40px] shrink-0">
              <div class="relative w-[10px] h-[18px] mr-[8px] shrink-0">
                <path view-box="0 0 10 18" d="M1 1l8 8-8 8" class="absolute left-0 top-0 w-[10px] h-[18px] fill-none stroke-[#81B29A] stroke-[2.5]" />
              </div>
              <text id="cta-typed-2" class="text-[20px] font-[hf-jbm] text-[#2a2418]"></text>
              <div id="cta-cursor-2" class="w-[11px] h-[22px] bg-[#2a2418] ml-[1px] shrink-0"></div>
            </div>
          </div>
          {/* right preview */}
          <div id="cta-right" class="flex-1 relative bg-[#faf8f4] overflow-hidden">
            <div class="h-[44px] bg-[#f0ebe3] border-b border-[#e0d8ca] flex items-center justify-between px-[20px]">
              <text class="text-[13px] font-[hf-geist] text-[#8a7e6a] tracking-[0.04em] uppercase">Preview</text>
              <div class="relative">
                <text id="cta-status-build" class="text-[12px] font-[hf-jbm] text-[#c8860a]">Building...</text>
                <text id="cta-status-ready" class="absolute right-0 top-0 text-[12px] font-[hf-jbm] text-[#5a8a6e] opacity-0">Ready</text>
              </div>
            </div>
            <div class="absolute left-0 top-[44px] right-0 bottom-[120px] overflow-hidden">
              <div class="absolute inset-0 bg-[linear-gradient(135deg,#f9f5ef_0%,#f0e8d8_100%)]"></div>
              <div id="mini-title" class="absolute left-[60px] top-[80px] text-[72px] leading-[1.05] tracking-[-0.03em] font-[hf-serif] text-[#1a1510]">
                <text>Kinetic</text>
                <text>Typography</text>
              </div>
              <div id="mini-b1" class="absolute top-[100px] right-[60px] w-[200px] h-[200px] rounded-[12px] bg-[#E07A5F]"></div>
              <div id="mini-b2" class="absolute top-[320px] right-[60px] w-[200px] h-[80px] rounded-[12px] bg-[#81B29A]"></div>
              <div id="mini-b3" class="absolute bottom-[100px] left-[60px] w-[280px] h-[60px] rounded-[8px] bg-[#3D405B]"></div>
              <div id="mini-l1" class="absolute top-[300px] left-[60px] w-[320px] h-[8px] rounded-[4px] bg-[rgba(60,50,30,0.1)] [transform-origin:0%_50%]"></div>
              <div id="mini-l2" class="absolute top-[330px] left-[60px] w-[220px] h-[8px] rounded-[4px] bg-[rgba(60,50,30,0.1)] [transform-origin:0%_50%]"></div>
              <div id="mini-circ" class="absolute bottom-[100px] right-[80px] w-[100px] h-[100px] rounded-full border-[4px] border-[#F2CC8F]"></div>
              <div class="absolute bottom-[30px] left-[40px] right-[40px] h-[6px] rounded-[3px] bg-[rgba(60,50,30,0.08)] overflow-hidden">
                <div id="mini-tl-fill" class="h-full w-full rounded-[3px] bg-[#E07A5F] [transform-origin:0%_50%]"></div>
              </div>
            </div>
            <div id="cta-code" class="absolute left-0 right-0 bottom-0 h-[120px] bg-[#2a2820] border-t border-[#3a3830] overflow-hidden pt-[12px] pb-[12px] pl-[16px] pr-[16px]">
              <text id="cta-code-text" class="text-[11px] leading-[1.6] font-[hf-jbm] text-[rgba(255,255,255,0.5)]"></text>
            </div>
            <div id="cta-ready" class="absolute top-[52px] right-[12px] flex items-center gap-[6px] rounded-[8px] pl-[8px] pr-[14px] py-[6px] bg-[rgba(129,178,154,0.15)] border-[1.5px] border-[rgba(129,178,154,0.3)]">
              <div class="relative w-[18px] h-[18px] shrink-0">
                <path view-box="0 0 20 20" d="M2 10a8 8 0 1 0 16 0a8 8 0 1 0 -16 0Z" class="absolute left-0 top-0 w-[18px] h-[18px] fill-[#81B29A]" />
                <path view-box="0 0 20 20" d="M6 10l3 3 5-5" class="absolute left-0 top-0 w-[18px] h-[18px] fill-none stroke-[#fff] stroke-[1.5]" />
              </div>
              <text class="text-[13px] font-[hf-geist] text-[#5a8a6e]">Ready</text>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* ══ canvas-close (master 46.02-49.77, compositions/canvas-close.html) ══
   ASCII-terminal lightning reveal. The reference is a <canvas> with 2D
   fillText per character cell (10x16px grid), driven by a GSAP proxy
   tween 0→3.93 linear → here the scene clock `t` (useCanvas passes
   ctx.current.time) drives the same renderAtTime. Text goes through ly.surface (2D subset, fontdb
   resolves the registered "JetBrains Mono" face — same 0.6em advance as
   the reference's DejaVu Sans Mono; baseline +13px = its top-baseline
   offset at 14px), then ly.draw samples it on the stage canvas. Master
   slot is 3.75s so the last 0.18s of the 3.93s timeline
   is truncated by the master, exactly like the reference cut. */
var CC_START = 46.02, CC_DUR = 3.75;
var CC_CELL_W = 10, CC_CELL_H = 16;
var CC_COLS = Math.floor(1920 / CC_CELL_W); /* 192 */
var CC_ROWS = Math.floor(1080 / CC_CELL_H); /* 67 */
var CC_GLYPHS = ' .,:;+*#@W';
var CC_GLYPH_MAX = CC_GLYPHS.length - 1;
var CC_BAYER = [
  [0.03125, 0.53125, 0.15625, 0.65625],
  [0.78125, 0.28125, 0.90625, 0.40625],
  [0.21875, 0.71875, 0.09375, 0.59375],
  [0.96875, 0.46875, 0.84375, 0.34375]
];

function CChash(x, y) {
  var n = x * 374761393 + y * 668265263;
  n = (n ^ (n >> 13)) * 1274126177;
  return ((n ^ (n >> 16)) & 0x7fffffff) / 0x7fffffff;
}
function CCvnoise(x, y) {
  var ix = Math.floor(x), iy = Math.floor(y);
  var fx = x - ix, fy = y - iy;
  fx = fx * fx * (3 - 2 * fx);
  fy = fy * fy * (3 - 2 * fy);
  var a = CChash(ix, iy), b = CChash(ix + 1, iy);
  var c = CChash(ix, iy + 1), d = CChash(ix + 1, iy + 1);
  return a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy;
}
function CCfbm(x, y) {
  return CCvnoise(x, y) * 0.5 + CCvnoise(x * 2.1 + 5.3, y * 2.1 + 3.7) * 0.3 +
         CCvnoise(x * 4.3 + 11.7, y * 4.3 + 7.1) * 0.2;
}
function CCclamp01(v) { return v < 0 ? 0 : (v > 1 ? 1 : v); }

/* 5x5 bitmap font (canvas-close.html:71-98) */
var CC_BFONT = {
  'A': [0x0E, 0x11, 0x1F, 0x11, 0x11],
  'B': [0x1E, 0x11, 0x1E, 0x11, 0x1E],
  'C': [0x0F, 0x10, 0x10, 0x10, 0x0F],
  'D': [0x1E, 0x11, 0x11, 0x11, 0x1E],
  'E': [0x1F, 0x10, 0x1E, 0x10, 0x1F],
  'F': [0x1F, 0x10, 0x1E, 0x10, 0x10],
  'G': [0x0F, 0x10, 0x13, 0x11, 0x0F],
  'H': [0x11, 0x11, 0x1F, 0x11, 0x11],
  'I': [0x1F, 0x04, 0x04, 0x04, 0x1F],
  'J': [0x07, 0x02, 0x02, 0x12, 0x0C],
  'K': [0x11, 0x12, 0x1C, 0x12, 0x11],
  'L': [0x10, 0x10, 0x10, 0x10, 0x1F],
  'M': [0x11, 0x1B, 0x15, 0x11, 0x11],
  'N': [0x11, 0x19, 0x15, 0x13, 0x11],
  'O': [0x0E, 0x11, 0x11, 0x11, 0x0E],
  'P': [0x1E, 0x11, 0x1E, 0x10, 0x10],
  'R': [0x1E, 0x11, 0x1E, 0x12, 0x11],
  'S': [0x0F, 0x10, 0x0E, 0x01, 0x1E],
  'T': [0x1F, 0x04, 0x04, 0x04, 0x04],
  'U': [0x11, 0x11, 0x11, 0x11, 0x0E],
  'V': [0x11, 0x11, 0x11, 0x0A, 0x04],
  'W': [0x11, 0x11, 0x15, 0x1B, 0x11],
  'X': [0x11, 0x0A, 0x04, 0x0A, 0x11],
  'Y': [0x11, 0x0A, 0x04, 0x04, 0x04],
  'Z': [0x1F, 0x02, 0x04, 0x08, 0x1F],
  ' ': [0, 0, 0, 0, 0]
};

function CCtextWS(text, scale) {
  var stride = 5 * scale + scale;
  return text.length * stride - scale;
}
function CCtextHit(text, sx, sy, c, r, scale) {
  var charW = 5 * scale, charH = 5 * scale;
  var stride = charW + scale;
  if (r < sy || r >= sy + charH || c < sx) return 0;
  var lc = c - sx;
  if (lc >= text.length * stride) return 0;
  var ci = Math.floor(lc / stride);
  var ic = lc - ci * stride;
  if (ci >= text.length || ic >= charW) return 0;
  var bx = Math.floor(ic / scale);
  var by = Math.floor((r - sy) / scale);
  var g = CC_BFONT[text[ci].toUpperCase()];
  if (!g) return 0;
  return (g[by] >> (4 - bx)) & 1;
}

/* text layout (canvas-close.html:125-160) */
var CC_MAIN_SCALE = 2, CC_SUB_SCALE = 1;
var CC_MAIN_TEXT = 'HYPERFRAMES';
var CC_SUB_TEXT = 'GO MAKE SOMETHING';
var CC_mainCharH = 5 * CC_MAIN_SCALE, CC_subCharH = 5 * CC_SUB_SCALE;
var CC_gap = 6;
var CC_groupY = Math.floor((CC_ROWS - (CC_mainCharH + CC_gap + CC_subCharH)) / 2);
var CC_mainX = Math.floor((CC_COLS - CCtextWS(CC_MAIN_TEXT, CC_MAIN_SCALE)) / 2);
var CC_mainY = CC_groupY;
var CC_subX = Math.floor((CC_COLS - CCtextWS(CC_SUB_TEXT, CC_SUB_SCALE)) / 2);
var CC_subY = CC_groupY + CC_mainCharH + CC_gap;
var CC_MASK = new Uint8Array(CC_ROWS * CC_COLS);
(function () {
  var mainW = CCtextWS(CC_MAIN_TEXT, CC_MAIN_SCALE);
  for (var r = CC_mainY; r < CC_mainY + CC_mainCharH && r < CC_ROWS; r++) {
    for (var c = CC_mainX; c < CC_mainX + mainW && c < CC_COLS; c++) {
      if (CCtextHit(CC_MAIN_TEXT, CC_mainX, CC_mainY, c, r, CC_MAIN_SCALE)) CC_MASK[r * CC_COLS + c] = 1;
    }
  }
  var subW = CCtextWS(CC_SUB_TEXT, CC_SUB_SCALE);
  for (var r2 = CC_subY; r2 < CC_subY + CC_subCharH && r2 < CC_ROWS; r2++) {
    for (var c2 = CC_subX; c2 < CC_subX + subW && c2 < CC_COLS; c2++) {
      if (CCtextHit(CC_SUB_TEXT, CC_subX, CC_subY, c2, r2, CC_SUB_SCALE)) CC_MASK[r2 * CC_COLS + c2] = 2;
    }
  }
})();

/* lightning bolts (canvas-close.html:167-278) */
function CCgenerateBolt(startX, targetX, seed, branchCount, targetY) {
  var tgtY = (targetY !== undefined) ? targetY : CC_mainY;
  var segments = [];
  var x = startX, y = 0;
  var step = 0;
  while (y < tgtY) {
    var stepY = 3 + Math.floor(CChash(seed + step, 100) * 4);
    var progress = Math.min(1.0, y / tgtY);
    var attract = (targetX - x) * progress * 0.35;
    var jitter = (CChash(seed + step, 200) - 0.5) * 14 * (1.0 - progress * 0.8);
    var stepX = attract + jitter;
    var newX = Math.max(3, Math.min(CC_COLS - 3, Math.floor(x + stepX)));
    var newY = Math.min(tgtY, y + stepY);
    segments.push({ x1: x, y1: y, x2: newX, y2: newY });
    x = newX; y = newY; step++;
  }
  if (segments.length > 0) {
    var last = segments[segments.length - 1];
    if (last.y2 < tgtY) segments.push({ x1: last.x2, y1: last.y2, x2: targetX, y2: tgtY });
  }
  var strikeCol = segments.length > 0 ? segments[segments.length - 1].x2 : targetX;
  var branches = [];
  var bc = branchCount || Math.floor(CChash(seed, 300) * 3) + 2;
  for (var bi = 0; bi < bc; bi++) {
    var segIdx = Math.floor(CChash(seed + bi * 13, 400) * segments.length);
    var seg = segments[Math.min(segIdx, segments.length - 1)];
    var bx = seg.x2, by = seg.y2;
    var bsegs = [];
    var bseed = seed + 1000 + bi * 77;
    var bstep = 0;
    var blen = 3 + Math.floor(CChash(bseed, 500) * 4);
    var dir = CChash(bseed, 600) > 0.5 ? 1 : -1;
    for (var bs = 0; bs < blen && by < tgtY; bs++) {
      var bStepY = 2 + Math.floor(CChash(bseed + bstep, 700) * 3);
      var bStepX = dir * (2 + CChash(bseed + bstep, 800) * 6);
      var nbx = Math.max(2, Math.min(CC_COLS - 2, Math.floor(bx + bStepX)));
      var nby = Math.min(tgtY, by + bStepY);
      bsegs.push({ x1: bx, y1: by, x2: nbx, y2: nby });
      bx = nbx; by = nby; bstep++;
    }
    branches.push({ segments: bsegs, brightness: 0.3 + CChash(bseed, 900) * 0.35 });
  }
  return { segments: segments, branches: branches, strikeCol: strikeCol, strikeRow: tgtY };
}
function CCbuildCrawlMap(startCol, startRow, bfsGap) {
  var GAP = bfsGap || 3;
  var dist = new Float32Array(CC_ROWS * CC_COLS);
  for (var i = 0; i < dist.length; i++) dist[i] = -1;
  var bestD = 9999, bestC = startCol, bestR = startRow;
  for (var dr = -8; dr <= 8; dr++) {
    for (var dc = -8; dc <= 8; dc++) {
      var nr = startRow + dr, nc = startCol + dc;
      if (nr >= 0 && nr < CC_ROWS && nc >= 0 && nc < CC_COLS && CC_MASK[nr * CC_COLS + nc]) {
        var d2 = dr * dr + dc * dc;
        if (d2 < bestD) { bestD = d2; bestC = nc; bestR = nr; }
      }
    }
  }
  if (bestD === 9999) return dist;
  var queue = [bestR * CC_COLS + bestC];
  dist[bestR * CC_COLS + bestC] = 0;
  var head = 0;
  while (head < queue.length) {
    var cur = queue[head++];
    var curDist = dist[cur];
    var cr = Math.floor(cur / CC_COLS), cc = cur % CC_COLS;
    for (var dr2 = -GAP; dr2 <= GAP; dr2++) {
      for (var dc2 = -GAP; dc2 <= GAP; dc2++) {
        if (dr2 === 0 && dc2 === 0) continue;
        var nr3 = cr + dr2, nc3 = cc + dc2;
        if (nr3 < 0 || nr3 >= CC_ROWS || nc3 < 0 || nc3 >= CC_COLS) continue;
        var ni = nr3 * CC_COLS + nc3;
        if (dist[ni] >= 0) continue;
        if (!CC_MASK[ni]) continue;
        dist[ni] = curDist + Math.sqrt(dr2 * dr2 + dc2 * dc2);
        queue.push(ni);
      }
    }
  }
  return dist;
}
function CCletterX(idx) {
  return CC_mainX + idx * (5 * CC_MAIN_SCALE + CC_MAIN_SCALE) + Math.floor(5 * CC_MAIN_SCALE / 2);
}
function CCsubLetterX(idx) {
  return CC_subX + idx * (5 * CC_SUB_SCALE + CC_SUB_SCALE) + Math.floor(5 * CC_SUB_SCALE / 2);
}
/* Bolts and their crawl maps depend only on the constant glyph geometry above
   (never on `t`), but the JSX module body re-runs every frame to reset
   globals. Rebuilding the four Dijkstra crawl maps per frame costs ~55 ms,
   which dominated the whole render. Memoize the table on the realm (user
   globals persist across frames) so it is built once. ccRender only reads
   CC_BOLTS / crawlMap, so sharing the tables is pixel-exact. */
var CC_BOLTS = globalThis.__ccBolts || (globalThis.__ccBolts = (function () {
  var bolts = [
    { strikeTime: 0.25, decayRate: 8, bolt: CCgenerateBolt(30, CCletterX(0), 3001, 3), mega: false, crawlSpeed: 70 },
    { strikeTime: 0.55, decayRate: 8, bolt: CCgenerateBolt(160, CCletterX(10), 3002, 3), mega: false, crawlSpeed: 70 },
    { strikeTime: 0.85, decayRate: 0, bolt: CCgenerateBolt(96, CCletterX(5), 3777, 8), mega: true, crawlSpeed: 300, holdTime: 3.0 },
    { strikeTime: 1.9, decayRate: 0, bolt: CCgenerateBolt(96, CCsubLetterX(8), 4888, 14, CC_subY), mega: true, superMega: true, crawlSpeed: 500, holdTime: 2.0 }
  ];
  for (var i = 0; i < bolts.length; i++) {
    bolts[i].crawlMap = CCbuildCrawlMap(bolts[i].bolt.strikeCol, bolts[i].bolt.strikeRow, i === 3 ? 8 : 3);
  }
  return bolts;
})());

/* illumination (canvas-close.html:283-376) */
var CC_ILLUM = new Float32Array(CC_ROWS * CC_COLS);
var CC_CRAWL = new Float32Array(CC_ROWS * CC_COLS);

function CCboltBrightness(bolt, t) {
  var dt = t - bolt.strikeTime;
  if (dt < 0) return 0;
  if (bolt.mega) {
    if (dt > (bolt.holdTime || 0) + 1.0) return 0;
    var attack = CCclamp01(dt / 0.02);
    var decayRate = bolt.superMega ? 2.5 : 4.0;
    var decay = Math.exp(-dt * decayRate);
    var flicker = 0;
    if (dt > 0.04 && dt < 0.08) flicker = (bolt.superMega ? 0.8 : 0.5) * Math.exp(-Math.pow(dt - 0.06, 2) * 8000);
    if (dt > 0.10 && dt < 0.16) flicker = (bolt.superMega ? 0.6 : 0.3) * Math.exp(-Math.pow(dt - 0.13, 2) * 5000);
    if (bolt.superMega && dt > 0.18 && dt < 0.25) flicker += 0.4 * Math.exp(-Math.pow(dt - 0.21, 2) * 4000);
    var holdGlow = (dt < (bolt.holdTime || 0)) ? (bolt.superMega ? 0.25 : 0.15) : 0;
    return attack * Math.max(decay + flicker, holdGlow);
  }
  if (dt > 1.5) return 0;
  var atk = CCclamp01(dt / 0.02);
  var dec = Math.exp(-dt * bolt.decayRate);
  var fl = 0;
  if (dt > 0.04 && dt < 0.08) fl = 0.5 * Math.exp(-Math.pow(dt - 0.06, 2) * 8000);
  if (dt > 0.10 && dt < 0.16) fl = 0.3 * Math.exp(-Math.pow(dt - 0.13, 2) * 5000);
  return atk * (dec + fl);
}
function CCpaintSegGlow(x1, y1, x2, y2, strength, radius) {
  var mnR = Math.max(0, Math.floor(Math.min(y1, y2) - radius));
  var mxR = Math.min(CC_ROWS - 1, Math.ceil(Math.max(y1, y2) + radius));
  var mnC = Math.max(0, Math.floor(Math.min(x1, x2) - radius));
  var mxC = Math.min(CC_COLS - 1, Math.ceil(Math.max(x1, x2) + radius));
  var dx = x2 - x1, dy = y2 - y1;
  var lenSq = dx * dx + dy * dy;
  for (var r = mnR; r <= mxR; r++) {
    for (var c = mnC; c <= mxC; c++) {
      var dist;
      if (lenSq < 0.001) {
        var ex = c - x1, ey = r - y1;
        dist = Math.sqrt(ex * ex + ey * ey);
      } else {
        var t2 = ((c - x1) * dx + (r - y1) * dy) / lenSq;
        if (t2 < 0) t2 = 0; if (t2 > 1) t2 = 1;
        var px = x1 + t2 * dx, py = y1 + t2 * dy;
        var ex2 = c - px, ey2 = r - py;
        dist = Math.sqrt(ex2 * ex2 + ey2 * ey2);
      }
      if (dist < radius) {
        var val = strength * (1.0 - dist / radius);
        var idx = r * CC_COLS + c;
        if (val > CC_ILLUM[idx]) CC_ILLUM[idx] = val;
      }
    }
  }
}
function CCpaintImpact(cx, cy, strength, radius) {
  var mnR = Math.max(0, Math.floor(cy - radius));
  var mxR = Math.min(CC_ROWS - 1, Math.ceil(cy + radius));
  var mnC = Math.max(0, Math.floor(cx - radius));
  var mxC = Math.min(CC_COLS - 1, Math.ceil(cx + radius));
  for (var r = mnR; r <= mxR; r++) {
    for (var c = mnC; c <= mxC; c++) {
      var dx = c - cx, dy = r - cy;
      var dist = Math.sqrt(dx * dx + dy * dy);
      if (dist < radius) {
        var falloff = 1.0 - (dist / radius);
        var val = strength * falloff * falloff;
        var idx = r * CC_COLS + c;
        if (val > CC_ILLUM[idx]) CC_ILLUM[idx] = val;
      }
    }
  }
}
function CCpaintBolt(boltData, bb, mega, superMega) {
  var radius = superMega ? 65 : (mega ? 45 : 20);
  var segs = boltData.segments;
  for (var i = 0; i < segs.length; i++) CCpaintSegGlow(segs[i].x1, segs[i].y1, segs[i].x2, segs[i].y2, bb, radius);
  var brs = boltData.branches;
  for (var j = 0; j < brs.length; j++) {
    var br = brs[j];
    var brSegs = br.segments;
    var brStr = bb * br.brightness;
    var brRad = radius * 0.6;
    for (var k = 0; k < brSegs.length; k++) CCpaintSegGlow(brSegs[k].x1, brSegs[k].y1, brSegs[k].x2, brSegs[k].y2, brStr, brRad);
  }
  var impactRadius = superMega ? 80 : (mega ? 55 : 28);
  CCpaintImpact(boltData.strikeCol, boltData.strikeRow, bb * (superMega ? 1.8 : 1.2), impactRadius);
}

/* render one frame into the surface, then draw it onto the stage canvas
   (canvas-close.html:381-510; fillText baseline = top + 13px ascent) */
function ccRender(cv, ctx, t) {
  var surf = ly.surface(1920, 1080);
  surf.font = '400 14px "JetBrains Mono", monospace';
  surf.clearRect(0, 0, 1920, 1080);

  var fadeIn = CCclamp01(t / 0.4);
  var i;
  for (i = 0; i < CC_ILLUM.length; i++) CC_ILLUM[i] = 0;
  CC_CRAWL.fill(0);

  var bi, dt;
  for (bi = 0; bi < CC_BOLTS.length; bi++) {
    var B = CC_BOLTS[bi];
    var bb = CCboltBrightness(B, t);
    dt = t - B.strikeTime;
    if (bb > 0.001) CCpaintBolt(B.bolt, bb, B.mega, B.superMega);
    if (dt > 0) {
      var crawlMap = B.crawlMap;
      var crawlRadius = dt * B.crawlSpeed;
      var holdTime = B.holdTime || 0;
      var crawlDecay;
      if (B.mega) crawlDecay = (dt < holdTime) ? 1.0 : Math.exp(-(dt - holdTime) * 3.0);
      else crawlDecay = Math.exp(-dt * 5.0);
      if (crawlDecay < 0.005) continue;
      for (var ci = 0; ci < crawlMap.length; ci++) {
        if (crawlMap[ci] < 0) continue;
        if (crawlMap[ci] > crawlRadius) continue;
        var waveDist = crawlRadius - crawlMap[ci];
        var waveBright = crawlDecay;
        if (waveDist < 8 && dt < 1.0) waveBright *= (1.0 + (1.0 - waveDist / 8) * 2.0);
        if (waveBright > CC_CRAWL[ci]) CC_CRAWL[ci] = waveBright;
      }
    }
  }

  var lastFill = '';
  for (var row = 0; row < CC_ROWS; row++) {
    for (var col = 0; col < CC_COLS; col++) {
      var idx = row * CC_COLS + col;
      var ambN = CCfbm(col * 0.03 + t * 0.08, row * 0.04 + t * 0.06);
      var ambBright = 0.02 + ambN * 0.04;
      if (t < 0.25) {
        var flickerN = CChash(col * 7 + Math.floor(t * 15), row * 11 + Math.floor(t * 15));
        if (flickerN > 0.997) ambBright += 0.15 * (1.0 - t / 0.25);
      }
      var localIllum = CC_ILLUM[idx];
      var crawlIllum = CC_CRAWL[idx];
      var textType = CC_MASK[idx];
      var brightness, cr, cg, cb;
      var effectiveCrawl = crawlIllum;
      if (textType && effectiveCrawl > 0.01) {
        var ci2 = CCclamp01(effectiveCrawl);
        brightness = ci2 * fadeIn;
        var whiteness = CCclamp01(effectiveCrawl - 0.8) * 3.0;
        cr = Math.round((40 + whiteness * 215 + ci2 * 180) * brightness);
        cg = Math.round((15 + whiteness * 160 + ci2 * 100) * brightness);
        cb = Math.round((5 + whiteness * 140 + ci2 * 30) * brightness);
        if (localIllum > 0.1) {
          var boost = localIllum * 0.4;
          cr = Math.min(255, cr + Math.round(boost * 220));
          cg = Math.min(255, cg + Math.round(boost * 150));
          cb = Math.min(255, cb + Math.round(boost * 80));
          brightness = Math.min(1.0, brightness + boost);
        }
      } else if (localIllum > 0.03) {
        brightness = localIllum * fadeIn;
        var coreAmt = CCclamp01((localIllum - 0.4) * 2.5);
        cr = Math.round((coreAmt * 240 + localIllum * 80) * brightness);
        cg = Math.round((coreAmt * 160 + localIllum * 50) * brightness);
        cb = Math.round((coreAmt * 60 + localIllum * 20) * brightness);
      } else {
        brightness = ambBright * fadeIn;
        cr = Math.round((8 + ambN * 15) * brightness);
        cg = Math.round((4 + ambN * 8) * brightness);
        cb = Math.round((2 + ambN * 4) * brightness);
      }

      var dither = CC_BAYER[row & 3][col & 3];
      var sl = brightness * CC_GLYPH_MAX;
      var bl = Math.floor(sl);
      var fr = sl - bl;
      var rl = bl + (fr > dither ? 1 : 0);
      if (rl > CC_GLYPH_MAX) rl = CC_GLYPH_MAX;
      if (rl < 1 && brightness > 0.01) rl = 1;
      if (brightness < 0.01) rl = 0;
      if (rl === 0) continue;

      var scanline = 0.92 + 0.08 * Math.sin(row * 0.6 + t * 0.8);
      var vx = col / CC_COLS - 0.5, vy = row / CC_ROWS - 0.5;
      var vig = 1.0 - (vx * vx + vy * vy) * 0.35;
      if (vig < 0.55) vig = 0.55;
      var m = scanline * vig;
      var frr = Math.round(cr * m);
      var fgg = Math.round(cg * m);
      var fbb = Math.round(cb * m);
      if (frr > 255) frr = 255;
      if (fgg > 255) fgg = 255;
      if (fbb > 255) fbb = 255;

      var fill = 'rgb(' + frr + ',' + fgg + ',' + fbb + ')';
      if (fill !== lastFill) {
        surf.fillStyle = fill;
        lastFill = fill;
      }
      surf.fillText(CC_GLYPHS[rl], col * CC_CELL_W, row * CC_CELL_H + 13);
    }
  }

  /* The surface is sampled straight by an identity kernel: ly.draw takes the
     surface as a source and bakes it on demand, so the script no longer
     manages a frame-scoped generated-image key. */
  var ccIdent = ly.pixel((uv, src) => {
    var c = src.eval(uv);
    if (c.a > 0.0) { return [c.r / c.a, c.g / c.a, c.b / c.a, c.a]; }
    return [0.0, 0.0, 0.0, 0.0];
  });
  ly.draw(cv, ccIdent, { rect: [0, 0, 1920, 1080], sources: [surf] });
}

function CanvasClose() {
  const canvas = useRef();

  useCanvas(canvas, (c, t, ctx) => ccRender(c, ctx, Math.min(t, 3.93)));

  return (
    <div id="cc-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#000]">
      <canvas ref={canvas} id="cc-canvas" class="w-[1920px] h-[1080px]" />
    </div>
  );
}

/* ══ master — index.html: 16 sub-compositions on a 49.77s stage,
   voiceover.mp3 @0, 14 caption chips. Master cuts several template
   timelines short (flex-gsap, thesis, engine exits) — each component's
   window matches the master data-start/duration table, so the truncation
   falls out of the scene window. ═════════════════════════════ */
function HyperframesLaunch() {
  return (
    <div id="hf-root" class="relative w-[1920px] h-[1080px] bg-[#0a0a0a] overflow-hidden">
      {/* The master stage is a `<tl>`: 16 sub-compositions on a 49.77s timeline.
          Each scene declares its own segment length; `ctx.current` inside a
          scene is its local clock, starting at 0 where the scene becomes
          visible. glass-intro and flex-css share a 0.94s fade, which is why
          they overlap in the reference. The rest hard-cut. The 1.13s gap
          between flex-gsap and flex-shader is the reference's own empty beat —
          a blank spacer segment holds it. */}
      <tl id="hf-tl" class="absolute inset-0">
        <GlassIntro duration={17.30} />
        {/* The reference hard-cuts at 17.30 — glass-intro's tree is removed and
            flex-css is drawn over it. The overlap still counts as time, so the
            transition declares the span with no effect. */}
        <transition from="gi-root" to="fc-root" effect="cut" duration={0.94} />
        {/* flex-css shows for 1.71s: the 0.94s fade it shares with glass-intro
            plus its own 0.77s segment. */}
        <FlexCss duration={0.77} />
        <FlexGsap duration={0.67} />
        <Spacer duration={1.13} />
        <FlexShader duration={1.20} />
        <FlexThreejs duration={1.21} />
        <Thesis duration={3.82} />
        <FlexDrop duration={0.36} />
        <FlexMusic duration={0.58} />
        <FlexSfx duration={0.93} />
        <FlexFootage duration={0.83} />
        <FlexIt duration={0.15} />
        <FlexAll duration={0.33} />
        <FlexCompose duration={2.22} />
        <Engine duration={9.04} />
        <Cta duration={4.54} />
        <CanvasClose duration={3.75} />
      </tl>
      <CaptionLayer />
      <audio id="hf-vo" path="assets/voiceover.mp3" attach="hf-root" />
    </div>
  );
}

/* An empty beat. Holds timeline time without rendering anything — the
   reference leaves 1.13s of bare background between flex-gsap and
   flex-shader. */
function Spacer() {
  return <div id="spacer" class="absolute left-0 top-0 w-[1920px] h-[1080px]"></div>;
}

export default HyperframesLaunch;
