var intro = ctx.animate({
  from: { opacity: 0, translateY: 40 },
  to: { opacity: 1, translateY: 0 },
  duration: 20,
  easing: 'spring-gentle',
  clamp: true
});
ctx.getNode('s2-title').opacity(intro.opacity).translateY(intro.translateY);
ctx.getNode('s2-editor').opacity(intro.opacity).translateY(30 * (1 - intro.opacity)).scale(0.97 + intro.opacity * 0.03);
ctx.getNode('s2-tip').opacity(intro.opacity).translateY(24 * (1 - intro.opacity));
var rows = ctx.stagger(3, {
  from: { opacity: 0, translateX: 34, translateY: 12, scale: 0.96 },
  to: { opacity: 1, translateX: 0, translateY: 0, scale: 1 },
  gap: 4,
  duration: 16,
  easing: 'spring-default',
  clamp: true
});
['s2-preview-row1', 's2-preview-row2', 's2-preview-row3'].forEach(function(id, index) {
  ctx.getNode(id).opacity(rows[index].opacity).translateX(rows[index].translateX).translateY(rows[index].translateY).scale(rows[index].scale);
});
ctx.getNode('s2-preview').opacity(intro.opacity).translateY(16 * (1 - intro.opacity));
[
  { id: 's2-block-a', from: { opacity: 0, translateX: -180, translateY: -140, rotate: -24, scale: 0.78 }, delay: 0 },
  { id: 's2-block-b', from: { opacity: 0, translateX: 200, translateY: -180, rotate: 20, scale: 0.8 }, delay: 4 },
  { id: 's2-block-c', from: { opacity: 0, translateX: 180, translateY: 160, rotate: -18, scale: 0.82 }, delay: 8 },
  { id: 's2-block-d', from: { opacity: 0, translateX: -140, translateY: 140, rotate: 15, scale: 0.82 }, delay: 11 }
].forEach(function(item, index) {
  var anim = ctx.animate({
    from: item.from,
    to: { opacity: 0.8, translateX: 0, translateY: 0, rotate: 0, scale: 1 },
    duration: 18,
    delay: item.delay,
    easing: 'spring-wobbly',
    clamp: true
  });
  var tilt = Math.sin(ctx.currentFrame / 12 + index * 0.6) * 2.2;
  ctx.getNode(item.id).opacity(anim.opacity).translateX(anim.translateX).translateY(anim.translateY).rotate(anim.rotate + tilt).scale(anim.scale);
});
