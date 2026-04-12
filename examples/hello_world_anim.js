(function() {
  var title = ctx.getNode("showcase-title");
  var subtitle = ctx.getNode("showcase-subtitle");

  var hero = ctx.animate({
    from: { opacity: 0, translateY: 40, scale: 0.95, rotate: -4 },
    to: { opacity: 1, translateY: 0, scale: 1, rotate: 0 },
    easing: 'spring-gentle',
  });

  title
    .opacity(hero.opacity)
    .translateY(hero.translateY)
    .scale(hero.scale)
    .rotate(hero.rotate);

  subtitle
    .opacity(Math.min(0.85, hero.opacity * 0.85))
    .translateY(hero.translateY * 0.6)
    .scale(0.98 + hero.scale * 0.03);
})();

(function() {
  var cards = [
    "card-play",
    "card-heart",
    "card-star",
    "card-badge",
    "card-bell",
    "card-shield",
  ];

  var anims = ctx.stagger(cards.length, {
    from: { opacity: 0, translateY: 30, scale: 0.9 },
    to: { opacity: 1, translateY: 0, scale: 1 },
    gap: 4,
    easing: { spring: { stiffness: 80, damping: 14, mass: 1 } },
  });

  cards.forEach(function(id, i) {
    ctx.getNode(id)
      .opacity(anims[i].opacity)
      .translateY(anims[i].translateY)
      .scale(anims[i].scale);
  });
})();

(function() {
  var icons = [
    "icon-play",
    "icon-heart",
    "icon-star",
    "icon-badge",
    "icon-bell",
    "icon-shield",
  ];
  var frame = ctx.frame;

  var entrance = ctx.stagger(icons.length, {
    from: { scale: 0.85, translateY: 18, rotate: -10 },
    to: { scale: 1, translateY: 0, rotate: 0 },
    gap: 4,
    easing: { spring: { stiffness: 120, damping: 12, mass: 0.9 } },
  });

  var cycleLen = 30;
  var totalCycle = icons.length * cycleLen;
  var cycleFrame = frame % totalCycle;
  var activeIndex = Math.floor(cycleFrame / cycleLen);
  var cycleStart = frame - (cycleFrame % cycleLen);

  icons.forEach(function(id, i) {
    var s = entrance[i].scale;
    var ty = entrance[i].translateY;
    var r = entrance[i].rotate;

    if (i === activeIndex) {
      var pulse = ctx.animate({
        from: { scale: 1, translateY: 0, rotate: 0 },
        to: { scale: 1.08, translateY: -6, rotate: 6 },
        duration: cycleLen,
        delay: cycleStart,
        easing: 'spring-wobbly',
      });
      s = pulse.scale;
      ty = pulse.translateY;
      r = pulse.rotate;
    }

    ctx.getNode(id)
      .scale(s)
      .translateY(ty)
      .rotate(r);
  });
})();

(function() {
  var ids = ["card-play-tag", "card-heart-tag"];
  var frame = ctx.frame;
  var focusIndex = Math.floor((frame % 120) / 60);
  var otherIndex = 1 - focusIndex;
  var cycleStart = Math.floor(frame / 60) * 60;

  var enter = ctx.animate({
    from: { opacity: 0, translateY: -12, scale: 0.9 },
    to: { opacity: 1, translateY: 0, scale: 1 },
    duration: 20,
    delay: cycleStart,
    easing: { spring: { stiffness: 90, damping: 14, mass: 1 } },
  });

  var leave = ctx.animate({
    from: { opacity: 1, translateY: 0, scale: 1 },
    to: { opacity: 0, translateY: 12, scale: 0.9 },
    duration: 20,
    delay: cycleStart,
    easing: { spring: { stiffness: 90, damping: 14, mass: 1 } },
  });

  ctx.getNode(ids[focusIndex])
    .opacity(enter.opacity)
    .translateY(enter.translateY)
    .scale(enter.scale);

  ctx.getNode(ids[otherIndex])
    .opacity(leave.opacity)
    .translateY(leave.translateY)
    .scale(leave.scale);
})();
