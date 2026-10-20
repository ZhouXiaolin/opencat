import { useGSAP } from 'opencat/runtime';
import { PlayerBar, vis, clamp, zoomBlur } from '../../shared.jsx';

/* BEAT 4 (:434,731-877,913-942): master 5.60-18.24. Poster -> clip ->
   lastframe -> logo, the cursor-click full-frame scale-up, the drag wipe and
   the typewriter. All four reference overlay layers open inside this window,
   so they are siblings of the zoom wrapper — NOT inside it: the wrapper scales
   1.45->1 and 1->1.4248, and the reference moves the cursor in canvas space. */
export default function Beat4() {
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

    /* ── master :797-812 — only #beat4-wrap scales up; #frame-3 stays put
       (the wrap covers it once it fills the canvas). ── */
    tl.fromTo('gi-beat4-wrap', { scale: 1, y: 0 }, { scale: 1.4248, y: 62, duration: 0.4, ease: 'power3.out' }, 1.6);
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
        <div id="gi-frame3" class="absolute left-[236.5px] top-[67.5px] w-[1447px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.16)] backdrop-blur-[14px] backdrop-saturate-[1.12] shadow-[0_30px_90px_rgba(0,0,0,0.42)] inset-shadow-[0_1px_0_rgba(255,255,255,0.22)] bg-[linear-gradient(160deg,rgba(255,255,255,0.075)_0%,rgba(255,255,255,0.025)_45%,rgba(255,255,255,0.010)_70%,rgba(255,255,255,0.055)_100%)]"></div>
        <div id="gi-beat4-wrap" class="absolute left-[286.5px] top-[117.5px] w-[1347px] h-[758px] rounded-[40px] overflow-hidden shadow-[0_10px_28px_rgba(0,0,0,0.35)] [transform-origin:50%_55.74%]">
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
