(function() {
  var hero = ctx.fromTo('showcase-title', {
    opacity: 0,
    y: 40,
    scale: 0.95,
    rotate: -4,
  }, {
    opacity: 1,
    y: 0,
    scale: 1,
    rotate: 0,
    ease: 'spring.gentle',
  });

  // Linked motion: subtitle derives from hero values.
  ctx.getNode('showcase-subtitle')
    .opacity(Math.min(0.85, hero.opacity * 0.85))
    .translateY(hero.y * 0.6)
    .scale(0.98 + hero.scale * 0.03);
})();

(function() {
  ctx.fromTo(
    ['card-play', 'card-heart', 'card-star', 'card-badge', 'card-bell', 'card-shield'],
    { opacity: 0, y: 30, scale: 0.9 },
    {
      opacity: 1,
      y: 0,
      scale: 1,
      stagger: 4,
      ease: { spring: { stiffness: 80, damping: 14, mass: 1 } },
    }
  );
})();

(function() {
  var icons = [
    'icon-play',
    'icon-heart',
    'icon-star',
    'icon-badge',
    'icon-bell',
    'icon-shield',
  ];
  var frame = ctx.frame;

  var entrance = ctx.fromTo(icons, {
    scale: 0.85,
    y: 18,
    rotate: -10,
  }, {
    scale: 1,
    y: 0,
    rotate: 0,
    stagger: 4,
    ease: { spring: { stiffness: 120, damping: 12, mass: 0.9 } },
  });

  var cycleLen = 30;
  var totalCycle = icons.length * cycleLen;
  var cycleFrame = frame % totalCycle;
  var activeIndex = Math.floor(cycleFrame / cycleLen);
  var cycleStart = frame - (cycleFrame % cycleLen);

  icons.forEach(function(id, i) {
    var s = entrance[i].scale;
    var ty = entrance[i].y;
    var r = entrance[i].rotate;

    if (i === activeIndex) {
      var pulse = ctx.fromTo(id, {
        scale: 1,
        y: 0,
        rotate: 0,
      }, {
        scale: 1.08,
        y: -6,
        rotate: 6,
        duration: cycleLen,
        delay: cycleStart,
        ease: 'spring.wobbly',
      });
      s = pulse.scale;
      ty = pulse.y;
      r = pulse.rotate;
    }

    ctx.getNode(id)
      .scale(s)
      .translateY(ty)
      .rotate(r);
  });
})();

(function() {
  var ids = ['card-play-tag', 'card-heart-tag'];
  var frame = ctx.frame;
  var focusIndex = Math.floor((frame % 120) / 60);
  var otherIndex = 1 - focusIndex;
  var cycleStart = Math.floor(frame / 60) * 60;

  var enter = ctx.fromTo(ids[focusIndex], {
    opacity: 0,
    y: -12,
    scale: 0.9,
  }, {
    opacity: 1,
    y: 0,
    scale: 1,
    duration: 20,
    delay: cycleStart,
    ease: { spring: { stiffness: 90, damping: 14, mass: 1 } },
  });

  var leave = ctx.fromTo(ids[otherIndex], {
    opacity: 1,
    y: 0,
    scale: 1,
  }, {
    opacity: 0,
    y: 12,
    scale: 0.9,
    duration: 20,
    delay: cycleStart,
    ease: { spring: { stiffness: 90, damping: 14, mass: 1 } },
  });

  ctx.getNode(ids[focusIndex])
    .opacity(enter.opacity)
    .translateY(enter.y)
    .scale(enter.scale);

  ctx.getNode(ids[otherIndex])
    .opacity(leave.opacity)
    .translateY(leave.y)
    .scale(leave.scale);
})();
