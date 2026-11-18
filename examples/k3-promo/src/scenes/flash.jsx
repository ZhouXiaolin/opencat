/* inversion flash (track 8, window 7.333→7.391s).
   Blend is wired on the node background rect; must stay at opacity 1.0 in
   the window — any opacity <1 wraps the node in an isolated SaveLayer and
   the difference degrades to a plain flash. z=5 (below grade-vig z=6) is
   MEASURED: the vignette multiplies the flash output. 显隐由窗口协议接管
   （data-start/duration → 引擎窗外树移除），不再用每帧 vis 门控。 */

export default function Flash() {
  return (
    <section id="flash" class="clip z-[5]" data-start="7.3333" data-duration="0.058">
      <div id="flashwhite" class="absolute inset-0 bg-[#fbfbfd] [mix-blend-mode:difference]"></div>
    </section>
  );
}
