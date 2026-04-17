var intro = ctx.animate({
  from: { opacity: 0, translateY: 48 },
  to: { opacity: 1, translateY: 0 },
  duration: 20,
  easing: 'spring-gentle',
  clamp: true
});
ctx.getNode('s1-kicker').opacity(intro.opacity).translateY(intro.translateY);
ctx.getNode('s1-title-1').opacity(intro.opacity).translateY(intro.translateY * 0.95);
ctx.getNode('s1-title-2').opacity(intro.opacity).translateY(intro.translateY * 0.78);
ctx.getNode('s1-subtitle').opacity(intro.opacity).translateY(intro.translateY * 0.55);
ctx.getNode('s1-note').opacity(intro.opacity).translateY(26 * (1 - intro.opacity));
var chips = ctx.stagger(4, {
  from: { opacity: 0, translateY: 22, scale: 0.94 },
  to: { opacity: 1, translateY: 0, scale: 1 },
  gap: 3,
  duration: 16,
  easing: 'spring-default',
  clamp: true
});
['s1-chip-1', 's1-chip-2', 's1-chip-3', 's1-chip-4'].forEach(function(id, index) {
  ctx.getNode(id).opacity(chips[index].opacity).translateY(chips[index].translateY).scale(chips[index].scale);
});
[
  { id: 's1-block-a', from: { opacity: 0, translateX: 220, translateY: -160, rotate: 22, scale: 0.8 }, delay: 0 },
  { id: 's1-block-b', from: { opacity: 0, translateX: 260, translateY: 120, rotate: -20, scale: 0.82 }, delay: 3 },
  { id: 's1-block-c', from: { opacity: 0, translateX: -240, translateY: 110, rotate: 18, scale: 0.84 }, delay: 6 },
  { id: 's1-block-d', from: { opacity: 0, translateX: 160, translateY: 180, rotate: -24, scale: 0.8 }, delay: 9 },
  { id: 's1-block-e', from: { opacity: 0, translateX: -180, translateY: 150, rotate: 16, scale: 0.82 }, delay: 12 }
].forEach(function(item, index) {
  var anim = ctx.animate({
    from: item.from,
    to: { opacity: 0.86, translateX: 0, translateY: 0, rotate: 0, scale: 1 },
    duration: 18,
    delay: item.delay,
    easing: 'spring-wobbly',
    clamp: true
  });
  var tilt = Math.sin(ctx.currentFrame / 10 + index * 0.7) * 2.5;
  ctx.getNode(item.id).opacity(anim.opacity).translateX(anim.translateX).translateY(anim.translateY).rotate(anim.rotate + tilt).scale(anim.scale);
});
ctx.getNode('s1-copy').translateY(Math.sin(ctx.currentFrame / ctx.sceneFrames * Math.PI * 2) * 6);
