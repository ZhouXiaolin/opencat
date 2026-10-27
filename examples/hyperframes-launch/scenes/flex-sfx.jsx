import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
/* ══ flex-sfx (master 27.98-28.91, compositions/flex-sfx.html) ══════════ */
var FX_START = 27.98, FX_DUR = 0.93;
var FX_HEIGHTS = [];
for (var _fxi = 0; _fxi < 48; _fxi++) {
  FX_HEIGHTS.push(Math.round(80 + 220 * Math.abs(Math.sin(_fxi * 0.7 + 0.3) * Math.cos(_fxi * 1.3 + 0.7)) + 60 * Math.abs(Math.sin(_fxi * 2.1 + 1.5))));
}

export default function FlexSfx() {
  useGSAP(() => {
    var T = ctx.current.time;
    var tl = gsap.timeline(), A = function (t) { return t; };
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
    <div id="fx-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#1a1208]">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#2a1f10_0%,#0e0a04_100%)]"></div>
      <div id="fx-glow" className="absolute left-[560px] top-[140px] w-[800px] h-[800px] bg-[radial-gradient(circle,rgba(200,132,10,0.25)_0%,transparent_70%)] opacity-0"></div>
      <div className="absolute left-[960px] top-[540px] w-0 h-0">
        {bars.map((i) => (
          <div id={'fx-b' + i} className="absolute bottom-0 left-[-3px] w-[6px] rounded-[3px] bg-[linear-gradient(to_top,#c8840a,#f5c842)] h-[4px] opacity-0 [transform-origin:50%_100%]"></div>
        ))}
      </div>
      <div className="absolute left-0 top-[540px] w-full flex justify-center">
        <p id="fx-text" className="mt-[-26px] text-[52px] leading-none tracking-[0.1em] font-[hf-inter-med] text-[#f5e8c8] uppercase [text-shadow:0_0_40px_rgba(200,132,10,0.5)] opacity-0">sound effects</p>
      </div>
    </div>
  );
}
