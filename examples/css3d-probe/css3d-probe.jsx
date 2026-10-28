/* CSS 3D 投影复现：web 侧 torus 环整层缺失的最小用例。
   四个象限各测一层能力：
   Q1 2D rotate 对照（两侧都必须正常）。
   Q2 rotateY(60deg) 无 perspective——纯仿射，应退化为水平压缩。
   Q3 translateZ(200px) + 父 perspective:900——应放大 900/(900-200)≈1.286。
   Q4 flex-threejs torus 微缩复刻：0x0 stage(perspective) + preserve-3d
      form + 16 段 rotateY/translateZ/rotateY + gsap 驱动 form 旋转。 */
export const meta = { width: 1920, height: 1080, fps: 30, duration: 2 };

import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';

var P_COLORS = ['#dbb0b0', '#c8b8a8', '#a8c0a0', '#c4b098', '#d4a0a0', '#b8c8b0', '#d0c0b0', '#c0a8b8'];
var P_SEGS = 8, P_RING_R = 260;

function pSegments() {
  var out = [];
  for (var i = 0; i < P_SEGS; i++) {
    var a = (i / P_SEGS) * Math.PI * 2;
    out.push({
      key: 'o' + i,
      style: {
        position: 'absolute', width: 130, height: 130, borderRadius: 30,
        marginLeft: -65, marginTop: -65,
        transform: 'rotateY(' + (-(a * 180 / Math.PI) + 90) + 'deg) translateZ(' + P_RING_R + 'px) rotateY(90deg)',
        backgroundImage: 'linear-gradient(135deg, ' + P_COLORS[i] + ', #7a6a5a)',
      },
    });
  }
  return out;
}
var P_SEGMENT_LIST = pSegments();

export default function Css3dProbe() {
  useGSAP(() => {
    var tl = gsap.timeline();
    tl.fromTo('p4-form', { rotationY: -30 }, { rotationY: 330, duration: 2, ease: 'sine.inOut' }, 0);
    tl.fromTo('p4-form', { rotationX: 25 }, { rotationX: 10, duration: 0.66, ease: 'sine.inOut', yoyo: true, repeat: 2 }, 0);
  });
  return (
    <div id="root" className="relative w-[1920px] h-[1080px] bg-[#f0ebe3]">
      <p className="absolute left-[40px] top-[20px] text-[36px] text-[#333]">Q1 2D rotate</p>
      <div id="q1" className="absolute left-[360px] top-[270px] w-[300px] h-[300px] rounded-[40px] bg-[linear-gradient(135deg,#dbb0b0,#7a6a5a)]" style={{ transform: 'rotate(15deg)' }}></div>

      <p className="absolute left-[1000px] top-[20px] text-[36px] text-[#333]">Q2 rotateY no perspective</p>
      <div id="q2" className="absolute left-[1320px] top-[270px] w-[300px] h-[300px] rounded-[40px] bg-[linear-gradient(135deg,#a8c0a0,#7a6a5a)]" style={{ transform: 'rotateY(60deg)' }}></div>

      <p className="absolute left-[40px] top-[560px] text-[36px] text-[#333]">Q3 translateZ in perspective</p>
      <div style={{ position: 'absolute', left: 360, top: 810, width: 0, height: 0, perspective: 900 }}>
        <div id="q3" style={{ position: 'absolute', width: 300, height: 300, borderRadius: 40, marginLeft: -150, marginTop: -150, transform: 'translateZ(200px)', backgroundImage: 'linear-gradient(135deg, #c4b098, #7a6a5a)' }}></div>
      </div>

      <p className="absolute left-[1000px] top-[560px] text-[36px] text-[#333]">Q4 torus replica</p>
      <div id="p4-stage" style={{ position: 'absolute', left: 1320, top: 810, width: 0, height: 0, perspective: 900 }}>
        <div id="p4-form" style={{ position: 'relative', width: 0, height: 0, transformStyle: 'preserve-3d' }}>
          {P_SEGMENT_LIST.map((s) => <div id={'p4-' + s.key} style={s.style}></div>)}
        </div>
      </div>
    </div>
  );
}

export function main() {
  return <Css3dProbe />;
}
