import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
export const meta = { width: 1920, height: 1080, fps: 30, duration: 0.5 };
function Bar({ top, id }) {
  return (
    <div id={id} class="absolute h-[28px] flex items-center gap-[18px] opacity-0" style={'top:' + top + 'px;left:210px;width:1500px'}>
      <div class="relative w-[22px] h-[22px] shrink-0">
        <path view-box="0 0 22 22" d="M5 3.5 L18 11 L5 18.5 Z" class="absolute left-0 top-0 w-[22px] h-[22px] fill-[rgba(255,255,255,0.92)]" />
      </div>
      <div class="relative h-[3px] rounded-[2px] bg-[rgba(255,255,255,0.28)]" style="width:1332px">
        <div class="absolute left-0 top-0 h-full rounded-[2px] bg-white" style="width:420px" />
        <div class="absolute left-[-6px] top-[-4.5px] w-[12px] h-[12px] rounded-full bg-white shadow-[0_2px_6px_rgba(0,0,0,0.25)]" />
      </div>
      <text class="w-[110px] text-right text-[20px] leading-none tracking-[0.02em] opacity-[0.85] font-[hf-inter-med] text-[rgba(255,255,255,0.92)]">0:00 / 0:02</text>
    </div>
  );
}
function Min() {
  useGSAP(() => {
    var tl = gsap.timeline();
    tl.fromTo('zoomw', { filter: 'blur(14px)' }, { filter: 'blur(0px)', duration: 0.9, ease: 'none' }, 0);
    tl.fromTo('ba', { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, 0);
    tl.set('bb', { opacity: 0.31 }, 0);
  });
  return (
    <div class="relative w-[1920px] h-[1080px]">
      {/* 真实结构复刻：bg 视频 + zoom wrapper(blur tween 中) + 卡内 bar */}
      <video id="bg" path="assets/bg.mp4" data-start="0" data-duration="0.5" class="absolute left-0 top-0 w-[1920px] h-[1080px] object-cover" />
      <div id="zoomw" class="absolute left-0 top-0 w-[1920px] h-[1080px]" style="transform:scale(0.85)">
        <Bar top={930} id="ba" />
      </div>
      {/* 对照1：无 blur wrapper，video 底 + bar 0.31 */}
      <Bar top={400} id="bb" />
    </div>
  );
}
export function main() { return <Min />; }
