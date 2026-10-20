import { useGSAP } from 'opencat/runtime';
export default function FlexDrop() {
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
