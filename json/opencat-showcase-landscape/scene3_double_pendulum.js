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
function acc1(t1, t2, w1, w2, l1, l2, m1, m2, g) {
  var num1 = -g * (2 * m1 + m2) * Math.sin(t1);
  var num2 = -m2 * g * Math.sin(t1 - 2 * t2);
  var num3 = -2 * Math.sin(t1 - t2) * m2 * (w2 * w2 * l2 + w1 * w1 * l1 * Math.cos(t1 - t2));
  var den = l1 * (2 * m1 + m2 - m2 * Math.cos(2 * t1 - 2 * t2));
  return (num1 + num2 + num3) / den;
}
function acc2(t1, t2, w1, w2, l1, l2, m1, m2, g) {
  var num = 2 * Math.sin(t1 - t2) * (w1 * w1 * l1 * (m1 + m2) + g * (m1 + m2) * Math.cos(t1) + w2 * w2 * l2 * m2 * Math.cos(t1 - t2));
  var den = l2 * (2 * m1 + m2 - m2 * Math.cos(2 * t1 - 2 * t2));
  return num / den;
}
var t = ctx.currentFrame / 30;
canvas.clear(CK.parseColorString('#020617'));
for (var gy = 0; gy < 8; gy++) {
  var y = 110 + gy * 120;
  canvas.drawLine(720, y, 1880, y + 20, stroke('rgba(148,163,184,0.06)', 1));
}
for (var gx = 0; gx < 8; gx++) {
  var x = 760 + gx * 130;
  canvas.drawLine(x, 40, x - 120, 1040, stroke('rgba(148,163,184,0.05)', 1));
}
for (var ring = 0; ring < 6; ring++) {
  canvas.drawCircle(1360, 510, 160 + ring * 70 + Math.sin(t * 0.7 + ring) * 8, stroke('rgba(251,191,36,0.07)', 2));
}
var cx = 1360;
var cy = 180;
var l1 = 240;
var l2 = 220;
var m1 = 1.1;
var m2 = 1.0;
var g = 9.81;
var theta1 = Math.PI * 0.93;
var theta2 = Math.PI * 0.67;
var omega1 = 0;
var omega2 = 0;
var dt = 0.014;
var steps = ctx.currentFrame * 6 + 1;
var trail = [];
var trail1 = [];
for (var i = 0; i < steps; i++) {
  var a1 = acc1(theta1, theta2, omega1, omega2, l1, l2, m1, m2, g);
  var a2 = acc2(theta1, theta2, omega1, omega2, l1, l2, m1, m2, g);
  omega1 += a1 * dt;
  omega2 += a2 * dt;
  theta1 += omega1 * dt;
  theta2 += omega2 * dt;
  if (i % 2 === 0) {
    var x1h = cx + l1 * Math.sin(theta1);
    var y1h = cy + l1 * Math.cos(theta1);
    var x2h = x1h + l2 * Math.sin(theta2);
    var y2h = y1h + l2 * Math.cos(theta2);
    trail1.push([x1h, y1h]);
    trail.push([x2h, y2h]);
    if (trail.length > 160) {
      trail.shift();
      trail1.shift();
    }
  }
}
var x1 = cx + l1 * Math.sin(theta1);
var y1 = cy + l1 * Math.cos(theta1);
var x2 = x1 + l2 * Math.sin(theta2);
var y2 = y1 + l2 * Math.cos(theta2);
if (trail1.length > 1) {
  var path1 = new CK.Path();
  path1.moveTo(trail1[0][0], trail1[0][1]);
  for (var p1 = 1; p1 < trail1.length; p1++) {
    path1.lineTo(trail1[p1][0], trail1[p1][1]);
  }
  canvas.drawPath(path1, stroke('rgba(125,211,252,0.20)', 3));
}
if (trail.length > 1) {
  var path2 = new CK.Path();
  path2.moveTo(trail[0][0], trail[0][1]);
  for (var p2 = 1; p2 < trail.length; p2++) {
    path2.lineTo(trail[p2][0], trail[p2][1]);
  }
  canvas.drawPath(path2, stroke('rgba(251,191,36,0.34)', 4));
}
canvas.drawLine(cx - 120, cy, cx + 120, cy, stroke('rgba(148,163,184,0.82)', 4));
for (var tick = -4; tick <= 4; tick++) {
  canvas.drawLine(cx + tick * 26, cy, cx + tick * 26 + 12, cy + 16, stroke('rgba(148,163,184,0.42)', 2));
}
canvas.drawLine(cx, cy, x1, y1, stroke('rgba(226,232,240,0.92)', 4));
canvas.drawLine(x1, y1, x2, y2, stroke('rgba(251,191,36,0.95)', 4));
canvas.drawCircle(cx, cy, 12, fill('rgba(148,163,184,1)'));
canvas.drawCircle(x1, y1, 24, fill('rgba(125,211,252,0.96)'));
canvas.drawCircle(x1, y1, 40, stroke('rgba(125,211,252,0.18)', 3));
canvas.drawCircle(x2, y2, 28, fill('rgba(251,191,36,0.98)'));
canvas.drawCircle(x2, y2, 52, stroke('rgba(251,191,36,0.16)', 3));
for (var dot = 0; dot < 22; dot++) {
  var ang = dot * Math.PI / 11 + t * 0.25;
  var dx = x2 + Math.cos(ang) * (74 + (dot % 3) * 16);
  var dy = y2 + Math.sin(ang) * (74 + (dot % 3) * 16);
  canvas.drawCircle(dx, dy, 4 + (dot % 2), fill('rgba(255,255,255,0.12)'));
}
