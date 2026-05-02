var CK = ctx.CanvasKit;
var c = ctx.getCanvas();
c.clear(CK.parseColorString('#020617'));
var p = new CK.Paint();
p.setStyle(CK.PaintStyle.Fill);
var S = 342;
for (var i = 0; i < 40; i++) {
  var tw = 0.2 + 0.5 * (0.5 + 0.5 * Math.sin(ctx.currentFrame * 0.03 * ctx.utils.random(0.3, 0.7, S + i * 7 + 4) + ctx.utils.random(0, 6.283, S + i * 7 + 3)));
  p.setColor(CK.Color4f(1, 1, 1, tw));
  c.drawCircle(ctx.utils.random(0, 1920, S + i * 7), ctx.utils.random(0, 1080, S + i * 7 + 1), ctx.utils.random(0.5, 2.5, S + i * 7 + 2), p);
}
