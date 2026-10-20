import { useGSAP } from 'opencat/runtime';
import { PlayerBar, entryEase, zoomBlur } from '../../shared.jsx';

/* BEAT 3 (:432,632-716): same shape @master 3.85, hold 0.6157s, bar @0.5001. */
export default function Beat3() {
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
        <div id="gi-frame2" class="absolute left-[120px] top-[67.5px] w-[1680px] h-[945px] rounded-[56px] border-[1px] border-[rgba(255,255,255,0.16)] backdrop-blur-[14px] backdrop-saturate-[1.12] shadow-[0_30px_90px_rgba(0,0,0,0.42)] inset-shadow-[0_1px_0_rgba(255,255,255,0.22)] bg-[linear-gradient(160deg,rgba(255,255,255,0.075)_0%,rgba(255,255,255,0.025)_45%,rgba(255,255,255,0.010)_70%,rgba(255,255,255,0.055)_100%)]"></div>
        <div id="gi-spotify-wrap" class="absolute left-[170px] top-[117.5px] w-[1580px] h-[758px] rounded-[40px] overflow-hidden shadow-[0_10px_28px_rgba(0,0,0,0.35)]">
          <video id="gi-spotify-video" path="assets/spotify-bento.mp4" data-start="0" data-duration="1.75" data-media-start="0" class="w-full h-full object-cover" />
        </div>
        <PlayerBar id="gi-bar3" left={210} w={1500} track={1332} time0="0:00 / 0:02" />
      </div>
    </div>
  );
}
