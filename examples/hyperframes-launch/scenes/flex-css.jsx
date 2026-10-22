import { gsap } from 'gsap';
import CanvasKitInit from 'canvaskit-wasm';
import { useGSAP, useEffect } from 'opencat/runtime';
import { clamp, hexRgb } from '../shared.jsx';
/* ══ flex-css (master 17.30-19.01, compositions/flex-css.html) ══════════ */
var CSS_START = 17.30, CSS_DUR = 1.71;
var CSS_ORDER = [0, 1, 2, 3, 4, 5, 11, 17, 16, 15, 14, 13, 12, 6, 7, 8, 10, 9];
var CSS_CELL = 'rounded-[16px] bg-white shadow-[0_2px_12px_rgba(0,0,0,0.05)] border-[1px] border-[rgba(0,0,0,0.04)] overflow-hidden flex items-center justify-center';
var CSS_P = { red: '#e63946', blue: '#457b9d', orange: '#f4a261', teal: '#2a9d8f', navy: '#1d3557', rose: '#d4738a', lime: '#84a98c', purple: '#6b4c9a' };

let FC_CK = null;
CanvasKitInit({}).then((ck) => { FC_CK = ck; });

export default function FlexCss() {

  useGSAP(() => {
    var T = ctx.current.time;
    var tl = gsap.timeline(), A = function (t) { return t; };

    tl.fromTo('fc-wrap', { scale: 0.9, opacity: 0, filter: 'blur(4px)' }, { scale: 1, opacity: 1, filter: 'blur(0px)', duration: 0.5, ease: 'power2.out' }, A(0));
    for (var i = 0; i < CSS_ORDER.length; i++) {
      tl.fromTo('fc-c' + CSS_ORDER[i], { scale: 0.7, opacity: 0, y: 20 }, { scale: 1, opacity: 1, y: 0, duration: 0.28, ease: 'back.out(1.2)' }, A(0.02 + i * 0.02));
    }
    tl.fromTo('fc-title', { opacity: 0, y: 14 }, { opacity: 1, y: 0, duration: 0.22, ease: 'power2.out' }, A(0.38));
    tl.fromTo('fc-sub', { opacity: 0 }, { opacity: 1, duration: 0.18, ease: 'power2.out' }, A(0.46));

    /* pulse (flex-css.html:181-187) — 3 bursts, dot squash + expanding rings */
    var pA = [0.10, 0.60, 1.10];
    ['fc-pulse-dot', 'fc-pulse2-dot'].forEach(function (dot, pi) {
      var r0 = pi === 0 ? 'fc-pulse-r0' : 'fc-pulse2-r0';
      var r1 = pi === 0 ? 'fc-pulse-r1' : 'fc-pulse2-r1';
      for (var p = 0; p < 3; p++) {
        var pt = pA[p];
        tl.fromTo(dot, { scale: 1 }, { scale: 1.3, duration: 0.12, ease: 'power2.out' }, A(pt));
        tl.fromTo(dot, { scale: 1.3 }, { scale: 1, duration: 0.15, ease: 'power2.in' }, A(pt + 0.12));
        tl.fromTo(r0, { scale: 1, opacity: 0.7 }, { scale: 3.5, opacity: 0, duration: 0.40, ease: 'power2.out' }, A(pt));
        tl.fromTo(r1, { scale: 1, opacity: 0.4 }, { scale: 5, opacity: 0, duration: 0.50, ease: 'power2.out' }, A(pt + 0.06));
      }
    });

    /* bars (:201-208) — cycles 0/2 move (cycle 1 targets h1 which is already
       the resting height, so it is a visual no-op in the reference too) */
    var barsDef = [
      { ids: ['fc-bars0', 'fc-bars1', 'fc-bars2', 'fc-bars3', 'fc-bars4'], h1: [30, 50, 70, 40, 60], h2: [80, 35, 90, 65, 45] },
      { ids: ['fc-bars2-0', 'fc-bars2-1', 'fc-bars2-2', 'fc-bars2-3', 'fc-bars2-4'], h1: [50, 30, 60, 80, 40], h2: [35, 75, 45, 30, 70] },
    ];
    barsDef.forEach(function (b) {
      [0.10, 1.10].forEach(function (bt) {
        b.ids.forEach(function (id, bi) {
          tl.fromTo(id, { height: b.h1[bi] }, { height: b.h2[bi], duration: 0.18, ease: 'power2.out' }, A(bt + bi * 0.03));
          tl.fromTo(id, { height: b.h2[bi] }, { height: b.h1[bi], duration: 0.18, ease: 'power2.in' }, A(bt + 0.22 + bi * 0.03));
        });
      });
    });

    /* morph (:215-218) — borderRadius % -> px on the fixed 80px box */
    var morphs = [
      { id: 'fc-morph', cols: [CSS_P.red, CSS_P.blue, CSS_P.orange, CSS_P.teal] },
      { id: 'fc-morph2', cols: [CSS_P.purple, CSS_P.lime, CSS_P.rose, CSS_P.navy] },
    ];
    morphs.forEach(function (m) {
      tl.fromTo(m.id, { borderRadius: 8, backgroundColor: m.cols[0], rotation: 0, scale: 1 }, { borderRadius: 40, backgroundColor: m.cols[1], duration: 0.30, ease: 'power2.inOut' }, A(0.15));
      tl.fromTo(m.id, { borderRadius: 40, backgroundColor: m.cols[1], rotation: 0, scale: 1 }, { borderRadius: 8, rotation: 45, scale: 0.75, backgroundColor: m.cols[2], duration: 0.30, ease: 'power2.inOut' }, A(0.50));
      tl.fromTo(m.id, { borderRadius: 8, backgroundColor: m.cols[2], rotation: 45, scale: 0.75 }, { borderRadius: 24, rotation: 0, scale: 1, backgroundColor: m.cols[3], duration: 0.30, ease: 'power2.inOut' }, A(0.85));
      tl.fromTo(m.id, { borderRadius: 24, backgroundColor: m.cols[3], scale: 1 }, { borderRadius: 40, backgroundColor: m.cols[0], scale: 0.9, duration: 0.30, ease: 'power2.inOut' }, A(1.20));
    });

    /* wave (:268-273) — widths in px on the 160px wrap */
    var waves = [
      { ids: ['fc-wave0', 'fc-wave1', 'fc-wave2'], c: CSS_P.blue },
      { ids: ['fc-wave2-0', 'fc-wave2-1', 'fc-wave2-2'], c: CSS_P.teal },
    ];
    var w0 = [160, 104, 136], w1 = [88, 160, 72], w2 = [144, 80, 160];
    waves.forEach(function (wv) {
      for (var li = 0; li < 3; li++) {
        tl.fromTo(wv.ids[li], { width: w0[li] }, { width: w1[li], duration: 0.38, ease: 'sine.inOut', yoyo: true, repeat: 1 }, A(0.10 + li * 0.05));
        tl.fromTo(wv.ids[li], { width: w0[li] }, { width: w2[li], duration: 0.32, ease: 'sine.inOut' }, A(1.10 + li * 0.05));
      }
    });

    /* bounce (:297-301) — 4 hops */
    for (var bi = 0; bi < 4; bi++) {
      var bt = 0.08 + bi * 0.40;
      tl.fromTo('fc-bounce', { y: 0 }, { y: -60, duration: 0.18, ease: 'power2.out' }, A(bt));
      tl.fromTo('fc-bounce', { y: -60 }, { y: 0, duration: 0.18, ease: 'bounce.out' }, A(bt + 0.18));
    }

    /* scale (:316-322) — 3x3 ripple pulse */
    var rippleOrder = [4, 1, 3, 5, 7, 0, 2, 6, 8];
    for (var c = 0; c < 3; c++) {
      rippleOrder.forEach(function (idx, ri) {
        var st = 0.08 + c * 0.55 + ri * 0.025;
        tl.fromTo('fc-sc' + idx, { scale: 1, opacity: 0.3 }, { scale: 1.4, opacity: 1, duration: 0.10, ease: 'power2.out' }, A(st));
        tl.fromTo('fc-sc' + idx, { scale: 1.4, opacity: 1 }, { scale: 1, opacity: 0.3, duration: 0.15, ease: 'power2.in' }, A(st + 0.10));
      });
    }

    /* gradient cells: conic ramp is a ly.pixel lambda (engine has no conic
       gradient); the rotation/breathing stay DOM tweens on the canvas node */
    tl.fromTo('fc-cv-grad1', { rotation: 0 }, { rotation: 360, duration: CSS_DUR, ease: 'none' }, A(0));
    tl.fromTo('fc-cv-grad2', { rotation: 0 }, { rotation: 360, duration: CSS_DUR, ease: 'none' }, A(0));
    tl.fromTo('fc-cv-grad1', { scale: 1 }, { scale: 1.12, duration: 0.28, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0.10));
    tl.fromTo('fc-cv-grad2', { scale: 1 }, { scale: 1.12, duration: 0.28, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0.10));

    /* flip (:288-289) — rotationY card, faces swapped per-frame (no
       backface-visibility in the engine; cos>0 picks the visible face) */
    var r1s = 0.28, r1e = 0.66, r2s = 0.88, r2e = 1.26;
    var rot = 0;
    if (T >= r1s && T < r1e) { var p1 = (T - r1s) / 0.38; rot = 180 * (p1 < 0.5 ? 2 * p1 * p1 : 1 - 2 * (1 - p1) * (1 - p1)); }
    else if (T >= r1e && T < r2s) rot = 180;
    else if (T >= r2s) { var p2 = clamp((T - r2s) / 0.38, 0, 1); rot = 180 + 180 * (p2 < 0.5 ? 2 * p2 * p2 : 1 - 2 * (1 - p2) * (1 - p2)); }
    var ff = ctx.getNode('fc-flip-front'), fb = ctx.getNode('fc-flip-back');
    ff.transformPerspective(600); fb.transformPerspective(600);
    ff.rotationY(rot); fb.rotationY(rot + 180);
    var frontOn = Math.cos(rot * Math.PI / 180) >= 0;
    ff.opacity(frontOn ? 1 : 0); fb.opacity(frontOn ? 0 : 1);

  });

  /* CanvasKit 绘制走 useEffect + 官方 MakeCanvasSurface；useGSAP 只留 tween */
  useEffect(() => {
    var t = ctx.current.time;
    if (!FC_CK) return;
    drawSpin(FC_CK.MakeCanvasSurface('fc-cv-spin1').getCanvas(), t, 1, CSS_P.red, CSS_P.blue, FC_CK);
    drawSpin(FC_CK.MakeCanvasSurface('fc-cv-spin2').getCanvas(), t, 0.8, CSS_P.teal, CSS_P.orange, FC_CK);
    drawOrbit(FC_CK.MakeCanvasSurface('fc-cv-orb1').getCanvas(), t, 1, [CSS_P.red, CSS_P.blue, CSS_P.orange], FC_CK);
    drawOrbit(FC_CK.MakeCanvasSurface('fc-cv-orb2').getCanvas(), t, -1, [CSS_P.teal, CSS_P.purple, CSS_P.lime], FC_CK);
    drawConic(FC_CK.MakeCanvasSurface('fc-cv-grad1').getCanvas(), [[230, 57, 70], [244, 162, 97], [42, 157, 143], [69, 123, 157]]);
    drawConic(FC_CK.MakeCanvasSurface('fc-cv-grad2').getCanvas(), [[107, 76, 154], [212, 115, 138], [132, 169, 140], [29, 53, 87]]);
  });

  return (
    <div id="fc-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#fafaf2]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#fafaf2_0%,#f0efe6_100%)]"></div>
      <div id="fc-wrap" class="absolute inset-0">
        <div class="absolute left-[12px] top-[12px] w-[1896px] h-[1056px] grid grid-cols-6 grid-rows-3 gap-[12px]">
          <div id="fc-c0" class={CSS_CELL}><canvas id="fc-cv-spin1" class="w-[80px] h-[80px]" /></div>
          <div id="fc-c1" class={CSS_CELL}>
            <div class="relative w-[80px] h-[80px]">
              <div id="fc-pulse-dot" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full bg-[#2a9d8f]"></div>
              <div id="fc-pulse-r0" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#2a9d8f] opacity-0"></div>
              <div id="fc-pulse-r1" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#2a9d8f] opacity-0"></div>
            </div>
          </div>
          <div id="fc-c2" class={CSS_CELL}>
            <div class="flex items-end gap-[6px] h-[90px]">
              <div id="fc-bars0" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[30px]"></div>
              <div id="fc-bars1" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[50px]"></div>
              <div id="fc-bars2" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[70px]"></div>
              <div id="fc-bars3" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[40px]"></div>
              <div id="fc-bars4" class="w-[18px] rounded-t-[4px] bg-[#f4a261] h-[60px]"></div>
            </div>
          </div>
          <div id="fc-c3" class={CSS_CELL}><div id="fc-morph" class="w-[80px] h-[80px] rounded-[8px] bg-[#e63946]"></div></div>
          <div id="fc-c4" class={CSS_CELL}><canvas id="fc-cv-orb1" class="w-[100px] h-[100px]" /></div>
          <div id="fc-c5" class={CSS_CELL}><canvas id="fc-cv-grad1" class="w-[110px] h-[110px]" /></div>

          <div id="fc-c6" class={CSS_CELL}>
            <div class="flex flex-col gap-[10px] w-[160px]">
              <div id="fc-wave0" class="h-[5px] rounded-[3px] bg-[#457b9d] w-[160px]"></div>
              <div id="fc-wave1" class="h-[5px] rounded-[3px] bg-[#457b9d] w-[104px]"></div>
              <div id="fc-wave2" class="h-[5px] rounded-[3px] bg-[#457b9d] w-[136px]"></div>
            </div>
          </div>
          <div id="fc-c7" class={CSS_CELL}>
            <div class="relative w-[100px] h-[70px]">
              <div id="fc-flip-front" class="absolute inset-0 rounded-[8px] bg-[#2a9d8f]"></div>
              <div id="fc-flip-back" class="absolute inset-0 rounded-[8px] bg-[#e63946]"></div>
            </div>
          </div>
          <div id="fc-c8" class={CSS_CELL}><div id="fc-bounce" class="w-[36px] h-[36px] rounded-full bg-[#f4a261]"></div></div>
          <div id="fc-c9" class="rounded-[16px] bg-[#1a1a1a] overflow-hidden flex flex-col items-center justify-center gap-[6px]">
            <text id="fc-title" class="text-[80px] leading-none tracking-[-0.04em] font-[hf-grotesk-bold] text-[#fafaf2]">CSS</text>
            <text id="fc-sub" class="text-[20px] leading-none tracking-[0.16em] font-[hf-grotesk] text-[#777] uppercase">animations</text>
          </div>
          <div id="fc-c10" class={CSS_CELL}>
            <div class="grid grid-cols-3 gap-[8px]">
              {[0, 1, 2, 3, 4, 5, 6, 7, 8].map((i) => (
                <div id={'fc-sc' + i} class="w-[28px] h-[28px] rounded-[6px] bg-[#457b9d] opacity-[0.3]"></div>
              ))}
            </div>
          </div>
          <div id="fc-c11" class={CSS_CELL}><canvas id="fc-cv-spin2" class="w-[80px] h-[80px]" /></div>

          <div id="fc-c12" class={CSS_CELL}><canvas id="fc-cv-grad2" class="w-[110px] h-[110px]" /></div>
          <div id="fc-c13" class={CSS_CELL}><canvas id="fc-cv-orb2" class="w-[100px] h-[100px]" /></div>
          <div id="fc-c14" class={CSS_CELL}>
            <div class="flex items-end gap-[6px] h-[90px]">
              <div id="fc-bars2-0" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[50px]"></div>
              <div id="fc-bars2-1" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[30px]"></div>
              <div id="fc-bars2-2" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[60px]"></div>
              <div id="fc-bars2-3" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[80px]"></div>
              <div id="fc-bars2-4" class="w-[18px] rounded-t-[4px] bg-[#457b9d] h-[40px]"></div>
            </div>
          </div>
          <div id="fc-c15" class={CSS_CELL}><div id="fc-morph2" class="w-[80px] h-[80px] rounded-[8px] bg-[#6b4c9a]"></div></div>
          <div id="fc-c16" class={CSS_CELL}>
            <div class="relative w-[80px] h-[80px]">
              <div id="fc-pulse2-dot" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full bg-[#d4738a]"></div>
              <div id="fc-pulse2-r0" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#d4738a] opacity-0"></div>
              <div id="fc-pulse2-r1" class="absolute left-[30px] top-[30px] w-[20px] h-[20px] rounded-full border-[2px] border-[#d4738a] opacity-0"></div>
            </div>
          </div>
          <div id="fc-c17" class={CSS_CELL}>
            <div class="flex flex-col gap-[10px] w-[160px]">
              <div id="fc-wave2-0" class="h-[5px] rounded-[3px] bg-[#2a9d8f] w-[160px]"></div>
              <div id="fc-wave2-1" class="h-[5px] rounded-[3px] bg-[#2a9d8f] w-[104px]"></div>
              <div id="fc-wave2-2" class="h-[5px] rounded-[3px] bg-[#2a9d8f] w-[136px]"></div>
            </div>
          </div>
        </div>
      </div>
    </div>
  );
}

/* spinner cell: two offset ring arcs, 720/-540 deg per beat. `c` is the
   official MakeCanvasSurface handle. */
function drawSpin(c, t, sp, c1, c2, ck) {
  c.clear();
  var paint = new ck.Paint();
  paint.setStyle(ck.PaintStyle.Stroke);
  paint.setStrokeWidth(3);
  paint.setAntiAlias(true);
  var c1rgb = hexRgb(c1);
  paint.setColor(ck.Color(c1rgb[0], c1rgb[1], c1rgb[2], 1));
  c.save();
  c.rotate(720 * sp * (t / CSS_DUR), 40, 40);
  c.drawArc([3, 3, 77, 77], -90, 180, false, paint);
  c.restore();
  var c2rgb = hexRgb(c2);
  paint.setColor(ck.Color(c2rgb[0], c2rgb[1], c2rgb[2], 1));
  c.save();
  c.rotate(-540 * sp * (t / CSS_DUR), 40, 40);
  c.drawArc([17, 17, 63, 63], -135, 90, false, paint);
  c.drawArc([17, 17, 63, 63], 45, 90, false, paint);
  c.restore();
}

/* orbit cell: 3 dots on R=36, proxy angle -> per-frame maths. */
function drawOrbit(c, t, dir, cols, ck) {
  c.clear();
  var paint = new ck.Paint();
  paint.setStyle(ck.PaintStyle.Fill);
  paint.setAntiAlias(true);
  var ang = dir * Math.PI * 2 * (t / CSS_DUR);
  for (var di = 0; di < 3; di++) {
    var a = ang + di * (2 * Math.PI / 3);
    var rgb = hexRgb(cols[di]);
    paint.setColor(ck.Color(rgb[0], rgb[1], rgb[2], 1));
    c.drawCircle(50 + Math.cos(a) * 36, 50 + Math.sin(a) * 36, 8, paint);
  }
}

/* conic-gradient ramp as a ly.pixel lambda (flex-css.html:344-345). Stops
   are uniform over the circle, starting at 12 o'clock, clockwise. The kernel
   is uniform-driven, so one module-level instance serves both cells. */
function drawConic(c, stops) {
  if (!drawConic._kern) {
    drawConic._kern = ly.pixel((uv, u) => {
      const dx = uv.x - 55;
      const dy = uv.y - 55;
      const d = sqrt(dx * dx + dy * dy);
      let a = atan2(dx, -dy);
      if (a < 0) a = a + 6.283185307179586;
      const t = a / 6.283185307179586;
      let col = u.c0;
      if (t < 0.25) col = u.c0 + (u.c1 - u.c0) * (t * 4);
      else if (t < 0.5) col = u.c1 + (u.c2 - u.c1) * (t * 4 - 1);
      else if (t < 0.75) col = u.c2 + (u.c3 - u.c2) * (t * 4 - 2);
      else col = u.c3 + (u.c0 - u.c3) * (t * 4 - 3);
      const al = d > 54.5 ? 0 : 1;
      return [col.x / 255, col.y / 255, col.z / 255, al];
    }, { uniforms: { c0: 'float3', c1: 'float3', c2: 'float3', c3: 'float3' } });
  }
  ly.draw(c, drawConic._kern, {
    rect: [0, 0, 110, 110],
    uniforms: { c0: stops[0], c1: stops[1], c2: stops[2], c3: stops[3] },
  });
}
