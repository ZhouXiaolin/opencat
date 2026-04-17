var intro = ctx.animate({
  from: { opacity: 0, translateY: 44 },
  to: { opacity: 1, translateY: 0 },
  duration: 20,
  easing: 'spring-gentle',
  clamp: true
});
ctx.getNode('s3-kicker').opacity(intro.opacity).translateY(intro.translateY);
ctx.getNode('s3-title').opacity(intro.opacity).translateY(intro.translateY * 0.82);
ctx.getNode('s3-subtitle').opacity(intro.opacity).translateY(intro.translateY * 0.58);
var metrics = ctx.stagger(3, {
  from: { opacity: 0, translateY: 26, scale: 0.96 },
  to: { opacity: 1, translateY: 0, scale: 1 },
  gap: 5,
  duration: 16,
  easing: 'spring-default',
  clamp: true
});
['s3-metric-1', 's3-metric-2', 's3-metric-3'].forEach(function(id, index) {
  ctx.getNode(id).opacity(metrics[index].opacity).translateY(metrics[index].translateY).scale(metrics[index].scale);
});
[
  { id: 's3-block-a', from: { opacity: 0, translateX: 180, translateY: -120, rotate: 18, scale: 0.82 }, delay: 2 },
  { id: 's3-block-b', from: { opacity: 0, translateX: 220, translateY: 100, rotate: -20, scale: 0.8 }, delay: 6 },
  { id: 's3-block-c', from: { opacity: 0, translateX: -180, translateY: 120, rotate: 16, scale: 0.82 }, delay: 10 }
].forEach(function(item, index) {
  var anim = ctx.animate({
    from: item.from,
    to: { opacity: 0.8, translateX: 0, translateY: 0, rotate: 0, scale: 1 },
    duration: 18,
    delay: item.delay,
    easing: 'spring-wobbly',
    clamp: true
  });
  var tilt = Math.sin(ctx.currentFrame / 13 + index * 0.7) * 1.8;
  ctx.getNode(item.id).opacity(anim.opacity).translateX(anim.translateX).translateY(anim.translateY).rotate(anim.rotate + tilt).scale(anim.scale);
});
