var CK = ctx.CanvasKit;
var c = ctx.getCanvas();
c.clear(CK.parseColorString('#020617'));
var p = new CK.Paint();
p.setStyle(CK.PaintStyle.Fill);
var S = 42;
for (var i = 0; i < 50; i++) {
  var tw = 0.2 + 0.5 * (0.5 + 0.5 * Math.sin(ctx.currentFrame * 0.03 * ctx.utils.random(0.3, 0.8, S + i * 7 + 4) + ctx.utils.random(0, 6.283, S + i * 7 + 3)));
  p.setColor(CK.Color4f(1, 1, 1, tw));
  c.drawCircle(ctx.utils.random(0, 1920, S + i * 7), ctx.utils.random(0, 1080, S + i * 7 + 1), ctx.utils.random(0.5, 2.5, S + i * 7 + 2), p);
}
var rp = new CK.Paint();
rp.setStyle(CK.PaintStyle.Stroke);
rp.setStrokeWidth(1.5);
rp.setColor(CK.parseColorString('#475569'));
rp.setAntiAlias(true);
c.save();
var rock = 6 + 4 * Math.sin(ctx.currentFrame * 0.015);
c.rotate(rock, 960, 700);
c.drawOval(CK.XYWHRect(560, 580, 800, 240), rp);
c.restore();
var gp = new CK.Paint();
gp.setStyle(CK.PaintStyle.Fill);
gp.setColor(CK.Color4f(0.98, 0.76, 0.14, 0.04 + 0.02 * Math.sin(ctx.currentFrame * 0.02)));
c.drawCircle(960, 540, 350, gp);
