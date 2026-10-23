/* ══ glass-intro (master 0-18.24, glass-intro.html) ═════════════════════

   The reference is one flat 18.24s stage: 4 beats that tile end to end, plus
   4 `.clip` overlay layers.

     beat         master window     length   reference layer
     text-intro   0.00 -  2.10       2.10   .clip data-start=0     duration=2.1
     figma card   2.10 -  3.85       1.75   GSAP opacity on #frame
     spotify      3.85 -  5.60       1.75   GSAP opacity on #frame-2
     beat4        5.60 - 18.24      12.64   GSAP opacity on #frame-3
     cursor       6.40 - 17.30              .clip data-start=6.4   duration=10.9
     ripple       6.80 - 15.70              .clip data-start=6.8   duration=8.9
     drag-overlay 16.14 - 18.24             .clip data-start=16.14 duration=2.1
     text-ask     16.74 - 18.24             .clip data-start=16.74 duration=1.5

   Every overlay window starts at or after 5.60, so all four live INSIDE beat4.
   The whole composition is therefore exactly 4 scenes and no transitions — the
   reference hard-cuts at 2.10 / 3.85 / 5.60, and adjacent scenes without a
   `<transition>` cut straight from one to the next.

   What this buys: a beat is no longer parsed, laid out and then hidden with a
   per-frame `vis()` gate. Only the beat whose window holds the playhead is
   resolved at all. The `vis()` calls that remain are the beat-5 typewriter,
   which is an intra-scene reveal, not a scene window.

   NOTE on `data-start`: inside a scene a video's `data-start` is SCENE-RELATIVE
   (the resolver adds the scene's own start to it), so every video reads 0 — and
   the times below are beat-local, not master-absolute. */

import Beat1 from './beat1.jsx';
import Beat2 from './beat2.jsx';
import Beat3 from './beat3.jsx';
import Beat4 from './beat4.jsx';

export default function GlassIntro() {
  return (
    <div id="gi-root" className="absolute inset-0 w-[1920px] h-[1080px]">
      <tl id="gi-tl" className="absolute inset-0">
        <Beat1 duration={2.1} />
        <Beat2 duration={1.75} />
        <Beat3 duration={1.75} />
        <Beat4 duration={12.64} />
      </tl>
    </div>
  );
}
