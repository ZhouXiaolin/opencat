import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
/* ══ flex-footage (master 28.91-29.74, compositions/flex-footage.html) ══ */
var FF_START = 28.91, FF_DUR = 0.83;
var FF_VIDEOS = [
  ['ff-v1', 'assets/Frame 1.mp4'],
  ['ff-v2', 'assets/option2.mp4'],
  ['ff-v3', 'assets/Frame 3.mp4'],
];

export default function FlexFootage() {
  useGSAP(() => {
    var T = ctx.current.time;
    var tl = gsap.timeline(), A = function (t) { return t; };
    tl.fromTo('ff-text', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power2.out' }, A(0));
    tl.fromTo('ff-strip-top', { x: 0 }, { x: -100, duration: FF_DUR, ease: 'none' }, A(0));
    tl.fromTo('ff-strip-bottom', { x: -50 }, { x: -150, duration: FF_DUR, ease: 'none' }, A(0));
    tl.fromTo('ff-track', { x: 0 }, { x: -100, duration: FF_DUR, ease: 'none' }, A(0));
  });
  var STRIP = 'bg-[repeating-linear-gradient(90deg,transparent_0px,transparent_40px,#0d0d0d_40px,#0d0d0d_44px,transparent_44px,transparent_60px,rgba(255,255,255,0.06)_60px,rgba(255,255,255,0.06)_80px,transparent_80px,transparent_100px)]';
  return (
    <div id="ff-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#1a1a1a]">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#262626_0%,#1a1a1a_100%)]"></div>
      <div id="ff-track" className="absolute left-[-100px] top-[120px] flex gap-[40px]">
        {FF_VIDEOS.map((v) => (
          <div className="w-[680px] h-[840px] rounded-[16px] overflow-hidden shrink-0 shadow-[0_4px_20px_rgba(0,0,0,0.5)]">
            <video id={v[0]} src={v[1]} dataStart={0} dataDuration={FF_DUR} dataMediaStart="0" className="w-full h-full object-cover" />
          </div>
        ))}
      </div>
      <div id="ff-strip-top" className={'absolute left-0 top-0 w-[2200px] h-[80px] opacity-[0.7] ' + STRIP}></div>
      <div id="ff-strip-bottom" className={'absolute left-0 bottom-0 w-[2200px] h-[80px] opacity-[0.7] ' + STRIP}></div>
      <div className="absolute left-0 top-[540px] w-full flex justify-center">
        <text id="ff-text" className="mt-[-80px] text-[160px] leading-none tracking-[0.01em] italic font-[hf-eb-it] text-[#f0e6d2] [text-shadow:0_6px_24px_rgba(0,0,0,0.8),0_2px_8px_rgba(0,0,0,0.6)] opacity-0">footage</text>
      </div>
      {/* reference grain = one node with 3 radial-gradient layers at 3/5/7px
          tiles; engine bg-[length:...] takes a single layer, so 3 stacked
          nodes with identical visual result */}
      <div className="absolute inset-0 opacity-[0.6] bg-[radial-gradient(circle_at_20%_30%,rgba(255,255,255,0.03)_1px,transparent_1px)] bg-[length:3px_3px]"></div>
      <div className="absolute inset-0 opacity-[0.6] bg-[radial-gradient(circle_at_80%_70%,rgba(255,255,255,0.02)_1px,transparent_1px)] bg-[length:5px_5px]"></div>
      <div className="absolute inset-0 opacity-[0.6] bg-[radial-gradient(circle_at_50%_50%,rgba(255,255,255,0.015)_1px,transparent_1px)] bg-[length:7px_7px]"></div>
    </div>
  );
}
