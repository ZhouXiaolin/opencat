(function() {
  const frame = ctx.frame;
  const totalFrames = ctx.totalFrames;

  const blueBox = ctx.getNode("2");
  const pinkBox = ctx.getNode("3");
  const mainText = ctx.getNode("4");
  const blueLabel = ctx.getNode("5");
  const pinkLabel = ctx.getNode("6");

  const opacity = Math.min(frame / 20, 1);
  const rotation = Math.min(Math.max((frame - 20) * 1.8, 0), 24);
  const blueProgress = Math.min(frame / 50, 1);
  const blueTranslate = 180 * blueProgress;
  const blueScale = 0.8 + blueProgress * 0.7;
  const pinkTranslate = 140 + Math.min(frame / 45, 1) * 40;
  const pinkScale = 1 + Math.min(frame / 35, 1) * 0.35;
  const labelOffset = Math.min(Math.max((frame - 10) / 25, 0), 1) * 36;

  blueBox
    .translateX(blueTranslate)
    .scale(blueScale);

  pinkBox
    .scale(pinkScale)
    .translateX(pinkTranslate);

  mainText
    .opacity(opacity)
    .rotate(rotation)
    .scale(1 + opacity * 0.05);

  blueLabel
    .translateX(-labelOffset)
    .opacity(Math.min(frame / 24, 1));

  pinkLabel
    .translateX(labelOffset)
    .opacity(Math.min(frame / 28, 1));
})();
