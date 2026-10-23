import { gsap } from 'gsap';
import { useCanvasKit, useEffect, useGSAP } from 'opencat/runtime';
import { Bigword, StageCanvas, drawPlusses, drawQmarks, drawStrip, vis } from './shared.jsx';

export default function SceneFG() {
  const CK = useCanvasKit();
  useGSAP((ctx, t) => {
    var T = t;
    if (T >= 11.233 && T < 13.7) {
      // Braces pop + widen (11.333 → 11.673, power2.out) then retype to rest.
      // Declarative tweens at absolute scene times; only the canvas furniture
      // below stays a per-frame gate. (index.html:1710-1730)
      gsap.timeline()
        .set(['fw1', 'fw2', 'fd1', 'fd2', 'gw1', 'gw2', 'gw3'], { opacity: 0 }, 11.233)
        .set(['fgbl', 'fgbr'], { opacity: 1 }, 11.333)
        .fromTo('fgbl', { x: 380 }, { x: 50, duration: 0.34, ease: 'power2.out' }, 11.333)
        .fromTo('fgbr', { x: -369 }, { x: -39, duration: 0.34, ease: 'power2.out' }, 11.333)
        .set('fstem', { opacity: 0.9 }, 11.333)
        .fromTo('fstem', { scaleY: 1 }, { scaleY: 0, duration: 0.5, ease: 'power2.out' }, 11.333)
        .set('fstem', { opacity: 0 }, 11.85)
        .set('fw1', { opacity: 1 }, 11.4)
        .set('fw2', { opacity: 1 }, 11.533)
        .set(['fd1', 'fd2'], { opacity: 1 }, 11.633)
        // F -> G retype: braces ease back to their measured rest position.
        .set(['fw1', 'fw2', 'fd1', 'fd2'], { opacity: 0 }, 12.717)
        .to('fgbl', { x: 0, duration: 0.233, ease: 'power2.out' }, 12.717)
        .to('fgbr', { x: 0, duration: 0.233, ease: 'power2.out' }, 12.717)
        .set('gw1', { opacity: 1 }, 12.7297)
        .set('gw2', { opacity: 1 }, 12.7663)
        .set('gw3', { opacity: 1 }, 12.7997);
    }
  });
  useEffect(() => {
    var t = ctx.current.time;
    var T = t;
    var FG_ON = (T >= 11.233 && T < 13.7);
    vis(ctx, 'sceneFG', FG_ON);
    if (FG_ON) {
      // word positions: baked from the calibrated F/G run geometry
      // (F run center 958.5, G run center 962.5, cap 108, ls -2.5)
      document.getElementById('fw1').style.left = '611.4953px'; document.getElementById('fw2').style.left = '894.5192px';
      document.getElementById('gw1').style.left = '486.758px'; document.getElementById('gw2').style.left = '738.6634px'; document.getElementById('gw3').style.left = '1071.4648px';
      // F furniture: stripFL/FR inside #ffurn; G furniture: stripGL/GR +
      // gqmarks + plusses inside #gfurn.
      var ffCv = CK.MakeCanvasSurface('ffurn-canvas').getCanvas();
      if (ffCv) {
        ffCv.clear();
        if (T >= 11.633 && T < 12.717) {
          drawStrip(CK, ffCv, 239, 532, 85, 12, 13); drawStrip(CK, ffCv, 1595, 532, 85, 12, 13);
        }
      }
      var gfCv = CK.MakeCanvasSurface('gfurn-canvas').getCanvas();
      if (gfCv) {
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
    <div id="sceneFG" className="absolute inset-0 z-[3] opacity-0">
      <div id="fgwrap" className="absolute inset-0">
        {/* center stem that retracts as braces settle */}
        <div id="fstem" className="absolute w-[3px] bg-[#bdbdb9] left-[958px] top-[0] h-[330px] [transform-origin:top_center] opacity-0"></div>
        {/* F/G braces (light weight) */}
        <text id="fgbl" className="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[390.564px] top-[468.467px] text-[131.0573px] opacity-0">{'{'}</text>
        <text id="fgbr" className="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[1460.35px] top-[468.467px] text-[131.0573px] opacity-0">{'}'}</text>
        {/* F words: "One" + "model" */}
        <Bigword id="fw1" left="611.4953px" t="One" />
        <Bigword id="fw2" left="894.5192px" t="model" />
        {/* G words: "K3." + "Now" + "Open" */}
        <Bigword id="gw1" left="486.758px" t="K3." />
        <Bigword id="gw2" left="738.6634px" t="Now" />
        <Bigword id="gw3" left="1071.4648px" t="Open" />
        {/* flanking dashes */}
        <div id="fd1" className="absolute h-[3px] bg-[#a9a9ac] left-[197px] top-[537px] w-[42px] opacity-0"></div>
        <div id="fd2" className="absolute h-[3px] bg-[#a9a9ac] left-[1680px] top-[537px] w-[42px] opacity-0"></div>
        {/* F/G furniture canvases: hosts #ffurn (11.633→12.717) / #gfurn
            (12.7297→clip end) gate visibility */}
        <StageCanvas id="ffurn-canvas" />
        <StageCanvas id="gfurn-canvas" />
      </div>
    </div>
  );
}
