import { useEffect, useCanvasKit } from 'opencat/runtime';
/* ══ canvas-close (master 46.02-49.77, compositions/canvas-close.html) ══
   ASCII-terminal lightning reveal. The reference is a <canvas> with 2D
   fillText per character cell (10x16px grid), driven by a GSAP proxy
   tween 0→3.93 linear → here the scene clock `t` (read as
   ctx.current.time inside useEffect) drives the same renderAtTime. Text goes through an offscreen 2D context (fontdb
   resolves the registered "JetBrains Mono" face — same 0.6em advance as
   the reference's DejaVu Sans Mono; baseline +13px = its top-baseline
   offset at 14px), then ly.draw samples it on the stage canvas. Master
   slot is 3.75s so the last 0.18s of the 3.93s timeline
   is truncated by the master, exactly like the reference cut. */

var CC_START = 46.02, CC_DUR = 3.75;
var CC_CELL_W = 10, CC_CELL_H = 16;
var CC_COLS = Math.floor(1920 / CC_CELL_W); /* 192 */
var CC_ROWS = Math.floor(1080 / CC_CELL_H); /* 67 */
var CC_GLYPHS = ' .,:;+*#@W';
var CC_GLYPH_MAX = CC_GLYPHS.length - 1;
var CC_BAYER = [
  [0.03125, 0.53125, 0.15625, 0.65625],
  [0.78125, 0.28125, 0.90625, 0.40625],
  [0.21875, 0.71875, 0.09375, 0.59375],
  [0.96875, 0.46875, 0.84375, 0.34375]
];

function CChash(x, y) {
  var n = x * 374761393 + y * 668265263;
  n = (n ^ (n >> 13)) * 1274126177;
  return ((n ^ (n >> 16)) & 0x7fffffff) / 0x7fffffff;
}
function CCvnoise(x, y) {
  var ix = Math.floor(x), iy = Math.floor(y);
  var fx = x - ix, fy = y - iy;
  fx = fx * fx * (3 - 2 * fx);
  fy = fy * fy * (3 - 2 * fy);
  var a = CChash(ix, iy), b = CChash(ix + 1, iy);
  var c = CChash(ix, iy + 1), d = CChash(ix + 1, iy + 1);
  return a + (b - a) * fx + (c - a) * fy + (a - b - c + d) * fx * fy;
}
function CCfbm(x, y) {
  return CCvnoise(x, y) * 0.5 + CCvnoise(x * 2.1 + 5.3, y * 2.1 + 3.7) * 0.3 +
         CCvnoise(x * 4.3 + 11.7, y * 4.3 + 7.1) * 0.2;
}
function CCclamp01(v) { return v < 0 ? 0 : (v > 1 ? 1 : v); }

/* 5x5 bitmap font (canvas-close.html:71-98) */
var CC_BFONT = {
  'A': [0x0E, 0x11, 0x1F, 0x11, 0x11],
  'B': [0x1E, 0x11, 0x1E, 0x11, 0x1E],
  'C': [0x0F, 0x10, 0x10, 0x10, 0x0F],
  'D': [0x1E, 0x11, 0x11, 0x11, 0x1E],
  'E': [0x1F, 0x10, 0x1E, 0x10, 0x1F],
  'F': [0x1F, 0x10, 0x1E, 0x10, 0x10],
  'G': [0x0F, 0x10, 0x13, 0x11, 0x0F],
  'H': [0x11, 0x11, 0x1F, 0x11, 0x11],
  'I': [0x1F, 0x04, 0x04, 0x04, 0x1F],
  'J': [0x07, 0x02, 0x02, 0x12, 0x0C],
  'K': [0x11, 0x12, 0x1C, 0x12, 0x11],
  'L': [0x10, 0x10, 0x10, 0x10, 0x1F],
  'M': [0x11, 0x1B, 0x15, 0x11, 0x11],
  'N': [0x11, 0x19, 0x15, 0x13, 0x11],
  'O': [0x0E, 0x11, 0x11, 0x11, 0x0E],
  'P': [0x1E, 0x11, 0x1E, 0x10, 0x10],
  'R': [0x1E, 0x11, 0x1E, 0x12, 0x11],
  'S': [0x0F, 0x10, 0x0E, 0x01, 0x1E],
  'T': [0x1F, 0x04, 0x04, 0x04, 0x04],
  'U': [0x11, 0x11, 0x11, 0x11, 0x0E],
  'V': [0x11, 0x11, 0x11, 0x0A, 0x04],
  'W': [0x11, 0x11, 0x15, 0x1B, 0x11],
  'X': [0x11, 0x0A, 0x04, 0x0A, 0x11],
  'Y': [0x11, 0x0A, 0x04, 0x04, 0x04],
  'Z': [0x1F, 0x02, 0x04, 0x08, 0x1F],
  ' ': [0, 0, 0, 0, 0]
};

function CCtextWS(text, scale) {
  var stride = 5 * scale + scale;
  return text.length * stride - scale;
}
function CCtextHit(text, sx, sy, c, r, scale) {
  var charW = 5 * scale, charH = 5 * scale;
  var stride = charW + scale;
  if (r < sy || r >= sy + charH || c < sx) return 0;
  var lc = c - sx;
  if (lc >= text.length * stride) return 0;
  var ci = Math.floor(lc / stride);
  var ic = lc - ci * stride;
  if (ci >= text.length || ic >= charW) return 0;
  var bx = Math.floor(ic / scale);
  var by = Math.floor((r - sy) / scale);
  var g = CC_BFONT[text[ci].toUpperCase()];
  if (!g) return 0;
  return (g[by] >> (4 - bx)) & 1;
}

/* text layout (canvas-close.html:125-160) */
var CC_MAIN_SCALE = 2, CC_SUB_SCALE = 1;
var CC_MAIN_TEXT = 'HYPERFRAMES';
var CC_SUB_TEXT = 'GO MAKE SOMETHING';
var CC_mainCharH = 5 * CC_MAIN_SCALE, CC_subCharH = 5 * CC_SUB_SCALE;
var CC_gap = 6;
var CC_groupY = Math.floor((CC_ROWS - (CC_mainCharH + CC_gap + CC_subCharH)) / 2);
var CC_mainX = Math.floor((CC_COLS - CCtextWS(CC_MAIN_TEXT, CC_MAIN_SCALE)) / 2);
var CC_mainY = CC_groupY;
var CC_subX = Math.floor((CC_COLS - CCtextWS(CC_SUB_TEXT, CC_SUB_SCALE)) / 2);
var CC_subY = CC_groupY + CC_mainCharH + CC_gap;
var CC_MASK = new Uint8Array(CC_ROWS * CC_COLS);
(function () {
  var mainW = CCtextWS(CC_MAIN_TEXT, CC_MAIN_SCALE);
  for (var r = CC_mainY; r < CC_mainY + CC_mainCharH && r < CC_ROWS; r++) {
    for (var c = CC_mainX; c < CC_mainX + mainW && c < CC_COLS; c++) {
      if (CCtextHit(CC_MAIN_TEXT, CC_mainX, CC_mainY, c, r, CC_MAIN_SCALE)) CC_MASK[r * CC_COLS + c] = 1;
    }
  }
  var subW = CCtextWS(CC_SUB_TEXT, CC_SUB_SCALE);
  for (var r2 = CC_subY; r2 < CC_subY + CC_subCharH && r2 < CC_ROWS; r2++) {
    for (var c2 = CC_subX; c2 < CC_subX + subW && c2 < CC_COLS; c2++) {
      if (CCtextHit(CC_SUB_TEXT, CC_subX, CC_subY, c2, r2, CC_SUB_SCALE)) CC_MASK[r2 * CC_COLS + c2] = 2;
    }
  }
})();

/* lightning bolts (canvas-close.html:167-278) */
function CCgenerateBolt(startX, targetX, seed, branchCount, targetY) {
  var tgtY = (targetY !== undefined) ? targetY : CC_mainY;
  var segments = [];
  var x = startX, y = 0;
  var step = 0;
  while (y < tgtY) {
    var stepY = 3 + Math.floor(CChash(seed + step, 100) * 4);
    var progress = Math.min(1.0, y / tgtY);
    var attract = (targetX - x) * progress * 0.35;
    var jitter = (CChash(seed + step, 200) - 0.5) * 14 * (1.0 - progress * 0.8);
    var stepX = attract + jitter;
    var newX = Math.max(3, Math.min(CC_COLS - 3, Math.floor(x + stepX)));
    var newY = Math.min(tgtY, y + stepY);
    segments.push({ x1: x, y1: y, x2: newX, y2: newY });
    x = newX; y = newY; step++;
  }
  if (segments.length > 0) {
    var last = segments[segments.length - 1];
    if (last.y2 < tgtY) segments.push({ x1: last.x2, y1: last.y2, x2: targetX, y2: tgtY });
  }
  var strikeCol = segments.length > 0 ? segments[segments.length - 1].x2 : targetX;
  var branches = [];
  var bc = branchCount || Math.floor(CChash(seed, 300) * 3) + 2;
  for (var bi = 0; bi < bc; bi++) {
    var segIdx = Math.floor(CChash(seed + bi * 13, 400) * segments.length);
    var seg = segments[Math.min(segIdx, segments.length - 1)];
    var bx = seg.x2, by = seg.y2;
    var bsegs = [];
    var bseed = seed + 1000 + bi * 77;
    var bstep = 0;
    var blen = 3 + Math.floor(CChash(bseed, 500) * 4);
    var dir = CChash(bseed, 600) > 0.5 ? 1 : -1;
    for (var bs = 0; bs < blen && by < tgtY; bs++) {
      var bStepY = 2 + Math.floor(CChash(bseed + bstep, 700) * 3);
      var bStepX = dir * (2 + CChash(bseed + bstep, 800) * 6);
      var nbx = Math.max(2, Math.min(CC_COLS - 2, Math.floor(bx + bStepX)));
      var nby = Math.min(tgtY, by + bStepY);
      bsegs.push({ x1: bx, y1: by, x2: nbx, y2: nby });
      bx = nbx; by = nby; bstep++;
    }
    branches.push({ segments: bsegs, brightness: 0.3 + CChash(bseed, 900) * 0.35 });
  }
  return { segments: segments, branches: branches, strikeCol: strikeCol, strikeRow: tgtY };
}
function CCbuildCrawlMap(startCol, startRow, bfsGap) {
  var GAP = bfsGap || 3;
  var dist = new Float32Array(CC_ROWS * CC_COLS);
  for (var i = 0; i < dist.length; i++) dist[i] = -1;
  var bestD = 9999, bestC = startCol, bestR = startRow;
  for (var dr = -8; dr <= 8; dr++) {
    for (var dc = -8; dc <= 8; dc++) {
      var nr = startRow + dr, nc = startCol + dc;
      if (nr >= 0 && nr < CC_ROWS && nc >= 0 && nc < CC_COLS && CC_MASK[nr * CC_COLS + nc]) {
        var d2 = dr * dr + dc * dc;
        if (d2 < bestD) { bestD = d2; bestC = nc; bestR = nr; }
      }
    }
  }
  if (bestD === 9999) return dist;
  var queue = [bestR * CC_COLS + bestC];
  dist[bestR * CC_COLS + bestC] = 0;
  var head = 0;
  while (head < queue.length) {
    var cur = queue[head++];
    var curDist = dist[cur];
    var cr = Math.floor(cur / CC_COLS), cc = cur % CC_COLS;
    for (var dr2 = -GAP; dr2 <= GAP; dr2++) {
      for (var dc2 = -GAP; dc2 <= GAP; dc2++) {
        if (dr2 === 0 && dc2 === 0) continue;
        var nr3 = cr + dr2, nc3 = cc + dc2;
        if (nr3 < 0 || nr3 >= CC_ROWS || nc3 < 0 || nc3 >= CC_COLS) continue;
        var ni = nr3 * CC_COLS + nc3;
        if (dist[ni] >= 0) continue;
        if (!CC_MASK[ni]) continue;
        dist[ni] = curDist + Math.sqrt(dr2 * dr2 + dc2 * dc2);
        queue.push(ni);
      }
    }
  }
  return dist;
}
function CCletterX(idx) {
  return CC_mainX + idx * (5 * CC_MAIN_SCALE + CC_MAIN_SCALE) + Math.floor(5 * CC_MAIN_SCALE / 2);
}
function CCsubLetterX(idx) {
  return CC_subX + idx * (5 * CC_SUB_SCALE + CC_SUB_SCALE) + Math.floor(5 * CC_SUB_SCALE / 2);
}
/* Bolts and their crawl maps depend only on the constant glyph geometry above
   (never on `t`), but the JSX module body re-runs every frame to reset
   globals. Rebuilding the four Dijkstra crawl maps per frame costs ~55 ms,
   which dominated the whole render. Memoize the table on the realm (user
   globals persist across frames) so it is built once. ccRender only reads
   CC_BOLTS / crawlMap, so sharing the tables is pixel-exact. */
var CC_BOLTS = globalThis.__ccBolts || (globalThis.__ccBolts = (function () {
  var bolts = [
    { strikeTime: 0.25, decayRate: 8, bolt: CCgenerateBolt(30, CCletterX(0), 3001, 3), mega: false, crawlSpeed: 70 },
    { strikeTime: 0.55, decayRate: 8, bolt: CCgenerateBolt(160, CCletterX(10), 3002, 3), mega: false, crawlSpeed: 70 },
    { strikeTime: 0.85, decayRate: 0, bolt: CCgenerateBolt(96, CCletterX(5), 3777, 8), mega: true, crawlSpeed: 300, holdTime: 3.0 },
    { strikeTime: 1.9, decayRate: 0, bolt: CCgenerateBolt(96, CCsubLetterX(8), 4888, 14, CC_subY), mega: true, superMega: true, crawlSpeed: 500, holdTime: 2.0 }
  ];
  for (var i = 0; i < bolts.length; i++) {
    bolts[i].crawlMap = CCbuildCrawlMap(bolts[i].bolt.strikeCol, bolts[i].bolt.strikeRow, i === 3 ? 8 : 3);
  }
  return bolts;
})());

/* illumination (canvas-close.html:283-376) */
var CC_ILLUM = new Float32Array(CC_ROWS * CC_COLS);
var CC_CRAWL = new Float32Array(CC_ROWS * CC_COLS);

function CCboltBrightness(bolt, t) {
  var dt = t - bolt.strikeTime;
  if (dt < 0) return 0;
  if (bolt.mega) {
    if (dt > (bolt.holdTime || 0) + 1.0) return 0;
    var attack = CCclamp01(dt / 0.02);
    var decayRate = bolt.superMega ? 2.5 : 4.0;
    var decay = Math.exp(-dt * decayRate);
    var flicker = 0;
    if (dt > 0.04 && dt < 0.08) flicker = (bolt.superMega ? 0.8 : 0.5) * Math.exp(-Math.pow(dt - 0.06, 2) * 8000);
    if (dt > 0.10 && dt < 0.16) flicker = (bolt.superMega ? 0.6 : 0.3) * Math.exp(-Math.pow(dt - 0.13, 2) * 5000);
    if (bolt.superMega && dt > 0.18 && dt < 0.25) flicker += 0.4 * Math.exp(-Math.pow(dt - 0.21, 2) * 4000);
    var holdGlow = (dt < (bolt.holdTime || 0)) ? (bolt.superMega ? 0.25 : 0.15) : 0;
    return attack * Math.max(decay + flicker, holdGlow);
  }
  if (dt > 1.5) return 0;
  var atk = CCclamp01(dt / 0.02);
  var dec = Math.exp(-dt * bolt.decayRate);
  var fl = 0;
  if (dt > 0.04 && dt < 0.08) fl = 0.5 * Math.exp(-Math.pow(dt - 0.06, 2) * 8000);
  if (dt > 0.10 && dt < 0.16) fl = 0.3 * Math.exp(-Math.pow(dt - 0.13, 2) * 5000);
  return atk * (dec + fl);
}
function CCpaintSegGlow(x1, y1, x2, y2, strength, radius) {
  var mnR = Math.max(0, Math.floor(Math.min(y1, y2) - radius));
  var mxR = Math.min(CC_ROWS - 1, Math.ceil(Math.max(y1, y2) + radius));
  var mnC = Math.max(0, Math.floor(Math.min(x1, x2) - radius));
  var mxC = Math.min(CC_COLS - 1, Math.ceil(Math.max(x1, x2) + radius));
  var dx = x2 - x1, dy = y2 - y1;
  var lenSq = dx * dx + dy * dy;
  for (var r = mnR; r <= mxR; r++) {
    for (var c = mnC; c <= mxC; c++) {
      var dist;
      if (lenSq < 0.001) {
        var ex = c - x1, ey = r - y1;
        dist = Math.sqrt(ex * ex + ey * ey);
      } else {
        var t2 = ((c - x1) * dx + (r - y1) * dy) / lenSq;
        if (t2 < 0) t2 = 0; if (t2 > 1) t2 = 1;
        var px = x1 + t2 * dx, py = y1 + t2 * dy;
        var ex2 = c - px, ey2 = r - py;
        dist = Math.sqrt(ex2 * ex2 + ey2 * ey2);
      }
      if (dist < radius) {
        var val = strength * (1.0 - dist / radius);
        var idx = r * CC_COLS + c;
        if (val > CC_ILLUM[idx]) CC_ILLUM[idx] = val;
      }
    }
  }
}
function CCpaintImpact(cx, cy, strength, radius) {
  var mnR = Math.max(0, Math.floor(cy - radius));
  var mxR = Math.min(CC_ROWS - 1, Math.ceil(cy + radius));
  var mnC = Math.max(0, Math.floor(cx - radius));
  var mxC = Math.min(CC_COLS - 1, Math.ceil(cx + radius));
  for (var r = mnR; r <= mxR; r++) {
    for (var c = mnC; c <= mxC; c++) {
      var dx = c - cx, dy = r - cy;
      var dist = Math.sqrt(dx * dx + dy * dy);
      if (dist < radius) {
        var falloff = 1.0 - (dist / radius);
        var val = strength * falloff * falloff;
        var idx = r * CC_COLS + c;
        if (val > CC_ILLUM[idx]) CC_ILLUM[idx] = val;
      }
    }
  }
}
function CCpaintBolt(boltData, bb, mega, superMega) {
  var radius = superMega ? 65 : (mega ? 45 : 20);
  var segs = boltData.segments;
  for (var i = 0; i < segs.length; i++) CCpaintSegGlow(segs[i].x1, segs[i].y1, segs[i].x2, segs[i].y2, bb, radius);
  var brs = boltData.branches;
  for (var j = 0; j < brs.length; j++) {
    var br = brs[j];
    var brSegs = br.segments;
    var brStr = bb * br.brightness;
    var brRad = radius * 0.6;
    for (var k = 0; k < brSegs.length; k++) CCpaintSegGlow(brSegs[k].x1, brSegs[k].y1, brSegs[k].x2, brSegs[k].y2, brStr, brRad);
  }
  var impactRadius = superMega ? 80 : (mega ? 55 : 28);
  CCpaintImpact(boltData.strikeCol, boltData.strikeRow, bb * (superMega ? 1.8 : 1.2), impactRadius);
}

/* render one frame into the surface, then draw it onto the stage canvas
   (canvas-close.html:381-510). The reference uses `monospace` at 14px with
   textBaseline "top"; the surface API only has the alphabetic baseline, so
   the offset is the ascent of the resolved face (JetBrains Mono at 14px
   sits 11px above the top edge — 13px was the pre-alignment guess). */
function ccRender(cv, ctx, t) {
  var surf = new OffscreenCanvas(1920, 1080).getContext('2d');
  surf.font = '400 14px "JetBrains Mono", monospace';
  surf.clearRect(0, 0, 1920, 1080);

  var fadeIn = CCclamp01(t / 0.4);
  var i;
  for (i = 0; i < CC_ILLUM.length; i++) CC_ILLUM[i] = 0;
  CC_CRAWL.fill(0);

  var bi, dt;
  for (bi = 0; bi < CC_BOLTS.length; bi++) {
    var B = CC_BOLTS[bi];
    var bb = CCboltBrightness(B, t);
    dt = t - B.strikeTime;
    if (bb > 0.001) CCpaintBolt(B.bolt, bb, B.mega, B.superMega);
    if (dt > 0) {
      var crawlMap = B.crawlMap;
      var crawlRadius = dt * B.crawlSpeed;
      var holdTime = B.holdTime || 0;
      var crawlDecay;
      if (B.mega) crawlDecay = (dt < holdTime) ? 1.0 : Math.exp(-(dt - holdTime) * 3.0);
      else crawlDecay = Math.exp(-dt * 5.0);
      if (crawlDecay < 0.005) continue;
      for (var ci = 0; ci < crawlMap.length; ci++) {
        if (crawlMap[ci] < 0) continue;
        if (crawlMap[ci] > crawlRadius) continue;
        var waveDist = crawlRadius - crawlMap[ci];
        var waveBright = crawlDecay;
        if (waveDist < 8 && dt < 1.0) waveBright *= (1.0 + (1.0 - waveDist / 8) * 2.0);
        if (waveBright > CC_CRAWL[ci]) CC_CRAWL[ci] = waveBright;
      }
    }
  }

  var lastFill = '';
  for (var row = 0; row < CC_ROWS; row++) {
    for (var col = 0; col < CC_COLS; col++) {
      var idx = row * CC_COLS + col;
      var ambN = CCfbm(col * 0.03 + t * 0.08, row * 0.04 + t * 0.06);
      var ambBright = 0.02 + ambN * 0.04;
      if (t < 0.25) {
        var flickerN = CChash(col * 7 + Math.floor(t * 15), row * 11 + Math.floor(t * 15));
        if (flickerN > 0.997) ambBright += 0.15 * (1.0 - t / 0.25);
      }
      var localIllum = CC_ILLUM[idx];
      var crawlIllum = CC_CRAWL[idx];
      var textType = CC_MASK[idx];
      var brightness, cr, cg, cb;
      var effectiveCrawl = crawlIllum;
      if (textType && effectiveCrawl > 0.01) {
        var ci2 = CCclamp01(effectiveCrawl);
        brightness = ci2 * fadeIn;
        var whiteness = CCclamp01(effectiveCrawl - 0.8) * 3.0;
        cr = Math.round((40 + whiteness * 215 + ci2 * 180) * brightness);
        cg = Math.round((15 + whiteness * 160 + ci2 * 100) * brightness);
        cb = Math.round((5 + whiteness * 140 + ci2 * 30) * brightness);
        if (localIllum > 0.1) {
          var boost = localIllum * 0.4;
          cr = Math.min(255, cr + Math.round(boost * 220));
          cg = Math.min(255, cg + Math.round(boost * 150));
          cb = Math.min(255, cb + Math.round(boost * 80));
          brightness = Math.min(1.0, brightness + boost);
        }
      } else if (localIllum > 0.03) {
        brightness = localIllum * fadeIn;
        var coreAmt = CCclamp01((localIllum - 0.4) * 2.5);
        cr = Math.round((coreAmt * 240 + localIllum * 80) * brightness);
        cg = Math.round((coreAmt * 160 + localIllum * 50) * brightness);
        cb = Math.round((coreAmt * 60 + localIllum * 20) * brightness);
      } else {
        brightness = ambBright * fadeIn;
        cr = Math.round((8 + ambN * 15) * brightness);
        cg = Math.round((4 + ambN * 8) * brightness);
        cb = Math.round((2 + ambN * 4) * brightness);
      }

      var dither = CC_BAYER[row & 3][col & 3];
      var sl = brightness * CC_GLYPH_MAX;
      var bl = Math.floor(sl);
      var fr = sl - bl;
      var rl = bl + (fr > dither ? 1 : 0);
      if (rl > CC_GLYPH_MAX) rl = CC_GLYPH_MAX;
      if (rl < 1 && brightness > 0.01) rl = 1;
      if (brightness < 0.01) rl = 0;
      if (rl === 0) continue;

      var scanline = 0.92 + 0.08 * Math.sin(row * 0.6 + t * 0.8);
      var vx = col / CC_COLS - 0.5, vy = row / CC_ROWS - 0.5;
      var vig = 1.0 - (vx * vx + vy * vy) * 0.35;
      if (vig < 0.55) vig = 0.55;
      var m = scanline * vig;
      var frr = Math.round(cr * m);
      var fgg = Math.round(cg * m);
      var fbb = Math.round(cb * m);
      if (frr > 255) frr = 255;
      if (fgg > 255) fgg = 255;
      if (fbb > 255) fbb = 255;

      var fill = 'rgb(' + frr + ',' + fgg + ',' + fbb + ')';
      if (fill !== lastFill) {
        surf.fillStyle = fill;
        lastFill = fill;
      }
      surf.fillText(CC_GLYPHS[rl], col * CC_CELL_W, row * CC_CELL_H + 11);
    }
  }

  /* The surface is sampled straight by an identity kernel: ly.draw takes the
     surface as a source and bakes it on demand, so the script no longer
     manages a frame-scoped generated-image key. */
  var ccIdent = ly.pixel((uv, src) => {
    var c = src.eval(uv);
    if (c.a > 0.0) { return [c.r / c.a, c.g / c.a, c.b / c.a, c.a]; }
    return [0.0, 0.0, 0.0, 0.0];
  });
  ly.draw(cv, ccIdent, { rect: [0, 0, 1920, 1080], sources: [surf] });
}

export default function CanvasClose() {
  const CC_CK = useCanvasKit();

  useEffect(() => {
    var t = ctx.current.time;
    ccRender(CC_CK.MakeCanvasSurface('cc-canvas').getCanvas(), ctx, Math.min(t, 3.93));
  });

  return (
    <div id="cc-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#000]">
      <canvas id="cc-canvas" className="w-[1920px] h-[1080px]" />
    </div>
  );
}
