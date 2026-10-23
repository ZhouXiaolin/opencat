import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
import { zoomBlur } from '../../shared.jsx';

export default function Beat1() {
  useGSAP(() => {
    var tl = gsap.timeline();
    /* BEAT 1 (:505-526): per-word slide-from-right, expo.out 0.33s */
    var WS = [0.09, 0.42, 0.55, 0.85, 1.09, 1.40, 1.64];
    var WSL = [360, 180, 120, 80, 50, 30, 20];
    for (var w = 0; w < 7; w++) {
      tl.fromTo('gi-w' + w, { x: WSL[w], y: 14, opacity: 0 }, { x: 0, y: 0, opacity: 1, duration: 0.33, ease: 'expo.out' }, WS[w]);
    }
    tl.fromTo('gi-intro-text', { scale: 1, filter: 'blur(0px)' }, { scale: 0.6, filter: 'blur(16px)', duration: 0.2, ease: 'expo.in' }, 1.9);
  });
  return (
    <div id="gi-text-intro" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[radial-gradient(ellipse_at_50%_45%,#fcf6e7_0%,#f6edd4_100%)]">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_55%,transparent_45%,rgba(110,70,15,0.08)_100%)]"></div>
      <div id="gi-intro-mask" className="absolute left-0 top-0 w-[1920px] h-[1080px] overflow-hidden">
        <div className="absolute left-0 top-0 w-[1720px] h-full overflow-hidden">
          <div id="gi-intro-text" className="absolute left-0 top-0 w-[1920px] h-full flex items-center justify-center px-[40px]">
            <text id="gi-w0" className="text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">Imagine</text>
            <text id="gi-w1" className="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">you</text>
            <text id="gi-w2" className="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">could</text>
            <text id="gi-w3" className="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">make</text>
            <text id="gi-w4" className="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">videos</text>
            <text id="gi-w5" className="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">like</text>
            <text id="gi-w6" className="ml-[21.6px] text-[108px] leading-[1.08] tracking-[-0.02em] font-[hf-serif] text-[#14110a]">this</text>
          </div>
        </div>
      </div>
    </div>
  );
}
