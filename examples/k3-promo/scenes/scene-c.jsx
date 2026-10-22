import { gsap } from 'gsap';
import CanvasKitInit from 'canvaskit-wasm';
import { useGSAP, useEffect } from 'opencat/runtime';
import { Cbar, QX, QY, StageCanvas, drawQmarks, getK3Stream, vis } from './shared.jsx';

export default function SceneC() {
  let CK = null;
  CanvasKitInit({}).then((ck) => { CK = ck; });
  useGSAP((ctx, t) => {
    var T = t;
    if (T >= 5.433 && T < 7.35) {
      // chrome fade-ins + panel pan + pale-bar dissolve are plain tweens;
      // the bar bodies stay a per-frame fitted model (BAR_CHAINS) because
      // the recording deviates from stock GSAP there.
      var cTl = gsap.timeline()
        .fromTo('ctab', { opacity: 0 }, { opacity: 1, duration: 0.1, ease: 'power1.out' }, 5.5)
        .fromTo('cicons', { opacity: 0 }, { opacity: 1, duration: 0.1, ease: 'power1.out' }, 5.533)
        .fromTo('cliverow', { opacity: 0 }, { opacity: 1, duration: 0.14, ease: 'power1.out' }, 5.6)
        .fromTo('cpanel', { x: 18, y: 27 }, { x: -17.5, y: -27, duration: 1.417, ease: 'none' }, 5.933);
      for (var pi2 = 0; pi2 < 18; pi2++) {
        cTl.set('pale-' + pi2, { opacity: 1 }, 5.433)
           .to('pale-' + pi2, { opacity: 0, duration: 0.42, ease: 'power1.inOut' }, 5.83 + pi2 * 0.022);
      }
    }
  });
  useEffect(() => {
    var t = ctx.current.time;
    if (!CK) return;
    var T = t;
    var C_ON = (T >= 5.433 && T < 7.35);
    vis(ctx, 'sceneC', C_ON);
    if (C_ON) {
      // cicons: 3 mini bars + dot + trend line
      var ciCv = CK.MakeCanvasSurface('cicons').getCanvas();
      if (ciCv) {
        ciCv.clear();
        var ciP = new CK.Paint(); ciP.setStyle(CK.PaintStyle.Stroke); ciP.setStrokeWidth(6); ciP.setStrokeCap(CK.StrokeCap.Round); ciP.setColor(CK.parseColorString('#b9b9b5'));
        ciCv.drawLine(10, 66, 10, 40, ciP); ciCv.drawLine(28, 66, 28, 22, ciP); ciCv.drawLine(46, 66, 46, 49, ciP);
        var ciDot = new CK.Paint(); ciDot.setStyle(CK.PaintStyle.Fill); ciDot.setColor(CK.parseColorString('#b9b9b5'));
        ciCv.drawCircle(63, 64, 4, ciDot);
        var ciFill = new CK.Paint(); ciFill.setStyle(CK.PaintStyle.Fill); ciFill.setColor(CK.Color(185, 185, 181, 0.28));
        ciFill.setStrokeWidth(0);
        var fp = new CK.Path(); fp.moveTo(150, 66); fp.lineTo(173, 35); fp.lineTo(191, 51); fp.lineTo(216, 20); fp.lineTo(216, 66); fp.close(); ciCv.drawPath(fp, ciFill);
        var ciLn = new CK.Paint(); ciLn.setStyle(CK.PaintStyle.Stroke); ciLn.setStrokeWidth(5); ciLn.setStrokeJoin(CK.StrokeJoin.Round); ciLn.setColor(CK.parseColorString('#b9b9b5'));
        var lp = new CK.Path(); lp.moveTo(150, 66); lp.lineTo(173, 35); lp.lineTo(191, 51); lp.lineTo(216, 20); ciCv.drawPath(lp, ciLn);
      }
      // 18 bars: DOM nodes with real CSS gradients; width + pale opacity are
      // driven per-frame from the reference GSAP timeline. BAR_INIT and
      // BAR_EV are handed over by K3STREAM.
      var K3STREAM = getK3Stream();
      var BAR_INIT = K3STREAM.barInit;
      var BAR_EV = K3STREAM.barEv;
      function barEase(kind, p) {
        p = p < 0 ? 0 : (p > 1 ? 1 : p);
        if (kind === 'p1i') return p * p;
        if (kind === 'p1o') return 1 - (1 - p) * (1 - p);
        if (kind === 'p1io') return p < 0.5 ? 2 * p * p : 1 - 2 * (1 - p) * (1 - p);
        if (kind === 'p2o') return 1 - Math.pow(1 - p, 3);
        return p;
      }
      function barSample(ch, t) {
        var v = 0;
        for (var k = 0; k < ch.length; k++) {
          var ev = ch[k];
          var st = ev.jump ? ev.seff : ev.s;
          if (t < st) break;
          if (ev.jump) { v = ev.to; continue; }
          v = ev.from + (ev.to - ev.from) * barEase(ev.e, (t - ev.s) / ev.d);
        }
        return v;
      }
      // Per-bar tween chain. The reference RECORDING deviates from stock GSAP
      // semantics at three measured points (handoff §29, r17): an event whose
      // slot overlaps the birth tween fires as an instant STEP at the birth
      // end (shallow overlap) or is swallowed outright (birth overlap >=
      // 0.11s); and the second of a same-slot pair fires as a STEP at the
      // first event's end. Fitted model reaches mean|dScaleX| = 0.0058 over
      // 272 samples (f182-198).
      var BAR_CHAINS = (function () {
        var chains = [], i, k;
        for (i = 0; i < 18; i++) {
          chains.push([{ s: 5.663 + i * 0.028, d: 0.34, e: 'p2o', from: 0, to: BAR_INIT[i] }]);
        }
        var prevS = [], prevEnd = [], birthEnd = [], cur = [];
        for (i = 0; i < 18; i++) {
          birthEnd[i] = 5.663 + i * 0.028 + 0.34;
          prevS[i] = -1; prevEnd[i] = birthEnd[i]; cur[i] = BAR_INIT[i];
        }
        for (k = 0; k < BAR_EV.length; k++) {
          var bi = BAR_EV[k][1], s = BAR_EV[k][0], to = BAR_EV[k][2];
          var jump = false, seff = s;
          if (s === prevS[bi]) {
            jump = true; seff = prevEnd[bi];
          } else if (s < birthEnd[bi]) {
            if (birthEnd[bi] - s >= 0.11) { prevS[bi] = s; continue; }
            jump = true; seff = birthEnd[bi];
          }
          chains[bi].push({ s: s, d: 0.09, e: 'p1o', from: cur[bi], to: to, jump: jump, seff: seff });
          prevS[bi] = s; prevEnd[bi] = jump ? seff : s + 0.09; cur[bi] = to;
        }
        return chains;
      })();
      // bar bodies only: the fitted deviation model picks the live value per
      // bar; bar width = sx*660.
      for (var bi = 0; bi < 18; bi++) {
        var sx = barSample(BAR_CHAINS[bi], T);
        if (sx < 0) sx = 0;
        var barN = document.getElementById('bar-' + bi); if (barN) barN.width(sx * 660);
      }
      // corner qmarks — visible for the whole scene C clip
      var cqCv = CK.MakeCanvasSurface('cqmarks-canvas').getCanvas();
      if (cqCv) {
        cqCv.clear();
        drawQmarks(CK, cqCv, [[310, 275, -1, -1], [1490, 275, 1, -1], [310, 805, -1, 1], [1490, 805, 1, 1]]);
      }
    }
  });
  return (
    <div id="sceneC" class="absolute inset-0 z-[4] opacity-0">
      <div id="cpanel" class="absolute inset-0">
        <div id="d-505" class="absolute top-[-60px] w-[2.5px] h-[1250px] bg-[#2e2e2e] left-[403px]"></div>
        <div id="d-506" class="absolute top-[-60px] w-[2.5px] h-[1250px] bg-[#2e2e2e] left-[1442px]"></div>
        <div id="ctab" class="absolute flex items-center justify-center bg-[#1c1c1c] left-[467px] top-[117px] w-[250px] h-[130px] opacity-0">
          <text id="t-645" class="font-[k3-mono] text-[#d8d8da] text-[42px] tracking-[3px]">DECODE</text>
        </div>
        <canvas id="cicons" class="absolute left-[1147px] top-[133px] w-[230px] h-[85px] opacity-0"></canvas>
        <div id="cliverow" class="absolute font-[k3-mono] left-[470px] top-[322px] w-[1000px] h-[52px] opacity-0">
          <div id="d-507" class="absolute rounded-full bg-[#0dbc8a] left-[2px] top-[14px] w-[23px] h-[23px]"></div>
          <text id="t-646" class="absolute text-[#92918a] left-[34px] top-[0] text-[47px]">Live</text>
          <text id="t-647" class="absolute text-[#cd7c74] left-[427px] top-[0] text-[47px]">↑ 6.3x</text>
          <text id="t-648" class="absolute text-[#89cfba] left-[687px] top-[0] text-[47px]">↓ 25% cost</text>
        </div>
        <div id="cbars" class="absolute left-[467px] top-[443px] w-[700px] h-[700px] overflow-hidden">
          <Cbar n="0" top="0px" g="#254a7e,#0a1830" />
          <Cbar n="1" top="38px" g="#254a7e,#0a1830" />
          <Cbar n="2" top="76px" g="#254a7e,#0a1830" />
          <Cbar n="3" top="114px" g="#3f74bf,#102a52" />
          <Cbar n="4" top="152px" g="#254a7e,#0a1830" />
          <Cbar n="5" top="190px" g="#254a7e,#0a1830" />
          <Cbar n="6" top="228px" g="#254a7e,#0a1830" />
          <Cbar n="7" top="266px" g="#254a7e,#0a1830" />
          <Cbar n="8" top="304px" g="#3f74bf,#102a52" />
          <Cbar n="9" top="342px" g="#254a7e,#0a1830" />
          <Cbar n="10" top="380px" g="#254a7e,#0a1830" />
          <Cbar n="11" top="418px" g="#254a7e,#0a1830" />
          <Cbar n="12" top="456px" g="#254a7e,#0a1830" />
          <Cbar n="13" top="494px" g="#3f74bf,#102a52" />
          <Cbar n="14" top="532px" g="#254a7e,#0a1830" />
          <Cbar n="15" top="570px" g="#254a7e,#0a1830" />
          <Cbar n="16" top="608px" g="#1a8a6a,#0b3d2e" />
          <Cbar n="17" top="646px" g="#1a8a6a,#0b3d2e" />
        </div>
        {/* corner qmarks: drawn on one canvas in cpanel space (the global
            (-23,+66) offset rides QX/QY in drawQmarks) */}
        <StageCanvas id="cqmarks-canvas" />
      </div>
    </div>
  );
}
