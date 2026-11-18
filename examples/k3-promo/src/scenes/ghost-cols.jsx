// ghost order-book columns（track 1，窗口 4.583→7.35s）—— Solid 标准写法移植。
// 旧 opencat/runtime 的 useGSAP/useEffect 改为：
//   - gsap 补间：useTween 把绝对时间 tween 挂到唯一注册的主时间轴
//     （注册序=树序=旧 hook 序）。
//   - CanvasKit 每帧绘制：onFrame 回调里直接画；CK 是引擎垫片预置的官方
//     CanvasKit 面（MakeCanvasSurface(id) 由引擎实现）。
// 显隐由场景根节点的时间窗协议接管（data-start/data-duration → 引擎窗外
// 整子树移除），不再用每帧 vis 门控。
import { gsap } from 'gsap';
import { CK, onFrame, useTween } from '../runtime';
import { StageCanvas, getK3Stream } from '../shared';

const WIN = { start: 4.583, end: 7.35 };

// gsap code：ghost 列整体淡入（power1.inOut，0.55s）。绝对时间定位，
// 主时间轴按绝对时间自然裁剪——不再需要旧 hook 里的窗口判断。
useTween(function (master) {
  master.set('#ghostinner', { opacity: 0 }, 4.583);
  master.to('#ghostinner', { opacity: 0.9, duration: 0.55, ease: 'power1.inOut' }, 4.683);
});

export default function GhostCols() {
  // 帧钩子注册点（组件体，树序=注册序）：窗口内每帧重画 42 列 ghost 单元格。
  onFrame(function (T) {
    if (T >= WIN.start && T < WIN.end) {
      var K3STREAM = getK3Stream();
      var surface = CK && CK.MakeCanvasSurface && CK.MakeCanvasSurface('ghostcols-canvas');
      var gcv = surface ? surface.getCanvas() : null;
      if (gcv) {
        gcv.clear();
        var gPaint = new CK.Paint();
        gPaint.setStyle(CK.PaintStyle.Fill);
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
    <section id="ghostcols" class="clip z-[1]" data-start="4.583" data-duration="2.767">
      <div id="ghostinner" class="absolute inset-0">
        <StageCanvas id="ghostcols-canvas" />
      </div>
    </section>
  );
}
