var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();
function fill(color) {
  var p = new CK.Paint();
  p.setStyle(CK.PaintStyle.Fill);
  p.setColor(CK.parseColorString(color));
  p.setAntiAlias(true);
  return p;
}
function stroke(color, width) {
  var p = new CK.Paint();
  p.setStyle(CK.PaintStyle.Stroke);
  p.setColor(CK.parseColorString(color));
  p.setStrokeWidth(width || 1);
  p.setAntiAlias(true);
  return p;
}
var t = ctx.currentFrame / 30;
canvas.clear(CK.parseColorString('#020617'));
for (var i = 0; i < 16; i++) {
  var sx = -100 + i * 180;
  canvas.drawLine(sx, 40, sx + 820, 1080, stroke('rgba(56,189,248,0.08)', 2));
}
for (var j = 0; j < 10; j++) {
  var sy = 40 + j * 110;
  canvas.drawLine(920, sy, 1880, sy - 120, stroke('rgba(251,191,36,0.06)', 1));
}
for (var ring = 0; ring < 6; ring++) {
  canvas.drawCircle(1380, 530, 170 + ring * 90 + Math.sin(t * 0.8 + ring) * 10, stroke('rgba(255,255,255,0.05)', 2));
}
for (var p = 0; p < 28; p++) {
  var ang = t * 0.35 + p * Math.PI / 14;
  var x = 1370 + Math.cos(ang) * (320 + (p % 4) * 28);
  var y = 520 + Math.sin(ang) * (260 + (p % 5) * 18);
  canvas.drawCircle(x, y, 5 + (p % 3), fill('rgba(255,255,255,0.14)'));
}
var path = new CK.Path();
path.moveTo(990, 880);
path.lineTo(1240, 730);
path.lineTo(1510, 810);
path.lineTo(1790, 650);
canvas.drawPath(path, stroke('rgba(125,211,252,0.18)', 4));
