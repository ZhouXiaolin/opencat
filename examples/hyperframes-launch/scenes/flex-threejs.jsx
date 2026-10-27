import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';

/* ══ flex-threejs (master 22.01-23.22, compositions/flex-threejs.html) ══
   Pure CSS 3D, same structure as the reference: .three-stage is a 0x0 box at
   (960, 453.6) carrying perspective:900px, .three-form is a 0x0
   transform-style:preserve-3d box inside it, and the 32 torus segments are
   absolutely positioned boxes whose own transform puts them on the ring.
   GSAP drives only the form's rotationY/rotationX; the shadow and the label
   stay in 2D DOM. */
var FT_START = 22.01, FT_DUR = 1.21;
var FT_COLORS = [
  ['#dbb0b0', '#c49090'], ['#c8b8a8', '#b0a090'], ['#a8c0a0', '#8ea888'], ['#c4b098', '#ae9a82'],
  ['#d4a0a0', '#be8a8a'], ['#b8c8b0', '#98b088'], ['#d0c0b0', '#baa898'], ['#c0a8b8', '#a890a0'],
  ['#dbb0b0', '#c49090'], ['#c8b8a8', '#b0a090'], ['#a8c0a0', '#8ea888'], ['#c4b098', '#ae9a82'],
  ['#d4a0a0', '#be8a8a'], ['#b8c8b0', '#98b088'], ['#d0c0b0', '#baa898'], ['#c0a8b8', '#a890a0'],
];
var FT_SEGS = 16, FT_RING_R = 400;

/* Segment bases: centre (R cos a, 0, R sin a), local +x = inward radial,
   +y = up, so each blade is turned to face outward and then stood up with a
   trailing rotateY(90deg). The reference uses margin -87 for a 175px box
   (and -62 for 125px), which leaves the box centre at (+0.5,+0.5) rather
   than exactly on the form origin; keeping the half pixel keeps the whole
   ring from drifting a sub-pixel against the reference raster. */
function ftSegments() {
  var out = [], i, a, j;
  for (i = 0; i < FT_SEGS; i++) {
    a = (i / FT_SEGS) * Math.PI * 2;
    out.push({
      key: 'o' + i,
      style: {
        position: 'absolute', width: 175, height: 175, borderRadius: 40,
        marginLeft: -87, marginTop: -87,
        transform: 'rotateY(' + (-(a * 180 / Math.PI) + 90) + 'deg) translateZ(' + FT_RING_R + 'px) rotateY(90deg)',
        backgroundImage: 'linear-gradient(135deg, ' + FT_COLORS[i][0] + ', ' + FT_COLORS[i][1] + ')',
      },
    });
  }
  for (j = 0; j < FT_SEGS; j++) {
    a = ((j + 0.5) / FT_SEGS) * Math.PI * 2;
    out.push({
      key: 'i' + j,
      style: {
        position: 'absolute', width: 125, height: 125, borderRadius: 30, opacity: 0.85,
        marginLeft: -62, marginTop: -62,
        transform: 'rotateY(' + (-(a * 180 / Math.PI) + 90) + 'deg) translateZ(' + (FT_RING_R * 0.7) + 'px) rotateY(90deg) translateY(62px)',
        backgroundImage: 'linear-gradient(135deg, ' + FT_COLORS[(j + 4) % FT_SEGS][0] + ', ' + FT_COLORS[(j + 4) % FT_SEGS][1] + ')',
      },
    });
  }
  return out;
}
var FT_SEGMENTS = ftSegments();

export default function FlexThreejs() {
  useGSAP(() => {
    var T = ctx.current.time;
    var tl = gsap.timeline(), A = function (t) { return t; };
    /* The reference seeds the form with gsap.set({rotationY:-30, rotationX:25})
       and then tweens each property on its own; fromTo is the same thing here
       because both tweens render their start value at t=0 and touch disjoint
       fields of the form's single rotate3 transform. */
    tl.fromTo('ft-form', { rotationY: -30 }, { rotationY: 310, duration: 1.2, ease: 'sine.inOut' }, A(0));
    tl.fromTo('ft-form', { rotationX: 25 }, { rotationX: 10, duration: 0.4, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0));
    tl.fromTo('ft-shadow', { scaleX: 1, scaleY: 1 }, { scaleX: 1.3, scaleY: 0.7, duration: 0.4, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0));
    tl.fromTo('ft-label', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, A(0.2));
  });

  return (
    <div id="ft-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f0ebe3]">
      <div className="absolute inset-0 bg-[radial-gradient(ellipse_at_40%_35%,rgba(255,245,230,0.6)_0%,transparent_50%),radial-gradient(ellipse_at_50%_50%,#f0ebe3_0%,#e4ddd2_100%)]"></div>
      <div className="absolute left-[672px] top-[162px] w-[600px] h-[600px] bg-[radial-gradient(circle,rgba(255,235,200,0.25)_0%,transparent_60%)]"></div>
      <div id="ft-shadow" className="absolute left-[780px] top-[680px] w-[360px] h-[50px] rounded-full bg-[radial-gradient(ellipse,rgba(80,60,40,0.18)_0%,transparent_70%)]"></div>
      {/* perspective-origin:50% 50% of a 0x0 box is that box's own corner, so
          the vanishing point sits exactly at (960, 453.6). */}
      <div id="ft-stage" className="absolute" style={{ left: 960, top: 453.6, width: 0, height: 0, perspective: 900 }}>
        <div id="ft-form" className="relative" style={{ width: 0, height: 0, transformStyle: 'preserve-3d' }}>
          {FT_SEGMENTS.map((s) => <div id={'ft-' + s.key} className="torus-seg" style={s.style}></div>)}
        </div>
      </div>
      <div className="absolute left-0 bottom-[140px] w-full flex justify-center">
        <p id="ft-label" className="text-[64px] leading-none tracking-[0.02em] italic font-medium font-[hf-cg-it] text-[#5a4a3a] opacity-0">Three.js</p>
      </div>
    </div>
  );
}
