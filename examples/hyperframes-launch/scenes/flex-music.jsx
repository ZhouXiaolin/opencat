import { gsap } from 'gsap';
import CanvasKitInit from 'canvaskit-wasm';
import { useGSAP, useEffect } from 'opencat/runtime';
/* ══ flex-music (master 27.40-27.98, compositions/flex-music.html) ══════
   The vinyl/tone-arm SVGs become canvas draws (jsx svg supports <path>
   only). The reference's back.out rotation tween is fully overwritten by
   the later linear spin (same start, same property) — only the linear one
   is emitted. Record label text stays DOM. */
var FM_START = 27.40, FM_DUR = 0.58;

let FM_CK = null;
CanvasKitInit({}).then((ck) => { FM_CK = ck; });

export default function FlexMusic() {

  useGSAP(() => {
    var tl = gsap.timeline(), A = function (t) { return t; };
    tl.fromTo('fm-record', { scale: 0.4, opacity: 0 }, { scale: 1, opacity: 1, duration: 0.3, ease: 'back.out(1.4)' }, A(0));
    tl.fromTo('fm-record', { rotation: -90 }, { rotation: 15, duration: FM_DUR, ease: 'none' }, A(0));
    tl.set('fm-arm', { opacity: 1 }, A(0.08));
    tl.fromTo('fm-arm', { rotation: 35 }, { rotation: 18, duration: 0.25, ease: 'power2.out' }, A(0.1));
    tl.fromTo('fm-text', { opacity: 0, y: 8 }, { opacity: 1, y: 0, duration: 0.15, ease: 'power2.out' }, A(0.12));
  });

  /* vinyl disc (svg 600 -> 520px canvas, k=0.8667). Static art: the spin is a
     DOM tween on #fm-record, so the canvas only needs its resting pixels. */
  useEffect(() => {
    if (!FM_CK) return;
    var c = FM_CK.MakeCanvasSurface('fm-record-canvas').getCanvas();
    var ck = FM_CK;
    c.clear();
    var p = new ck.Paint();
    p.setStyle(ck.PaintStyle.Fill);
    p.setAntiAlias(true);
    p.setColor(ck.Color(26, 18, 8, 1));
    c.drawCircle(260, 260, 280 * 0.8667, p);
    p.setStyle(ck.PaintStyle.Stroke);
    var grooves = [[260, 0.3, 1], [240, 0.25, 0.8], [220, 0.3, 1], [200, 0.2, 0.8], [180, 0.3, 1], [160, 0.25, 0.8], [140, 0.3, 1]];
    grooves.forEach(function (g) {
      p.setStrokeWidth(g[2] * 0.8667);
      p.setColor(ck.Color(80, 60, 20, g[1]));
      c.drawCircle(260, 260, g[0] * 0.8667, p);
    });
    p.setStyle(ck.PaintStyle.Fill);
    p.setColor(ck.Color(255, 220, 140, 0.06));
    c.save();
    c.translate(208, 208);
    c.rotate(-30);
    c.drawOval([-104, -69.3, 104, 69.3], p);
    c.restore();
    p.setColor(ck.Color(200, 132, 10, 1));
    c.drawCircle(260, 260, 90 * 0.8667, p);
    p.setStyle(ck.PaintStyle.Stroke);
    p.setStrokeWidth(0.87);
    p.setColor(ck.Color(255, 255, 255, 0.12));
    c.drawCircle(260, 260, 88 * 0.8667, p);
    p.setStyle(ck.PaintStyle.Fill);
    p.setColor(ck.Color(26, 18, 8, 1));
    c.drawCircle(260, 260, 8 * 0.8667, p);
  });

  /* tone arm (svg 200x400 -> 120x280 canvas, meet: k=0.6, y+20) */
  useEffect(() => {
    if (!FM_CK) return;
    var c = FM_CK.MakeCanvasSurface('fm-arm-canvas').getCanvas();
    var ck = FM_CK;
    c.clear();
    var q = new ck.Paint();
    q.setStyle(ck.PaintStyle.Fill);
    q.setAntiAlias(true);
    function X(v) { return v * 0.6; }
    function Y(v) { return v * 0.6 + 20; }
    q.setColor(ck.Color(42, 31, 16, 1));
    c.drawCircle(X(100), Y(30), X(18), q);
    q.setColor(ck.Color(61, 46, 24, 1));
    c.drawCircle(X(100), Y(30), X(12), q);
    q.setColor(ck.Color(42, 31, 16, 1));
    c.drawRRect(ck.RRectXY([X(97), Y(30), X(103), Y(310)], 1.8, 1.8), q);
    q.setColor(ck.Color(61, 46, 24, 1));
    c.drawRRect(ck.RRectXY([X(88), Y(300), X(112), Y(340)], 2.4, 2.4), q);
    q.setColor(ck.Color(26, 18, 8, 1));
    c.drawRRect(ck.RRectXY([X(94), Y(336), X(106), Y(352)], 1.2, 1.2), q);
    q.setColor(ck.Color(200, 132, 10, 1));
    c.drawCircle(X(100), Y(356), X(2), q);
  });

  return (
    <div id="fm-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f5a623]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_45%_48%,#f7c04a_0%,#e8960a_100%)]"></div>
      <div id="fm-record" class="absolute left-[700px] top-[250px] w-[520px] h-[520px] opacity-0 drop-shadow-[0_20px_60px_rgba(20,12,0,0.4)]">
        <canvas id="fm-record-canvas" class="w-[520px] h-[520px]" />
        <text class="absolute left-0 top-[243px] w-full text-center text-[13.9px] tracking-[0.1em] font-[hf-inter-med] text-[#1a1208]">HYPERFRAMES</text>
        <text class="absolute left-0 top-[264px] w-full text-center text-[9.5px] font-[hf-inter] text-[rgba(26,18,8,0.6)]">A-SIDE</text>
      </div>
      <div id="fm-arm" class="absolute left-[1140px] top-[250px] w-[120px] h-[280px] [transform-origin:50%_15px] opacity-0 drop-shadow-[0_4px_12px_rgba(20,12,0,0.3)]">
        <canvas id="fm-arm-canvas" class="w-[120px] h-[280px]" />
      </div>
      <div class="absolute left-0 bottom-[120px] w-full flex justify-center">
        <text id="fm-text" class="text-[64px] leading-none tracking-[0.08em] font-[hf-inter-med] text-[#2a1808] uppercase opacity-0">music</text>
      </div>
    </div>
  );
}
