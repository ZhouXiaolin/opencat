import { onFrame, useTween } from '../../runtime';
import { gsap } from 'gsap';
import { PlayerBar, entryEase, zoomBlur } from '../shared';

/* BEAT 2 (:430,530-622): entry 1.45->0.88 entryEase 0.9s, hold ->0.80 blur2
   linear 0.6333s, exit ->0.40 blur16 expo.in 0.2s. Master 2.10-3.85. */
export default function Beat2() {
  useTween('glass-intro', 0, function (tl) {
    var T = tl.time();
    var B = ['gi-zoom-b2'];

    tl.set('gi-zoom-b2', { scale: 1.45 }, 2.1);
    tl.set('gi-bar2', { opacity: 0, y: 12 }, 2.1);

    tl.fromTo('gi-zoom-b2', { scale: 1.45 }, { scale: 0.88, duration: 0.9, ease: entryEase }, 2.1);
    zoomBlur(tl, B, 'blur(14px)', 'blur(0px)', 0.9, entryEase, 0);
    tl.fromTo('gi-zoom-b2', { scale: 0.88 }, { scale: 0.80, duration: 0.6333, ease: 'none' }, 3.0);
    zoomBlur(tl, B, 'blur(0px)', 'blur(2px)', 0.6333, 'none', 0.9);
    zoomBlur(tl, ['gi-bar2'], 'blur(0px)', 'blur(2px)', 0.6333, 'none', 0.9);
    tl.fromTo('gi-bar2', { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 2.6);
    tl.to('gi-bar2-fill', { width: 1332, duration: 1.75, ease: 'none' }, 2.1);
    tl.to('gi-bar2-scrub', { x: 1332, duration: 1.75, ease: 'none' }, 2.1);
    tl.fromTo('gi-zoom-b2', { scale: 0.80 }, { scale: 0.40, duration: 0.2, ease: 'expo.in' }, 3.6333);
    zoomBlur(tl, B, 'blur(2px)', 'blur(16px)', 0.2, 'expo.in', 1.5333);
    zoomBlur(tl, ['gi-bar2'], 'blur(2px)', 'blur(16px)', 0.2, 'expo.in', 1.5333);

    /* time readout — reference drives it with an onUpdate proxy
       (:583-598): sec = floor(progress). */
    if (T >= 0 && T < 1.75) document.getElementById('gi-bar2-time').textContent = '0:0' + Math.min(2, Math.floor(T)) + ' / 0:02';
  });
  return (
    <div id="gi-beat2" data-start="2.1" data-duration="1.75" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
      <video id="gi-bg-video" src="assets/bg.mp4" data-start="2.1" data-duration="1.75" class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover" />
      <div id="gi-zoom-b2" class="absolute left-0 top-0 w-[1920px] h-[1080px]">
        <div id="gi-frame" class="absolute left-[120px] top-[67.5px] w-[1680px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.16)] backdrop-blur-[14px] backdrop-saturate-[1.12] shadow-[0_30px_90px_rgba(0,0,0,0.42)] inset-shadow-[0_1px_0_rgba(255,255,255,0.22)] bg-[linear-gradient(160deg,rgba(255,255,255,0.075)_0%,rgba(255,255,255,0.025)_45%,rgba(255,255,255,0.010)_70%,rgba(255,255,255,0.055)_100%)]"></div>
        <div id="gi-figma-wrap" class="absolute left-[170px] top-[117.5px] w-[1580px] h-[758px] rounded-[40px] overflow-hidden shadow-[0_10px_28px_rgba(0,0,0,0.35)]">
          <video id="gi-figma-video" src="assets/figma-promo.mp4" data-start="2.1" data-duration="1.75" data-media-start="0" class="w-full h-full object-cover" />
        </div>
        <PlayerBar id="gi-bar2" left={210} w={1500} track={1332} time0="0:00 / 0:02" />
      </div>
    </div>
  );
}
