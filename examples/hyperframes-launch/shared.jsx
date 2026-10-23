/* hex '#rrggbb' -> [r,g,b] (canvas helpers) */
export function hexRgb(hx) {
  return [parseInt(hx.slice(1, 3), 16), parseInt(hx.slice(3, 5), 16), parseInt(hx.slice(5, 7), 16)];
}

/* ══ shared per-frame helpers (used by several scenes) ═════════════ */

/* ══ shared per-frame helpers ═══════════════════════════════════════════ */
export function vis(ctx, id, on) { document.getElementById(id).style.opacity = on ? 1 : 0; }
export function clamp(v, a, b) { return Math.max(a, Math.min(b, v)); }
/* Zoom containers: the reference scales each beat's card/wrap/player about
   the canvas centre (glass-intro.html:100,122-124,159 transform-origins all
   resolve to (960,540)). A full-stage wrapper keeps that origin while inner
   elements stay static — identical geometry, one tween instead of three. */

/* glass-intro.html:1011-1025 -> one bar per beat (the reference re-stages a
   single #player-bar; three copies keep the readouts/fills independent).
   Beat-2/3: left 210 w 1500 (track 1332px); beat-4: left 326.5 w 1267
   (track 1099px, glass-intro.html:722-728). */
export function PlayerBar({ id, left, w, track, time0 }) {
  return (
    <div id={id} className="absolute top-[930px] h-[28px] flex items-center gap-[18px] opacity-0" style={{ left: left, width: w }}>
      <div id={id + '-icon'} className="relative w-[22px] h-[22px] shrink-0">
        <path viewBox="0 0 22 22" d="M5 3.5 L18 11 L5 18.5 Z" className="absolute left-0 top-0 w-[22px] h-[22px] fill-[rgba(255,255,255,0.92)]" />
      </div>
      <div id={id + '-track'} className="relative h-[3px] rounded-[2px] bg-[rgba(255,255,255,0.28)]" style={{ width: track }}>
        <div id={id + '-fill'} className="absolute left-0 top-0 w-0 h-full rounded-[2px] bg-white"></div>
        <div id={id + '-scrub'} className="absolute left-[-6px] top-[-4.5px] w-[12px] h-[12px] rounded-full bg-white shadow-[0_2px_6px_rgba(0,0,0,0.25)]"></div>
      </div>
      <text id={id + '-time'} className="w-[110px] text-right text-[20px] leading-none tracking-[0.02em] opacity-[0.85] font-[hf-inter-med] text-[rgba(255,255,255,0.92)]">{time0}</text>
    </div>
  );
}

/* entryEase (glass-intro.html:464) — quadratic matched-velocity ease with a
   non-standard shape, so the reference hands GSAP the function itself. */
export function entryEase(t) { return -0.806 * t * t + 1.806 * t; }

/* Blur+scale stay on the beat wrapper, matching what the shipped r5 render
   measured best. Splitting the blur onto the card (#frame) and the video wrap
   separately (the reference's own structure) regressed beat 2-3 by ~0.09
   avg_p8: the card's own backdrop-filter already blurs the stage behind it,
   so an extra ancestor blur double-counts. zoomBlur keeps the call sites
   readable. */
export function zoomBlur(tl, ids, from, to, duration, ease, at) {
  for (var i = 0; i < ids.length; i++) {
    tl.fromTo(ids[i], { filter: from }, { filter: to, duration: duration, ease: ease }, at);
  }
}
