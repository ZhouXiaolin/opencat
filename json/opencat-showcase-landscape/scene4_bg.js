var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();
function stroke(color, width) {
  var p = new CK.Paint();
  p.setStyle(CK.PaintStyle.Stroke);
  p.setColor(CK.parseColorString(color));
  p.setStrokeWidth(width || 1);
  p.setAntiAlias(true);
  return p;
}
canvas.clear(CK.parseColorString('#f3ece2'));
for (var i = 0; i < 14; i++) {
  var x = -120 + i * 170;
  canvas.drawLine(x, 0, x + 420, 1080, stroke('rgba(15,23,42,0.05)', 2));
}
for (var j = 0; j < 10; j++) {
  var y = 80 + j * 90;
  canvas.drawLine(980, y, 1880, y - 110, stroke('rgba(15,23,42,0.04)', 1));
}
