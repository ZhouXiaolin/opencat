import { useGSAP } from 'opencat/runtime';
/* ══ flex-all (master 29.89-30.22, compositions/flex-all.html) ══════════
   backgroundPosition 0%->100% on a 400% bg == sliding a 4x layer under an
   overflow-hidden box: (4-1)*2320 = 6960px x, (4-1)*1480 = 4440px y. */
var FA_START = 29.89, FA_DUR = 0.33;

export default function FlexAll() {
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
