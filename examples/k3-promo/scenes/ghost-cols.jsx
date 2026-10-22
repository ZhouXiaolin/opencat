import { gsap } from 'gsap';
import CanvasKitInit from 'canvaskit-wasm';
import { useFrame, useGSAP } from 'opencat/runtime';
import { StageCanvas, getK3Stream, vis } from './shared.jsx';


/* ghost order-book columns (track 1, window 4.583→7.35s) */
export default function GhostCols() {
  let CK = null;
  CanvasKitInit({}).then((ck) => { CK = ck; });
  useGSAP((ctx, t) => {
    var T = t;
    if (T >= 4.583 && T < 7.35) {
      gsap.timeline()
        .set('ghostinner', { opacity: 0 }, 4.583)
        .to('ghostinner', { opacity: 0.9, duration: 0.55, ease: 'power1.inOut' }, 4.683);
    }
  });
  useFrame((ctx, t) => {
    if (!CK) return;
    var T = t;
    var G_ON = (T >= 4.583 && T < 7.35);
    vis(ctx, 'ghostcols', G_ON);
    if (G_ON) {
      var K3STREAM = getK3Stream();
      var gcv = CK.MakeCanvasSurface('ghostcols-canvas').getCanvas();
      if (gcv) {
        gcv.clear();
        var gPaint = new CK.Paint(); gPaint.setStyle(CK.PaintStyle.Fill);
        var GHOST_COLS = K3STREAM.ghost;
        for (var c = 0; c < GHOST_COLS.length; c++) {
          var colOp = GHOST_COLS[c][0], gcells = GHOST_COLS[c][1];
          for (var gi = 0; gi < gcells.length; gi += 3) {
            var gy = gcells[gi], gh = gcells[gi + 1], gop = gcells[gi + 2];
            gPaint.setColor(CK.Color(16, 16, 16, colOp * gop));
            gcv.drawRRect(CK.RRectXY([c * 46, gy, c * 46 + 30, gy + gh], 3, 3), gPaint);
          }
        }
      }
    }
  });
  return (
    <div id="ghostcols" class="absolute inset-0 z-[1] opacity-0">
      <div id="ghostinner" class="absolute inset-0">
        <StageCanvas id="ghostcols-canvas" />
      </div>
    </div>
  );
}
