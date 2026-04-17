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
for (var i = 0; i < 8; i++) {
  canvas.drawCircle(1430, 540, 150 + i * 70 + Math.sin(t * 0.7 + i) * 10, stroke('rgba(34,211,238,0.08)', 2));
}
for (var p = 0; p < 24; p++) {
  var angle = p * Math.PI / 12 + t * 0.45;
  var x = 1430 + Math.cos(angle) * (240 + (p % 4) * 40);
  var y = 540 + Math.sin(angle) * (220 + (p % 5) * 20);
  canvas.drawCircle(x, y, 5 + (p % 3), fill('rgba(255,255,255,0.16)'));
}
for (var line = 0; line < 4; line++) {
  var ly = 160 + line * 220 + Math.sin(t + line) * 18;
  canvas.drawLine(980, ly, 1860, ly + 30, stroke('rgba(125,211,252,0.10)', 3));
}
canvas.drawCircle(1430, 540, 34 + Math.sin(t * 2) * 4, fill('rgba(34,211,238,0.45)'));
