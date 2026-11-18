/* ══ glass-intro (master 0-18.24, glass-intro.html) ═════════════════════
   4 个 beat 作为 <timeline> 的直接子场景，各带绝对 data-start/data-duration。
   引擎按时间窗裁剪节点树（opencat 的树移除，非 hyperframes 的 opacity
   门控）：参考在 2.10 / 3.85 / 5.60 硬切。 */
import Beat1 from './beat1';
import Beat2 from './beat2';
import Beat3 from './beat3';
import Beat4 from './beat4';

export function GlassIntroBeats() {
  return [
    <Beat1 key="b1" duration={2.1} />,
    <Beat2 key="b2" duration={1.75} />,
    <Beat3 key="b3" duration={1.75} />,
    <Beat4 key="b4" duration={12.64} />,
  ];
}
