import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
/* ══ flex-compose (master 30.22-32.44, compositions/flex-compose.html) ══
   Stage 840x540 centred at (960,510) (y -30 baked into top). The iris
   clip-circle is static and the glow never animates — both omitted. */
var FC_START = 30.22, FC_DUR = 2.22;
var FC_WAVE_H = [8, 18, 12, 22, 10, 20, 14, 24, 8, 16, 22, 10, 18, 14, 20, 12, 24, 8, 16, 22, 10, 18, 14, 20, 8];

export default function FlexCompose() {
  useGSAP(() => {
    var T = ctx.current.time;
    var tl = gsap.timeline(), A = function (t) { return t; };
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
    <div id="fc2-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f8f4ed]">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_45%,#faf7f0_0%,#f0ebe2_100%)]"></div>
      <div id="fc2-iris" className="absolute inset-0">
        <div id="fc2-stage" className="absolute left-[540px] top-[240px] w-[840px] h-[540px]">
          <div id="fc2-video" className="absolute inset-0 rounded-[16px] overflow-hidden shadow-[0_20px_50px_rgba(40,30,10,0.18),0_4px_12px_rgba(40,30,10,0.1)] opacity-0">
            <video id="fc2-aroll" src="assets/pill.mp4" dataStart={0} dataDuration={FC_DUR} dataMediaStart="0" className="w-full h-full object-cover" />
          </div>
          <div id="fc2-frame" className="absolute left-[-6px] top-[-6px] w-[852px] h-[552px] border-[3px] border-[#c84f1c] rounded-[22px] opacity-0"></div>
          <div id="fc2-lower" className="absolute left-[30px] bottom-[40px] px-[18px] py-[12px] bg-[rgba(248,244,237,0.94)] rounded-[6px] border-l-[4px] border-[#c84f1c] opacity-0">
            <p className="text-[20px] leading-none tracking-[0.01em] font-[hf-geist-semi] text-[#1e1e1e]">Sarah Chen</p>
            <p className="mt-[2px] text-[14px] leading-none font-[hf-geist] text-[#6a5a40]">Product Designer</p>
          </div>
          <div className="absolute left-0 bottom-[120px] w-full flex justify-center">
            <p id="fc2-caption" className="px-[20px] py-[8px] rounded-[6px] bg-[rgba(26,20,10,0.75)] text-[22px] leading-none font-[hf-geist] text-[#f8f4ed] whitespace-nowrap opacity-0">building something people actually want to use</p>
          </div>
          <div id="fc2-wave" className="absolute left-0 bottom-[-50px] w-full h-[30px] flex items-center justify-center gap-[3px] opacity-0">
            {FC_WAVE_H.map(function (_, i) {
              return (
                <div id={'fc2-wb' + i} className="w-[4px] rounded-[2px] bg-[#c84f1c] h-[2px]"></div>
              );
            })}
          </div>
        </div>
        <div className="absolute left-0 bottom-[80px] w-full flex justify-center">
          <p id="fc2-label" className="text-[28px] leading-none tracking-[0.06em] font-[hf-jbm] text-[#8a6a2a] opacity-0">composes together</p>
        </div>
      </div>
    </div>
  );
}
