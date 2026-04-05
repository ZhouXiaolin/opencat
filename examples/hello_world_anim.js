(function() {
  const frame = ctx.frame;
  const totalFrames = ctx.totalFrames;

  const opacity = Math.min(frame / 20, 1);
  const rotation = Math.min(Math.max((frame - 20) * 1.8, 0), 24);
  const blueProgress = Math.min(frame / 50, 1);
  const blueTranslate = 180 * blueProgress;
  const blueScale = 0.8 + blueProgress * 0.7;
  const pinkTranslate = 140 + Math.min(frame / 45, 1) * 40;
  const pinkScale = 1 + Math.min(frame / 35, 1) * 0.35;
  const labelOffset = Math.min(Math.max((frame - 10) / 25, 0), 1) * 36;

  set_translate_x("2", blueTranslate);
  set_scale("2", blueScale);

  set_scale("3", pinkScale);
  set_translate_x("3", pinkTranslate);

  set_opacity("4", opacity);
  set_rotate("4", rotation);
  set_scale("4", 1 + opacity * 0.05);

  set_translate_x("5", -labelOffset);
  set_opacity("5", Math.min(frame / 24, 1));

  set_translate_x("6", labelOffset);
  set_opacity("6", Math.min(frame / 28, 1));
})();
