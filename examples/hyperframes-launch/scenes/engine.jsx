import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';
/* ══ engine (master 32.44-41.48 slot 9.04s; template 10.68s — exit tween
   clipped like the master, engine.html) ══════════════════════════════════
   6000px vertical canvas, camera pans 0 -> -1460 -> -4310. SVG icons are
   flattened to <path> (jsx svg supports paths only). Flow lines draw via
   strokeDashoffset 700 -> 0. */
var EN_START = 32.44, EN_DUR = 9.04;

export default function Engine() {
  useGSAP(() => {
    var T = ctx.current.time;
    var tl = gsap.timeline(), A = function (t) { return t; };

    tl.fromTo('en-wrap', { y: 150, filter: 'blur(30px)' }, { y: 0, filter: 'blur(0px)', duration: 1.0, ease: 'power2.out' }, A(0));
    ['en-r1o', 'en-r2o', 'en-r3o'].forEach(function (id) {
      tl.fromTo(id, { rotation: 0 }, { rotation: 360, duration: 10.68, ease: 'none' }, A(0));
    });
    ['en-r1i', 'en-r2i', 'en-r3i'].forEach(function (id) {
      tl.fromTo(id, { rotation: 0 }, { rotation: -360, duration: 10.68, ease: 'none' }, A(0));
    });

    tl.fromTo('en-step1', { opacity: 0, y: 80, scale: 0.88 }, { opacity: 1, y: 0, scale: 1, duration: 0.6, ease: 'expo.out' }, A(0.3));
    tl.fromTo('en-flow1', { strokeDashoffset: 700 }, { strokeDashoffset: 0, duration: 0.7, ease: 'power1.inOut' }, A(1.5));
    tl.fromTo('en-pip1', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power2.out' }, A(2.0));
    tl.fromTo('en-canvas', { y: 0 }, { y: -1460, duration: 1.2, ease: 'power2.inOut' }, A(1.8));
    tl.fromTo('en-step2', { opacity: 0, y: 80, scale: 0.88 }, { opacity: 1, y: 0, scale: 1, duration: 0.6, ease: 'expo.out' }, A(2.5));
    tl.fromTo('en-flow2', { strokeDashoffset: 700 }, { strokeDashoffset: 0, duration: 0.7, ease: 'power1.inOut' }, A(3.2));
    tl.fromTo('en-pip2', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power2.out' }, A(3.7));
    tl.fromTo('en-step3', { opacity: 0, y: 80, scale: 0.88 }, { opacity: 1, y: 0, scale: 1, duration: 0.5, ease: 'expo.out' }, A(3.8));
    tl.fromTo('en-canvas', { y: -1460 }, { y: -4310, duration: 2.3, ease: 'power2.inOut' }, A(3.3));

    tl.fromTo('en-word', { opacity: 0, y: 40, scale: 0.9 }, { opacity: 1, y: 0, scale: 1, duration: 0.5, ease: 'expo.out' }, A(5.2));
    tl.fromTo('en-card-f', { x: 0, rotation: 0 }, { x: 180, rotation: 3, duration: 0.7, ease: 'power2.out' }, A(6.0));
    tl.fromTo('en-card-b', { x: 0, rotation: 0 }, { x: -180, rotation: -3, duration: 0.7, ease: 'power2.out' }, A(6.0));
    tl.fromTo('en-proof', { opacity: 0, y: 20 }, { opacity: 1, y: 0, duration: 0.4, ease: 'back.out(1.4)' }, A(7.0));
    tl.fromTo('en-card-f', { y: 0 }, { y: -6, duration: 0.6, ease: 'sine.inOut', yoyo: true, repeat: 1 }, A(9.2));
    tl.fromTo('en-card-b', { y: 0 }, { y: 6, duration: 0.6, ease: 'sine.inOut', yoyo: true, repeat: 1 }, A(9.2));
    tl.fromTo('en-wrap', { y: 0, filter: 'blur(0px)' }, { y: -150, filter: 'blur(30px)', duration: 0.33, ease: 'power2.in' }, A(10.35));
  });

  /* duplicated det-card content (front/back) */
  function DetCard(id) {
    return (
      <div id={id} className="absolute left-0 top-0 w-[640px] h-[420px] rounded-[20px] overflow-hidden shadow-[0_24px_64px_rgba(0,0,0,0.15)] border-[2px] border-[rgba(61,64,91,0.1)]">
        <div className="w-full h-full bg-[#2a2a30] px-[32px] py-[32px]">
          <div className="w-[288px] h-[14px] rounded-[7px] bg-[#E07A5F] mb-[28px]"></div>
          <div className="flex items-center gap-[16px] mb-[20px]">
            <div className="w-[32px] h-[32px] rounded-[6px] bg-[#81B29A] shrink-0"></div>
            <div className="h-[10px] rounded-[5px] bg-[rgba(255,255,255,0.12)] w-[432px]"></div>
          </div>
          <div className="flex items-center gap-[16px] mb-[20px]">
            <div className="w-[32px] h-[32px] rounded-full bg-[#F2CC8F] shrink-0"></div>
            <div className="h-[10px] rounded-[5px] bg-[rgba(255,255,255,0.12)] w-[316.8px]"></div>
          </div>
          <div className="flex items-center gap-[16px] mb-[20px]">
            <div className="relative w-[32px] h-[28px] shrink-0">
              <path viewBox="0 0 32 28" d="M16 0L32 28L0 28Z" className="absolute left-0 top-0 w-[32px] h-[28px] fill-[#E07A5F]" />
            </div>
            <div className="h-[10px] rounded-[5px] bg-[rgba(255,255,255,0.12)] w-[374.4px]"></div>
          </div>
          <div className="mt-[20px] h-[24px] rounded-[4px] bg-[repeating-linear-gradient(90deg,rgba(255,255,255,0.08)_0px,rgba(255,255,255,0.08)_6px,transparent_6px,transparent_10px)]"></div>
        </div>
      </div>
    );
  }

  var RING_I = 'absolute left-[36px] top-[36px] w-[288px] h-[288px] rounded-full border-[2px] border-dashed opacity-[0.2]';
  var RING_O = 'absolute inset-0 rounded-full border-[2.5px] opacity-[0.12]';
  var TITLE = 'text-[96px] leading-none tracking-[-0.02em] font-[hf-serif] text-[#3D405B]';
  var SUB = 'text-[28px] leading-none tracking-[0.06em] font-[hf-jbm] text-[#E07A5F]';
  return (
    <div id="en-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#F4F1DE] overflow-hidden">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_30%,rgba(255,252,240,0.6)_0%,transparent_50%),linear-gradient(180deg,#F4F1DE_0%,#ece8d2_100%)]"></div>
      <div id="en-wrap" className="absolute inset-0">
        <div id="en-canvas" className="absolute left-0 top-0 w-[1920px] h-[6000px]">
          {/* STEP 1 — Agent */}
          <div id="en-step1" className="absolute left-[510px] top-[360px] w-[900px] flex flex-col items-center gap-[32px] opacity-0">
            <div className="relative w-[360px] h-[360px]">
              <div id="en-r1o" className={RING_O + ' border-[#3D405B]'}></div>
              <div id="en-r1i" className={RING_I + ' border-[#3D405B]'}></div>
              <div className="absolute left-[72px] top-[72px] w-[216px] h-[216px] rounded-full bg-[#eaeaf2] shadow-[0_16px_48px_rgba(0,0,0,0.1)] flex items-center justify-center">
                <div className="relative w-[100px] h-[100px]">
                  <path viewBox="0 0 80 80" d="M30 26a10 10 0 1 0 20 0a10 10 0 1 0 -20 0Z" className="absolute left-0 top-0 w-[100px] h-[100px] fill-[#3D405B]" />
                  <path viewBox="0 0 80 80" d="M18 60c0-12 10-22 22-22s22 10 22 22" className="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#3D405B] stroke-[4]" />
                  <path viewBox="0 0 80 80" d="M52 16L58 10M56 24L62 24M54 32L60 35" className="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#3D405B] stroke-[3] opacity-[0.6]" />
                </div>
              </div>
            </div>
            <text className={TITLE}>Agent</text>
            <text className={SUB}>writes HTML</text>
          </div>
          <path id="en-flow1" viewBox="0 0 4 700" d="M2 0L2 700" className="absolute left-[958px] top-[920px] w-[4px] h-[700px] fill-none stroke-[#c8c0a8] stroke-[3] [stroke-dasharray:700px] [stroke-dashoffset:700px]" />
          <div id="en-pip1" className="absolute left-[950px] top-[1610px] w-[20px] h-[20px] rounded-full bg-[#E07A5F] opacity-0"></div>

          {/* STEP 2 — Renderer */}
          <div id="en-step2" className="absolute left-[510px] top-[1820px] w-[900px] flex flex-col items-center gap-[32px] opacity-0">
            <div className="relative w-[360px] h-[360px]">
              <div id="en-r2o" className={RING_O + ' border-[#E07A5F]'}></div>
              <div id="en-r2i" className={RING_I + ' border-[#E07A5F]'}></div>
              <div className="absolute left-[72px] top-[72px] w-[216px] h-[216px] rounded-full bg-[#fef0eb] shadow-[0_16px_48px_rgba(0,0,0,0.1)] flex items-center justify-center">
                <div className="relative w-[100px] h-[100px]">
                  <path viewBox="0 0 80 80" d="M14 40a26 26 0 1 0 52 0a26 26 0 1 0 -52 0" className="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#E07A5F] stroke-[4]" />
                  <path viewBox="0 0 80 80" d="M26 40a14 14 0 1 0 28 0a14 14 0 1 0 -28 0" className="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#E07A5F] stroke-[2] opacity-[0.4]" />
                  <path viewBox="0 0 80 80" d="M35 40a5 5 0 1 0 10 0a5 5 0 1 0 -10 0Z" className="absolute left-0 top-0 w-[100px] h-[100px] fill-[#E07A5F]" />
                  <path viewBox="0 0 80 80" d="M40 6L40 16M40 64L40 74M6 40L16 40M64 40L74 40" className="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#E07A5F] stroke-[3]" />
                </div>
              </div>
            </div>
            <text className={TITLE}>Renderer</text>
            <text className={SUB}>captures every frame</text>
          </div>
          <path id="en-flow2" viewBox="0 0 4 700" d="M2 0L2 700" className="absolute left-[958px] top-[2380px] w-[4px] h-[700px] fill-none stroke-[#c8c0a8] stroke-[3] [stroke-dasharray:700px] [stroke-dashoffset:700px]" />
          <div id="en-pip2" className="absolute left-[950px] top-[3070px] w-[20px] h-[20px] rounded-full bg-[#E07A5F] opacity-0"></div>

          {/* STEP 3 — MP4 */}
          <div id="en-step3" className="absolute left-[510px] top-[3280px] w-[900px] flex flex-col items-center gap-[32px] opacity-0">
            <div className="relative w-[360px] h-[360px]">
              <div id="en-r3o" className={RING_O + ' border-[#81B29A]'}></div>
              <div id="en-r3i" className={RING_I + ' border-[#81B29A]'}></div>
              <div className="absolute left-[72px] top-[72px] w-[216px] h-[216px] rounded-full bg-[#eaf5ef] shadow-[0_16px_48px_rgba(0,0,0,0.1)] flex items-center justify-center">
                <div className="relative w-[100px] h-[100px]">
                  <path viewBox="0 0 80 80" d="M22 10H58A12 12 0 0 1 70 22V58A12 12 0 0 1 58 70H22A12 12 0 0 1 10 58V22A12 12 0 0 1 22 10Z" className="absolute left-0 top-0 w-[100px] h-[100px] fill-none stroke-[#81B29A] stroke-[4]" />
                  <path viewBox="0 0 80 80" d="M32 24L56 40L32 56Z" className="absolute left-0 top-0 w-[100px] h-[100px] fill-[#81B29A]" />
                </div>
              </div>
            </div>
            <text className={TITLE}>MP4</text>
            <text className={SUB}>ready to ship</text>
          </div>

          {/* DETERMINISTIC */}
          <div className="absolute left-[260px] top-[4400px] w-[1400px] flex flex-col items-center gap-[48px]">
            <div className="relative w-[640px] h-[420px]">
              {DetCard('en-card-b')}
              {DetCard('en-card-f')}
            </div>
            <text id="en-word" className="text-[120px] leading-none tracking-[-0.03em] font-[hf-serif] text-[#3D405B] opacity-0">Deterministic.</text>
            <div id="en-proof" className="flex items-center gap-[16px] px-[32px] py-[14px] rounded-[14px] bg-[rgba(129,178,154,0.1)] border-[2px] border-[rgba(129,178,154,0.25)] opacity-0">
              <div className="relative w-[40px] h-[40px] shrink-0">
                <path viewBox="0 0 40 40" d="M2 20a18 18 0 1 0 36 0a18 18 0 1 0 -36 0Z" className="absolute left-0 top-0 w-[40px] h-[40px] fill-[#81B29A]" />
                <path viewBox="0 0 40 40" d="M12 20l6 6 10-10" className="absolute left-0 top-0 w-[40px] h-[40px] fill-none stroke-[#F4F1DE] stroke-[3]" />
              </div>
              <text className="text-[28px] italic font-[hf-serif-it] text-[#3D405B]">Identical output, every time.</text>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
