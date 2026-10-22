import { gsap } from 'gsap';
import CanvasKitInit from 'canvaskit-wasm';
import { useFrame, useGSAP } from 'opencat/runtime';
import { AgentPill, Cdot, GS, StageCanvas, clamp, getK3Field, makeMeasure, mulberry32, pillWidths, vis } from './shared.jsx';


export default function SceneA() {
  let CK = null;
  CanvasKitInit({}).then((ck) => { CK = ck; });
  // gsap code: declarative timelines (zoom/field envelope, title fade).
  useGSAP((ctx, t) => {
    var T = t;
    if (T >= 0.75 && T < 4.62) {
      gsap.timeline()
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
      // selection box + title fade-in (index.html:1607-1619)
      gsap.timeline()
        .set(['atitlegrp', 'atitlegrpp'], { opacity: 0 }, 0.75)
        .to('atitlegrp', { opacity: 1, duration: 0.06, ease: 'power1.out' }, 1.73)
        .set('atitlegrpp', { opacity: 1 }, 1.79);
    }
  });
  // CanvasKit drawing + per-frame gating.
  useFrame((ctx, t) => {
    if (!CK) return;
    var T = t;
    var A_ON = (T >= 0.75 && T < 4.62);
    vis(ctx, 'sceneA', A_ON);
    if (A_ON) {

      var K3FIELD = getK3Field();
      var FIELD_BORN = K3FIELD.born;
      var FIELD_RING = K3FIELD.ring;
      function ioEase(p) { p = p < 0 ? 0 : (p > 1 ? 1 : p); return p < 0.5 ? 2 * p * p : 1 - 2 * (1 - p) * (1 - p); }

      function paintField(canvasId, purple) {
        var cv = CK.MakeCanvasSurface(canvasId).getCanvas(); if (!cv) return;
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
      if (T >= 3.5) document.getElementById('afield').filter('brightness(0.5) sepia(1) hue-rotate(215deg) saturate(2.2)');
      paintField('afield-canvas', false);

      // purple clone crossfade (3.03→3.23 in, 3.83→4.00 out; power1.inOut) is
      // declarative (see timeline); ppOp below only gates the purple-child work.
      var ppOp = (T >= 3.03 && T <= 4.0)
        ? (T < 3.23 ? ioEase((T - 3.03) / 0.2) : (T > 3.83 ? ioEase(1 - (T - 3.83) / 0.17) : 1)) : 0;
      if (ppOp > 0) { paintField('afieldp-canvas', true); drawLanes('afieldp-canvas', true); }
      vis(ctx, 'aw1', T >= 1.8); vis(ctx, 'aline1', T >= 1.8); vis(ctx, 'aline1p', T >= 1.8 && ppOp > 0);
      vis(ctx, 'aw2', T >= 1.8330);
      vis(ctx, 'aline2', T >= 1.8663); vis(ctx, 'aline2p', T >= 1.8663 && ppOp > 0);

      // Box ghost lanes: CSS .glane = 2x360 strip, rotate(27deg) about its
      // centre ("/" direction), inside the clipped 467x185 aselrect at
      // (755,444). Dashes 3x13 glide down-left. Drawn on BOTH canvases.
      function drawLanes(canvasId, purple) {
        var cv = CK.MakeCanvasSurface(canvasId).getCanvas(); if (!cv) return;
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
      var sCvAll = CK.MakeCanvasSurface('apill-canvas').getCanvas();
      if (sCvAll) sCvAll.clear();
      if (T >= 1.4 && T <= 4.6) {
        var th = orbTheta(T);
        for (var pi = 0; pi < pillIds.length; pi++) {
          var pn = document.getElementById(pillIds[pi]); if (!pn) continue;
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
        var sAL = CK.MakeCanvasSurface('stripAL').getCanvas(), sAR = CK.MakeCanvasSurface('stripAR').getCanvas();
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

