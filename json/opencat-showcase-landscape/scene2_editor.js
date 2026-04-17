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
var uiFont = new CK.Font(null, 18);
uiFont.setEdging(CK.FontEdging.SubpixelAntiAlias);
var codeFont = new CK.Font(null, 26);
codeFont.setEdging(CK.FontEdging.SubpixelAntiAlias);
var bodyFont = new CK.Font(null, 28);
bodyFont.setEdging(CK.FontEdging.SubpixelAntiAlias);
var lines = [
  '{"type":"composition","width":1920,"height":1080,"fps":30,"frames":480}',
  '{"id":"scene3","parentId":null,"type":"div","duration":120}',
  '{"id":"sim","parentId":"scene3","type":"canvas","className":"w-[1920px] h-[1080px]"}',
  '{"id":"title","parentId":"scene3","type":"text","text":"Double pendulum"}',
  '{"type":"script","parentId":"sim","path":"opencat-showcase-landscape/scene3_double_pendulum.js"}',
  '{"type":"transition","from":"scene2","to":"scene3","effect":"wipe","direction":"from_top_right"}'
];
var full = lines.join('\n');
var typing = ctx.animate({
  from: { chars: 0 },
  to: { chars: full.length },
  duration: 78,
  easing: 'linear',
  clamp: true
});
var count = Math.max(0, Math.min(full.length, Math.floor(typing.chars)));
var visible = full.slice(0, count);
var visibleLines = visible.split('\n');
var caretBlink = (ctx.currentFrame % 20) < 10;
canvas.clear(CK.parseColorString('#0f172a'));
canvas.drawRRect(CK.RRectXY(CK.XYWHRect(0, 0, 920, 600), 40, 40), fill('#0f172a'));
canvas.drawRRect(CK.RRectXY(CK.XYWHRect(0, 0, 920, 600), 40, 40), stroke('#1e293b', 2));
canvas.drawRect(CK.XYWHRect(0, 0, 920, 76), fill('#111827'));
canvas.drawCircle(40, 38, 8, fill('#fb7185'));
canvas.drawCircle(68, 38, 8, fill('#f59e0b'));
canvas.drawCircle(96, 38, 8, fill('#22c55e'));
canvas.drawText('opencat-project-showcase-landscape.jsonl', 130, 45, fill('#cbd5e1'), uiFont);
canvas.drawText('system edit', 800, 45, fill('#64748b'), uiFont);
canvas.drawText('> JSONL authoring', 46, 122, fill('#38bdf8'), bodyFont);
for (var i = 0; i < 6; i++) {
  var y = 200 + i * 62;
  canvas.drawText(String(i + 1), 34, y, fill('#475569'), codeFont);
  var line = visibleLines[i] || '';
  canvas.drawText(line, 92, y, fill('#e2e8f0'), codeFont);
}
var caretLineIndex = Math.max(0, visibleLines.length - 1);
var caretLine = visibleLines[caretLineIndex] || '';
var caretX = 92 + codeFont.measureText(caretLine);
var caretY = 200 + caretLineIndex * 62;
if (caretBlink) {
  canvas.drawLine(caretX + 8, caretY - 30, caretX + 8, caretY + 6, stroke('#38bdf8', 4));
}
canvas.drawRect(CK.XYWHRect(46, 548, 828, 10), fill('#1e293b'));
canvas.drawRRect(CK.RRectXY(CK.XYWHRect(46, 548, 828 * (count / Math.max(1, full.length)), 10), 5, 5), fill('#38bdf8'));
canvas.drawText('One scene file can stage UI, code, canvas, and transitions together.', 46, 586, fill('#94a3b8'), uiFont);
