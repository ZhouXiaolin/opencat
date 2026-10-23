// Shared per-frame tools and small components (module body of the original
// single-file k3-promo.jsx). Pure functions/pure data; every scene imports
// what it needs. Line-verbatim split — see git history for the pre-split file.
export function StageCanvas({ id }) {
  return <canvas id={id} className="absolute inset-0 w-[1920px] h-[1080px]" />;
}

// prop 用 `ht` 而非 `h`：JSX transform 在组件体内生成 h(...)，prop 名 h 会
// 遮蔽元素创建函数。
export function Tick({ id, ht, mb }) {
  return <div id={id} className={'w-[2px] h-[' + ht + 'px] bg-[#cfcfd2]' + (mb ? ' ' + mb : '')} />;
}

// `top` 与渐变由 props 决定，写进 class 的话 tailwind CLI 静态扫描不到；
// 用内联 style，和原版 index.html 的 `b.style.background` 一致。
export function Cbar({ n, top, g }) {
  return (
    <div id={'bar-' + n} className="absolute left-0 w-[0px] h-[38px]"
         style={{ top: top, backgroundImage: 'linear-gradient(90deg,' + g + ')' }}>
      <div id={'pale-' + n} className="absolute left-0 top-0 w-full h-[38px] opacity-0"
           style={{ backgroundImage: 'linear-gradient(90deg,rgba(233,229,224,0.95),rgba(206,199,193,0.6))' }}></div>
    </div>
  );
}

export function AgentPill({ id, t }) {
  return (
    <div id={id} className="absolute left-[0] top-[0] opacity-0">
      <div id={id + '-box'} className="px-[14px] pt-[10px] pb-[9px] border-[1.5px] border-[#dcdcde] bg-[rgba(2,2,2,0.88)]">
        <text id={id + '-txt'} className="font-[k3-sans-medium] text-[#dcdcde] whitespace-nowrap text-[26px] font-[500] tracking-[0.2px] leading-none">{t}</text>
      </div>
    </div>
  );
}

export function Cdot({ id, s, c, className: extra }) {
  return <div id={id} className={'absolute w-[' + s + 'px] h-[' + s + 'px] bg-[' + c + ']' + (extra ? ' ' + extra : '')} />;
}

export function Bigword({ id, left, t }) {
  return <text id={id} className={'absolute font-[k3-sans] font-normal text-[#d8d8da] leading-none whitespace-nowrap text-[148.4536px] tracking-[-2.5px] left-[' + left + 'px] top-[458.3299px] opacity-0'}>{t}</text>;
}

/* ════════ 共享每帧工具（XML script 顶层；模块体每帧重估，纯函数/纯数据） ════════ */
export function mulberry32(a) {
  return function () {
    a |= 0; a = (a + 0x6D2B79F5) | 0;
    var t = Math.imul(a ^ (a >>> 15), 1 | a);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}
export var pick = function (arr, r) { return arr[Math.floor((r || rnd)() * arr.length)]; };
export var range = function (a, b, r) { return a + (r || rnd)() * (b - a); };
export var clamp = function (v, a, b) { return Math.max(a, Math.min(b, v)); };
export var rnd = mulberry32(1337);
export // Window gate helper — the standard DOM face of a composition node.
function vis(ctx, id, on) { document.getElementById(id).style.cssText = 'opacity:' + (on ? 1 : 0); }
export function frac(v) { return v - Math.floor(v); }

export var FAM = '"Inter", "Helvetica Neue", Arial, sans-serif';  // index.html:438

/* inkMetrics + natW/swapW over the real-font measure surface (1x1 —
   measurement never touches pixels). One measure stack shared by the type
   calibration, pill widths and the dissolve field (OffscreenCanvas is
   get-or-create by size, so re-entering per consumer is free). */
export function makeMeasure(ctx) {
  var MSR = new OffscreenCanvas(1, 1).getContext('2d');
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
export function pillWidths(m) {
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
export function applyTypeCalibration(ctx) {
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
    var calN = document.getElementById(calId);
    if (calN) calN.style.cssText = 'scaleX:' + CAL_SCALEX[calId];
  }
}

/* ═══ corner marks + ruler strips ═══════════════════════════════════
   makeQmarks / makePlusses and makeStrip, plus the reference's global
   `gsap.set(".sq"|".qmark .lv"|".lh"|".plusm", { x:-23, y:66 })`
   applied as the QX/QY offset — all four furniture classes displace by
   the same (-23,+66), so screen rects = source coords + QOFF. */
export var QX = -23, QY = 66; // gsap.set x/y, index.html:1527-1532
export function drawQmarks(CK, cv, pts) {
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
export function drawPlusses(CK, cv, pts, size, op) {
  var p = new CK.Paint(); p.setStyle(CK.PaintStyle.Fill); p.setColor(CK.Color(207, 207, 210, op));
  for (var i = 0; i < pts.length; i++) {
    var px = pts[i][0] + QX, py = pts[i][1] + QY, h = size / 2; // .plusm center
    cv.drawRect([px - 0.5, py - h, px + 0.5, py + h], p);
    cv.drawRect([px - h, py - 0.5, px + h, py + 0.5], p);
  }
}
export function drawStrip(CK, cv, x, y, w, h, n) {
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
export function getK3Stream() {
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
export var GS = 94;
export function getK3Field() {
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
   Dissolve is a pure-JS per-pixel port (index.html:1448-1523 verbatim),
   the liangyi/两仪 lambda path retired. V8 runs the loops fast enough
   offline (~324K px/frame):
     1. fillText the card lockup ONCE into an offscreen surface
        (OffscreenCanvas 2D context → real Inter faces via swash);
     2. read the lockup region back once (surface.getImageData → engine
        native zero-marshal path), then build mask + 3-4 chamfer distance
        field as plain typed arrays in JS (Uint8Array / Int16Array, dist
        in px*3, sentinel 3000);
     3. each frame the scramble noise / dust / glow loop writes an
        ImageData and uploads it via canvas putImageData (frame-unique
        key → frame-scoped generated image).
   The build-once flag and pixel arrays live on non-configurable globals
   — realm frame-boundary cleanup deletes only configurable keys. */

/* ---- dissolve field construction (index.html:1249-1349 → lambdas) ---- */
export var DIS_R = { X0: 232, Y0: 430, W: 1460, H: 222 };         // index.html:1253
export var CARD_TOP = 478, CARD_CAP = 108;                        // index.html:1177
export var FGB_TOP = 480, FGB_H = 119;                            // index.html:1175

export function rebuildDissolveField(ctx) {
  var m = makeMeasure(ctx);
  var ox = new OffscreenCanvas(1920, 1080).getContext('2d');
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
  /* mask + 3-4 chamfer distance field — pure JS over a one-shot readback
     of the lockup surface (index.html:1279-1313 verbatim). dist is px*3
     with sentinel 3000, held in an Int16Array; mask is the raw source
     alpha. Arrays go on a non-configurable global so they survive the
     realm's per-frame cleanup (only configurable keys are deleted). */
  var lock = ox.getImageData(DIS_R.X0, DIS_R.Y0, DIS_R.W, DIS_R.H);
  var W = DIS_R.W, H = DIS_R.H, n = W * H;
  var mask = new Uint8Array(n);
  var dist = new Int16Array(n);
  var ld = lock.data;
  for (var si = 0; si < n; si++) {
    mask[si] = ld[si * 4 + 3];
    dist[si] = mask[si] > 120 ? 0 : 3000;
  }
  var fx, fy, fi, fd;
  for (fy = 0; fy < H; fy++) {
    for (fx = 0; fx < W; fx++) {
      fi = fy * W + fx;
      fd = dist[fi];
      if (fx > 0 && dist[fi - 1] + 3 < fd) fd = dist[fi - 1] + 3;
      if (fy > 0) {
        if (dist[fi - W] + 3 < fd) fd = dist[fi - W] + 3;
        if (fx > 0 && dist[fi - W - 1] + 4 < fd) fd = dist[fi - W - 1] + 4;
        if (fx < W - 1 && dist[fi - W + 1] + 4 < fd) fd = dist[fi - W + 1] + 4;
      }
      dist[fi] = fd;
    }
  }
  for (fy = H - 1; fy >= 0; fy--) {
    for (fx = W - 1; fx >= 0; fx--) {
      fi = fy * W + fx;
      fd = dist[fi];
      if (fx < W - 1 && dist[fi + 1] + 3 < fd) fd = dist[fi + 1] + 3;
      if (fy < H - 1) {
        if (dist[fi + W] + 3 < fd) fd = dist[fi + W] + 3;
        if (fx < W - 1 && dist[fi + W + 1] + 4 < fd) fd = dist[fi + W + 1] + 4;
        if (fx > 0 && dist[fi + W - 1] + 4 < fd) fd = dist[fi + W - 1] + 4;
      }
      dist[fi] = fd;
    }
  }
  Object.defineProperty(globalThis, '__k3dis', {
    value: { mask: mask, dist: dist },
    writable: true, enumerable: true, configurable: false
  });
  Object.defineProperty(globalThis, '__k3disBuilt', {
    value: true, writable: true, enumerable: true, configurable: false
  });
}

/* deterministic pixel hash (index.html:1316-1321 verbatim) */
export function h01(a, b, c) {
  var h = Math.imul(a + 374761, 0x9E3779B1) ^ Math.imul(b + 668265, 0x85EBCA6B) ^ Math.imul(c + 951274, 0xC2B2AE35);
  h = Math.imul(h ^ (h >>> 16), 0x27D4EB2F);
  h = (h ^ (h >>> 15)) >>> 0;
  return h / 4294967296;
}

/* ══ 溶解（纯 JS 逐像素，liangyi/两仪退役）════════════════════════
   field（mask/dist）由 rebuildDissolveField 以 typed arrays 建一次，
   存非配置全局 __k3dis。逐帧循环是 index.html:1448-1523 的逐位转写，
   h01/bandP/streakEnv 与原版逐字节同源；结果 ImageData 经 putImageData
   走引擎二进制快速通道（帧级生成图像，key=帧号保证幂等）。 */

/* 距离带点亮概率（index.html:1323-1331）+ 线性插值 */
export var DIS_TAB = [
  [412.0, [0.97, 0.88, 0.74, 0.34, 0.06, 0.010]],
  [414.0, [0.97, 0.88, 0.84, 0.56, 0.19, 0.033]],
  [425.0, [0.94, 0.83, 0.80, 0.54, 0.21, 0.036]],
  [435.0, [0.87, 0.77, 0.74, 0.51, 0.23, 0.036]],
  [441.0, [0.83, 0.735, 0.70, 0.49, 0.24, 0.036]],
  [442.0, [0.76, 0.66, 0.61, 0.41, 0.19, 0.036]],
  [443.0, [0.60, 0.47, 0.37, 0.23, 0.10, 0.030]]
];
export function bandP(f) {
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
export function streakEnv(f) {
  if (f < 416 || f > 439) return 0;
  if (f < 420) return 0.42 + 0.145 * (f - 416);
  if (f <= 436) return 1;
  if (f === 437) return 0.58;
  if (f === 438) return 0.33;
  return 0.125;
}

/* per-frame：index.html:1448-1523 的逐位转写。put 位置 (232, 230)
   = hcanvas 本地坐标（屏面 (232, 430)），与 applyDissolve 旧路一致。 */
export function drawDissolve(ctx, cvx, t) {
  var DIS = globalThis.__k3dis;
  if (t < 13.699 || t > 15.2 || !DIS || !DIS.mask) return false;
  var f = Math.floor(t * 30 + 1e-4);
  if (f < 0 || f > 455) return false;
  var W = DIS_R.W, H = DIS_R.H;
  var mask = DIS.mask, dist = DIS.dist;
  var img = cvx.createImageData(W, H);
  var px8 = img.data;
  var pB = bandP(f);
  var ep = Math.floor(f / 7), fr = (f % 7) / 7;
  var scramble = f >= 412;
  var dust = f >= 444;
  var dustBase = dust ? (f === 444 ? 0.40 : 0.32 * Math.max(0, (455 - f)) / 10) : 0;
  var sEnv = streakEnv(f);
  // streak x-extent retracts ~60px/side while dying
  var sxL = f >= 437 ? 240 + (f - 436) * 18 : 240;
  var sxR = f >= 437 ? 1686 - (f - 436) * 18 : 1686;
  for (var y = 0; y < H; y++) {
    var py = DIS_R.Y0 + y;
    var rb = py % 3;                       // scanline banding, period 3px
    var dyS = py - 540;
    var gY = sEnv > 0 && dyS > -46 && dyS < 46 ? Math.exp(-dyS * dyS / 441.6) : 0;
    var rowOff = y * W;
    for (var x = 0; x < W; x++) {
      var i = rowOff + x;
      var pxx = DIS_R.X0 + x;
      var a = mask[i];
      var d = dist[i] / 3;
      var v = 0;
      if (!scramble) {
        if (a > 10) v = 214 * a / 255;       // crisp text (pre-effect)
      } else {
        var cx = (pxx / 3) | 0;              // 3x1 px noise cells (micro-streaks)
        var nz = h01(cx, py, ep) * (1 - fr) + h01(cx, py, ep + 1) * fr;
        var P;
        if (dust) {
          P = d < 24 ? dustBase * (1 - 0.28 * d / 24) : 0.03 * Math.max(0, 455 - f) / 10;
        } else {
          var bi = d < 1 ? 0 : d < 4 ? 1 : d < 8 ? 2 : d < 16 ? 3 : d < 24 ? 4 : 5;
          P = pB[bi];
          if (bi === 5 && (pxx < 402 || pxx > 1548)) P = 0;   // sprinkle only inside box
          // scanline banding: gentle on the core, strong on the fringe
          P *= bi < 2 ? (rb === 0 ? 1.1 : rb === 1 ? 1.0 : 0.8)
                      : (rb === 0 ? 1.4 : rb === 1 ? 1.05 : 0.58);
          if (P > 0.985) P = 0.985;
        }
        if (nz < P) {
          var nb = h01(cx * 3 + 911, py + 377, ep);
          v = dust ? 150 + 80 * nb
                   : (d <= 1.6 ? 255 : 128 + 96 * nb);   // cores clip white
        } else if (!dust && d <= 10) {
          v = 44 + 12 * nz;                  // pinholes lifted by bloom
        }
      }
      // additive grainy glow band at the text midline (static, y=540)
      if (gY > 0 && pxx > sxL - 8 && pxx < sxR + 8) {
        var xp = 1;
        if (pxx < sxL + 60) xp = Math.max(0, (pxx - sxL + 8) / 68);
        else if (pxx > sxR - 36) xp = Math.max(0, (sxR + 8 - pxx) / 44);
        if (xp > 0) {
          var cs = ((pxx / 3) | 0) + 4096;
          var ng = h01(cs, py, ep) * (1 - fr) + h01(cs, py, ep + 1) * fr;
          var gm = ng < 0.91 ? (0.85 + 1.15 * ng) : (3.2 + 2.2 * (ng - 0.91) / 0.09);
          v += 20 * sEnv * gY * xp * gm;
          if (v > 255) v = 255;
        }
      }
      var o = i * 4;
      if (v >= 40) {
        px8[o] = px8[o + 1] = px8[o + 2] = v | 0; px8[o + 3] = 255;
      } else if (v > 0.8) {
        px8[o] = px8[o + 1] = px8[o + 2] = 40; px8[o + 3] = (v * 6.375) | 0;
      }
    }
  }
  // frame-unique key: same frame ⇒ same pixels (idempotent on pipeline
  // reuse), different frames never collide in the generated-image table.
  cvx.putImageData(img, DIS_R.X0, DIS_R.Y0 - 200, 'k3dis-' + f);
  return true;
}

