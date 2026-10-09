// JSX frontend port of examples/k3-promo/k3-promo.xml — one continuous stage;
// every layer is gated by its time window via opacity, driven by ctx.time.
// The XML <template>s become components (props replace $param), and the
// <script> sections split into per-scene components: each scene keeps its
// section body verbatim inside one useGSAP hook, so hook order (tree order)
// mirrors the XML script order. Shared prologue work (type calibration) runs
// in the entry component's own hook, which registers before all children.

import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 16.4667 };

export const fonts = {
  default: 'k3-sans',
  faces: [
    { id: 'k3-sans', family: 'Inter', path: 'assets/fonts/Inter-Regular.ttf', role: 'sans' },
    { id: 'k3-sans-light', family: 'Inter', path: 'assets/fonts/Inter-Light.ttf' },
    { id: 'k3-sans-medium', family: 'Inter', path: 'assets/fonts/Inter-Medium.ttf' },
    { id: 'k3-mono', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Regular.ttf', role: 'mono' },
    { id: 'k3-mono-bold', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Bold.ttf', role: 'mono' },
  ],
};

/* ── 结构模板 → 组件（解析前展开语义：调用点 class 以 " delta" 并入模板 class） ── */
function StageCanvas({ id }) {
  return <canvas id={id} class="absolute inset-0 w-[1920px] h-[1080px]" />;
}

// prop 用 `ht` 而非 `h`：JSX transform 在组件体内生成 h(...)，prop 名 h 会
// 遮蔽元素创建函数。
function Tick({ id, ht, mb }) {
  return <div id={id} class={'w-[2px] h-[' + ht + '] bg-[#cfcfd2]' + (mb ? ' ' + mb : '')} />;
}

function Cbar({ n, top, g }) {
  return (
    <div id={'bar-' + n} class={'absolute left-0 top-[' + top + '] w-[0px] h-[38px] bg-[linear-gradient(90deg,' + g + ')]'}>
      <div id={'pale-' + n} class="absolute left-0 top-0 w-full h-[38px] opacity-0 bg-[linear-gradient(90deg,rgba(233,229,224,0.95),rgba(206,199,193,0.6))]"></div>
    </div>
  );
}

function AgentPill({ id, t }) {
  return (
    <div id={id} class="absolute left-[0] top-[0] opacity-0">
      <div id={id + '-box'} class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]">
        <text id={id + '-txt'} class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">{t}</text>
      </div>
    </div>
  );
}

function Cdot({ id, s, c, class: extra }) {
  return <div id={id} class={'absolute w-[' + s + '] h-[' + s + '] bg-[' + c + ']' + (extra ? ' ' + extra : '')} />;
}

function Bigword({ id, left, t }) {
  return <text id={id} class={'absolute font-[k3-sans] font-normal text-[#d8d8da] leading-none whitespace-nowrap text-[148.4536px] tracking-[-2.5px] left-[' + left + '] top-[458.3299px] opacity-0'}>{t}</text>;
}

/* ════════ 共享每帧工具（XML script 顶层；模块体每帧重估，纯函数/纯数据） ════════ */
function mulberry32(a) {
  return function () {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    var t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
var pick = function (arr, r) { return arr[Math.floor((r || rnd)() * arr.length)]; };
var range = function (a, b, r) { return a + (r || rnd)() * (b - a); };
var clamp = function (v, a, b) { return Math.max(a, Math.min(b, v)); };
var rnd = mulberry32(1337);
function vis(ctx, id, on) { ctx.getNode(id).opacity(on ? 1 : 0); }
function frac(v) { return v - Math.floor(v); }

var FAM = '"Inter", "Helvetica Neue", Arial, sans-serif';  // index.html:438

/* inkMetrics + natW/swapW over the real-font measure surface (1x1 —
   measurement never touches pixels). One measure stack shared by the type
   calibration, pill widths and the dissolve field (createSurface is
   get-or-create by name, so re-entering per consumer is free). */
function makeMeasure(ctx) {
  var MSR = ctx.createSurface('k3dis-msr', 1, 1);
  function inkM(weight, probe) {
    MSR.font = weight + ' 100px ' + FAM;
    MSR.letterSpacing = '0px';
    var m = MSR.measureText(probe);
    return { capAsc: m.actualBoundingBoxAscent / 100, desc: m.actualBoundingBoxDescent / 100,
             fbAsc: m.fontBoundingBoxAscent / 100, fbDesc: m.fontBoundingBoxDescent / 100 };
  }
  function natW(weight, fs, ls, s) {
    MSR.font = weight + ' ' + fs + 'px ' + FAM;
    MSR.letterSpacing = ls + 'px';
    return MSR.measureText(s).width;
  }
  function swapW(oldTarget, newStr, oldStr, fs, ls, weight) {
    var wt = weight || 400;
    return oldTarget * natW(wt, fs, ls || 0, newStr) / natW(wt, fs, ls || 0, oldStr);
  }
  function natRunOf(s, fs) {
    var t = 0, i;
    for (i = 0; i < s.length; i++) t += natW(400, fs, -2.5, s[i]);
    return t;
  }
  return { inkM: inkM, natW: natW, swapW: swapW, natRunOf: natRunOf };
}

/* agent-pill offsetWidth replica: round(advance(26px w500 ls+0.2) + 14*2 +
   1.5*2). Engine stack reproduces all five baked widths exactly
   (parity harness k3_type_calibration_parity_report). */
function pillWidths(m) {
  return ['Agentic', 'Delta Attention', '1M Context', 'Vision', 'Coding'].map(function (l) {
    return Math.round(m.natW(500, 26, 0.2, l) + 31);
  });
}

/* ════════════ TYPE CALIBRATION (index.html:1356-1439) ═════════════
   The reference computes every calibrated run's font-size from a target
   cap height, then squeezes the run horizontally to an exact measured
   width (transform-origin 0 50% + left = centre - W/2). Our <text> box
   width equals the run's ADVANCE, and ctx scaleX() squeezes about the
   box CENTRE, so the equivalent is scaleX = W/advance_natural with
   left = centre - advance_natural/2.

   Squeeze factors the engine measure reproduces (parity harness, all
   Δ <= 1.1e-5) are derived at runtime below; the rest stay baked —
   their constants embed PIL-measured nats (kerned/hinted stack) that
   the engine advance cannot reproduce to <0.5px: aline1 Δ-6.4e-4,
   aline2 Δ+4.0e-4, t-649 Δ-1.6e-3, t-650 Δ-7.4e-4, btitle Δ+1.8e-2,
   fw2/gw2 Δ-8.7e-3/-8.1e-3, gw3 Δ-2.2e-1, open family Δ~-4.5e-3.
   Font sizes / lefts / tops stay in the classes: tops were calibrated
   to a 0.86em baseline (Chrome true factor via fontBoundingBox is
   0.86377em → +0.24..0.56px at our sizes — kept baked to preserve the
   pixel-validated layout), and the F/G word lefts carry measured
   corrections vs the stock fStart/gStart formulas (~9px, intentional). */
function applyTypeCalibration(ctx) {
  var m = makeMeasure(ctx);
  var CAL_SCALEX = {
    aline1: 0.9963656, aline1p: 0.9963656,
    aline2: 0.9284404, aline2p: 0.9284404,
    opentext: 0.9391232, openmid: 0.9427169, openrem: 0.948126,
    btitle: 0.961545,
    otxt: 663 / m.natRunOf('Every order', 133.3333),
    't-649': 0.9227896, 't-650': 1.046992,
    fw1: 246 / m.natW(400, 148.4536, -2.5, 'One'),
    fw2: 0.8756328,
    gw1: 260 / m.natW(400, 148.4536, -2.5, 'JTX.'),
    gw2: 0.9546101, gw3: 0.8845424,
    't-651': 132 / m.natW(500, 107.2165, -2.5, 'K3')
  };
  for (var calId in CAL_SCALEX) {
    var calN = ctx.getNode(calId);
    if (calN) calN.scaleX(CAL_SCALEX[calId]);
  }
}

/* ═══ corner marks + ruler strips ═══════════════════════════════════
   makeQmarks / makePlusses and makeStrip, plus the reference's global
   `gsap.set(".sq"|".qmark .lv"|".lh"|".plusm", { x:-23, y:66 })`
   applied as the QX/QY offset — all four furniture classes displace by
   the same (-23,+66), so screen rects = source coords + QOFF. */
var QX = -23, QY = 66; // gsap.set x/y, index.html:1527-1532
function drawQmarks(ctx, cv, pts) {
  var CK = ctx.CanvasKit;
  var sq = new CK.Paint(); sq.setStyle(CK.PaintStyle.Fill); sq.setColor(CK.parseColorString('#b9b9bc'));
  var ln = new CK.Paint(); ln.setStyle(CK.PaintStyle.Fill); ln.setColor(CK.parseColorString('#9a9a9e'));
  for (var i = 0; i < pts.length; i++) {
    var cx = pts[i][0], cy = pts[i][1], fx = pts[i][2], fy = pts[i][3];
    var bx = cx - 6.5 + QX, by = cy - 6.5 + QY; // .sq anchor (13x13)
    cv.drawRect([bx, by, bx + 13, by + 13], sq);
    var lvx = bx + (fx > 0 ? 16 : -11), lvy = by + (fy > 0 ? 17 : -28);
    cv.drawRect([lvx, lvy, lvx + 1.5, lvy + 20], ln);
    var lhx = bx + (fx > 0 ? 20 : -27), lhy = by + (fy > 0 ? 16 : -10);
    cv.drawRect([lhx, lhy, lhx + 20, lhy + 1.5], ln);
  }
}
function drawPlusses(ctx, cv, pts, size, op) {
  var CK = ctx.CanvasKit;
  var p = new CK.Paint(); p.setStyle(CK.PaintStyle.Fill); p.setColor(CK.Color(207, 207, 210, op));
  for (var i = 0; i < pts.length; i++) {
    var px = pts[i][0] + QX, py = pts[i][1] + QY, h = size / 2; // .plusm center
    cv.drawRect([px - 0.5, py - h, px + 0.5, py + h], p);
    cv.drawRect([px - h, py - 0.5, px + h, py + 0.5], p);
  }
}
function drawStrip(ctx, cv, x, y, w, h, n) {
  var CK = ctx.CanvasKit;
  var tick = new CK.Paint(); tick.setStyle(CK.PaintStyle.Fill); tick.setColor(CK.Color(207, 207, 210, 0.9));
  var dot = new CK.Paint(); dot.setStyle(CK.PaintStyle.Fill); dot.setColor(CK.Color(185, 185, 188, 0.7));
  var pitch = w / (n - 1);
  for (var i = 0; i < n; i++) {
    var lx = x + i * pitch;
    if (i % 2 === 0) cv.drawRect([lx, y, lx + 2, y + h], tick);
    else cv.drawRect([lx, y + h * 0.28, lx + 2, y + h * 0.73], tick);
    if (i < n - 1) cv.drawRect([lx + pitch / 2 - 1, y + h * 0.55, lx + pitch / 2 + 1, y + h * 0.55 + 2], dot);
  }
}

/* ════════════ GLOBAL PRNG STREAM replay (mulberry32(1337)) ═══════ */
/* The reference draws its procedural layout from ONE global
   mulberry32(1337) stream, consumed strictly in code order: 42 ghost
   columns, then the 18 barInit draws, then the 45 shimmer `to()`
   events. This IIFE replays that stream once per frame and caches the
   three tables on globalThis (same pattern as __k3_field). */
function getK3Stream() {
  return globalThis.__k3_stream || (globalThis.__k3_stream = (function () {
    var g = mulberry32(1337);   // fresh independent stream; top-level `rnd` untouched
    var cols = [], i, y, cells, h, t, j;
    for (var c = 0; c < 42; c++) {                     // ghost columns
      var colOp = range(0.3, 1, g);
      y = -Math.floor(range(0, 26, g));
      cells = [];
      while (y < 1080) {
        h = pick([18, 22, 26, 30], g);
        if (g() < 0.6) cells.push(y, h, range(0.3, 1, g));
        y += h + 6;
      }
      cols.push([colOp, cells]);
    }
    var barInit = [];
    for (i = 0; i < 18; i++) barInit.push(range(0.42, 1, g));
    var barEv = [];
    for (t = 6.16; t < 7.32; t += 0.1333)
      for (j = 0; j < 5; j++)
        barEv.push([Math.round(t * 10000) / 10000, Math.floor(g() * 18), range(0.4, 1, g)]);
    return { ghost: cols, barInit: barInit, barEv: barEv };
  })());
}

/* ════════════ SCENE A lattice-field tables ═════════════════════════
   Built at runtime by porting the reference's own generator 1:1 — no
   baked data. Four independent mulberry32 streams: buildField layout
   4242, RING_CELLS 3131, births 777, ringSched 9091. Verified against
   tools/k3replay.py: ring 39/39 cells + 363/363 events bit-exact,
   births within 1 ulp.
   FIELD_BORN row: [kind, x, y, param, birth]
     kind 0 = outlined 94px box at (x,y); border col.line
     kind 1 = pinwheel at (x+10,y+10), two (GS-20)/2 quadrants; param=flip
     kind 2 = plus: 1px cross centred (x+47,y+47), arm 9px
     kind 3 = dot: 3x3 at (x+45.5,y+45.5)
     kind 4 = fleck: (x,y) top-left, size param
     kind 5 = outlineCell: 86px box at (x,y) (=cell+4) + 5px corner ticks
   FIELD_RING row: [x, y, [t, kindIdx, alpha, ...]] kindIdx:
     0=empty 1=solid 2=pin 3=outline 4=scan 5=hatch */
var GS = 94;
function getK3Field() {
  return globalThis.__k3_field || (globalThis.__k3_field = (function () {
    // buildField placements — white layer; the purple clone
    // reuses the identical layout (same seed 4242)
    function snap(v) { return Math.round(v / GS) * GS; }
    var r = mulberry32(4242);
    function ringSnap() {
      for (var tries = 0; tries < 60; tries++) {
        var x = snap(range(430, 1500, r)), y = snap(range(0, 940, r));
        var cx = x + 47, cy = y + 47;
        if (cx < 455 || cx > 1560) continue;
        if (cx <= 690 || cx >= 1140 || cy <= 265 || cy >= 845) return [x, y];
      }
      return [564, 94];
    }
    var born = []; // [kind, x, y, param, slot]
    var i, rc;
    for (i = 0; i < 3; i++) { rc = ringSnap(); born.push([0, rc[0], rc[1], 0, 'early']); }
    for (i = 0; i < 4; i++) { rc = ringSnap(); born.push([1, rc[0], rc[1], r() < 0.5 ? 1 : 0, 'mid']); }
    for (i = 0; i < 14; i++) { rc = ringSnap(); born.push([2, rc[0], rc[1], 0, 'mid']); }
    for (i = 0; i < 12; i++) { rc = ringSnap(); born.push([3, rc[0], rc[1], 0, 'mid']); }
    for (i = 0; i < 6; i++) { rc = ringSnap(); born.push([4, rc[0] + 47 + range(-20, 20, r), rc[1] + 47 + range(-16, 16, r), range(5, 10, r), 'mid']); }
    for (i = 0; i < 4; i++) {
      rc = ringSnap();
      born.push([5, rc[0], rc[1], 0, 'early']);
      if (i === 0) born.push([5, rc[0] + GS, rc[1], 0, 'early']);
    }
    // births — near-TR test on each element's final CSS left/top, i.e. the
    // kind's +offset (pin +10, plus +42.5, dot +45.5, cell +4); fleck
    // already stores its fractional left/top
    var rB = mulberry32(777);
    var OFF = [0, 10, 42.5, 45.5, 0, 4];
    var bornRows = born.map(function (g) {
      var px = g[0] === 4 ? g[1] : g[1] + OFF[g[0]];
      var py = g[0] === 4 ? g[2] : g[2] + OFF[g[0]];
      var t = g[4] === 'early' ? (px > 1000 && py < 500 ? range(0.78, 0.95, rB) : range(0.95, 1.45, rB))
                               : range(1.0, 1.42, rB);
      // kind5 draws [x, y, x+GS-8, y+GS-8] directly, so it carries the
      // +4 already; every other kind applies its offset in the draw code
      var ex = g[0] === 5 ? g[1] + 4 : g[1], ey = g[0] === 5 ? g[2] + 4 : g[2];
      return [g[0], ex, ey, g[3], t];
    });
    // RING_CELLS — lattice ring around the title; ~15% dropped / ~40%
    // jittered one cell so the silhouette stays organic
    var rj = mulberry32(3131), cells = [];
    function put(x, y, px, py) {
      if (rj() < 0.15) return;
      if (rj() < 0.4) { x += px * GS * (rj() < 0.5 ? 1 : -1); y += py * GS * (rj() < 0.5 ? 1 : -1); }
      cells.push([x, y]);
    }
    var x2, y2;
    for (x2 = 564; x2 <= 1410; x2 += GS) { put(x2, 94, 0, 1); put(x2, 940, 0, 1); }
    for (y2 = 188; y2 <= 846; y2 += GS) { put(564, y2, 1, 0); put(1410, y2, 1, 0); }
    [[658,188],[1316,188],[658,846],[1316,846],[470,470],[1504,376],[470,752],[1504,658]].forEach(function (p) { cells.push(p); });
    // ringSched — two clockwise arms 180° apart, one lap ~0.9333s;
    // flurry windows [1.0,2.2] / [2.633,3.2]; per visit a 2-4-state
    // flurry at 1-2f steps. Reference kind names → kindIdx.
    var rR = mulberry32(9091);
    var T_REV = 0.9333, SPURTS = [[1.0, 2.2], [2.633, 3.2]];
    var KINDS_MID = ['solid', 'pin', 'outline', 'solid', 'scan', 'hatch'];
    var KINDS_HOLD = ['hatch', 'hatch', 'outline', 'solid', 'pin', 'hatch'];
    var KIDX = { empty: 0, solid: 1, pin: 2, outline: 3, scan: 4, hatch: 5 };
    var ringRows = cells.map(function (c) {
      var prog = (Math.atan2(c[1] + 47 - 540, c[0] + 47 - 960) / (2 * Math.PI) + 1) % 1;
      var seq = [];
      seq.push([0.9 + rR() * 0.6, rR() < 0.5 ? 0 : KIDX[pick(KINDS_HOLD, rR)], 0.55 + rR() * 0.45]);
      var passes = [], m, tp;
      for (m = 0; m < 10; m++) {
        tp = 0.62 + (prog + m * 0.5) * T_REV;
        if (SPURTS.some(function (s) { return tp >= s[0] && tp <= s[1]; })) passes.push(tp);
      }
      passes.forEach(function (p, pi) {
        if (rR() < 0.35 && pi !== passes.length - 1) return;
        var t = p + rR() * 0.04;
        var nCh = 2 + Math.floor(rR() * 3), k;
        for (k = 0; k < nCh; k++) {
          var last = k === nCh - 1, kind;
          if (last) kind = rR() < (pi === passes.length - 1 ? 0.55 : 0.72) ? 0 : KIDX[pick(KINDS_HOLD, rR)];
          else kind = KIDX[pick(KINDS_MID, rR)];
          seq.push([t, kind, 0.55 + rR() * 0.45]);
          t += 0.0333 * (1 + Math.floor(rR() * 2));
        }
      });
      seq.sort(function (a2, b2) { return a2[0] - b2[0]; });
      var ev = [];
      seq.forEach(function (e) { ev.push(e[0], e[1], e[2]); });
      return [c[0], c[1], ev];
    });
    return { born: bornRows, ring: ringRows };
  })());
}

/* ════════════ SCENE H+I: glitch dissolve → K3 wordmark ═════════════
   Dissolve is JS-authored over generic render-target ops (pixels never
   cross the JS bridge, no per-pixel loops in JS — JS writes the ALGORITHM
   as lambdas, Rust's generic executors run them):
     1. fillText the card lockup ONCE into an offscreen surface
        (ctx.createSurface → real Inter faces via swash);
     2. build the mask + 3-4 chamfer distance field with our own lambdas
        into the 'k3dis-field' render target: a seed pixel lambda
        (target.run, samples 'k3dis-off' as a render-target child)
        writes R=mask / G,B=dist lo,hi bytes, then two scan kernels
        (target.sweep forward/backward) do the chamfer sweeps;
     3. each frame a lambda-effect drawRect evaluates the scramble
        noise / dust / glow, sampling the field surface directly (no
        per-frame bake on the wire).
   The build-once flag lives on a non-configurable global — realm
   frame-boundary cleanup deletes only configurable keys. */

/* ---- dissolve field construction (index.html:1249-1349 → lambdas) ---- */
var DIS_R = { X0: 232, Y0: 430, W: 1460, H: 222 };         // index.html:1253
var CARD_TOP = 478, CARD_CAP = 108;                        // index.html:1177
var FGB_TOP = 480, FGB_H = 119;                            // index.html:1175

function rebuildDissolveField(ctx) {
  var m = makeMeasure(ctx);
  var ox = ctx.createSurface('k3dis-off', 1920, 1080);
  ox.clearRect(0, 0, 1920, 1080);
  ox.fillStyle = '#fff';
  var capM = m.inkM(400, 'H');
  var braceM = m.inkM(300, '{');
  ox.textBaseline = 'alphabetic';
  var base = CARD_TOP + CARD_CAP;
  // G card words (calibrateType, index.html:1415-1422)
  var fsT = CARD_CAP / capM.capAsc;
  var wK3 = m.swapW(260, 'K3.', 'JTX.', fsT, -2.5);
  var wOpen = m.swapW(245, 'Open', 'Live', fsT, -2.5);
  var gStart = 962.5 - (wK3 + 55 + 298 + 49 + wOpen) / 2;
  var G_RASTER = [['K3.', gStart, wK3], ['Now', gStart + wK3 + 55, 298],
                  ['Open', gStart + wK3 + 55 + 298 + 49, wOpen]];
  G_RASTER.forEach(function (w) {
    ox.save();
    ox.font = '400 ' + fsT + 'px ' + FAM;
    ox.letterSpacing = '-2.5px';
    var nat = ox.measureText(w[0]).width;
    ox.translate(w[1], 0);
    ox.scale(w[2] / nat, 1);
    ox.fillText(w[0], 0, base);
    ox.restore();
  });
  // braces (index.html:1270-1277; ink lefts 400/1469 = index.html:1413-1414)
  var totalB = braceM.capAsc + braceM.desc;
  var fsB = FGB_H / totalB;
  ox.font = '300 ' + fsB + 'px ' + FAM;
  ox.letterSpacing = '0px';
  var bBase = FGB_TOP + fsB * braceM.capAsc;
  var mL = ox.measureText('{');
  ox.fillText('{', 400 + mL.actualBoundingBoxLeft, bBase);
  var mR = ox.measureText('}');
  ox.fillText('}', 1469 + mR.actualBoundingBoxLeft, bBase);
  /* mask + 3-4 chamfer distance field, fully JS-authored over the
     generic render-target ops (§render target). Packing matches the
     reference field exactly: R=mask (source alpha), G=dist lo byte,
     B=dist hi byte, A=255 (dist in px*3, sentinel 3000). Seed byte()
     truncation ≡ the engine's u8 casts; % and floor are exact in f64,
     so the packed bytes round-trip losslessly through the 0..1 channels. */
  var field = ctx.createSurface('k3dis-field', DIS_R.W, DIS_R.H);
  field.run(
    ly.pixel(
    /* seed: sample 'k3dis-off' (render-target child, offset X0/Y0 baked
       into the source) → mask = alpha, dist = alpha>120 ? 0 : 3000, pack. */
    (uv, src) => {
      const c = src.eval(uv + [232, 430]);
      const m = byte(c.a * 255 + 0.5);
      const d = m > 120 ? 0 : 3000;
      return [m / 255, d % 256 / 255, floor(d / 256) / 255, 1];
    },
    ), { child: ox }
  );
  field.sweep(ly.scan(
    /* forward 3-4 chamfer sweep (index.html:1278-1291): read left /
       up / up-left / up-right at weights 3/3/4/4, take the min, write
       back. get(dx,dy) reads the in-progress buffer; the executor owns
       the row-major order. Ties keep the incumbent, same as the
       reference strict `<` updates (value-wise identical). */
    (get) => {
      const c = get(0, 0);
      const d = byte(c.g * 255 + 0.5) + byte(c.b * 255 + 0.5) * 256;
      const l = get(-1, 0);
      const t = get(0, -1);
      const tl = get(-1, -1);
      const tr = get(1, -1);
      const m = min(min(d, byte(l.g * 255 + 0.5) + byte(l.b * 255 + 0.5) * 256 + 3),
                    min(byte(t.g * 255 + 0.5) + byte(t.b * 255 + 0.5) * 256 + 3,
                        min(byte(tl.g * 255 + 0.5) + byte(tl.b * 255 + 0.5) * 256 + 4,
                            byte(tr.g * 255 + 0.5) + byte(tr.b * 255 + 0.5) * 256 + 4)));
      return [c.r, m % 256 / 255, floor(m / 256) / 255, 1];
  }, { dir: 'forward' }));
  field.sweep(ly.scan(
    /* backward sweep (index.html:1292-1313): right / down / down-right /
       down-left at 3/3/4/4, mirrored traversal. */
    (get) => {
      const c = get(0, 0);
      const d = byte(c.g * 255 + 0.5) + byte(c.b * 255 + 0.5) * 256;
      const r = get(1, 0);
      const b = get(0, 1);
      const br = get(1, 1);
      const bl = get(-1, 1);
      const m = min(min(d, byte(r.g * 255 + 0.5) + byte(r.b * 255 + 0.5) * 256 + 3),
                    min(byte(b.g * 255 + 0.5) + byte(b.b * 255 + 0.5) * 256 + 3,
                        min(byte(br.g * 255 + 0.5) + byte(br.b * 255 + 0.5) * 256 + 4,
                            byte(bl.g * 255 + 0.5) + byte(bl.b * 255 + 0.5) * 256 + 4)));
      return [c.r, m % 256 / 255, floor(m / 256) / 255, 1];
  }, { dir: 'backward' }));
  Object.defineProperty(globalThis, '__k3disBuilt', {
    value: true, writable: true, enumerable: true, configurable: false
  });
}

/* ══ lambda 溶解（ly.pixel 内核，Liangyi/两仪）══════════════════════
   场景 H scramble 的泛化路径：逐像素算法写成 JS lambda（只被编译、
   从不执行），Rust 解析源码后重建计算。`h01` 属 CpuOnly 能力 → 自动
   派发纯 CPU 后端（f64 AST 解释器）。field surface（mask/dist）以
   surface child 直接采样（R=mask、G/B=dist 低/高字节 px*3）——session
   级 render target 不上 wire、无需每帧烘焙。逐帧标量（bandP 行、ep/fr、
   streak 包络、dust 基线）= "简单数据计算"，留在 JS 以 uniforms 传入。
   像素缓冲不跨 JS 桥。 */

/* 距离带点亮概率（index.html:1323-1331）+ 线性插值 */
var DIS_TAB = [
  [412.0, [0.97, 0.88, 0.74, 0.34, 0.06, 0.010]],
  [414.0, [0.97, 0.88, 0.84, 0.56, 0.19, 0.033]],
  [425.0, [0.94, 0.83, 0.80, 0.54, 0.21, 0.036]],
  [435.0, [0.87, 0.77, 0.74, 0.51, 0.23, 0.036]],
  [441.0, [0.83, 0.735, 0.70, 0.49, 0.24, 0.036]],
  [442.0, [0.76, 0.66, 0.61, 0.41, 0.19, 0.036]],
  [443.0, [0.60, 0.47, 0.37, 0.23, 0.10, 0.030]]
];
function bandP(f) {
  if (f <= DIS_TAB[0][0]) return DIS_TAB[0][1];
  for (var i = 0; i < DIS_TAB.length - 1; i++) {
    if (f <= DIS_TAB[i + 1][0]) {
      var fa = DIS_TAB[i][0], va = DIS_TAB[i][1];
      var fb = DIS_TAB[i + 1][0], vb = DIS_TAB[i + 1][1];
      var tt = (f - fa) / (fb - fa);
      var out = [];
      for (var j = 0; j < 6; j++) out.push(va[j] + (vb[j] - va[j]) * tt);
      return out;
    }
  }
  return DIS_TAB[DIS_TAB.length - 1][1];
}

/* glow 带包络（index.html:1344-1349）：f416 出现、f420-436 全开、f440 消亡 */
function streakEnv(f) {
  if (f < 416 || f > 439) return 0;
  if (f < 420) return 0.42 + 0.145 * (f - 416);
  if (f <= 436) return 1;
  if (f === 437) return 0.58;
  if (f === 438) return 0.33;
  return 0.125;
}

/* 渲染 lambda：参考 index.html:1456-1520 的逐位转写。参数约定：
   uv=rect 本地像素坐标（中心 i+0.5）；field=烘焙 field child；
   u=uniforms（x0/y0 field 在 surface 中的原点、f/ep/fr 帧驱动、
   dust_base/s_env/sx_l/sx_r 逐帧标量、pa/pb2 bandP 六值）。返回
   0..1 straight RGBA。`byte()` = JS |0 截断语义（正数域）。 */
var DIS_LAMBDA = (uv, field, u) => {
  const px = byte(uv.x);
  const py = byte(uv.y);
  const sx = u.x0 + px;
  const sy = u.y0 + py;
  const c = field.eval(uv);
  const a = byte(c.r * 255 + 0.5);
  const d3 = byte(c.g * 255 + 0.5) + 256 * byte(c.b * 255 + 0.5);
  const d = d3 / 3;
  const cx = byte(sx / 3);
  const rb = sy % 3;
  const dys = sy - 540;
  let gy = 0;
  if (u.s_env > 0 && dys > -46 && dys < 46) {
    gy = exp(-(dys * dys) / 441.6);
  }
  let v = 0;
  if (u.f < 412) {
    if (a > 10) {
      v = 214 * a / 255;
    }
  } else {
    const nz = h01(cx, sy, u.ep) * (1 - u.fr) + h01(cx, sy, u.ep + 1) * u.fr;
    let p = 0;
    if (u.f >= 444) {
      p = d < 24 ? u.dust_base * (1 - 0.28 * d / 24) : 0.03 * (455 - u.f) / 10;
    } else {
      let bi = 5;
      if (d < 1) { bi = 0; } else if (d < 4) { bi = 1; } else if (d < 8) { bi = 2; } else if (d < 16) { bi = 3; } else if (d < 24) { bi = 4; }
      let pb = bi === 0 ? u.pa.x : bi === 1 ? u.pa.y : bi === 2 ? u.pa.z : bi === 3 ? u.pa.w : bi === 4 ? u.pb2.x : u.pb2.y;
      if (bi === 5 && (sx < 402 || sx > 1548)) {
        pb = 0;
      }
      pb = pb * (bi < 2 ? (rb === 0 ? 1.1 : rb === 1 ? 1 : 0.8) : (rb === 0 ? 1.4 : rb === 1 ? 1.05 : 0.58));
      p = pb < 0.985 ? pb : 0.985;
    }
    if (nz < p) {
      const nb = h01(cx * 3 + 911, sy + 377, u.ep);
      if (u.f >= 444) {
        v = 150 + 80 * nb;
      } else if (d <= 1.6) {
        v = 255;
      } else {
        v = 128 + 96 * nb;
      }
    } else if (u.f < 444 && d <= 10) {
      v = 44 + 12 * nz;
    }
  }
  if (gy > 0) {
    if (sx > u.sx_l - 8 && sx < u.sx_r + 8) {
      let xp = 1;
      if (sx < u.sx_l + 60) {
        xp = (sx - u.sx_l + 8) / 68;
        xp = xp > 0 ? xp : 0;
      } else if (sx > u.sx_r - 36) {
        xp = (u.sx_r + 8 - sx) / 44;
        xp = xp > 0 ? xp : 0;
      }
      if (xp > 0) {
        const cs = cx + 4096;
        const ng = h01(cs, sy, u.ep) * (1 - u.fr) + h01(cs, sy, u.ep + 1) * u.fr;
        const gm = ng < 0.91 ? 0.85 + 1.15 * ng : 3.2 + 2.2 * (ng - 0.91) / 0.09;
        v = v + 20 * u.s_env * gy * xp * gm;
        v = v > 255 ? 255 : v;
      }
    }
  }
  if (v >= 40) {
    const g = byte(v);
    return [g / 255, g / 255, g / 255, 1];
  }
  if (v > 0.8) {
    const al = byte(v * 6.375);
    return [40 / 255, 40 / 255, 40 / 255, al / 255];
  }
  return [0, 0, 0, 0];
};

var DIS_SPEC = {
  uniforms: {
    x0: 'float', y0: 'float', f: 'float', ep: 'float', fr: 'float',
    dust_base: 'float', s_env: 'float', sx_l: 'float', sx_r: 'float',
    pa: 'float4', pb2: 'float2'
  },
  backend: 'auto'
};

/* per-frame：lambda drawRect 通用路径。field 是 session 级 render
   target（rebuildDissolveField 构建一次），以渲染目标句柄直采，
   不再每帧烘焙上 wire。put 位置 (232, 230) 与尺寸 1460x222 同
   applyDissolve 旧路。 */
function drawDissolve(ctx, cvx, t) {
  var CK = ctx.CanvasKit;
  var DIS_EFFECT = ly.pixel(DIS_LAMBDA, DIS_SPEC);
  if (t < 13.699 || t > 15.2) return false;
  var f = Math.floor(t * 30 + 1e-4);
  if (f < 0 || f > 455) return false;
  var row = bandP(f);
  var ep = Math.floor(f / 7);
  var fr = (f % 7) / 7;
  var dustBase = 0;
  if (f >= 444) {
    dustBase = (f === 444) ? 0.40 : 0.32 * Math.max(455 - f, 0) / 10;
  }
  var sEnv = streakEnv(f);
  var sxL = 240, sxR = 1686;
  if (f >= 437) {
    sxL = 240 + (f - 436) * 18;
    sxR = 1686 - (f - 436) * 18;
  }
  var field = ctx.createSurface('k3dis-field', DIS_R.W, DIS_R.H);
  var p = new CK.Paint();
  p.setShader(DIS_EFFECT.sample({
    x0: DIS_R.X0, y0: DIS_R.Y0, f: f, ep: ep, fr: fr,
    dust_base: dustBase, s_env: sEnv, sx_l: sxL, sx_r: sxR,
    pa: [row[0], row[1], row[2], row[3]],
    pb2: [row[4], row[5]]
  }, [field]));
  cvx.drawRect(CK.XYWHRect(DIS_R.X0, DIS_R.Y0 - 200, DIS_R.W, DIS_R.H), p);
  return true;
}

/* ════════════ 场景组件：每段 XML script 原样进各自的 useGSAP ════════════ */

/* ghost order-book columns (track 1, window 4.583→7.35s) */
function GhostCols() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.time;
    var K3STREAM = getK3Stream();
    var G_ON = (T >= 4.583 && T < 7.35);
    vis(ctx, 'ghostcols', G_ON);
    if (G_ON) {
      ctx.timeline()
        .set('ghostinner', { opacity: 0 }, 4.583)
        .to('ghostinner', { opacity: 0.9, duration: 0.55, ease: 'power1.inOut' }, 4.683);
      var gcv = ctx.getCanvasById('ghostcols-canvas');
      if (gcv) {
        gcv.clear();
        var gPaint = new CK.Paint(); gPaint.setStyle(CK.PaintStyle.Fill);
        var GHOST_COLS = K3STREAM.ghost;
        for (var c = 0; c < GHOST_COLS.length; c++) {
          var colOp = GHOST_COLS[c][0], gcells = GHOST_COLS[c][1];
          for (var gi = 0; gi < gcells.length; gi += 3) {
            var gy = gcells[gi], gh = gcells[gi + 1], gop = gcells[gi + 2];
            gPaint.setColor(CK.Color(16, 16, 16, colOp * gop));
            gcv.drawRRect(CK.RRectXY([c * 46, gy, c * 46 + 30, gy + gh], 3, 3), gPaint);
          }
        }
      }
    }
  });
  return (
    <div id="ghostcols" class="absolute inset-0 z-[1] opacity-0">
      <div id="ghostinner" class="absolute inset-0">
        <StageCanvas id="ghostcols-canvas" />
      </div>
    </div>
  );
}

/* SCENE A: node field (track 2, window 0.75→4.62s) */
function SceneA() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.time;
    var A_ON = (T >= 0.75 && T < 4.62);
    vis(ctx, 'sceneA', A_ON);
    if (A_ON) {
      // A zoom + field-energy envelope are declarative (index.html:1569,1576-1580).
      // The zoom holds at its CSS scale until 0.933, then tweens 1.07→0.9417
      // (a `.set` seeds the 1.07 from-value; a `fromTo` would immediate-render
      // 1.07 from t=0, which measured 0.07pp worse at f23-27). The envelope
      // targets both #afield and #afieldp.
      ctx.timeline()
        .set('azoom', { scale: 1.07 }, 0.933)
        .to('azoom', { scale: 0.9417, duration: 3.667, ease: 'none' }, 0.933)
        .set(['afield', 'afieldp'], { opacity: 1 }, 0.75)
        .to(['afield', 'afieldp'], { opacity: 0.35, duration: 0.4333, ease: 'power1.inOut' }, 1.9667)
        .to(['afield', 'afieldp'], { opacity: 0.60, duration: 0.4, ease: 'power1.inOut' }, 2.4)
        .to(['afield', 'afieldp'], { opacity: 0.08, duration: 1.4333, ease: 'none' }, 2.8)
        .to(['afield', 'afieldp'], { opacity: 0.02, duration: 0.3, ease: 'none' }, 4.2333)
        // ruler strips retire (index.html:1634); the ref ease is power1.in
        // (the former hand-written fade was linear).
        .to('afurn', { opacity: 0, duration: 0.18, ease: 'power1.in' }, 4.35)
        // A exit hard-cuts azoom + apills to black at 4.5667 (index.html:1636,
        // "hard 2-frame black dip at n137-138" before scene B).
        .set(['azoom', 'apills'], { opacity: 0 }, 4.5667)
        // purple clone crossfade (index.html:1613-1614, power1.inOut)
        .fromTo('apwrap', { opacity: 0 }, { opacity: 1, duration: 0.2, ease: 'power1.inOut' }, 3.03)
        .to('apwrap', { opacity: 0, duration: 0.17, ease: 'power1.inOut' }, 3.83);

      var K3FIELD = getK3Field();
      var FIELD_BORN = K3FIELD.born;
      var FIELD_RING = K3FIELD.ring;
      function ioEase(p) { p = p < 0 ? 0 : (p > 1 ? 1 : p); return p < 0.5 ? 2 * p * p : 1 - 2 * (1 - p) * (1 - p); }

      function paintField(canvasId, purple) {
        var cv = ctx.getCanvasById(canvasId); if (!cv) return;
        cv.clear();
        var pf = new CK.Paint(); pf.setStyle(CK.PaintStyle.Fill);
        var ps = new CK.Paint(); ps.setStrokeWidth(1); ps.setStyle(CK.PaintStyle.Stroke);
        // CSS mask-image semantics (r12): the reference .afield mask-image
        // multiplies ONLY the masked subtree's own pixels; white-field pixels
        // behind the purple clone are BACKDROP and are not masked a second
        // time. Purple paint alphas are pre-multiplied by the mask remaining
        // factor w(r) = 1-clamp((r-0.52)/0.48,0,1) (ellipse 64%/70% at 50%,
        // stop1 52%, field coords); the white afield-mask overlay stays.
        function maskW(px, py) {
          var dx = (px - 960) / 1228.8, dy = (py - 540) / 756;
          return 1 - clamp((Math.sqrt(dx * dx + dy * dy) - 0.52) / 0.48, 0, 1);
        }
        // lattice: #alat set 0 @0.75 then power1.inOut to 1 over 1.0-1.4s;
        // #alatp set 1 @0.75 (constant)
        var latA = purple ? 1 : ioEase((T - 1.0) / 0.4);
        if (latA > 0) {
          var gp = new CK.Paint(); gp.setStyle(CK.PaintStyle.Stroke); gp.setStrokeWidth(1);
          // .alat uses inset:-120px → grid lines start at -120 (phase-verified
          // at f46). r18: CSS repeating gradients rasterize at DEVICE pixels —
          // every line is a crisp 1px column/row at screen round((x_l-960)*Z)+960.
          var ZL = 1;
          if (T >= 0.933) ZL = 1.07 + (0.9417 - 1.07) * clamp((T - 0.933) / 3.667, 0, 1);
          // Skia geometry coords are pixel-EDGE based: to fill screen pixel c
          // exactly the centre must be c+0.5.
          var snapX = function (xs) { return 960 + (Math.round(xs) + 0.5 - 960) / ZL; };
          var snapY = function (ys) { return 540 + (Math.round(ys) + 0.5 - 540) / ZL; };
          // with the stroke pre-divided by ZL the zoomed line is exactly 1
          // DEVICE px wide, so centring it on an integer screen pixel gives
          // full coverage of that pixel.
          gp.setStrokeWidth(1 / ZL);
          if (purple) {
            for (var gxp = 0; gxp <= 24; gxp++) {
              var xl = -120 + gxp * GS;
              var xs = 960 + (xl - 960) * ZL;
              if (xs < -1 || xs > 1921) continue;
              gp.setColor(CK.Color(159, 120, 220, 0.28 * latA * maskW(xl, 540)));
              cv.drawLine(snapX(xs), 0, snapX(xs), 1080, gp);
            }
            // CSS 0deg gradient anchors at the container BOTTOM: each 1px line
            // ends a tile, so y_src ≡ 1199 mod 94 = 71
            for (var gyp = -2; gyp <= 12; gyp++) {
              var yl = 71 + gyp * GS;
              var ys = 540 + (yl - 540) * ZL;
              if (ys < -1 || ys > 1081) continue;
              gp.setColor(CK.Color(159, 120, 220, 0.28 * latA * maskW(960, yl)));
              cv.drawLine(0, snapY(ys), 1920, snapY(ys), gp);
            }
          } else {
            gp.setColor(CK.Color(255, 255, 255, 0.105 * latA));
            for (var gxw = 0; gxw <= 24; gxw++) {
              var xs = 960 + (-120 + gxw * GS - 960) * ZL;
              if (xs < -1 || xs > 1921) continue;
              cv.drawLine(snapX(xs), 0, snapX(xs), 1080, gp);
            }
            for (var gyw = -2; gyw <= 12; gyw++) {
              var ys = 540 + (71 + gyw * GS - 540) * ZL;
              if (ys < -1 || ys > 1081) continue;
              cv.drawLine(0, snapY(ys), 1920, snapY(ys), gp);
            }
          }
        }
        // ---- born elements (appended before blinkers in the DOM → below) ----
        var capCol = purple ? CK.Color(157, 120, 200, 1) : CK.Color(166, 166, 163, 1);
        FIELD_BORN.forEach(function (g) {
          if (T < g[4]) return;
          var k = g[0], x = g[1], y = g[2], prm = g[3];
          var mw = purple ? maskW(x + 47, y + 47) : 1; // mask-image w(r), r12
          if (k === 0) {
            ps.setColor(purple ? CK.Color(171, 132, 207, 0.55 * mw) : CK.Color(255, 255, 255, 0.30));
            cv.drawRect([x, y, x + GS, y + GS], ps);
          } else if (k === 1) {
            pf.setColor(purple ? CK.Color(157, 120, 200, mw) : capCol);
            var hw = (GS - 20) / 2, x0 = x + 10, y0 = y + 10;
            if (prm) { cv.drawRect([x0, y0, x0 + hw, y0 + hw], pf); cv.drawRect([x0 + hw, y0 + hw, x0 + 2 * hw, y0 + 2 * hw], pf); }
            else { cv.drawRect([x0 + hw, y0, x0 + 2 * hw, y0 + hw], pf); cv.drawRect([x0, y0 + hw, x0 + hw, y0 + 2 * hw], pf); }
          } else if (k === 2) {
            pf.setColor(purple ? CK.Color(171, 132, 207, 0.85 * mw) : CK.Color(255, 255, 255, 0.42));
            cv.drawRect([x + 46.5, y + 42.5, x + 47.5, y + 51.5], pf);
            cv.drawRect([x + 42.5, y + 46.5, x + 51.5, y + 47.5], pf);
          } else if (k === 3) {
            pf.setColor(purple ? CK.Color(171, 132, 207, 0.75 * mw) : CK.Color(255, 255, 255, 0.30));
            cv.drawRect([x + 45.5, y + 45.5, x + 48.5, y + 48.5], pf);
          } else if (k === 4) {
            pf.setColor(purple ? CK.parseColorString('#8a63b0') : CK.parseColorString('#6b468a'));
            if (purple) pf.setAlphaf(mw);
            cv.drawRect([x, y, x + prm, y + prm], pf);
            if (purple) pf.setAlphaf(1);
          } else {
            ps.setColor(purple ? CK.Color(171, 132, 207, 0.7 * mw) : CK.Color(255, 255, 255, 0.42));
            cv.drawRect([x, y, x + GS - 8, y + GS - 8], ps);
            pf.setColor(purple ? CK.Color(171, 132, 207, mw) : CK.Color(181, 181, 178, 1));
            var xe = x + GS - 8, ye = y + GS - 8;
            cv.drawRect([x - 2, y - 2, x + 3, y + 3], pf);
            cv.drawRect([xe - 3, y - 2, xe + 2, y + 3], pf);
            cv.drawRect([x - 2, ye - 3, x + 3, ye + 2], pf);
            cv.drawRect([xe - 3, ye - 3, xe + 2, ye + 2], pf);
          }
        });
        // ---- ring blinkers: 78px cell at (+8,+8); the whiteCap palette is
        // selected PER EVENT (t<2.2 → white), frozen by each tl.set
        var gsp = new CK.Paint(); gsp.setStyle(CK.PaintStyle.Stroke); gsp.setStrokeWidth(2);
        FIELD_RING.forEach(function (rc) {
          var ev = rc[2], kind = 0, al = 0, evCap = false;
          for (var ei = 0; ei < ev.length; ei += 3) {
            if (T >= ev[ei]) { kind = ev[ei + 1]; al = ev[ei + 2]; evCap = ev[ei] >= 2.2; }
          }
          if (!kind) return;
          var x = rc[0] + 8, y = rc[1] + 8, cs = GS - 16;
          var mw = purple ? maskW(x + cs / 2, y + cs / 2) : 1; // mask-image w(r), r12
          var SOLID = purple ? [157, 120, 200] : (evCap ? [165, 165, 162] : [223, 223, 221]);
          var SCANC = purple ? [165, 134, 204] : (evCap ? [165, 165, 162] : [228, 228, 226]);
          var HATC = purple ? [157, 120, 200] : (evCap ? [160, 158, 155] : [216, 216, 213]);
          if (kind === 1) {
            pf.setColor(CK.Color(SOLID[0], SOLID[1], SOLID[2], al * mw));
            cv.drawRect([x, y, x + cs, y + cs], pf);
          } else if (kind === 2) {
            pf.setColor(CK.Color(SOLID[0], SOLID[1], SOLID[2], al * mw));
            var h2 = cs / 2;
            cv.drawRect([x, y, x + h2, y + h2], pf);
            cv.drawRect([x + h2, y + h2, x + cs, y + cs], pf);
          } else if (kind === 4) {
            // scan: repeating-linear-gradient(0deg, c 0 3px, 0.38c 3px 6px);
            // with H=78 the 3px solid band sits at (p_y-y) mod 6 ∈ (3,6] →
            // start solid at +3 (r10: ref f96 (752,0) band trace).
            cv.save(); cv.clipRect([x, y, x + cs, y + cs]);
            pf.setColor(CK.Color(SCANC[0], SCANC[1], SCANC[2], (purple ? 0.35 : 0.38) * al * mw));
            cv.drawRect([x, y, x + cs, y + cs], pf);
            pf.setColor(CK.Color(SCANC[0], SCANC[1], SCANC[2], al * mw));
            for (var sy = 3; sy < cs; sy += 6) cv.drawRect([x, y + sy, x + cs, y + sy + 3], pf);
            cv.restore();
          } else if (kind === 5) {
            // hatch: repeating-linear-gradient(45deg, c 0 2px, t 2px 9px).
            // CSS 45deg gradient axis points NE (up-right); bands run
            // PERPENDICULAR to the axis = "\" (down-right). Band centres sit
            // at (p-c)·axis ≡ -3.1px (mod 9) [calibrated: ref f96 six-cell
            // cross-correlation sweep, minimum at -3.1±0.15].
            cv.save(); cv.clipRect([x, y, x + cs, y + cs]);
            gsp.setColor(CK.Color(HATC[0], HATC[1], HATC[2], al * mw));
            var dd = 0.70710678, xc2 = x + cs / 2 - 3.1 * dd, yc2 = y + cs / 2 + 3.1 * dd;
            for (var hs = -cs; hs <= cs; hs += 9) {
              var mx = xc2 + hs * dd, my = yc2 - hs * dd;
              cv.drawLine(mx - cs * dd, my - cs * dd, mx + cs * dd, my + cs * dd, gsp);
            }
            cv.restore();
          }
          if (kind === 3) {
            ps.setColor(purple ? CK.Color(171, 132, 207, 0.8 * al * mw) : CK.Color(255, 255, 255, 0.55 * al));
            cv.drawRect([x, y, x + cs, y + cs], ps);
            pf.setColor(purple ? CK.Color(171, 132, 207, al * mw) : CK.Color(216, 216, 214, al));
            var xe = x + cs, ye = y + cs;
            cv.drawRect([x - 2, y - 2, x + 3, y + 3], pf);
            cv.drawRect([xe - 3, y - 2, xe + 2, y + 3], pf);
            cv.drawRect([x - 2, ye - 3, x + 3, ye + 2], pf);
            cv.drawRect([xe - 3, ye - 3, xe + 2, ye + 2], pf);
          }
        });
      }

      // #afield opacity comes from the field-energy timeline above.
      // hue-shift to dark purple after n105 (index.html:1583) — a thin
      // marker call; the engine supports the full CSS filter chain natively.
      if (T >= 3.5) ctx.getNode('afield').filter('brightness(0.5) sepia(1) hue-rotate(215deg) saturate(2.2)');
      paintField('afield-canvas', false);

      // purple clone crossfade (3.03→3.23 in, 3.83→4.00 out; power1.inOut) is
      // declarative (see timeline); ppOp below only gates the purple-child work.
      var ppOp = (T >= 3.03 && T <= 4.0)
        ? (T < 3.23 ? ioEase((T - 3.03) / 0.2) : (T > 3.83 ? ioEase(1 - (T - 3.83) / 0.17) : 1)) : 0;
      if (ppOp > 0) { paintField('afieldp-canvas', true); drawLanes('afieldp-canvas', true); }

      // selection box + title fade-in (index.html:1607-1619)
      ctx.timeline()
        .set(['atitlegrp', 'atitlegrpp'], { opacity: 0 }, 0.75)
        .to('atitlegrp', { opacity: 1, duration: 0.06, ease: 'power1.out' }, 1.73)
        .set('atitlegrpp', { opacity: 1 }, 1.79);
      vis(ctx, 'aw1', T >= 1.8); vis(ctx, 'aline1', T >= 1.8); vis(ctx, 'aline1p', T >= 1.8 && ppOp > 0);
      vis(ctx, 'aw2', T >= 1.8330);
      vis(ctx, 'aline2', T >= 1.8663); vis(ctx, 'aline2p', T >= 1.8663 && ppOp > 0);

      // Box ghost lanes: CSS .glane = 2x360 strip, rotate(27deg) about its
      // centre ("/" direction), inside the clipped 467x185 aselrect at
      // (755,444). Dashes 3x13 glide down-left. Drawn on BOTH canvases.
      function drawLanes(canvasId, purple) {
        var cv = ctx.getCanvasById(canvasId); if (!cv) return;
        var lineA = purple ? 0.22 : 0.10, dashA = purple ? 0.78 : 0.6;
        var lc = purple ? [171, 132, 207] : [255, 255, 255];
        var dc = purple ? [171, 132, 207] : [224, 224, 228];
        cv.save(); cv.clipRect([755, 444, 755 + 467, 444 + 185]);
        var lp = new CK.Paint(); lp.setStyle(CK.PaintStyle.Stroke); lp.setStrokeWidth(2);
        var dp = new CK.Paint(); dp.setStyle(CK.PaintStyle.Fill);
        var ct = Math.cos(27 * Math.PI / 180), st = Math.sin(27 * Math.PI / 180);
        var cx0 = 757.55, cy0 = 544; // aselrect origin + lane centre (1,100);
        // +1.55 local (≈+1.6 screen @Z1.037): r18 f56 four-row fit — ref
        // diagonal family sits +1.6px right of the nominal formula.
        if (T >= 1.75 && T <= 4.63) {
          for (var li = 0; li < 7; li++) {
            var cx = cx0 + (-60 + li * 100);
            lp.setColor(CK.Color(lc[0], lc[1], lc[2], lineA));
            cv.drawLine(cx + st * 180, cy0 - ct * 180, cx - st * 180, cy0 + ct * 180, lp);
            var C = 0.4667, G = 0.3667;
            var u = (T - 1.75) / C + li * 0.214;
            var c = Math.floor(u), tau = (u - c) * C;
            if (tau <= G) {
              for (var j = 0; j < 3; j++) {
                var h = mulberry32((li * 131) ^ (c * 7919) ^ 0x77)();
                var y0 = 20 + h * 60 + j * 95;
                var a = Math.min(1, tau / 0.0667, (G - tau) / 0.1);
                var yl = y0 + 61.5 * tau + 6.5 - 180; // dash centre vs lane centre
                dp.setColor(CK.Color(dc[0], dc[1], dc[2], a * dashA));
                cv.drawRect([cx - yl * st - 1.5, cy0 + yl * ct - 6.5,
                             cx - yl * st + 1.5, cy0 + yl * ct + 6.5], dp);
              }
            }
          }
        }
        cv.restore();
      }
      drawLanes('afield-canvas', false);

      // pills orbit (orbitPills, 1.4 → 4.6s)
      var PBIRTH = [1.483, 1.483, 1.483, 1.567, 1.633];
      var PILL_KF = [
        [[1.70,570,367],[2.30,817,154],[3.00,946,139],[4.00,1089,171],[4.50,1287,375]],
        [[2.30,1316,266],[3.00,1373,373],[4.00,1396,494],[4.50,1257,729]],
        [[1.70,737,815],[2.30,580,547],[3.00,585,435],[4.00,674,328],[4.50,910,218]],
        [[1.70,1172,831],[2.30,860,879],[3.00,752,825],[4.00,665,737],[4.50,594,472]],
        [[2.00,1399,668],[2.30,1364,769],[3.00,1254,848],[4.00,1086,891],[4.50,769,832]],
      ];
      var ORB_CX = 985, ORB_CY = 540, PILL_EXIT_Y = [884, 428, 655, 768, 541];
      function orbTheta(t) {
        if (t <= 3.25) return 54.65 * (1 - Math.exp(-(t - 1.7) / 0.3906)) + 10.1 * (t - 1.7);
        return 69.27 + 0.7934 * (Math.exp((t - 3.25) / 0.3005) - 1) + 10.56 * (t - 3.25);
      }
      var pillTracks = PILL_KF.map(function (kfs) {
        var prev = null;
        return kfs.map(function (k) {
          var a = Math.atan2(k[1] - ORB_CX, ORB_CY - k[2]) * 180 / Math.PI;
          if (a < 0) a += 360; if (prev !== null) { while (a < prev) a += 360; } prev = a;
          var dx = k[1] - ORB_CX, dy = k[2] - ORB_CY;
          return [k[0], a, Math.sqrt(dx * dx + dy * dy)];
        });
      });
      var pillIds = ['ap1', 'ap2', 'ap3', 'ap4', 'ap5'];
      // pill box widths = offsetWidth replica over the real-font surface
      // measure; bstack reuses the same map (bsw).
      var pillW = pillWidths(makeMeasure(ctx));
      // per-pill hstrip stripe phase (r18): Chrome rasterises each .hstrip's
      // repeating gradient with a per-element subpixel phase — static per
      // pill, frame-independent.
      var aph = [0.7, 3.2, -1.0, 2.1, -1.1];
      // .hstrip hatch lives on its own canvas inside #apills (same z-layer
      // as the pill boxes, unaffected by #afield's envelope fade)
      var F = Math.floor(T * 30 + 1e-4); // frame number
      var sCvAll = ctx.getCanvasById('apill-canvas');
      if (sCvAll) sCvAll.clear();
      if (T >= 1.4 && T <= 4.6) {
        var th = orbTheta(T);
        for (var pi = 0; pi < pillIds.length; pi++) {
          var pn = ctx.getNode(pillIds[pi]); if (!pn) continue;
          if (T < PBIRTH[pi] - 0.001) { pn.opacity(0); continue; }
          pn.opacity(1);
          var trk = pillTracks[pi], a, r;
          if (T <= trk[0][0]) { a = trk[0][1] - (orbTheta(trk[0][0]) - th); r = trk[0][2]; }
          else if (T >= trk[trk.length - 1][0]) {
            var kl = trk[trk.length - 1], kp = trk[trk.length - 2];
            var dth = orbTheta(kl[0]) - orbTheta(kp[0]) || 1;
            a = kl[1] + (th - orbTheta(kl[0])); r = kl[2] + (kl[2] - kp[2]) * (th - orbTheta(kl[0])) / dth;
          } else {
            var k = 0; while (trk[k + 1][0] < T) k++;
            var t0 = orbTheta(trk[k][0]), t1 = orbTheta(trk[k + 1][0]), w = (th - t0) / (t1 - t0);
            a = trk[k][1] + (trk[k + 1][1] - trk[k][1]) * w;
            r = trk[k][2] + (trk[k + 1][2] - trk[k][2]) * w;
          }
          var rad = a * Math.PI / 180;
          var cx = ORB_CX + r * Math.sin(rad), cy = ORB_CY - r * Math.cos(rad);
          if (F === 90) { cx += 9 * Math.cos(rad); cy += 9 * Math.sin(rad); }
          if (T > 4.53) {
            var u = T - 4.53, tx = 500 - cx, ty = PILL_EXIT_Y[pi] - cy;
            var dn = Math.max(1, Math.sqrt(tx * tx + ty * ty)), D = 18000 * u * u;
            cx += tx / dn * D; cy += ty / dn * D;
          }
          var eIn = clamp((T - PBIRTH[pi]) / 0.4, 0, 1);
          var s = (0.79 + 0.21 * (1 - Math.pow(1 - eIn, 2))) * (1 - 0.08 * clamp((T - 2.0) / 2.5, 0, 1));
          pn.translateX(cx - pillW[pi] / 2); pn.translateY(cy - 24); pn.scale(s);

          // .hstrip diagonal hatch under each pill (index.html:76-81):
          // repeating-linear-gradient(115deg,#c9c9c6 0 2px,transparent 2px 8px)
          // at opacity 0.9; the stripes sweep the full band horizontally.
          if (sCvAll) {
            // CSS box model: .pill W×48 with 1.5 border; hstrip containing
            // block = pill PADDING box ((W-3)×45).
            var Wp = pillW[pi];
            var sx0 = cx - (Wp / 2 - 1.5) * s, sy0 = cy + 29.5 * s;
            var sw2 = 1.04 * (Wp - 3) * s, sh2 = 13 * s;
            var syMid = sy0 + sh2 / 2;
            // GSAP hatch scroll: x 0→-44, dur 3.6, ease none, @0.933 — the
            // visual shift is -44*q*s.
            var roll = -44 * clamp((T - 0.933) / 3.6, 0, 1) * s;
            sCvAll.save(); sCvAll.clipRect([sx0, sy0, sx0 + sw2, sy0 + sh2]);
            var sp = new CK.Paint(); sp.setStyle(CK.PaintStyle.Stroke); sp.setStrokeWidth(2 * s);
            sp.setColor(CK.Color(201, 201, 198, 0.9));
            var sax = Math.sin(115 * Math.PI / 180), say = -Math.cos(115 * Math.PI / 180);
            var sdx = say, sdy = -sax;               // stripe dir (0.4226,-0.9063) "/"
            var shl = sh2 / 2 / sax + 3;             // half-length: full band + AA
            var per = 8 * s / sax;                   // horizontal period
            // Gradient phase anchor = inner div box CENTRE + roll, plus the
            // measured per-pill residual.
            var c0x = sx0 + 1.04 * (Wp - 3) * s;
            var xc0 = c0x + roll + aph[pi];
            var xL = sx0 - 6, xR = sx0 + sw2 + 6;
            var xc = xc0 - Math.ceil((xc0 - xL) / per) * per;
            for (; xc <= xR; xc += per) {
              sCvAll.drawLine(xc - sdx * shl, syMid - sdy * shl, xc + sdx * shl, syMid + sdy * shl, sp);
            }
            sCvAll.restore();
          }
        }
      }

      // Ruler strips stripAL/AR (makeStrip): 19 ticks pitch 7.7222, tall/
      // short alternating, 18 mid dots; container #afurn fades via the
      // timeline above.
      if (T >= 0.75 && T < 4.62) {
        var sAL = ctx.getCanvasById('stripAL'), sAR = ctx.getCanvasById('stripAR');
        var scvs = [sAL, sAR];
        for (var si = 0; si < 2; si++) {
          var scv = scvs[si]; if (!scv) continue;
          scv.clear();
          var SW2 = 139, SH2 = 15, SN2 = 19, pit2 = SW2 / (SN2 - 1);
          var ip2 = new CK.Paint(); ip2.setColor(CK.Color(207, 207, 210, 0.9));
          var bp2 = new CK.Paint(); bp2.setColor(CK.Color(185, 185, 188, 0.7));
          for (var ti = 0; ti < SN2; ti++) {
            var tall = ti % 2 === 0;
            scv.drawRect([ti * pit2, tall ? 0 : SH2 * 0.28,
                          ti * pit2 + 2, tall ? SH2 : SH2 * 0.28 + SH2 * 0.45], ip2);
          }
          for (var ti = 0; ti < SN2 - 1; ti++) {
            scv.drawRect([ti * pit2 + pit2 / 2 - 1, SH2 * 0.55,
                          ti * pit2 + pit2 / 2 + 1, SH2 * 0.55 + 2], bp2);
          }
        }
      }
    }
  });
  return (
    <div id="sceneA" class="absolute inset-0 z-[2] opacity-0">
      {/* zoom layer (scale 1.07 → 0.9417); field + title sit inside */}
      <div id="azoom" class="absolute inset-0 [transform-origin:50%_50%]">
        {/* white lattice field, masked by a radial gradient */}
        <div id="afield" class="absolute inset-0">
          <StageCanvas id="afield-canvas" />
        </div>
        {/* field edge fade overlay (CSS mask-image not supported, use overlay) */}
        <div id="afield-mask" class="absolute inset-0 bg-[radial-gradient(ellipse_64%_70%_at_50%_50%,transparent_52%,#010101_100%)]"></div>
        <div id="atitlegrp" class="absolute left-[755px] top-[444px] w-[467px] h-[185px] opacity-0">
          <div id="a-sel" class="absolute inset-0 border-[1px] border-white/25"></div>
          <Cdot id="a-tl" s="13px" c="#d8d8da" class="-left-[7px] -top-[7px]" />
          <Cdot id="a-tr" s="13px" c="#d8d8da" class="-right-[7px] -top-[7px]" />
          <Cdot id="a-bl" s="13px" c="#d8d8da" class="-left-[7px] -bottom-[7px]" />
          <Cdot id="a-br" s="13px" c="#d8d8da" class="-right-[7px] -bottom-[7px]" />
          <div id="aline1" class="absolute flex whitespace-nowrap leading-none left-[60.864px] top-[26.567px] text-[#d8d8da] text-[63.6426px] tracking-[-0.8px] opacity-0"><text id="aw1">Every</text><text id="aw2">{" token"}</text></div>
          <text id="aline2" class="absolute whitespace-nowrap leading-none left-[85.7902px] top-[88.4674px] text-[#d8d8da] text-[63.6426px] tracking-[-0.8px] opacity-0">of context</text>
        </div>
        <div id="apwrap" class="absolute inset-0 opacity-0">
          <div id="afieldp" class="absolute inset-0">
            <StageCanvas id="afieldp-canvas" />
          </div>
          {/* afieldp-mask overlay REMOVED (r12): purple paint alphas are
              pre-multiplied by w(r) inside paintField instead. */}
          <div id="atitlegrpp" class="absolute left-[755px] top-[444px] w-[467px] h-[185px] opacity-0">
            <div id="a-pp-sel" class="absolute inset-0 border-[1px] border-[rgba(171,132,207,0.75)]"></div>
            <Cdot id="a-pp-tl" s="13px" c="#ab84cf" class="-left-[7px] -top-[7px]" />
            <Cdot id="a-pp-tr" s="13px" c="#ab84cf" class="-right-[7px] -top-[7px]" />
            <Cdot id="a-pp-bl" s="13px" c="#ab84cf" class="-left-[7px] -bottom-[7px]" />
            <Cdot id="a-pp-br" s="13px" c="#ab84cf" class="-right-[7px] -bottom-[7px]" />
            <text id="aline1p" class="absolute whitespace-nowrap leading-none left-[60.864px] top-[26.567px] text-[#ab84cf] text-[63.6426px] tracking-[-0.8px] opacity-0">Every token</text>
            <text id="aline2p" class="absolute whitespace-nowrap leading-none left-[85.7902px] top-[88.4674px] text-[#ab84cf] text-[63.6426px] tracking-[-0.8px] opacity-0">of context</text>
          </div>
        </div>
      </div>
      {/* pills sit OUTSIDE the zoom layer (screen-space orbit) */}
      <div id="apills" class="absolute inset-0">
        <StageCanvas id="apill-canvas" />
        <AgentPill id="ap1" t="Agentic" />
        <AgentPill id="ap2" t="Delta Attention" />
        <AgentPill id="ap3" t="1M Context" />
        <AgentPill id="ap4" t="Vision" />
        <AgentPill id="ap5" t="Coding" />
      </div>
      {/* A furniture (ruler strips): canvas is 4px wider than the 139px box —
          tick #19 sits at left=139 and was being clipped by the surface. */}
      <div id="afurn" class="absolute inset-0">
        <canvas id="stripAL" class="absolute left-[157px] top-[530px] w-[143px] h-[15px]"></canvas>
        <canvas id="stripAR" class="absolute left-[1626px] top-[530px] w-[143px] h-[15px]"></canvas>
      </div>
    </div>
  );
}

/* SCENE B: one model (track 3, window 4.563→5.853s) */
function SceneB() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.time;
    var B_ON = (T >= 4.563 && T < 5.853);
    vis(ctx, 'sceneB', B_ON);
    if (B_ON) {
      // index.html:1644-1649 — cluster slides in (power2.out/cubic) and fades
      // (power1.in/quad); pill waterfall is a 1-frame stagger.
      var bTl = ctx.timeline()
        .fromTo('bcluster', { x: 190 }, { x: 0, duration: 0.577, ease: 'power2.out' }, 4.653)
        .set('btitle', { opacity: 1 }, 4.6333)
        .to('bcluster', { opacity: 0, duration: 0.3, ease: 'power1.in' }, 5.463);
      ['bs1', 'bs2', 'bs3', 'bs4', 'bs5'].forEach(function (id, i) {
        bTl.set(id, { opacity: 1 }, 4.6333 + i * 0.0333);
      });
      // .pill .hstrip diagonal hatch under each stack pill; the roll is
      // parked at -44 (finished before B starts). Stripe phase anchors carry
      // per-pill residuals measured on f160: [bs1 +2.215, bs2 -1.805,
      // bs3 -1.884, bs4 +1.891, bs5 -0.247]px.
      var bph = [2.215, -1.805, -1.884, 1.891, -0.247];
      // Coordinates are bcluster-local (s = 1 — the stack has no scale).
      // Pill widths reuse the measured pillW map (the STACK labels are the
      // same five strings as the A pills).
      var bsCv = ctx.getCanvasById('bstack-canvas');
      if (bsCv) {
        bsCv.clear();
        var PILL_W = pillWidths(makeMeasure(ctx));
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
    <div id="sceneB" class="absolute inset-0 z-[3] opacity-0">
      <div id="bcluster" class="absolute left-[390px] top-[240px] w-[700px] h-[700px]">
        <text id="btitle" class="absolute whitespace-nowrap leading-none left-[-8.62px] top-[9.6px] text-[#d8d8da] text-[90px] tracking-[-1px] opacity-0 [text-shadow:1.2px_0_rgba(255,70,70,0.10),-1.2px_0_rgba(90,210,255,0.10)]">One model</text>
        <div id="bstack" class="absolute left-[77px] top-[164px]">
          <div id="bs1" class="absolute top-[0px] opacity-0"><div id="d-500" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><text id="t-640" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Delta Attention</text></div></div>
          <div id="bs2" class="absolute top-[113px] opacity-0"><div id="d-501" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><text id="t-641" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Coding</text></div></div>
          <div id="bs3" class="absolute top-[227px] opacity-0"><div id="d-502" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><text id="t-642" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">1M Context</text></div></div>
          <div id="bs4" class="absolute top-[340px] opacity-0"><div id="d-503" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><text id="t-643" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Vision</text></div></div>
          <div id="bs5" class="absolute top-[456px] opacity-0"><div id="d-504" class="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]"><text id="t-644" class="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">Agentic</text></div></div>
        </div>
        {/* .pill .hstrip hatch carrier: lives inside #bcluster so
            translateX(bx) applies. */}
        <canvas id="bstack-canvas" class="absolute left-0 top-0 w-[700px] h-[700px]"></canvas>
      </div>
    </div>
  );
}

/* SCENE C: decode (track 4, window 5.433→7.35s) */
function SceneC() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.time;
    var C_ON = (T >= 5.433 && T < 7.35);
    vis(ctx, 'sceneC', C_ON);
    if (C_ON) {
      // chrome fade-ins + panel pan + pale-bar dissolve are plain tweens;
      // the bar bodies stay a per-frame fitted model (BAR_CHAINS) because
      // the recording deviates from stock GSAP there.
      var cTl = ctx.timeline()
        .fromTo('ctab', { opacity: 0 }, { opacity: 1, duration: 0.1, ease: 'power1.out' }, 5.5)
        .fromTo('cicons', { opacity: 0 }, { opacity: 1, duration: 0.1, ease: 'power1.out' }, 5.533)
        .fromTo('cliverow', { opacity: 0 }, { opacity: 1, duration: 0.14, ease: 'power1.out' }, 5.6)
        .fromTo('cpanel', { x: 18, y: 27 }, { x: -17.5, y: -27, duration: 1.417, ease: 'none' }, 5.933);
      for (var pi2 = 0; pi2 < 18; pi2++) {
        cTl.set('pale-' + pi2, { opacity: 1 }, 5.433)
           .to('pale-' + pi2, { opacity: 0, duration: 0.42, ease: 'power1.inOut' }, 5.83 + pi2 * 0.022);
      }
      // cicons: 3 mini bars + dot + trend line
      var ciCv = ctx.getCanvasById('cicons');
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
        var barN = ctx.getNode('bar-' + bi); if (barN) barN.width(sx * 660);
      }
      // corner qmarks — visible for the whole scene C clip
      var cqCv = ctx.getCanvasById('cqmarks-canvas');
      if (cqCv) {
        cqCv.clear();
        drawQmarks(ctx, cqCv, [[310, 275, -1, -1], [1490, 275, 1, -1], [310, 805, -1, 1], [1490, 805, 1, 1]]);
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

/* inversion flash (track 8, window 7.333→7.391s).
   Blend is wired on the node background rect; must stay at opacity 1.0 in
   the window — any opacity <1 wraps the node in an isolated SaveLayer and
   the difference degrades to a plain flash. z=5 (below grade-vig z=6) is
   MEASURED: the vignette multiplies the flash output. */
function Flash() {
  useGSAP((ctx) => {
    var T = ctx.time;
    vis(ctx, 'flash', T >= 7.3333 && T < 7.391);
  });
  return <div id="flash" class="absolute inset-0 z-[5] bg-[#fbfbfd] opacity-0 [mix-blend-mode:difference]"></div>;
}

/* SCENE D+E: pin waves + boxes (track 2, window 7.35→11.293s) */
function SceneDE() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.time;
    var DE_ON = (T >= 7.35 && T < 11.293);
    vis(ctx, 'sceneDE', DE_ON);
    if (DE_ON) {
      // Node tweens (index.html:1668-1712). The canvas pin waves, helix, box
      // hatch and the pin-wave envelope stay per-frame; the otxt per-char
      // entry is declarative below (splitText + timeline).
      var dTl = ctx.timeline()
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
      // otxt per-char entry — declarative via splitText + timeline; otxt is
      // flex-centred in the card and CAL_SCALEX.otxt squeezes the node. Ref:
      // "Every" shows @7.5333, " modality" @7.5663; every glyph starts at
      // x=(i-6.5)*21 and settles to 0 by 7.9963 (dur 0.43, power2.out).
      vis(ctx, 'otxt', true); /* node visible for the whole D/E window; visibility is per-glyph */
      (function () {
        var parts = ctx.splitText('otxt', { type: 'chars' });
        var gTl = ctx.timeline();
        for (var oi = 0; oi < parts.length; oi++) {
          if (parts[oi].text === ' ') continue; /* inter-word space: a real unit, no ink */
          var ocOn = oi < 5 ? 7.5333 : 7.5663; /* ow1 "Every" / ow2 " modality" */
          gTl.set(parts[oi], { opacity: 0 }, 7.35)
             .set(parts[oi], { opacity: 1 }, ocOn)
             .fromTo(parts[oi], { x: (oi - 6.5) * 21 },
                                  { x: 0, duration: 0.43, ease: 'power2.out' }, 7.5663);
        }
      })();
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
        drawQmarks(ctx, dqCv, [[205, 215, -1, -1], [1720, 215, 1, -1], [205, 870, -1, 1], [1720, 870, 1, 1]]);
        drawPlusses(ctx, dqCv, [[75, 100], [1845, 100], [75, 970], [1845, 970]], 25, 0.5);
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

/* SCENE F+G: brace card (track 3, window 11.233→13.7s) */
function SceneFG() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.time;
    var FG_ON = (T >= 11.233 && T < 13.7);
    vis(ctx, 'sceneFG', FG_ON);
    if (FG_ON) {
      // Braces pop + widen (11.333 → 11.673, power2.out) then retype to rest.
      // Declarative tweens at absolute scene times; only the canvas furniture
      // below stays a per-frame gate. (index.html:1710-1730)
      ctx.timeline()
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
      // word positions: baked from the calibrated F/G run geometry
      // (F run center 958.5, G run center 962.5, cap 108, ls -2.5)
      ctx.getNode('fw1').left(611.4953); ctx.getNode('fw2').left(894.5192);
      ctx.getNode('gw1').left(486.758); ctx.getNode('gw2').left(738.6634); ctx.getNode('gw3').left(1071.4648);
      // F furniture: stripFL/FR inside #ffurn; G furniture: stripGL/GR +
      // gqmarks + plusses inside #gfurn.
      var ffCv = ctx.getCanvasById('ffurn-canvas');
      if (ffCv) {
        ffCv.clear();
        if (T >= 11.633 && T < 12.717) {
          drawStrip(ctx, ffCv, 239, 532, 85, 12, 13); drawStrip(ctx, ffCv, 1595, 532, 85, 12, 13);
        }
      }
      var gfCv = ctx.getCanvasById('gfurn-canvas');
      if (gfCv) {
        gfCv.clear();
        if (T >= 12.7297) {
          drawStrip(ctx, gfCv, 240, 534, 120, 12, 17); drawStrip(ctx, gfCv, 1559, 534, 120, 12, 17);
          drawQmarks(ctx, gfCv, [[343, 265, -1, -1], [1575, 265, 1, -1], [343, 813, -1, 1], [1575, 813, 1, 1]]);
          drawPlusses(ctx, gfCv, [[252, 178], [1668, 178], [252, 898], [1668, 898]], 22, 0.4);
        }
      }
    }
  });
  return (
    <div id="sceneFG" class="absolute inset-0 z-[3] opacity-0">
      <div id="fgwrap" class="absolute inset-0">
        {/* center stem that retracts as braces settle */}
        <div id="fstem" class="absolute w-[3px] bg-[#bdbdb9] left-[958px] top-[0] h-[330px] [transform-origin:top_center] opacity-0"></div>
        {/* F/G braces (light weight) */}
        <text id="fgbl" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[390.564px] top-[468.467px] text-[131.0573px] opacity-0">{'{'}</text>
        <text id="fgbr" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[1460.35px] top-[468.467px] text-[131.0573px] opacity-0">{'}'}</text>
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
    </div>
  );
}

/* SCENE H+I: glitch dissolve → K3 wordmark (track 2, 13.7→16.467s).
   hbox keeps its source CSS position (left 400 top 265); the gsap.set
   x:-23 y:66 rides the node transform. */
function SceneHI() {
  useGSAP((ctx) => {
    var CK = ctx.CanvasKit, T = ctx.time;
    if (typeof globalThis.__k3disBuilt === 'undefined') rebuildDissolveField(ctx);
    var HI_ON = (T >= 13.7);
    vis(ctx, 'sceneHI', HI_ON);
    if (HI_ON) {
      // gsap.set("#hbox", { x:-23, y:66 }): the box CSS stays at its source
      // position (400,265); the offset rides the transform.
      // Pop-in (power2.out), then shrink+fade (power2.in).
      ctx.timeline()
        .set('hbox', { opacity: 0, x: -23, y: 66 }, 13.7)
        .set('hbox', { opacity: 1 }, 13.74)
        .fromTo('hbox', { scale: 0.94 }, { scale: 1, duration: 0.12, ease: 'power2.out' }, 13.74)
        .to('hbox', { scale: 0.4, opacity: 0, duration: 0.14, ease: 'power2.in' }, 14.6);

      // hatch lines inside the box (hbox-canvas coords = box-local 1150x385);
      // exact 26° CSS geometry, staggered 13.77 + k*0.04.
      var hCv = ctx.getCanvasById('hbox-canvas');
      if (hCv) {
        hCv.clear();
        var hatchP = new CK.Paint(); hatchP.setStyle(CK.PaintStyle.Stroke); hatchP.setStrokeWidth(2);
        hatchP.setColor(CK.Color(74, 74, 74, 0.85));
        var HS = Math.sin(26 * Math.PI / 180), HC = Math.cos(26 * Math.PI / 180);
        for (var hk = 0; hk < 6; hk++) {
          var hOn = 13.77 + hk * 0.04;
          if (T < hOn) continue;
          var hbx = -40 + hk * 230 + 1;               // bottom anchor (left+1, 435)
          hCv.drawLine(hbx, 435, hbx + 540 * HS, 435 - 540 * HC, hatchP);
        }
      }

      // scramble dissolve: lambda drawRect over the field render target;
      // the draw lands at hcanvas-local (232, 230) = screen (232, 430)
      var hDCv = ctx.getCanvasById('hcanvas');
      if (hDCv) {
        hDCv.clear();
        drawDissolve(ctx, hDCv, T);
      }

      // wordmark + glow formation and bloom decay (index.html:1758-1772).
      // NOTE: the reference decays #iglow1 *to* 0.28 (power2.out) — it falls
      // 0.95 -> 0.28 and stays legible. The dashes' fade-out uses GSAP's
      // default power1.out.
      ctx.timeline()
        .fromTo('iwordmark', { opacity: 0 }, { opacity: 1, duration: 0.1, ease: 'power1.inOut' }, 14.7333)
        .fromTo('iglow1', { opacity: 0 }, { opacity: 0.95, duration: 0.1, ease: 'power1.out' }, 14.7333)
        .fromTo('iglow2', { opacity: 0 }, { opacity: 1, duration: 0.1, ease: 'power1.out' }, 14.7667)
        .fromTo('iglow3', { opacity: 0 }, { opacity: 0.9, duration: 0.1, ease: 'power1.out' }, 14.7667)
        .fromTo('iglow3', { opacity: 0.9 }, { opacity: 0, duration: 0.8, ease: 'power2.out' }, 14.95)
        .fromTo('iglow2', { opacity: 1 }, { opacity: 0, duration: 0.95, ease: 'power2.out' }, 14.95)
        .fromTo('iglow1', { opacity: 0.95 }, { opacity: 0.28, duration: 0.95, ease: 'power2.out' }, 14.95)
        .set(['idashL', 'idashR'], { opacity: 0 }, 13.7)
        .set(['idashL', 'idashR'], { opacity: 0.85 }, 14.7)
        .to(['idashL', 'idashR'], { opacity: 0, duration: 0.25, ease: 'power1.out' }, 15.1)
        // g2qmarks fade-out (ref #gfurn2) and ifurn fade-in (ref #ifurn)
        .set('g2qmarks-canvas', { opacity: 1 }, 13.7)
        .to('g2qmarks-canvas', { opacity: 0, duration: 0.12, ease: 'power1.out' }, 14.75)
        .set('ifurn-canvas', { opacity: 0 }, 13.7)
        .to('ifurn-canvas', { opacity: 1, duration: 0.16, ease: 'power1.out' }, 15.3);
      // g2qmarks furniture (ref makeQmarks/makePlusses); opacity is declarative.
      var g2Cv = ctx.getCanvasById('g2qmarks-canvas');
      if (g2Cv) {
        g2Cv.clear();
        drawQmarks(ctx, g2Cv, [[343, 265, -1, -1], [1575, 265, 1, -1], [343, 813, -1, 1], [1575, 813, 1, 1]]);
        drawPlusses(ctx, g2Cv, [[252, 178], [1668, 178], [252, 898], [1668, 898]], 22, 0.4);
      }
      // ifurn: stripIL/IR + iqmarks + plusses (opacity is declarative).
      var ifCv = ctx.getCanvasById('ifurn-canvas');
      if (ifCv) {
        ifCv.clear();
        drawStrip(ctx, ifCv, 306, 535, 109, 9, 15); drawStrip(ctx, ifCv, 1504, 535, 109, 9, 15);
        drawQmarks(ctx, ifCv, [[400, 290, -1, -1], [1520, 290, 1, -1], [400, 780, -1, 1], [1520, 780, 1, 1]]);
        drawPlusses(ctx, ifCv, [[300, 205], [1615, 205], [300, 875], [1615, 875]], 25, 0.45);
      }
    }
  });
  return (
    <div id="sceneHI" class="absolute inset-0 z-[2] opacity-0">
      {/* g2qmarks live in #gfurn2, the FIRST child of sceneHI — must paint
          under hbox/iwordmark */}
      <StageCanvas id="g2qmarks-canvas" />
      {/* wireframe dissolve box */}
      <div id="hbox" class="absolute border-[1px] border-[#6a6a6a] left-[400px] top-[265px] w-[1150px] h-[385px] opacity-0">
        <Cdot id="d-558" s="12px" c="#e6e6e6" class="-left-[24px] -top-[24px]" />
        <Cdot id="d-559" s="12px" c="#e6e6e6" class="-right-[24px] -top-[24px]" />
        <Cdot id="d-560" s="12px" c="#e6e6e6" class="-left-[24px] -bottom-[24px]" />
        <Cdot id="d-561" s="12px" c="#e6e6e6" class="-right-[24px] -bottom-[24px]" />
        <Cdot id="d-721" s="7px" c="#cfcfcc" class="-left-[9px] -top-[9px]" />
        <Cdot id="d-722" s="7px" c="#cfcfcc" class="-right-[9px] -top-[9px]" />
        <Cdot id="d-723" s="7px" c="#cfcfcc" class="-left-[9px] -bottom-[9px]" />
        <Cdot id="d-724" s="7px" c="#cfcfcc" class="-right-[9px] -bottom-[9px]" />
        <canvas id="hbox-canvas" class="absolute inset-0 w-[1150px] h-[385px]"></canvas>
      </div>
      {/* dissolve canvas: full-bleed layer at screen (0,200) like #hcanvas */}
      <canvas id="hcanvas" class="absolute left-0 top-[200px] w-[1920px] h-[560px]"></canvas>
      {/* bloom glow layers: each is the lockup itself blurred. CSS
          blur(<length>) takes the STANDARD DEVIATION directly, and with
          scale applied after the filter σ_eff = σ*scale. */}
      <div id="iglow3" class="absolute left-[746.94px] top-[462.45px] w-[426.12px] h-[154.1px] opacity-0 blur-[69.6px] brightness-[2.2]">
        <div id="d-562" class="absolute left-[30.82px] top-0 w-[154.1px] h-[154.1px]">
          <path id="g3-p1" view-box="0 0 512 512" class="absolute inset-0 w-[154.1px] h-[154.1px] fill-[#ffffff]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="g3-p2" view-box="0 0 512 512" class="absolute inset-0 w-[154.1px] h-[154.1px] fill-[#4d9dff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="g3-p3" view-box="0 0 512 512" class="absolute inset-0 w-[154.1px] h-[154.1px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="g3-t" class="absolute left-[214.45px] top-[5.75px] font-[k3-sans] font-medium text-[#ffffff] leading-none text-[143.67px] tracking-[-3.35px]">K3</text>
      </div>
      <div id="iglow2" class="absolute left-[801px] top-[482px] w-[318px] h-[115px] opacity-0 blur-[24.6px] brightness-[2.1]">
        <div id="d-563" class="absolute left-[23px] top-0 w-[115px] h-[115px]">
          <path id="g2-p1" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#ffffff]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="g2-p2" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#4d9dff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="g2-p3" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="g2-t" class="absolute left-[160.0345px] top-[4.2938px] font-[k3-sans] font-medium text-[#ffffff] leading-none text-[107.2165px] tracking-[-2.5px]">K3</text>
      </div>
      <div id="iglow1" class="absolute left-[801px] top-[482px] w-[318px] h-[115px] opacity-0 blur-[4.1px] brightness-[1.5]">
        <div id="d-564" class="absolute left-[23px] top-0 w-[115px] h-[115px]">
          <path id="g1-p1" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#e6e6e9]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="g1-p2" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#2b8bff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="g1-p3" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="g1-t" class="absolute left-[160.0345px] top-[4.2938px] font-[k3-sans] font-medium text-[#e6e6e9] leading-none text-[107.2165px] tracking-[-2.5px]">K3</text>
      </div>
      {/* K3 wordmark lockup (318×115) */}
      <div id="iwordmark" class="absolute flex items-center gap-[18px] left-[801px] top-[482px] w-[318px] h-[115px] opacity-0">
        <div id="d-565" class="absolute left-[23px] top-0 w-[115px] h-[115px]">
          <path id="p-652" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#dadade]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="p-653" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#027aff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="p-654" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="t-651" class="absolute left-[160.0345px] top-[4.2938px] font-[k3-sans] font-medium text-[#dadade] leading-none text-[107.2165px] tracking-[-2.5px]">K3</text>
      </div>
      {/* flanking formation dashes */}
      <div id="idashL" class="absolute h-[3px] bg-[#a9a9ac] left-[718px] top-[538px] w-[30px] opacity-0"></div>
      <div id="idashR" class="absolute h-[3px] bg-[#a9a9ac] left-[1172px] top-[538px] w-[30px] opacity-0"></div>
      {/* ifurn: stripIL/IR + iqmarks + plusses, shown 15.3→15.46 */}
      <StageCanvas id="ifurn-canvas" />
    </div>
  );
}

/* OPEN brace card (track 9, window 0→1.12s) — on top at start */
function OpenCard() {
  useGSAP((ctx) => {
    var T = ctx.time;
    // Type-in two words, un-type via remnants, braces widen, fade.
    // Declarative tweens (absolute times); the `opencard` visibility window
    // stays a per-frame gate.
    var OPEN_ON = (T >= 0 && T < 1.12);
    vis(ctx, 'opencard', OPEN_ON);
    if (OPEN_ON) {
      // un-type: the braces snap inward to the remnant headline widths.
      // Offsets are the reference's computed values relative to the type-in
      // rest ink lefts (511 / 1371):
      //   bGapMid = midW/2 + 68  -> obl +67.5225, obr -66.5225
      //   bGapRem = remW/2 + 52  -> obl +325.5715, obr -324.5715
      ctx.timeline()
        .set(['openmid', 'openrem', 'ot1', 'ot2'], { opacity: 0 }, 0)
        .set('ot1', { opacity: 1 }, 0.0333)
        .set('ot2', { opacity: 1 }, 0.0663)
        .set('opentext', { opacity: 0 }, 0.9333)
        .set('openmid', { opacity: 1 }, 0.9333)
        .set('obl', { x: 67.5225 }, 0.9333)
        .set('obr', { x: -66.5225 }, 0.9333)
        .set('openmid', { opacity: 0 }, 0.9663)
        .set('openrem', { opacity: 0.75 }, 0.9663)
        .set('obl', { x: 325.5715 }, 0.9663)
        .set('obr', { x: -324.5715 }, 0.9663)
        // fade out 1.0 → 1.07 (braces drift a further +24 / -24).
        // The reference omits `ease` here, so GSAP's default power1.out
        // applies; our engine default is linear, so it is spelled out.
        .to(['obl', 'obr', 'openrem'], { opacity: 0, duration: 0.07, ease: 'power1.in' }, 1.0)
        .to('obl', { x: 325.5715 + 24, duration: 0.07, ease: 'power1.out' }, 1.0)
        .to('obr', { x: -324.5715 - 24, duration: 0.07, ease: 'power1.out' }, 1.0)
        .to(['stripOL', 'stripOR'], { opacity: 0, duration: 0.12, ease: 'power1.out' }, 0.95);
    }
  });
  return (
    <div id="opencard" class="absolute inset-0 z-[9] opacity-0">
      <div id="stripOL" class="absolute left-[32px] top-[528px] flex items-end gap-[6.8px] opacity-90">
        <Tick id="d-566" ht="23px" /><Tick id="d-567" ht="10px" mb="mb-[6px]" />
        <Tick id="d-568" ht="23px" /><Tick id="d-569" ht="10px" mb="mb-[6px]" />
        <Tick id="d-570" ht="23px" /><Tick id="d-571" ht="10px" mb="mb-[6px]" />
        <Tick id="d-572" ht="23px" /><Tick id="d-573" ht="10px" mb="mb-[6px]" />
        <Tick id="d-574" ht="23px" /><Tick id="d-575" ht="10px" mb="mb-[6px]" />
        <Tick id="d-576" ht="23px" /><Tick id="d-577" ht="10px" mb="mb-[6px]" />
        <Tick id="d-578" ht="23px" /><Tick id="d-579" ht="10px" mb="mb-[6px]" />
        <Tick id="d-580" ht="23px" /><Tick id="d-581" ht="10px" mb="mb-[6px]" />
        <Tick id="d-582" ht="23px" /><Tick id="d-583" ht="10px" mb="mb-[6px]" />
        <Tick id="d-584" ht="23px" /><Tick id="d-585" ht="10px" mb="mb-[6px]" />
        <Tick id="d-586" ht="23px" /><Tick id="d-587" ht="10px" mb="mb-[6px]" />
        <Tick id="d-588" ht="23px" /><Tick id="d-589" ht="10px" mb="mb-[6px]" />
        <Tick id="d-590" ht="23px" /><Tick id="d-591" ht="10px" mb="mb-[6px]" />
        <Tick id="d-592" ht="23px" /><Tick id="d-593" ht="10px" mb="mb-[6px]" />
        <Tick id="d-594" ht="23px" /><Tick id="d-595" ht="10px" mb="mb-[6px]" />
        <Tick id="d-596" ht="23px" /><Tick id="d-597" ht="10px" mb="mb-[6px]" />
        <Tick id="d-598" ht="23px" /><Tick id="d-599" ht="10px" mb="mb-[6px]" />
        <Tick id="d-600" ht="23px" /><Tick id="d-601" ht="10px" mb="mb-[6px]" />
        <Tick id="d-602" ht="23px" />
      </div>
      <div id="stripOR" class="absolute left-[1627px] top-[528px] flex items-end gap-[6.8px] opacity-90">
        <Tick id="d-603" ht="23px" /><Tick id="d-604" ht="10px" mb="mb-[6px]" />
        <Tick id="d-605" ht="23px" /><Tick id="d-606" ht="10px" mb="mb-[6px]" />
        <Tick id="d-607" ht="23px" /><Tick id="d-608" ht="10px" mb="mb-[6px]" />
        <Tick id="d-609" ht="23px" /><Tick id="d-610" ht="10px" mb="mb-[6px]" />
        <Tick id="d-611" ht="23px" /><Tick id="d-612" ht="10px" mb="mb-[6px]" />
        <Tick id="d-613" ht="23px" /><Tick id="d-614" ht="10px" mb="mb-[6px]" />
        <Tick id="d-615" ht="23px" /><Tick id="d-616" ht="10px" mb="mb-[6px]" />
        <Tick id="d-617" ht="23px" /><Tick id="d-618" ht="10px" mb="mb-[6px]" />
        <Tick id="d-619" ht="23px" /><Tick id="d-620" ht="10px" mb="mb-[6px]" />
        <Tick id="d-621" ht="23px" /><Tick id="d-622" ht="10px" mb="mb-[6px]" />
        <Tick id="d-623" ht="23px" /><Tick id="d-624" ht="10px" mb="mb-[6px]" />
        <Tick id="d-625" ht="23px" /><Tick id="d-626" ht="10px" mb="mb-[6px]" />
        <Tick id="d-627" ht="23px" /><Tick id="d-628" ht="10px" mb="mb-[6px]" />
        <Tick id="d-629" ht="23px" /><Tick id="d-630" ht="10px" mb="mb-[6px]" />
        <Tick id="d-631" ht="23px" /><Tick id="d-632" ht="10px" mb="mb-[6px]" />
        <Tick id="d-633" ht="23px" /><Tick id="d-634" ht="10px" mb="mb-[6px]" />
        <Tick id="d-635" ht="23px" /><Tick id="d-636" ht="10px" mb="mb-[6px]" />
        <Tick id="d-637" ht="23px" /><Tick id="d-638" ht="10px" mb="mb-[6px]" />
        <Tick id="d-639" ht="23px" />
      </div>
      {/* braces + headline centered in the 698px brace box */}
      <text id="obl" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[501.722px] top-[473.661px] text-[128.8546px]">{'{'}</text>
      <text id="obr" class="absolute font-[k3-sans-light] font-light text-[#dcdcde] leading-none left-[1362.496px] top-[473.661px] text-[128.8546px]">{'}'}</text>
      <div id="opentext" class="absolute font-[k3-sans] font-normal text-[#d8d8da] leading-none flex whitespace-nowrap left-[590.3768px] top-[511.5263px] text-[64.4305px] tracking-[-1.6px]"><text id="ot1">Open Frontier</text><text id="ot2">{" Intelligence"}</text></div>
      <text id="openmid" class="absolute font-[k3-sans] font-normal text-[#d8d8da] leading-none whitespace-nowrap left-[666.7227px] top-[511.5263px] text-[64.4305px] tracking-[-1.6px] opacity-0">en Frontier Intelligen</text>
      <text id="openrem" class="absolute font-[k3-sans] font-normal text-[#98989c] leading-none whitespace-nowrap left-[923.6878px] top-[511.5263px] text-[64.4305px] tracking-[-1.6px] opacity-0">ier</text>
    </div>
  );
}

/* k3-promo: single continuous stage (no tl/transition); layer z mirrors the
   source data-track-index: ghostcols(1) < sceneA/DE/HI(2) < sceneB/FG(3) <
   sceneC(4) < flash(5→8) < opencard(9) < grade(10). Components appear in
   tree order = hook order = XML script order (OPEN card last: it only
   touches its own subtree). */
export default function K3Promo() {
  useGSAP((ctx) => {
    // per-frame prologue: the type calibration runs before every scene hook,
    // exactly like the XML script top-level.
    applyTypeCalibration(ctx);
  });
  return (
<div id="root" class="relative w-[1920px] h-[1080px] bg-[#010101] overflow-hidden font-[k3-sans]">
    <GhostCols />
    <SceneA />
    <SceneB />
    <SceneC />
    <Flash />
    <SceneDE />
    <SceneFG />
    <SceneHI />
    <OpenCard />

    {/* film-grade layers (always on, full duration) */}
    <div id="grade-vig" class="absolute inset-0 z-[6] bg-[radial-gradient(ellipse_76%_80%_at_50%_50%,transparent_58%,rgba(0,0,0,0.28)_100%)]"></div>
    <div id="grade-lines" class="absolute inset-0 z-[10] bg-[repeating-linear-gradient(0deg,rgba(0,0,0,0.18)_0_1px,transparent_1px_3px)]"></div>
    {/* index.html #gradenoise: data-URL SVG feTurbulence(fractalNoise,0.9,2,7)
       tiled 240×240 at opacity 0.012 — Skia renders the same noise Chrome does */}
    <div id="grade-noise" class="absolute inset-0 z-[10] opacity-[0.012] bg-[fe-turbulence(fractalNoise,0.9,2,7)] bg-[length:240px_240px]"></div>
</div>
  );
}
