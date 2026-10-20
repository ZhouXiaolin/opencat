import { useGSAP, useThree, useRef } from 'opencat/runtime';
import { clamp, hexRgb } from '../shared.jsx';

/* ══ flex-threejs (master 22.01-23.22, compositions/flex-threejs.html) ══
   3D torus via ctx.three (opencat-three / wgpu). The reference is pure CSS 3D
   (no three.js library): .three-stage has perspective:900px +
   perspective-origin:50% 50%, so CSS (x,y,z) -> (960 + x*900/(900-z),
   540 + y*900/(900-z)). A three.js PerspectiveCamera at z=near+900 with
   fov=2*atan(540/900) inverts that exactly. Segment bases: centre
   (R cos a,0,R sin a), local +x = inward radial, +y = up. form: rotY
   -30->310 sine.inOut, rotX 25->10 yoyo x2. Shadow + label stay DOM. */
var FT_START = 22.01, FT_DUR = 1.21;
var FT_COLORS = [
  ['#dbb0b0', '#c49090'], ['#c8b8a8', '#b0a090'], ['#a8c0a0', '#8ea888'], ['#c4b098', '#ae9a82'],
  ['#d4a0a0', '#be8a8a'], ['#b8c8b0', '#98b088'], ['#d0c0b0', '#baa898'], ['#c0a8b8', '#a890a0'],
  ['#dbb0b0', '#c49090'], ['#c8b8a8', '#b0a090'], ['#a8c0a0', '#8ea888'], ['#c4b098', '#ae9a82'],
  ['#d4a0a0', '#be8a8a'], ['#b8c8b0', '#98b088'], ['#d0c0b0', '#baa898'], ['#c0a8b8', '#a890a0'],
];

export default function FlexThreejs() {
  const canvas = useRef();

  useGSAP((ctx) => {
    var T = ctx.current.time;
    var tl = ctx.timeline(), A = function (t) { return t; };
    tl.fromTo('ft-shadow', { scaleX: 1, scaleY: 1 }, { scaleX: 1.3, scaleY: 0.7, duration: 0.4, ease: 'sine.inOut', yoyo: true, repeat: 2 }, A(0));
    tl.fromTo('ft-label', { opacity: 0, y: 10 }, { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' }, A(0.2));
  });

  /* Real 3D via ctx.three (opencat-three / wgpu). The reference is pure CSS 3D
     (no three.js library): .three-stage has perspective:900px with
     perspective-origin:50% 50%, so a CSS point (x,y,z) projects to
     (960 + x*900/(900-z), 540 + y*900/(900-z)). A three.js PerspectiveCamera
     at z = near + 900 with fov = 2*atan(540/900) inverts that exactly
     (screen_x = 960 + x, screen_y = 540 - y on the z=0 plane). The world Y
     axis points up, so every CSS y offset is negated and the form's CSS
     rotateX negates too; the form's rotateY is unchanged. Each torus segment
     is a PlaneGeometry (segment-local +x = inward radial, +y = up) whose
     ShaderMaterial draws the rounded-rect mask + 135deg two-stop ramp. */
  useThree(canvas, (T3, node, t, ctx) => {
    var T = t;

    var scene = new T3.Scene();
    var camera = new T3.PerspectiveCamera(61.9275, 1920 / 1080, 0.1, 5000);
    camera.position.set(0, 0, 900.1);
    camera.lookAt(0, 0, 0);
    scene.add(new T3.AmbientLight(0xffffff, 1.0));
    scene.add(new T3.HemisphereLight(0xffffff, 0xffffff, 0.5));

    function face(col, half, rad, op) {
      var c1 = hexRgb(col[0]), c2 = hexRgb(col[1]);
      var mat = new T3.ShaderMaterial({
        // Every blade carries an antialiased clip edge, so all of them need the
        // src-alpha list: the opaque list replaces the destination and would
        // make a front blade's partial edge reveal the background instead of
        // the blade behind it. The transparent list is sorted far-to-near,
        // which is the painter's order CSS 3D uses for these planes.
        transparent: true,
        uniforms: {
          c1: { value: [c1[0] / 255, c1[1] / 255, c1[2] / 255] },
          c2: { value: [c2[0] / 255, c2[1] / 255, c2[2] / 255] },
          half: { value: half },
          rad: { value: rad },
          op: { value: op },
        },
        vertexShader: [
          'varying vec2 vUv;',
          'void main() {',
          '  vUv = uv;',
          '  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);',
          '}',
        ].join('\n'),
        fragmentShader: [
          'precision highp float;',
          'varying vec2 vUv;',
          'uniform vec3 c1;',
          'uniform vec3 c2;',
          'uniform float half;',
          'uniform float rad;',
          'uniform float op;',
          'void main() {',
          '  vec2 p = vUv * (2.0 * half);',
          // CSS `linear-gradient(135deg, c1, c2)`: for a square box the 135deg
          // gradient line normalizes to t = (x + y) / (W + H) with y measured
          // DOWNWARD from the box top. `vUv.y` grows upward, so the top-edge
          // y has to be flipped or the ramp runs along the opposite diagonal.
          '  float gy = 2.0 * half - p.y;',
          '  vec3 col = mix(c1, c2, (p.x + gy) / (4.0 * half));',
          '  vec2 q = abs(p - vec2(half)) - (half - rad);',
          '  float d = length(max(q, 0.0)) + min(max(q.x, q.y), 0.0) - rad;',
          // CSS clips a border-radius box: pixels outside the rounded rect do
          // not exist, and the clip edge is antialiased. Writing vec4(0) there
          // instead would keep the quad's square footprint in the depth buffer
          // and punch a background-coloured hole wherever one blade's corner
          // overlies another, so out-of-shape pixels are discarded outright and
          // the one-pixel band on the edge fades by distance-derivative
          // coverage.
          '  float cov = clamp(0.5 - d / max(fwidth(d), 1e-5), 0.0, 1.0);',
          '  if (cov <= 0.0) discard;',
          '  gl_FragColor = vec4(col, op * cov);',
          '}',
        ].join('\n'),
      });
      return new T3.Mesh(new T3.PlaneGeometry(2 * half, 2 * half), mat);
    }

    var SEGS = 16, RING_R = 400;
    var formX = new T3.Group();
    /* CSS 的 .torus-seg 无 left/top，margin −87 把 175px 盒子推到
       (−87,−87)，transform-origin（盒子中心）落在 form 本地 (0.5,0.5)——
       CSS y 向下，three y 向上即 (0.5,−0.5)。内圈 translateY(62) 叠加
       margin −62 的同一半像素 → y = −62.5。少这半像素，整环对参考呈
       系统性亚像素错位。 */
    for (var i = 0; i < SEGS; i++) {
      var a = (i / SEGS) * Math.PI * 2;
      var m = face(FT_COLORS[i], 87.5, 40, 1);
      m.position.set(Math.cos(a) * RING_R + 0.5, -0.5, Math.sin(a) * RING_R);
      m.rotation.y = -a + Math.PI;
      formX.add(m);
    }
    for (var j = 0; j < SEGS; j++) {
      var a2 = ((j + 0.5) / SEGS) * Math.PI * 2;
      var m2 = face(FT_COLORS[(j + 4) % SEGS], 62.5, 30, 0.85);
      m2.position.set(Math.cos(a2) * RING_R * 0.7 + 0.5, -62.5, Math.sin(a2) * RING_R * 0.7);
      m2.rotation.y = -a2 + Math.PI;
      formX.add(m2);
    }
    var formY = new T3.Group();
    formY.add(formX);
    /* The CSS stage (top:42%, width/height:0, perspective-origin:50% 50%)
       fixes the vanishing point at screen (960, 453.6) — 86.4px above the
       canvas centre. A world-space lift is NOT equivalent: the perspective
       divide scales it by 900/(900−z), which rubber-sheets the form by
       ±38px across its depth range. Keep the form on the camera axis and
       shift the whole rendered image up instead (see the canvas offset in
       the markup) — a principal-point shift, which is constant per pixel. */
    formY.position.set(0, 0, 0);
    scene.add(formY);

    /* GSAP form rotation: Ry(-30 -> 310) sine.inOut over 1.2s, Rx(25 -> 10)
       sine.inOut yoyo x2 over 0.4s each. */
    var ph = T / 0.4, k = Math.floor(ph), fr = ph - k;
    var sio = 0.5 - 0.5 * Math.cos(Math.PI * fr);
    var s = (k % 2 === 0) ? sio : 1 - sio;
    formY.rotation.y = (-30 + 340 * (0.5 - 0.5 * Math.cos(Math.PI * clamp(T / 1.2, 0, 1)))) * Math.PI / 180;
    formX.rotation.x = -(25 + (10 - 25) * s) * Math.PI / 180;

    new T3.Renderer('ft-canvas', 1920, 1080).render(scene, camera);
  });

  return (
    <div id="ft-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#f0ebe3]">
      <div class="absolute inset-0 bg-[radial-gradient(ellipse_at_40%_35%,rgba(255,245,230,0.6)_0%,transparent_50%),radial-gradient(ellipse_at_50%_50%,#f0ebe3_0%,#e4ddd2_100%)]"></div>
      <div class="absolute left-[672px] top-[162px] w-[600px] h-[600px] bg-[radial-gradient(circle,rgba(255,235,200,0.25)_0%,transparent_60%)]"></div>
      <div id="ft-shadow" class="absolute left-[780px] top-[680px] w-[360px] h-[50px] rounded-full bg-[radial-gradient(ellipse,rgba(80,60,40,0.18)_0%,transparent_70%)]"></div>
      <canvas id="ft-canvas" ref={canvas} class="absolute left-0 top-0 w-[1920px] h-[1080px]" style="top:-86.4px" />
      <div class="absolute left-0 bottom-[140px] w-full flex justify-center">
        <text id="ft-label" class="text-[64px] leading-none tracking-[0.02em] italic font-medium font-[hf-cg-it] text-[#5a4a3a] opacity-0">Three.js</text>
      </div>
    </div>
  );
}
