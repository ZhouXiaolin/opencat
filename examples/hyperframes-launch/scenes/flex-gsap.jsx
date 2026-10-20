import { useGSAP } from 'opencat/runtime';
/* ══ flex-gsap (master 19.01-19.68, compositions/flex-gsap.html) ════════
   15x25 dot matrix. Phase 1: per-column sweep with elastic snap-back.
   Phase 2: radial explosion (from 0/0 nominal — later items win the flush,
   matching GSAP's simultaneous-write behaviour). Master clips at 0.67s. */
var GS_START = 19.01, GS_DUR = 0.67, GS_ROWS = 15, GS_COLS = 25;

export default function FlexGsap() {
  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    var centerR = (GS_ROWS - 1) / 2, centerC = (GS_COLS - 1) / 2;
    var maxDist = Math.sqrt(centerR * centerR + centerC * centerC);

    for (var c = 0; c < GS_COLS; c++) {
      var colIds = [];
      for (var rr = 0; rr < GS_ROWS; rr++) colIds.push('fg-d' + rr + '-' + c);
      var colStart = c * 0.019;
      tl.fromTo(colIds, { x: 0, y: 0 }, { x: 8, y: -22, duration: 0.08, ease: 'power2.out', stagger: 0.004 }, A(colStart));
      tl.fromTo(colIds, { x: 8, y: -22 }, { x: 0, y: 0, duration: 0.28, ease: 'elastic.out(1, 0.3)', stagger: 0.004 }, A(colStart + 0.08));
    }
    for (var r = 0; r < GS_ROWS; r++) {
      for (var c2 = 0; c2 < GS_COLS; c2++) {
        var dr = r - centerR, dc = c2 - centerC;
        var dist = Math.sqrt(dr * dr + dc * dc);
        var normDist = dist / maxDist;
        var flyDist = 160 * (1 - normDist * 0.5);
        var angle = Math.atan2(dr, dc);
        tl.fromTo('fg-d' + r + '-' + c2, { x: 0, y: 0, scale: 1, opacity: 1 }, {
          x: Math.cos(angle) * flyDist,
          y: Math.sin(angle) * flyDist - 100,
          scale: 0.3, opacity: 0.2,
          duration: 0.18, ease: 'power3.out',
        }, A(0.48 + normDist * 0.08));
      }
    }
    tl.fromTo('fg-grid', { filter: 'blur(0px)' }, { filter: 'blur(20px)', duration: 0.18, ease: 'power2.in' }, A(0.52));
  });

  var rows = [];
  for (var r = 0; r < GS_ROWS; r++) rows.push(r);
  var cols = [];
  for (var c3 = 0; c3 < GS_COLS; c3++) cols.push(c3);
  return (
    <div id="fg-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#0e100f]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_50%_50%,#161816_0%,#0e100f_100%)]"></div>
      <div id="fg-grid" class="absolute left-[960px] top-[540px] ml-[-700px] mt-[-420px] flex flex-col">
        {rows.map((r) => (
          <div id={'fg-row' + r} class="flex">
            {cols.map((c) => (
              <div id={'fg-d' + r + '-' + c} class="w-[40px] h-[40px] m-[8px] rounded-full bg-[#fffce1]"></div>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}
