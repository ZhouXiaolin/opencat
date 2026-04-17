var intro = ctx.animate({
  from: { opacity: 0, translateY: 38 },
  to: { opacity: 1, translateY: 0 },
  duration: 18,
  easing: 'spring-gentle',
  clamp: true
});
ctx.getNode('s4-title').opacity(intro.opacity).translateY(intro.translateY);
ctx.getNode('s4-subtitle').opacity(intro.opacity).translateY(intro.translateY * 0.55);
var cards = ctx.stagger(4, {
  from: { opacity: 0, translateY: 26, translateX: 18, scale: 0.96 },
  to: { opacity: 1, translateY: 0, translateX: 0, scale: 1 },
  gap: 3,
  duration: 15,
  easing: 'spring-default',
  clamp: true
});
['s4-card-1', 's4-card-2', 's4-card-3', 's4-card-4'].forEach(function(id, index) {
  ctx.getNode(id).opacity(cards[index].opacity).translateY(cards[index].translateY).translateX(cards[index].translateX).scale(cards[index].scale);
});
[
  { id: 's4-block-a', from: { opacity: 0, translateX: -200, translateY: -140, rotate: -24, scale: 0.78 }, delay: 0 },
  { id: 's4-block-b', from: { opacity: 0, translateX: 220, translateY: -120, rotate: 20, scale: 0.8 }, delay: 3 },
  { id: 's4-block-c', from: { opacity: 0, translateX: -180, translateY: 150, rotate: 18, scale: 0.82 }, delay: 6 },
  { id: 's4-block-d', from: { opacity: 0, translateX: 180, translateY: 160, rotate: -18, scale: 0.82 }, delay: 9 }
].forEach(function(item, index) {
  var anim = ctx.animate({
    from: item.from,
    to: { opacity: 0.82, translateX: 0, translateY: 0, rotate: 0, scale: 1 },
    duration: 18,
    delay: item.delay,
    easing: 'spring-wobbly',
    clamp: true
  });
  var tilt = Math.sin(ctx.currentFrame / 12 + index * 0.5) * 1.9;
  ctx.getNode(item.id).opacity(anim.opacity).translateX(anim.translateX).translateY(anim.translateY).rotate(anim.rotate + tilt).scale(anim.scale);
});
