import { gsap } from 'gsap';
import { useCanvasKit, useEffect, useGSAP } from 'opencat/runtime';
import { makeMeasure, pillWidths, vis } from './shared.jsx';

export default function SceneB() {
  const CK = useCanvasKit();
  // gsap code: index.html:1644-1649 — cluster slides in (power2.out/cubic)
  // and fades (power1.in/quad); pill waterfall is a 1-frame stagger.
  useGSAP((ctx, t) => {
    var T = t;
    if (T >= 4.563 && T < 5.853) {
      var bTl = gsap.timeline()
        .fromTo('bcluster', { x: 190 }, { x: 0, duration: 0.577, ease: 'power2.out' }, 4.653)
        .set('btitle', { opacity: 1 }, 4.6333)
        .to('bcluster', { opacity: 0, duration: 0.3, ease: 'power1.in' }, 5.463);
      ['bs1', 'bs2', 'bs3', 'bs4', 'bs5'].forEach(function (id, i) {
        bTl.set(id, { opacity: 1 }, 4.6333 + i * 0.0333);
      });
    }
  });
  // CanvasKit code: .pill .hstrip diagonal hatch under each stack pill; the
  // roll is parked at -44 (finished before B starts). Stripe phase anchors
  // carry per-pill residuals measured on f160: [bs1 +2.215, bs2 -1.805,
  // bs3 -1.884, bs4 +1.891, bs5 -0.247]px.
  useEffect(() => {
    var t = ctx.current.time;
    var T = t;
    var B_ON = (T >= 4.563 && T < 5.853);
    vis(ctx, 'sceneB', B_ON);
    if (B_ON) {
      // Coordinates are bcluster-local (s = 1 — the stack has no scale).
      // Pill widths reuse the measured pillW map (the STACK labels are the
      // same five strings as the A pills).
      var bsCv = CK.MakeCanvasSurface('bstack-canvas').getCanvas();
      if (bsCv) {
        bsCv.clear();
        var PILL_W = pillWidths(makeMeasure(ctx));
        var bph = [2.215, -1.805, -1.884, 1.891, -0.247];
        var bsw = [PILL_W[1], PILL_W[4], PILL_W[2], PILL_W[3], PILL_W[0]], bst = [0, 113, 227, 340, 456];
        for (var bi = 0; bi < 5; bi++) {
          if (T < 4.6333 + bi * 0.0333) continue;
          var Wp = bsw[bi];
          var cxp = 77 + Wp / 2, cyp = 164 + bst[bi] + 24;
          var sx0 = cxp - (Wp / 2 - 1.5), sy0 = cyp + 29.5;
          var sw2 = 1.04 * (Wp - 3), sh2 = 13, syMid = sy0 + sh2 / 2;
          bsCv.save(); bsCv.clipRect([sx0, sy0, sx0 + sw2, sy0 + sh2]);
          var bsp = new CK.Paint(); bsp.setStyle(CK.PaintStyle.Stroke); bsp.setStrokeWidth(2);
          bsp.setColor(CK.Color(201, 201, 198, 0.9));
          var sax = Math.sin(115 * Math.PI / 180), say = -Math.cos(115 * Math.PI / 180);
          var sdx = say, sdy = -sax;
          var shl = sh2 / 2 / sax + 3, per = 8 / sax;
          var xc0 = sx0 + 1.04 * (Wp - 3) - 44 + bph[bi];
          var xL = sx0 - 6, xR = sx0 + sw2 + 6;
          var xc = xc0 - Math.ceil((xc0 - xL) / per) * per;
          for (; xc <= xR; xc += per) {
            bsCv.drawLine(xc - sdx * shl, syMid - sdy * shl, xc + sdx * shl, syMid + sdy * shl, bsp);
          }
          bsCv.restore();
        }
      }
    }
  });
  return (
    <div id="sceneB" className="absolute inset-0 z-[3] opacity-0">
      <div id="bcluster" className="absolute left-[390px] top-[240px] w-[700px] h-[700px]">
        <p id="btitle" className="absolute whitespace-nowrap leading-none left-[-8.62px] top-[9.6px] text-[#d8d8da] text-[90px] tracking-[-1px] opacity-0 [text-shadow:1.2px_0_rgba(255,70,70,0.10),-1.2px_0_rgba(90,210,255,0.10)]">One model</p>
        <div id="bstack" className="absolute left-[77px] top-[164px]">
          <div id="bs1" className="absolute top-[0px] opacity-0"><div id="d-500" className="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-640" className="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Delta Attention</p></div></div>
          <div id="bs2" className="absolute top-[113px] opacity-0"><div id="d-501" className="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-641" className="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Coding</p></div></div>
          <div id="bs3" className="absolute top-[227px] opacity-0"><div id="d-502" className="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-642" className="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">1M Context</p></div></div>
          <div id="bs4" className="absolute top-[340px] opacity-0"><div id="d-503" className="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-643" className="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Vision</p></div></div>
          <div id="bs5" className="absolute top-[456px] opacity-0"><div id="d-504" className="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-644" className="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Agentic</p></div></div>
        </div>
        {/* .pill .hstrip hatch carrier: lives inside #bcluster so
            translateX(bx) applies. */}
        <canvas id="bstack-canvas" className="absolute left-0 top-0 w-[700px] h-[700px]"></canvas>
      </div>
    </div>
  );
}

