var intro = ctx.animate({
  from: { opacity: 0, translateY: 52 },
  to: { opacity: 1, translateY: 0 },
  duration: 20,
  easing: 'spring-gentle',
  clamp: true
});
ctx.getNode('s5-line-1').opacity(intro.opacity).translateY(intro.translateY);
ctx.getNode('s5-line-2').opacity(intro.opacity).translateY(intro.translateY * 0.82);
ctx.getNode('s5-line-3').opacity(intro.opacity).translateY(intro.translateY * 0.62);
ctx.getNode('s5-subtitle').opacity(intro.opacity).translateY(intro.translateY * 0.46);
ctx.getNode('s5-mark').opacity(intro.opacity);
ctx.getNode('s5-mark-sub').opacity(intro.opacity);
var chips = ctx.stagger(3, {
  from: { opacity: 0, translateY: 18, scale: 0.95 },
  to: { opacity: 1, translateY: 0, scale: 1 },
  gap: 3,
  duration: 14,
  easing: 'spring-default',
  clamp: true
});
['s5-chip-1', 's5-chip-2', 's5-chip-3'].forEach(function(id, index) {
  ctx.getNode(id).opacity(chips[index].opacity).translateY(chips[index].translateY).scale(chips[index].scale);
});
[
  { id: 's5-block-a', from: { opacity: 0, translateX: 220, translateY: -140, rotate: 20, scale: 0.8 }, delay: 0 },
  { id: 's5-block-b', from: { opacity: 0, translateX: 220, translateY: 80, rotate: -18, scale: 0.82 }, delay: 4 },
  { id: 's5-block-c', from: { opacity: 0, translateX: -220, translateY: 130, rotate: 16, scale: 0.82 }, delay: 7 },
  { id: 's5-block-d', from: { opacity: 0, translateX: 180, translateY: 160, rotate: -16, scale: 0.82 }, delay: 10 }
].forEach(function(item, index) {
  var anim = ctx.animate({
    from: item.from,
    to: { opacity: 0.84, translateX: 0, translateY: 0, rotate: 0, scale: 1 },
    duration: 18,
    delay: item.delay,
    easing: 'spring-wobbly',
    clamp: true
  });
  var tilt = Math.sin(ctx.currentFrame / 12 + index * 0.6) * 2.1;
  ctx.getNode(item.id).opacity(anim.opacity).translateX(anim.translateX).translateY(anim.translateY).rotate(anim.rotate + tilt).scale(anim.scale);
});
ctx.getNode('s5-copy').scale(1 + Math.sin(ctx.currentFrame / 18) * 0.008);
