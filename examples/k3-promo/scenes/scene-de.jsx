import { gsap } from 'gsap';
import { useCanvasKit, useFrame, useGSAP } from 'opencat/runtime';
import { Cdot, StageCanvas, Tick, clamp, drawPlusses, drawQmarks, mulberry32, vis } from './shared.jsx';

export default function SceneDE() {
  var CK = useCanvasKit();
  useGSAP((ctx, t) => {
    var T = t;
    if (T >= 7.35 && T < 11.293) {
      // Node tweens (index.html:1668-1712). The canvas pin waves, helix, box
      // hatch and the pin-wave envelope stay per-frame; the otxt per-char
      // entry is declarative below (splitText + timeline).
      var dTl = gsap.timeline()
        .set(['ddashL', 'ddashR'], { opacity: 0 }, 7.35)
        .set(['mktgrp', 'astgrp'], { opacity: 0 }, 7.35)
        .set('ordergrp', { opacity: 1 }, 7.4)
        // box pops small then grows to final
        .fromTo('ordergrp', { scale: 0.7, x: -85, y: -55 },
                             { scale: 1, x: 0, y: 0, duration: 0.23, ease: 'power2.out' }, 7.4)
        .set(['ddashL', 'ddashR'], { opacity: 0.85 }, 7.6)
        // D -> E morph: box shrinks as the flanking boxes fly in
        .to('ordergrp', { scale: 0.493, x: 4, y: -11, duration: 0.55, ease: 'power3.out' }, 9.103)
        .fromTo('mktgrp', { opacity: 0, x: -700, y: 600, scale: 2.6 },
                           { opacity: 1, x: 0, y: 0, scale: 1, duration: 0.55, ease: 'power3.out' }, 9.133)
        .fromTo('astgrp', { opacity: 0, x: 700, y: 600, scale: 2.6 },
                           { opacity: 1, x: 0, y: 0, scale: 1, duration: 0.55, ease: 'power3.out' }, 9.18)
        // helix pins layer fade-in (ref #epins, index.html:1703-1705)
        .set('epins-canvas', { opacity: 0 }, 7.35)
        .to('epins-canvas', { opacity: 1, duration: 0.2, ease: 'power1.out' }, 9.0)
        // whole D/E layer fades out (ref fades #deinner + furniture together)
        .to(['deinner', 'furnDE'], { opacity: 0, duration: 0.13, ease: 'power1.in' }, 11.193);
      ['dl1', 'dl2', 'dl3'].forEach(function (id, i) {
        dTl.set(id, { opacity: 0, scaleY: 0 }, 7.35)
           .to(id, { opacity: 0.9, scaleY: 1, duration: 0.3, ease: 'power2.out' }, 10.653 + i * 0.09);
      });
      // otxt per-char entry — declarative via splitText + timeline; otxt is
      // flex-centred in the card and CAL_SCALEX.otxt squeezes the node. Ref:
      // "Every" shows @7.5333, " modality" @7.5663; every glyph starts at
      // x=(i-6.5)*21 and settles to 0 by 7.9963 (dur 0.43, power2.out).
      vis(ctx, 'otxt', true); /* node visible for the whole D/E window; visibility is per-glyph */
      (function () {
        var parts = ctx.splitText('otxt', { type: 'chars' });
        var gTl = gsap.timeline();
        for (var oi = 0; oi < parts.length; oi++) {
          if (parts[oi].text === ' ') continue; /* inter-word space: a real unit, no ink */
          var ocOn = oi < 5 ? 7.5333 : 7.5663; /* ow1 "Every" / ow2 " modality" */
          gTl.set(parts[oi], { opacity: 0 }, 7.35)
             .set(parts[oi], { opacity: 1 }, ocOn)
             .fromTo(parts[oi], { x: (oi - 6.5) * 21 },
                                  { x: 0, duration: 0.43, ease: 'power2.out' }, 7.5663);
        }
      })();
    }
  });
  useFrame((ctx, t) => {
    var T = t;
    var DE_ON = (T >= 7.35 && T < 11.293);
    vis(ctx, 'sceneDE', DE_ON);
    if (DE_ON) {
      // D bottom pin wave (7.34 → 9.5s)
      var dcv = ctx.getCanvasById('dpins-canvas');
      if (dcv) {
        dcv.clear();
        if (T >= 7.34 && T <= 9.5) {
          var rP = mulberry32(6060), dpins = [];
          /* ref stores p.x = left + 15 (pin centre) and feeds THAT into both
             the wave phase and waveSide; the +15 is a 15px spatial phase. */
          for (var dpi = 0; dpi < 28; dpi++) dpins.push({ x: -34 + dpi * 71 + (rP() - 0.5) * 8 + 15, jit: (rP() - 0.5) * 0.24 });
          var WAVE_K = 2 * Math.PI / 2250;
          function waveSide(u, x) { var cyc = Math.floor((u - Math.PI / 2) / (2 * Math.PI)); return Math.sin(cyc * 2.4 + x * 0.00185 + 0.9) >= 0 ? 1 : -1; }
          var w = 2 * Math.PI / 1.30, e = clamp((T - 7.36) / 0.4, 0, 1), rise = 1 - Math.pow(1 - e, 2);
          var mEnv = clamp((T - 7.58) / 0.5, 0, 1);
          /* ref: tl.to("#dpins", {autoAlpha: 0, duration: 0.28}, 9.15), default ease power1.out */
          var dFade = clamp((T - 9.15) / 0.28, 0, 1), dA = (1 - dFade) * (1 - dFade);
          var stem = new CK.Paint(); stem.setStyle(CK.PaintStyle.Stroke); stem.setStrokeWidth(9); stem.setStrokeCap(CK.StrokeCap.Butt); stem.setColor(CK.Color(156,156,152,dA));
          var head = new CK.Paint(); head.setStyle(CK.PaintStyle.Fill); head.setColor(CK.Color(181,181,177,dA));
          /* canvas is bottom-[-5px] h-[280px] → its bottom edge is screen
             y=1085 = the ref pin pivot; so baseY inside the canvas is 280. */
          var baseY = 280;
          for (var i = 0; i < dpins.length; i++) {
            var p = dpins[i], u = w * (T - 7.35) + WAVE_K * p.x - 0.79 + p.jit;
            var hh = 0.5 + 0.5 * Math.sin(u), hEff = 1 + (hh - 1) * mEnv;
            var sS = (0.16 + 0.84 * Math.pow(hEff, 0.75)) * (0.10 + 0.90 * rise);
            var stemH = 226 * sS, lean = waveSide(u, p.x) * 87 * Math.pow(1 - hEff, 1.15) * Math.pow(mEnv, 1.3);
            var lr = lean * Math.PI / 180;
            var x0 = p.x, topX = x0 + Math.sin(lr) * stemH, topY = baseY - Math.cos(lr) * stemH;
            var hX = x0 + Math.sin(lr) * (stemH + 14), hY = baseY - Math.cos(lr) * (stemH + 14);
            dcv.drawLine(x0, baseY, topX, topY, stem); dcv.drawCircle(hX, hY, 14, head);
          }
        }
      }
      // E helix pins via canvas projection. Perspective: rotateX(phi-90deg)
      // under perspective:4100px about the SCREEN centre. Dots ride the same
      // endpoints; near-side rod luma +10% (cosφ>0 → #515152).
      var hcv = ctx.getCanvasById('epins-canvas');
      if (hcv) {
        hcv.clear();
        if (T >= 9.0 && T <= 11.35) {
          var epA = 1;
          var HX_C = 1.33 - 2.405 * 9.6, P4 = 4100;
          var rod = new CK.Paint(); rod.setStyle(CK.PaintStyle.Stroke); rod.setStrokeWidth(3.5); rod.setStrokeCap(CK.StrokeCap.Round);
          var dot = new CK.Paint(); dot.setStyle(CK.PaintStyle.Fill); dot.setColor(CK.Color(87,87,90,epA));
          for (var hi = 0; hi < 73; hi++) {
            var hx = 5 + hi * 26.3, phi = 2.405 * T + 0.00343 * hx + HX_C;
            var ct = Math.sin(phi), st = -Math.cos(phi); /* cos(φ-90°)=sinφ, sin(φ-90°)=-cosφ */
            var near = Math.cos(phi) > 0;
            rod.setColor(CK.Color(near ? 81 : 69, near ? 81 : 69, near ? 82 : 70, epA));
            var kT = P4 / (P4 + 84 * st), kB = P4 / (P4 - 84 * st); /* z'=∓84·st */
            var xT = 960 + (hx - 960) * kT, yT = 534 - 84 * ct * kT - 400;
            var xB = 960 + (hx - 960) * kB, yB = 534 + 84 * ct * kB - 400;
            hcv.drawLine(xT, yT, xB, yB, rod);
            hcv.drawCircle(xT, yT, 3.5, dot); hcv.drawCircle(xB, yB, 3.5, dot);
          }
        }
      }
      // D/E box hatch — ref .bhatch divs (repeating-linear-gradient(132deg,
      // #252525 0 1.5px, transparent 1.5px P), clipped by the overflow:hidden
      // tickbox; drawn on box-local canvases. Per-box phase offset calibrated
      // by single-frame xcorr (r14 §26.3 method); glide X = hatchGlide()
      // verbatim (measured schedule).
      {
        var OC = ctx.getCanvasById('ohatch-canvas');
        var oHA = 0;
        if (T >= 7.73) oHA = 1;
        else if (T >= 7.4) {
          var hq = (T - 7.4) / 0.33;
          oHA = hq < 0.5 ? 2 * hq * hq : 1 - Math.pow(-2 * hq + 2, 2) / 2;
        }
        var hf = T * 30, Xo = 0;
        /* Xo IS the CSS backgroundPosition value (element-local px) — the
           canvas rides ordergrp's scale like the ref div does. */
        if (T >= 7.3 && T <= 11.3) {
          if (hf <= 229) Xo = 0;
          else if (hf <= 263) Xo = 10 * (hf - 229) + 210 * (1 - Math.exp(-(hf - 229) / 7));
          else if (hf < 285) Xo = 548.4;
          else { var hu = Math.min(hf - 285, 97.8); Xo = 548.4 + (4.4 * hu - 0.0225 * hu * hu) / 0.493; }
        }
        var HDX = Math.sin(132 * Math.PI / 180), HDY = -Math.cos(132 * Math.PI / 180);
        var drawBH = function (cv, W, H, X, col, P, ph, ph2, segX) {
          if (!cv) return;
          cv.clear();
          var Dw = W + 62, Dh = H + 62; /* div box: padding-box origin inset(-30) */
          var HL = Dw * HDX + Dh * (-HDY);
          var sx = W / 2 - (HL / 2) * HDX, sy = H / 2 - (HL / 2) * HDY;
          var hp = new CK.Paint(); hp.setStyle(CK.PaintStyle.Stroke); hp.setStrokeWidth(1.5);
          hp.setColor(CK.Color(col[0], col[1], col[2], col[3]));
          var hnx = -HDY, hny = HDX, hhalf = W + H;
          var hs0 = 0.75;
          var hkMin = Math.floor((-hs0 - hhalf) / P), hkMax = Math.ceil(hhalf / P) + 2;
          /* ref o-box shows a phase discontinuity at pattern-x ≈ 4.6: LEFT of
             it the chain runs at ph2 (= main −102.9), RIGHT at ph. Vertical
             boundary glides with X. */
          var segs = (ph2 === undefined) ? [[1, 1, W - 1, H - 1, ph]]
                                         : [[1, 1, segX + X, H - 1, ph2], [segX + X, 1, W - 1, H - 1, ph]];
          for (var si = 0; si < segs.length; si++) {
            cv.save(); cv.clipRect(segs[si].slice(0, 4));
            for (var hk = hkMin; hk <= hkMax; hk++) {
              var hs = hs0 + hk * P;
              var hax = sx + hs * HDX + X + segs[si][4], hay = sy + hs * HDY;
              cv.drawLine(hax - hnx * hhalf, hay - hny * hhalf, hax + hnx * hhalf, hay + hny * hhalf, hp);
            }
            cv.restore();
          }
        };
        drawBH(OC, 1010, 379, Xo, [48, 48, 48, oHA], 119.6, 88.4, -14.5, 0);
        /* m/a-box hatch: main chain = repeating-linear-gradient(132deg,#252525
           0 1.5px,transp 1.5px 65.4px) → slope dx/dy=-0.9004, pitch 88.0.
           Glide = hatchGlide Xm/Xa. m-box extra chain = main −50px (Chrome
           tile artifact, not in source CSS); a-box NEVER shows it. */
        if (T >= 9.1 && T <= 11.3) {
          var mHC = ctx.getCanvasById('mhatch-canvas'), aHC = ctx.getCanvasById('ahatch-canvas');
          var Xm2 = 0, Xa2 = 0;
          if (hf >= 285) { var mu2 = Math.min(hf - 285, 97.8); Xm2 = 4.4 * mu2 - 0.0225 * mu2 * mu2; }
          if (hf >= 286) {
            if (hf <= 299) Xa2 = -36 * (1 - Math.exp(-(hf - 286) / 6));
            else { var au2 = Math.min(hf - 300, 95); Xa2 = -31.88 + 1.9 * au2 - 0.01 * au2 * au2; }
          }
          var drawMA = function (cv, boxL, anchor, X, chains) {
            if (!cv) return;
            cv.clear();
            var hp2 = new CK.Paint(); hp2.setStyle(CK.PaintStyle.Stroke); hp2.setStrokeWidth(1.5);
            hp2.setColor(CK.Color(37, 37, 37, 1));
            var slope = -0.9004, pitch = 88.0, y0 = 18, yTop = -200, yBot = 400;
            for (var ci = 0; ci < chains.length; ci++) {
              var x0 = anchor + X + chains[ci] - boxL;
              /* line hits the box if xs + slope*(y-18) ∈ [0,520] for some
                 y∈[0,183] → xs ∈ [-345, 684]; pad and enumerate k. */
              var k0 = Math.floor((-350 - x0) / pitch) - 1, k1 = Math.ceil((690 - x0) / pitch) + 1;
              for (var k = k0; k <= k1; k++) {
                var xs = x0 + k * pitch;
                cv.drawLine(xs + slope * (yTop - y0), yTop, xs + slope * (yBot - y0), yBot, hp2);
              }
            }
          };
          drawMA(mHC, 130, 58.6, Xm2, [0]);
          drawMA(aHC, 1260, 44.9, Xa2, [0]);
        }
      }
      // deqmarks: 4 qmarks + 4 plusses, static while the clip is on (the
      // layer fade carries them out).
      var dqCv = ctx.getCanvasById('deqmarks-canvas');
      if (dqCv) {
        dqCv.clear();
        drawQmarks(CK, dqCv, [[205, 215, -1, -1], [1720, 215, 1, -1], [205, 870, -1, 1], [1720, 870, 1, 1]]);
        drawPlusses(CK, dqCv, [[75, 100], [1845, 100], [75, 970], [1845, 970]], 25, 0.5);
      }
    }
  });
  return (
    <div id="sceneDE" class="absolute inset-0 z-[2] opacity-0">
      <div id="deinner" class="absolute inset-0">
        {/* D bottom pin wave canvas (pins anchored to bottom edge) */}
        <canvas id="dpins-canvas" class="absolute left-0 bottom-[-5px] w-[1920px] h-[280px]"></canvas>
        {/* E helix pins canvas (corkscrew about y=534) */}
        <canvas id="epins-canvas" class="absolute left-0 top-[400px] w-[1920px] h-[280px]"></canvas>

        {/* center hatched box: "EVERY MODALITY" (1010×379 at 454,350) */}
        <div id="ordergrp" class="absolute border-[1px] border-[#5a5a5a] bg-[#010101] bg-[linear-gradient(100deg,rgba(255,255,255,0.010),rgba(255,255,255,0.018)_45%,rgba(255,255,255,0.006))] overflow-hidden flex items-center justify-center left-[454px] top-[350px] w-[1010px] h-[379px] opacity-0">
          <canvas id="ohatch-canvas" class="absolute left-0 top-0 w-[1010px] h-[379px]"></canvas>
          <Cdot id="d-508" s="13px" c="#e6e6e6" class="-left-[26px] -top-[26px]" />
          <Cdot id="d-509" s="13px" c="#e6e6e6" class="-right-[26px] -top-[26px]" />
          <Cdot id="d-510" s="13px" c="#e6e6e6" class="-left-[26px] -bottom-[26px]" />
          <Cdot id="d-511" s="13px" c="#e6e6e6" class="-right-[26px] -bottom-[26px]" />
          <text id="otxt" class="text-[#d8d8da] whitespace-nowrap leading-none text-[133.3333px] tracking-[-2.5px] opacity-0">Every modality</text>
        </div>
        {/* left flanking box: "Every market" */}
        <div id="mktgrp" class="absolute border-[1px] border-[#565656] bg-[#010101] bg-[linear-gradient(100deg,rgba(255,255,255,0.010),rgba(255,255,255,0.018)_45%,rgba(255,255,255,0.006))] overflow-hidden flex items-center justify-center left-[130px] top-[437px] w-[520px] h-[183px] opacity-0">
          <canvas id="mhatch-canvas" class="absolute left-0 top-0 w-[520px] h-[183px]"></canvas>
          <Cdot id="d-512" s="13px" c="#e6e6e6" class="-left-[20px] -top-[20px]" />
          <Cdot id="d-513" s="13px" c="#e6e6e6" class="-right-[20px] -top-[20px]" />
          <Cdot id="d-514" s="13px" c="#e6e6e6" class="-left-[20px] -bottom-[20px]" />
          <Cdot id="d-515" s="13px" c="#e6e6e6" class="-right-[20px] -bottom-[20px]" />
          <text id="t-649" class="text-[#e4e4e6] whitespace-nowrap leading-none text-[68.7285px]">Every token</text>
        </div>
        {/* right flanking box: "Every task" */}
        <div id="astgrp" class="absolute border-[1px] border-[#565656] bg-[#010101] bg-[linear-gradient(100deg,rgba(255,255,255,0.010),rgba(255,255,255,0.018)_45%,rgba(255,255,255,0.006))] overflow-hidden flex items-center justify-center left-[1260px] top-[437px] w-[520px] h-[183px] opacity-0">
          <canvas id="ahatch-canvas" class="absolute left-0 top-0 w-[520px] h-[183px]"></canvas>
          <Cdot id="d-516" s="13px" c="#e6e6e6" class="-left-[20px] -top-[20px]" />
          <Cdot id="d-517" s="13px" c="#e6e6e6" class="-right-[20px] -top-[20px]" />
          <Cdot id="d-518" s="13px" c="#e6e6e6" class="-left-[20px] -bottom-[20px]" />
          <Cdot id="d-519" s="13px" c="#e6e6e6" class="-right-[20px] -bottom-[20px]" />
          <text id="t-650" class="text-[#e4e4e6] whitespace-nowrap leading-none text-[68.7285px]">Every task</text>
        </div>
        {/* three drop-lines + flanking dashes */}
        <div id="dl1" class="absolute w-[2px] bg-[#cfcfcc] left-[389px] top-[622px] h-[95px] [transform-origin:top_center] opacity-0"></div>
        <div id="dl2" class="absolute w-[2px] bg-[#cfcfcc] left-[958px] top-[622px] h-[95px] [transform-origin:top_center] opacity-0"></div>
        <div id="dl3" class="absolute w-[2px] bg-[#cfcfcc] left-[1519px] top-[622px] h-[95px] [transform-origin:top_center] opacity-0"></div>
        <div id="ddashL" class="absolute h-[2px] bg-[#8f8f8b] left-[318px] top-[537px] w-[40px] opacity-0"></div>
        <div id="ddashR" class="absolute h-[2px] bg-[#8f8f8b] left-[1562px] top-[537px] w-[40px] opacity-0"></div>
      </div>
      {/* D/E furniture */}
      <div id="furnDE" class="absolute inset-0">
        <div id="stripDL" class="absolute left-[75px] top-[533px] flex items-end gap-[6px] opacity-90">
          <Tick id="d-520" ht="15px" /><Tick id="d-521" ht="7px" mb="mb-[3px]" />
          <Tick id="d-522" ht="15px" /><Tick id="d-523" ht="7px" mb="mb-[3px]" />
          <Tick id="d-524" ht="15px" /><Tick id="d-525" ht="7px" mb="mb-[3px]" />
          <Tick id="d-526" ht="15px" /><Tick id="d-527" ht="7px" mb="mb-[3px]" />
          <Tick id="d-528" ht="15px" /><Tick id="d-529" ht="7px" mb="mb-[3px]" />
          <Tick id="d-530" ht="15px" /><Tick id="d-531" ht="7px" mb="mb-[3px]" />
          <Tick id="d-532" ht="15px" /><Tick id="d-533" ht="7px" mb="mb-[3px]" />
          <Tick id="d-534" ht="15px" /><Tick id="d-535" ht="7px" mb="mb-[3px]" />
          <Tick id="d-536" ht="15px" /><Tick id="d-537" ht="7px" mb="mb-[3px]" />
          <Tick id="d-538" ht="15px" />
        </div>
        <div id="stripDR" class="absolute left-[1700px] top-[533px] flex items-end gap-[6px] opacity-90">
          <Tick id="d-539" ht="15px" /><Tick id="d-540" ht="7px" mb="mb-[3px]" />
          <Tick id="d-541" ht="15px" /><Tick id="d-542" ht="7px" mb="mb-[3px]" />
          <Tick id="d-543" ht="15px" /><Tick id="d-544" ht="7px" mb="mb-[3px]" />
          <Tick id="d-545" ht="15px" /><Tick id="d-546" ht="7px" mb="mb-[3px]" />
          <Tick id="d-547" ht="15px" /><Tick id="d-548" ht="7px" mb="mb-[3px]" />
          <Tick id="d-549" ht="15px" /><Tick id="d-550" ht="7px" mb="mb-[3px]" />
          <Tick id="d-551" ht="15px" /><Tick id="d-552" ht="7px" mb="mb-[3px]" />
          <Tick id="d-553" ht="15px" /><Tick id="d-554" ht="7px" mb="mb-[3px]" />
          <Tick id="d-555" ht="15px" /><Tick id="d-556" ht="7px" mb="mb-[3px]" />
          <Tick id="d-557" ht="15px" />
        </div>
        {/* deqmarks: fades with the D/E layer */}
        <StageCanvas id="deqmarks-canvas" />
      </div>
    </div>
  );
}
