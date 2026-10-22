/* inversion flash (track 8, window 7.333→7.391s).
   Blend is wired on the node background rect; must stay at opacity 1.0 in
   the window — any opacity <1 wraps the node in an isolated SaveLayer and
   the difference degrades to a plain flash. z=5 (below grade-vig z=6) is
   MEASURED: the vignette multiplies the flash output. */

import { useEffect } from 'opencat/runtime';
import { vis } from './shared.jsx';

export default function Flash() {
  useEffect(() => {
    var t = ctx.current.time;
    var T = t;
    vis(ctx, 'flash', T >= 7.3333 && T < 7.391);
  });
  return <div id="flash" class="absolute inset-0 z-[5] bg-[#fbfbfd] opacity-0 [mix-blend-mode:difference]"></div>;
}
