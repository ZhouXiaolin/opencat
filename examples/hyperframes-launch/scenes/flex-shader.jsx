import { gsap } from 'gsap';
import { useGSAP, useEffect, useCanvasKit } from 'opencat/runtime';
import { clamp } from '../shared.jsx';

/* ══ flex-shader (master 20.81-22.01, compositions/flex-shader.html) ════
   WebGL domain-warp fbm shader as an official Skia RuntimeEffect (SKSL),
   1:1 with the reference's four functions (hash/noise/fbm/palette) — the
   GLSL ported verbatim, same expression order as the reference. R is GLSL's
   column-major mat2(0.8,0.6,-0.6,0.8). u_time ramps 0.5->1.44 over the
   0.94s beat, then the frame holds (master slot is 1.20s). GLSL v_uv.y=1 is
   the CANVAS TOP; SKSL main() xy.y grows downward, so the reference's
   v_uv.y is 1 - xy.y/1080. */

var FS_SKSL = `
uniform float u_time;

float oc_hash(float2 p) {
    return fract(sin(dot(p, float2(127.1, 311.7))) * 43758.5453);
}
float oc_noise(float2 p) {
    float2 i = floor(p);
    float2 f = fract(p);
    float2 w = f * f * (3.0 - 2.0 * f);
    return mix(mix(oc_hash(i), oc_hash(i + float2(1.0, 0.0)), w.x),
               mix(oc_hash(i + float2(0.0, 1.0)), oc_hash(i + float2(1.0, 1.0)), w.x), w.y);
}
/* reference fbm is fixed at 5 octaves */
float oc_fbm(float2 p) {
    float v = 0.0;
    float a = 0.5;
    for (int o = 0; o < 5; o++) {
        v = v + a * oc_noise(p);
        p = float2(0.8 * p.x - 0.6 * p.y, 0.6 * p.x + 0.8 * p.y) * 2.02;
        a = a * 0.5;
    }
    return v;
}
float3 oc_palette(float s) {
    return float3(0.5 + 0.5 * cos(6.28318 * s),
                  0.5 + 0.5 * cos(6.28318 * (s + 0.33)),
                  0.5 + 0.5 * cos(6.28318 * (s + 0.67)));
}
half4 main(float2 xy) {
    float ux = xy.x / 1080.0;
    float uy = 1.0 - xy.y / 1080.0;
    float t = u_time * 0.4;
    float qx = oc_fbm(float2(ux * 3.0 + t * 0.3, uy * 3.0 + t * 0.3));
    float qy = oc_fbm(float2(ux * 3.0 + 5.2 + t * 0.2, uy * 3.0 + 1.3 + t * 0.2));
    float rx = oc_fbm(float2(ux * 3.0 + qx * 4.0 + 1.7 + t * 0.15, uy * 3.0 + qy * 4.0 + 9.2 + t * 0.15));
    float ry = oc_fbm(float2(ux * 3.0 + qx * 4.0 + 8.3 + t * 0.1, uy * 3.0 + qy * 4.0 + 2.8 + t * 0.1));
    float n = oc_fbm(float2(ux * 3.0 + rx * 2.0, uy * 3.0 + ry * 2.0));
    /* palette(n*2 + t*0.2), mixed 0.4 with palette(|q|*3 + t*0.1) */
    float3 col = oc_palette(n * 2.0 + t * 0.2);
    float3 q2 = oc_palette(sqrt(qx * qx + qy * qy) * 3.0 + t * 0.1);
    float cr = col.x + (q2.x - col.x) * 0.4;
    float cg = col.y + (q2.y - col.y) * 0.4;
    float cb = col.z + (q2.z - col.z) * 0.4;
    float m = 0.7 + 0.3 * n;
    cr = cr + (0.92 - cr) * 0.08; cg = cg + (0.58 - cg) * 0.08; cb = cb + (0.35 - cb) * 0.08;
    float vdx = xy.x / 1920.0 - 0.5; float vdy = xy.y / 1080.0 - 0.5;
    float vig = 1.0 - 0.4 * sqrt(vdx * vdx + vdy * vdy);
    return half4(cr * m * vig, cg * m * vig, cb * m * vig, 1.0);
}`;
var FS_START = 20.81, FS_DUR = 1.20, FS_ANIM = 0.94;


export default function FlexShader() {
  const FS_CK = useCanvasKit();

  useGSAP(() => {
    var tl = gsap.timeline();
    tl.fromTo('fs-label', { opacity: 0 }, { opacity: 0.35, duration: 0.3, ease: 'power2.out' }, 0.15);
  });

  /* module-level RuntimeEffect: the SKSL compiles once, uniforms per frame.
     The canvas draw goes through useEffect + the official MakeCanvasSurface. */
  useEffect(() => {
    var t = ctx.current.time;
    var c = FS_CK.MakeCanvasSurface('fs-canvas').getCanvas();
    if (!FlexShader._eff) {
      FlexShader._eff = FS_CK.RuntimeEffect.Make(FS_SKSL);
      if (!FlexShader._eff) {
        throw new Error('flex-shader: RuntimeEffect.Make rejected the SKSL');
      }
    }
    var paint = new FS_CK.Paint();
    paint.setShader(FlexShader._eff.makeShader([0.5 + clamp(t / FS_ANIM, 0, 1) * 0.94]));
    c.clear();
    c.drawRect(FS_CK.XYWHRect(0, 0, 1920, 1080), paint);
  });
  });

  return (
    <div id="fs-root" className="absolute left-0 top-0 w-[1920px] h-[1080px] bg-[#0a0a12]">
      <canvas id="fs-canvas" className="absolute left-0 top-0 w-[1920px] h-[1080px]" />
      <text id="fs-label" className="absolute right-[80px] bottom-[60px] text-[28px] tracking-[0.08em] font-[hf-azeret] text-[rgba(255,255,255,0.35)] opacity-0">Shaders</text>
    </div>
  );
}

