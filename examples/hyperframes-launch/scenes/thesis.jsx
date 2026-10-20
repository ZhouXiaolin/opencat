import { useGSAP } from 'opencat/runtime';
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

export default function Thesis() {
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
