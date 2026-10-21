import { gsap } from 'gsap';
import { useGSAP, useCanvas, useRef } from 'opencat/runtime';
import { clamp } from '../shared.jsx';

/* ══ flex-shader (master 20.81-22.01, compositions/flex-shader.html) ════
   WebGL domain-warp fbm shader -> ly.pixel lambda, 1:1 with the reference's
   four functions (hash/noise/fbm/palette). The DSL supports top-level
   `const f = (p) => ...` user functions and bounded `for` loops, so the
   octave loop is no longer unrolled by hand. R is GLSL's column-major
   mat2(0.8,0.6,-0.6,0.8). u_time ramps 0.5->1.44 over the 0.94s beat, then
   the frame holds (master slot is 1.20s). */
var FS_START = 20.81, FS_DUR = 1.20, FS_ANIM = 0.94;

export default function FlexShader() {
  const canvas = useRef();

  useGSAP(() => {
    var tl = gsap.timeline();
    tl.fromTo('fs-label', { opacity: 0 }, { opacity: 0.35, duration: 0.3, ease: 'power2.out' }, 0.15);
  });

  /* module-level kernel: source compiled once by Rust, sampled per frame.
     The canvas draw goes through useCanvas (ref-resolved, augmented handle),
     not useGSAP + getCanvasById. */
  useCanvas(canvas, (c, t, ctx) => {
    if (!FlexShader._kern) {
      FlexShader._kern = ly.pixel((uv, u) => {
        const ux = uv.x / 1080;
        /* GLSL v_uv.y=1 is the CANVAS TOP; ly.pixel uv.y grows downward,
           so the reference's v_uv.y is 1 - uv.y/1080. */
        const uy = 1 - uv.y / 1080;
        const t = u.time * 0.4;
        const hash = (p) => fract(sin(dot(p, [127.1, 311.7])) * 43758.5453);
        const noise = (p) => {
          const i = floor(p);
          const f = fract(p);
          const w = f * f * (3 - 2 * f);
          return mix(mix(hash(i), hash(i + [1, 0]), w.x),
                     mix(hash(i + [0, 1]), hash(i + [1, 1]), w.x), w.y);
        };
        /* reference fbm is fixed at 5 octaves */
        const fbm = (p) => {
          let v = 0;
          let a = 0.5;
          for (let o = 0; o < 5; o++) {
            v = v + a * noise(p);
            p = [0.8 * p.x - 0.6 * p.y, 0.6 * p.x + 0.8 * p.y] * 2.02;
            a = a * 0.5;
          }
          return v;
        };
        const palette = (s) => [0.5 + 0.5 * cos(6.28318 * s),
                                0.5 + 0.5 * cos(6.28318 * (s + 0.33)),
                                0.5 + 0.5 * cos(6.28318 * (s + 0.67))];
        const qx = fbm([ux * 3 + t * 0.3, uy * 3 + t * 0.3]);
        const qy = fbm([ux * 3 + 5.2 + t * 0.2, uy * 3 + 1.3 + t * 0.2]);
        const rx = fbm([ux * 3 + qx * 4 + 1.7 + t * 0.15, uy * 3 + qy * 4 + 9.2 + t * 0.15]);
        const ry = fbm([ux * 3 + qx * 4 + 8.3 + t * 0.1, uy * 3 + qy * 4 + 2.8 + t * 0.1]);
        const n = fbm([ux * 3 + rx * 2, uy * 3 + ry * 2]);
        /* palette(n*2 + t*0.2), mixed 0.4 with palette(|q|*3 + t*0.1) */
        const col = palette(n * 2 + t * 0.2);
        const q2 = palette(sqrt(qx * qx + qy * qy) * 3 + t * 0.1);
        let cr = col.x + (q2.x - col.x) * 0.4;
        let cg = col.y + (q2.y - col.y) * 0.4;
        let cb = col.z + (q2.z - col.z) * 0.4;
        const m = 0.7 + 0.3 * n;
        cr = cr + (0.92 - cr) * 0.08; cg = cg + (0.58 - cg) * 0.08; cb = cb + (0.35 - cb) * 0.08;
        const vdx = uv.x / 1920 - 0.5; const vdy = uv.y / 1080 - 0.5;
        const vig = 1 - 0.4 * sqrt(vdx * vdx + vdy * vdy);
        return [cr * m * vig, cg * m * vig, cb * m * vig, 1];
      }, { uniforms: { time: 'float' } });
    }

    c.clear();
    ly.draw(c, FlexShader._kern, {
      rect: [0, 0, 1920, 1080],
      uniforms: { time: 0.5 + clamp(t / FS_ANIM, 0, 1) * 0.94 },
    });
  });

  return (
    <div id="fs-root" class="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#0a0a12]">
      <canvas ref={canvas} id="fs-canvas" class="absolute left-0 top-0 w-[1920px] h-[1080px]" />
      <text id="fs-label" class="absolute right-[80px] bottom-[60px] text-[28px] tracking-[0.08em] font-[hf-azeret] text-[rgba(255,255,255,0.35)] opacity-0">Shaders</text>
    </div>
  );
}

