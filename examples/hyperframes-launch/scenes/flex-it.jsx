import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
/* ══ flex-it (master 29.74-29.89, compositions/flex-it.html) ════════════ */
var FI_START = 29.74, FI_DUR = 0.15;

export default function FlexIt() {
  useGSAP(() => {
    var T = ctx.current.time;
    gsap.timeline().fromTo('fi-text', { scale: 1.06 }, { scale: 1, duration: 0.12, ease: 'power2.out' }, 0);
  });
  return (
    <div id="fi-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-black">
      <div className="absolute left-0 top-[540px] w-full flex justify-center">
        <p id="fi-text" className="mt-[-160px] text-[320px] leading-none tracking-[-0.04em] font-[hf-inter-black] text-white">it</p>
      </div>
    </div>
  );
}
