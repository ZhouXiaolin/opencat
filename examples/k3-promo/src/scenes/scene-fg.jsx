// Scene F+G — Solid 标准写法移植。
// 旧 opencat/runtime 的 useGSAP/useEffect 改为：
//   - gsap 补间：useTween 把绝对时间 tween 挂到唯一注册的主时间轴
//     （注册序=树序=旧 hook 序），与参考语料的 masterTL 同构。旧代码的
//     时间窗判断去掉——主 tl 按绝对时间自然裁剪。
//   - CanvasKit 每帧绘制：onFrame 回调里直接画，CK 是引擎垫片预置的官方
//     CanvasKit 面（MakeCanvasSurface(id) 由引擎实现）。
// 帧钩子在组件体注册：Solid 组件只渲染一次，注册序即渲染树序。
import { gsap } from 'gsap';
import { CK, onFrame, useTween } from '../runtime';
import { Bigword, StageCanvas, drawPlusses, drawQmarks, drawStrip } from '../shared';

const WIN = { start: 11.233, end: 13.7 };

// gsap code: index.html:1710-1730 — braces pop + widen (11.333 → 11.673,
// power2.out) then retype to rest. Declarative tweens at absolute scene
// times; only the canvas furniture below stays a per-frame gate.
useTween(function (master) {
  master.set(['#fw1', '#fw2', '#fd1', '#fd2', '#gw1', '#gw2', '#gw3'], { opacity: 0 }, 11.233);
  master.set(['#fgbl', '#fgbr'], { opacity: 1 }, 11.333);
  master.fromTo('#fgbl', { x: 380 }, { x: 50, duration: 0.34, ease: 'power2.out' }, 11.333);
  master.fromTo('#fgbr', { x: -369 }, { x: -39, duration: 0.34, ease: 'power2.out' }, 11.333);
  master.set('#fstem', { opacity: 0.9 }, 11.333);
  master.fromTo('#fstem', { scaleY: 1 }, { scaleY: 0, duration: 0.5, ease: 'power2.out' }, 11.333);
  master.set('#fstem', { opacity: 0 }, 11.85);
  master.set('#fw1', { opacity: 1 }, 11.4);
  master.set('#fw2', { opacity: 1 }, 11.533);
  master.set(['#fd1', '#fd2'], { opacity: 1 }, 11.633);
  // F -> G retype: braces ease back to their measured rest position.
  master.set(['#fw1', '#fw2', '#fd1', '#fd2'], { opacity: 0 }, 12.717);
  master.to('#fgbl', { x: 0, duration: 0.233, ease: 'power2.out' }, 12.717);
  master.to('#fgbr', { x: 0, duration: 0.233, ease: 'power2.out' }, 12.717);
  master.set('#gw1', { opacity: 1 }, 12.7297);
  master.set('#gw2', { opacity: 1 }, 12.7663);
  master.set('#gw3', { opacity: 1 }, 12.7997);
});

export default function SceneFG() {
  // 帧钩子注册点：组件体顶部（Solid 组件只渲染一次，注册序即渲染树序）。
  onFrame(function (T) {
    if (T >= WIN.start && T < WIN.end) {
      // word positions: baked from the calibrated F/G run geometry
      // (F run center 958.5, G run center 962.5, cap 108, ls -2.5)
      var fw1 = document.getElementById('fw1'); if (fw1) fw1.style.left = '611.4953px';
      var fw2 = document.getElementById('fw2'); if (fw2) fw2.style.left = '894.5192px';
      var gw1 = document.getElementById('gw1'); if (gw1) gw1.style.left = '486.758px';
      var gw2 = document.getElementById('gw2'); if (gw2) gw2.style.left = '738.6634px';
      var gw3 = document.getElementById('gw3'); if (gw3) gw3.style.left = '1071.4648px';
      // F furniture: stripFL/FR inside #ffurn; G furniture: stripGL/GR +
      // gqmarks + plusses inside #gfurn.
      var ffSurface = CK && CK.MakeCanvasSurface && CK.MakeCanvasSurface('ffurn-canvas');
      if (ffSurface) {
        var ffCv = ffSurface.getCanvas();
        ffCv.clear();
        if (T >= 11.633 && T < 12.717) {
          drawStrip(CK, ffCv, 239, 532, 85, 12, 13); drawStrip(CK, ffCv, 1595, 532, 85, 12, 13);
        }
      }
      var gfSurface = CK && CK.MakeCanvasSurface && CK.MakeCanvasSurface('gfurn-canvas');
      if (gfSurface) {
        var gfCv = gfSurface.getCanvas();
        gfCv.clear();
        if (T >= 12.7297) {
          drawStrip(CK, gfCv, 240, 534, 120, 12, 17); drawStrip(CK, gfCv, 1559, 534, 120, 12, 17);
          drawQmarks(CK, gfCv, [[343, 265, -1, -1], [1575, 265, 1, -1], [343, 813, -1, 1], [1575, 813, 1, 1]]);
          drawPlusses(CK, gfCv, [[252, 178], [1668, 178], [252, 898], [1668, 898]], 22, 0.4);
        }
      }
    }
  });

  return (
    <section id="sceneFG" class="clip z-[3]" data-start="11.233" data-duration="2.467">
      <div id="fgwrap" class="absolute inset-0">
        {/* center stem that retracts as braces settle */}
        <div id="fstem" class="absolute w-[3px] bg-[#bdbdb9] left-[958px] top-[0] h-[330px] [transform-origin:top_center] opacity-0"></div>
        {/* F/G braces (light weight) */}
        <p id="fgbl" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[390.564px] top-[468.467px] text-[131.0573px] opacity-0">{'{'}</p>
        <p id="fgbr" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[1460.35px] top-[468.467px] text-[131.0573px] opacity-0">{'}'}</p>
        {/* F words: "One" + "model" */}
        <Bigword id="fw1" left="611.4953px" t="One" />
        <Bigword id="fw2" left="894.5192px" t="model" />
        {/* G words: "K3." + "Now" + "Open" */}
        <Bigword id="gw1" left="486.758px" t="K3." />
        <Bigword id="gw2" left="738.6634px" t="Now" />
        <Bigword id="gw3" left="1071.4648px" t="Open" />
        {/* flanking dashes */}
        <div id="fd1" class="absolute h-[3px] bg-[#a9a9ac] left-[197px] top-[537px] w-[42px] opacity-0"></div>
        <div id="fd2" class="absolute h-[3px] bg-[#a9a9ac] left-[1680px] top-[537px] w-[42px] opacity-0"></div>
        {/* F/G furniture canvases: hosts #ffurn (11.633→12.717) / #gfurn
            (12.7297→clip end) gate visibility */}
        <StageCanvas id="ffurn-canvas" />
        <StageCanvas id="gfurn-canvas" />
      </div>
    </section>
  );
}
