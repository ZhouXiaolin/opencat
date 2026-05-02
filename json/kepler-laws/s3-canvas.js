var CK = ctx.CanvasKit;
var c = ctx.getCanvas();
c.clear(CK.parseColorString('#020617'));
var p = new CK.Paint();
p.setStyle(CK.PaintStyle.Fill);
var S = 242;
for (var i = 0; i < 35; i++) {
  p.setColor(CK.Color4f(1, 1, 1, ctx.utils.random(0.15, 0.5, S + i * 5 + 3)));
  c.drawCircle(ctx.utils.random(0, 780, S + i * 5), ctx.utils.random(0, 680, S + i * 5 + 1), ctx.utils.random(0.5, 2.0, S + i * 5 + 2), p);
}
var cx = 340, cy = 340, rx = 260, ry = 170, foc = Math.sqrt(rx * rx - ry * ry);
var fx = cx - foc, fy = cy;
var op = new CK.Paint();
op.setStyle(CK.PaintStyle.Stroke);
op.setStrokeWidth(2);
op.setColor(CK.parseColorString('#22d3ee'));
op.setAntiAlias(true);
c.drawOval(CK.XYWHRect(cx - rx, cy - ry, rx * 2, ry * 2), op);
var sp = new CK.Paint();
sp.setStyle(CK.PaintStyle.Fill);
sp.setColor(CK.parseColorString('#fbbf24'));
c.drawCircle(fx, fy, 22, sp);
var t = ctx.currentFrame / ctx.sceneFrames;
var theta = 2 * Math.PI * (t + 0.15 * Math.sin(2 * Math.PI * t));
var px = cx + rx * Math.cos(theta);
var py = cy + ry * Math.sin(theta);
var tp = new CK.Paint();
tp.setStyle(CK.PaintStyle.Stroke);
tp.setStrokeWidth(2);
tp.setColor(CK.Color4f(0.13, 0.83, 0.93, 0.2));
tp.setAntiAlias(true);
c.drawArc(CK.XYWHRect(cx - rx, cy - ry, rx * 2, ry * 2), -90, (theta * 180 / Math.PI), false, tp);
var pp = new CK.Paint();
pp.setStyle(CK.PaintStyle.Fill);
pp.setColor(CK.WHITE);
c.drawCircle(px, py, 8, pp);
function drawSector(t1, t2, alp) {
  var path = new CK.Path();
  path.moveTo(fx, fy);
  var steps = 12;
  for (var j = 0; j <= steps; j++) {
    var a = t1 + (t2 - t1) * j / steps;
    path.lineTo(cx + rx * Math.cos(a), cy + ry * Math.sin(a));
  }
  path.close();
  var fill = new CK.Paint();
  fill.setStyle(CK.PaintStyle.Fill);
  fill.setColor(CK.Color4f(0.13, 0.83, 0.93, alp));
  c.drawPath(path, fill);
  var stroke = new CK.Paint();
  stroke.setStyle(CK.PaintStyle.Stroke);
  stroke.setStrokeWidth(1);
  stroke.setColor(CK.Color4f(0.13, 0.83, 0.93, alp + 0.15));
  c.drawPath(path, stroke);
}
var highlight = 0.5 + 0.5 * Math.sin(ctx.currentFrame * 0.025);
drawSector(-0.52, 0.52, 0.12 + 0.08 * highlight);
drawSector(2.62, 3.66, 0.12 + 0.08 * (1 - highlight));
