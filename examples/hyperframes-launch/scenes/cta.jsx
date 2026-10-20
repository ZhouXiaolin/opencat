import { useGSAP } from 'opencat/runtime';
/* ══ cta (master 41.48-46.02, compositions/cta.html) ═══════════════════
   Workspace window: left terminal types two commands; right preview panel
   assembles a mini composition + streaming code panel. Template runs 5.28s
   but the master slot is 4.54s — the zoom-out exit (@4.95) and the mini-b1
   settle (@4.6) are cut by the master and therefore omitted. Typing and the
   code stream are tl.call(textContent) in the reference → per-frame .text()
   here; cursor blinks are steps(1) yoyo tweens → per-frame square wave. */
var CT_START = 41.48, CT_DUR = 4.54;
var CT_CMD1 = 'npx hyperframes init';
var CT_CMD2 = 'a product intro with kinetic typography';
var CT_CODE_LINES = [
  '<div data-composition-id="intro"',
  '  data-width="1920"',
  '  data-height="1080"',
  '  data-duration="10">',
  '',
  '  <div class="title">',
  '    Kinetic Typography',
  '  </div>',
  '',
  '  <script>',
  '    const tl = gsap.timeline();',
  '    tl.from(".title", {',
  '      opacity: 0, y: 40,',
  '      duration: 0.8',
  '    });',
  '  </script>',
  '</div>'
];

export default function Cta() {
  useGSAP((ctx) => {
    var T = ctx.current.time;

    /* typed 1 — one char per 0.9/20s from 0.3 (index.html cta.html:458-464) */
    var n1 = T < 0.3 ? 0 : Math.min(CT_CMD1.length, Math.floor((T - 0.3) / (0.9 / CT_CMD1.length)) + 1);
    ctx.getNode('cta-typed-1').text(CT_CMD1.slice(0, n1));
    /* cursor 1 — blink 6x0.12s steps(1) @0.3, fade @1.25 (456, 467) */
    var c1;
    if (T < 0.3 || (T >= 1.02 && T < 1.25)) c1 = 1;
    else if (T < 1.02) c1 = (Math.floor((T - 0.3) / 0.12) % 2 === 0) ? 1 : 0;
    else if (T < 1.35) c1 = 1 - (T - 1.25) / 0.1;
    else c1 = 0;
    ctx.getNode('cta-cursor-1').opacity(c1);

    /* typed 2 — one char per 1.1/39s from 2.15 (488-494) */
    var n2 = T < 2.15 ? 0 : Math.min(CT_CMD2.length, Math.floor((T - 2.15) / (1.1 / CT_CMD2.length)) + 1);
    ctx.getNode('cta-typed-2').text(CT_CMD2.slice(0, n2));
    /* cursor 2 — fade in @2.05, blink 8x0.12s @2.15, stays on (479, 486) */
    var c2;
    if (T < 2.05) c2 = 0;
    else if (T < 2.15) c2 = (T - 2.05) / 0.1;
    else if (T < 3.11) c2 = (Math.floor((T - 2.15) / 0.12) % 2 === 0) ? 1 : 0;
    else c2 = 1;
    ctx.getNode('cta-cursor-2').opacity(c2);

    /* code stream — one CODE_LINES entry per 0.06s from 3.55 (509-518) */
    var nL = T < 3.55 ? 0 : Math.min(CT_CODE_LINES.length, Math.floor((T - 3.55) / 0.06) + 1);
    ctx.getNode('cta-code-text').text(nL > 0 ? CT_CODE_LINES.slice(0, nL).join('\n') : '');

    /* status swap at 4.5 (556-559) — two stacked spans, no color mutation */
    var building = T < 4.5;
    ctx.getNode('cta-status-build').opacity(building ? 1 : 0);
    ctx.getNode('cta-status-ready').opacity(building ? 0 : 1);

    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('cta-wrap', { y: 150, filter: 'blur(30px)' }, { y: 0, filter: 'blur(0px)', duration: 1.0, ease: 'power2.out' }, A(0));
    tl.fromTo('sl-1', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.15, ease: 'power2.out' }, A(1.3));
    tl.fromTo('sl-2', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.12, ease: 'power2.out' }, A(1.42));
    tl.fromTo('sl-3', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.12, ease: 'power2.out' }, A(1.5));
    tl.fromTo('sl-4', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.12, ease: 'power2.out' }, A(1.58));
    tl.fromTo('sl-5', { opacity: 0, y: 6 }, { opacity: 1, y: 0, duration: 0.2, ease: 'back.out(1.4)' }, A(1.72));
    tl.fromTo('cta-line-2', { opacity: 0 }, { opacity: 1, duration: 0.15, ease: 'power2.out' }, A(2.05));
    tl.fromTo('cta-right', { x: 400, opacity: 0 }, { x: 0, opacity: 1, duration: 0.5, ease: 'power2.out' }, A(3.3));
    tl.fromTo('cta-code', { opacity: 0 }, { opacity: 1, duration: 0.3, ease: 'power2.out' }, A(3.5));
    tl.fromTo('cta-code-text', { y: 0 }, { y: -40, duration: 0.8, ease: 'none' }, A(4.0));
    tl.fromTo('mini-title', { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.25, ease: 'power2.out' }, A(3.6));
    tl.fromTo('mini-b1', { opacity: 0, scale: 0.6 }, { opacity: 1, scale: 1, duration: 0.2, ease: 'back.out(1.5)' }, A(3.75));
    tl.fromTo('mini-b2', { opacity: 0, x: 60 }, { opacity: 1, x: 0, duration: 0.2, ease: 'power2.out' }, A(3.85));
    tl.fromTo('mini-l1', { scaleX: 0 }, { scaleX: 1, duration: 0.2, ease: 'power2.out' }, A(3.9));
    tl.fromTo('mini-l2', { scaleX: 0 }, { scaleX: 1, duration: 0.2, ease: 'power2.out' }, A(3.95));
    tl.fromTo('mini-b3', { opacity: 0, y: 20 }, { opacity: 1, y: 0, duration: 0.2, ease: 'power2.out' }, A(4.0));
    tl.fromTo('mini-circ', { opacity: 0, scale: 0.3 }, { opacity: 1, scale: 1, duration: 0.25, ease: 'back.out(1.5)' }, A(4.05));
    tl.fromTo('mini-tl-fill', { scaleX: 0 }, { scaleX: 1, duration: 0.6, ease: 'power1.out' }, A(3.8));
    tl.fromTo('cta-ready', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.25, ease: 'back.out(1.5)' }, A(4.5));
  });

  return (
    <div id="cta-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f7f3eb]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_40%_40%,rgba(255,252,240,0.7)_0%,transparent_50%),linear-gradient(160deg,#f7f3eb_0%,#efe9dd_100%)]"></div>
      <div id="cta-wrap" class="absolute left-[120px] top-[60px] w-[1680px] h-[960px] rounded-[20px] bg-white shadow-[0_24px_80px_rgba(60,50,30,0.12),0_4px_16px_rgba(60,50,30,0.06)] overflow-hidden flex flex-col">
        {/* toolbar */}
        <div class="h-[52px] shrink-0 bg-[#faf8f4] border-b border-[#e8e2d6] flex items-center gap-[8px] px-[20px]">
          <div class="w-[12px] h-[12px] rounded-full bg-[#ff5f57] shrink-0"></div>
          <div class="w-[12px] h-[12px] rounded-full bg-[#febc2e] shrink-0"></div>
          <div class="w-[12px] h-[12px] rounded-full bg-[#28c840] shrink-0"></div>
          <text class="ml-[16px] text-[15px] font-[hf-geist-semi] text-[#8a7e6a] tracking-[0.02em]">Terminal — zsh</text>
        </div>
        <div class="flex-1 flex overflow-hidden">
          {/* left terminal */}
          {/* 原版 cta.html 无 box-sizing 声明，`.cta-left { width:600px; padding:40px 36px }`
              按 content-box 解析成 672px 外宽。tailwind preflight 把 * 设成
              border-box，这里必须显式还原，否则左栏窄 72px。 */}
          <div class="w-[600px] shrink-0 border-r border-[#e8e2d6] pt-[40px] pb-[40px] pl-[36px] pr-[36px] flex flex-col overflow-hidden" style="box-sizing:content-box">
            <div class="flex items-center gap-[10px] h-[40px] shrink-0">
              <div class="relative w-[10px] h-[18px] mr-[8px] shrink-0">
                <path view-box="0 0 10 18" d="M1 1l8 8-8 8" class="absolute left-0 top-0 w-[10px] h-[18px] fill-none stroke-[#81B29A] stroke-[2.5]" />
              </div>
              <text id="cta-typed-1" class="text-[20px] font-[hf-jbm] text-[#2a2418]"></text>
              <div id="cta-cursor-1" class="w-[11px] h-[22px] bg-[#2a2418] ml-[1px] shrink-0"></div>
            </div>
            <div class="flex flex-col gap-[4px] pt-[12px] pb-[16px] pl-[28px]">
              <text id="sl-1" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">Creating project structure...</text>
              <text id="sl-2" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">index.html</text>
              <text id="sl-3" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">meta.json</text>
              <text id="sl-4" class="text-[15px] font-[hf-jbm] text-[#8a7e6a] leading-[1.6]">compositions/</text>
              <div id="sl-5" class="flex items-center text-[15px] leading-[1.6]">
                <div class="relative w-[11px] h-[11px] mr-[6px] shrink-0">
                  <path view-box="0 0 12 12" d="M1.5 6.5l3 3 6-7" class="absolute left-0 top-0 w-[11px] h-[11px] fill-none stroke-[#81B29A] stroke-[2]" />
                </div>
                <text class="font-[hf-jbm] text-[#8a7e6a]">{" Project initialized"}</text>
              </div>
            </div>
            <div id="cta-line-2" class="mt-[8px] flex items-center gap-[10px] h-[40px] shrink-0">
              <div class="relative w-[10px] h-[18px] mr-[8px] shrink-0">
                <path view-box="0 0 10 18" d="M1 1l8 8-8 8" class="absolute left-0 top-0 w-[10px] h-[18px] fill-none stroke-[#81B29A] stroke-[2.5]" />
              </div>
              <text id="cta-typed-2" class="text-[20px] font-[hf-jbm] text-[#2a2418]"></text>
              <div id="cta-cursor-2" class="w-[11px] h-[22px] bg-[#2a2418] ml-[1px] shrink-0"></div>
            </div>
          </div>
          {/* right preview */}
          <div id="cta-right" class="flex-1 relative bg-[#faf8f4] overflow-hidden">
            <div class="h-[44px] bg-[#f0ebe3] border-b border-[#e0d8ca] flex items-center justify-between px-[20px]">
              <text class="text-[13px] font-[hf-geist] text-[#8a7e6a] tracking-[0.04em] uppercase">Preview</text>
              <div class="relative">
                <text id="cta-status-build" class="text-[12px] font-[hf-jbm] text-[#c8860a]">Building...</text>
                <text id="cta-status-ready" class="absolute right-0 top-0 text-[12px] font-[hf-jbm] text-[#5a8a6e] opacity-0">Ready</text>
              </div>
            </div>
            <div class="absolute left-0 top-[44px] right-0 bottom-[120px] overflow-hidden">
              <div class="absolute inset-0 bg-[linear-gradient(135deg,#f9f5ef_0%,#f0e8d8_100%)]"></div>
              <div id="mini-title" class="absolute left-[60px] top-[80px] text-[72px] leading-[1.05] tracking-[-0.03em] font-[hf-serif] text-[#1a1510]">
                <text>Kinetic</text>
                <text>Typography</text>
              </div>
              <div id="mini-b1" class="absolute top-[100px] right-[60px] w-[200px] h-[200px] rounded-[12px] bg-[#E07A5F]"></div>
              <div id="mini-b2" class="absolute top-[320px] right-[60px] w-[200px] h-[80px] rounded-[12px] bg-[#81B29A]"></div>
              <div id="mini-b3" class="absolute bottom-[100px] left-[60px] w-[280px] h-[60px] rounded-[8px] bg-[#3D405B]"></div>
              <div id="mini-l1" class="absolute top-[300px] left-[60px] w-[320px] h-[8px] rounded-[4px] bg-[rgba(60,50,30,0.1)] [transform-origin:0%_50%]"></div>
              <div id="mini-l2" class="absolute top-[330px] left-[60px] w-[220px] h-[8px] rounded-[4px] bg-[rgba(60,50,30,0.1)] [transform-origin:0%_50%]"></div>
              <div id="mini-circ" class="absolute bottom-[100px] right-[80px] w-[100px] h-[100px] rounded-full border-[4px] border-[#F2CC8F]"></div>
              <div class="absolute bottom-[30px] left-[40px] right-[40px] h-[6px] rounded-[3px] bg-[rgba(60,50,30,0.08)] overflow-hidden">
                <div id="mini-tl-fill" class="h-full w-full rounded-[3px] bg-[#E07A5F] [transform-origin:0%_50%]"></div>
              </div>
            </div>
            <div id="cta-code" class="absolute left-0 right-0 bottom-0 h-[120px] bg-[#2a2820] border-t border-[#3a3830] overflow-hidden pt-[12px] pb-[12px] pl-[16px] pr-[16px]">
              <text id="cta-code-text" class="text-[11px] leading-[1.6] font-[hf-jbm] text-[rgba(255,255,255,0.5)]"></text>
            </div>
            <div id="cta-ready" class="absolute top-[52px] right-[12px] flex items-center gap-[6px] rounded-[8px] pl-[8px] pr-[14px] py-[6px] bg-[rgba(129,178,154,0.15)] border-[1.5px] border-[rgba(129,178,154,0.3)]">
              <div class="relative w-[18px] h-[18px] shrink-0">
                <path view-box="0 0 20 20" d="M2 10a8 8 0 1 0 16 0a8 8 0 1 0 -16 0Z" class="absolute left-0 top-0 w-[18px] h-[18px] fill-[#81B29A]" />
                <path view-box="0 0 20 20" d="M6 10l3 3 5-5" class="absolute left-0 top-0 w-[18px] h-[18px] fill-none stroke-[#fff] stroke-[1.5]" />
              </div>
              <text class="text-[13px] font-[hf-geist] text-[#5a8a6e]">Ready</text>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}
