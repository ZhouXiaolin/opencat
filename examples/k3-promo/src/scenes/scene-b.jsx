// Scene B — Solid 标准写法移植。
// 旧 opencat/runtime 的 useGSAP/useEffect 改为：
//   - gsap 补间：useTween 把绝对时间 tween 挂到唯一注册的主时间轴
//     （注册序=树序=旧 hook 序），与参考语料的 masterTL 同构。
//   - CanvasKit 每帧绘制：onFrame 回调里直接画，CK 是引擎垫片预置的官方
//     CanvasKit 面（MakeCanvasSurface(id) 由引擎实现）。
// 帧钩子在组件体注册：Solid 组件只渲染一次，注册序即渲染树序。
import { gsap } from 'gsap';
import { CK, onFrame, useTween } from '../runtime';
import { makeMeasure, pillWidths } from '../shared';

const WIN = { start: 4.563, end: 5.853 };

// gsap code: index.html:1644-1649 — cluster slides in (power2.out) and fades
// (power1.in); pill waterfall is a 1-frame stagger. Absolute timeline times.
useTween(function (master) {
  master.fromTo('#bcluster', { x: 190 }, { x: 0, duration: 0.577, ease: 'power2.out' }, 4.653);
  master.set('#btitle', { opacity: 1 }, 4.6333);
  master.to('#bcluster', { opacity: 0, duration: 0.3, ease: 'power1.in' }, 5.463);
  ['bs1', 'bs2', 'bs3', 'bs4', 'bs5'].forEach(function (id, i) {
    master.set('#' + id, { opacity: 1 }, 4.6333 + i * 0.0333);
  });
});

// CanvasKit hatch phase residuals measured on f160 (same as the old port).
const BPH = [2.215, -1.805, -1.884, 1.891, -0.247];

export default function SceneB() {
  onFrame(function (T) {
    if (T >= WIN.start && T < WIN.end) {
      // .pill .hstrip diagonal hatch under each stack pill; roll parked at
      // -44. Coordinates are bcluster-local (s = 1 — the stack has no scale).
      var surface = CK && CK.MakeCanvasSurface && CK.MakeCanvasSurface('bstack-canvas');
      if (surface) {
        var bsCv = surface.getCanvas();
        bsCv.clear();
        var PILL_W = pillWidths(makeMeasure());
        var bsw = [PILL_W[1], PILL_W[4], PILL_W[2], PILL_W[3], PILL_W[0]];
        var bst = [0, 113, 227, 340, 456];
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
          var xc0 = sx0 + 1.04 * (Wp - 3) - 44 + BPH[bi];
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
    <section id="sceneB" class="clip z-[3]" data-start="4.563" data-duration="1.29">
      <div id="bcluster" class="absolute left-[390px] top-[240px] w-[700px] h-[700px]">
        <p id="btitle" class="absolute whitespace-nowrap leading-none left-[-8.62px] top-[9.6px] text-[#d8d8da] text-[90px] tracking-[-1px] opacity-0 [text-shadow:1.2px_0_rgba(255,70,70,0.10),-1.2px_0_rgba(90,210,255,0.10)]">One model</p>
        <div id="bstack" class="absolute left-[77px] top-[164px]">
          <div id="bs1" class="absolute top-[0px] opacity-0"><div id="d-500" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-640" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Delta Attention</p></div></div>
          <div id="bs2" class="absolute top-[113px] opacity-0"><div id="d-501" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-641" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Coding</p></div></div>
          <div id="bs3" class="absolute top-[227px] opacity-0"><div id="d-502" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-642" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">1M Context</p></div></div>
          <div id="bs4" class="absolute top-[340px] opacity-0"><div id="d-503" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-643" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Vision</p></div></div>
          <div id="bs5" class="absolute top-[456px] opacity-0"><div id="d-504" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><p id="t-644" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Agentic</p></div></div>
        </div>
        {/* .pill .hstrip hatch carrier: lives inside #bcluster so translateX(bx) applies. */}
        <canvas id="bstack-canvas" class="absolute left-0 top-0 w-[700px] h-[700px]"></canvas>
      </div>
    </section>
  );
}
