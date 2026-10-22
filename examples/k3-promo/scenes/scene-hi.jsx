/* SCENE H+I: glitch dissolve → K3 wordmark (track 2, 13.7→16.467s).
   hbox keeps its source CSS position (left 400 top 265); the gsap.set
   x:-23 y:66 rides the node transform. */

import { gsap } from 'gsap';
import CanvasKitInit from 'canvaskit-wasm';
import { useFrame, useGSAP } from 'opencat/runtime';
import { Cdot, StageCanvas, drawDissolve, drawPlusses, drawQmarks, drawStrip, rebuildDissolveField, vis } from './shared.jsx';

export default function SceneHI() {
  let CK = null;
  CanvasKitInit({}).then((ck) => { CK = ck; });
  useGSAP((ctx, t) => {
    var T = t;
    if (T >= 13.7) {
      // gsap.set("#hbox", { x:-23, y:66 }): the box CSS stays at its source
      // position (400,265); the offset rides the transform.
      // Pop-in (power2.out), then shrink+fade (power2.in).
      gsap.timeline()
        .set('hbox', { opacity: 0, x: -23, y: 66 }, 13.7)
        .set('hbox', { opacity: 1 }, 13.74)
        .fromTo('hbox', { scale: 0.94 }, { scale: 1, duration: 0.12, ease: 'power2.out' }, 13.74)
        .to('hbox', { scale: 0.4, opacity: 0, duration: 0.14, ease: 'power2.in' }, 14.6);
      // wordmark + glow formation and bloom decay (index.html:1758-1772).
      // NOTE: the reference decays #iglow1 *to* 0.28 (power2.out) — it falls
      // 0.95 -> 0.28 and stays legible. The dashes' fade-out uses GSAP's
      // default power1.out.
      gsap.timeline()
        // index.html:1758-1772 verbatim: zero everything at 13.7, then .to
        // chains whose start values are captured when they activate. These
        // must NOT be fromTo() — fromTo immediate-renders its from value at
        // timeline build, and with per-frame rebuild + seek the build-time
        // write leaks onto the screen for every t before the tween activates.
        .set(['iwordmark', 'iglow1', 'iglow2', 'iglow3'], { opacity: 0 }, 13.7)
        .to('iwordmark', { opacity: 1, duration: 0.1, ease: 'power1.inOut' }, 14.7333)
        .to('iglow1', { opacity: 0.95, duration: 0.1, ease: 'power1.out' }, 14.7333)
        .to('iglow2', { opacity: 1, duration: 0.1, ease: 'power1.out' }, 14.7667)
        .to('iglow3', { opacity: 0.9, duration: 0.1, ease: 'power1.out' }, 14.7667)
        .to('iglow3', { opacity: 0, duration: 0.8, ease: 'power2.out' }, 14.95)
        .to('iglow2', { opacity: 0, duration: 0.95, ease: 'power2.out' }, 14.95)
        .to('iglow1', { opacity: 0.28, duration: 0.95, ease: 'power2.out' }, 14.95)
        .set(['idashL', 'idashR'], { opacity: 0 }, 13.7)
        .set(['idashL', 'idashR'], { opacity: 0.85 }, 14.7)
        .to(['idashL', 'idashR'], { opacity: 0, duration: 0.25, ease: 'power1.out' }, 15.1)
        // g2qmarks fade-out (ref #gfurn2) and ifurn fade-in (ref #ifurn)
        .set('g2qmarks-canvas', { opacity: 1 }, 13.7)
        .to('g2qmarks-canvas', { opacity: 0, duration: 0.12, ease: 'power1.out' }, 14.75)
        .set('ifurn-canvas', { opacity: 0 }, 13.7)
        .to('ifurn-canvas', { opacity: 1, duration: 0.16, ease: 'power1.out' }, 15.3);
    }
  });
  useFrame((ctx, t) => {
    if (!CK) return;
    var T = t;
    if (typeof globalThis.__k3disBuilt === 'undefined') rebuildDissolveField(ctx);
    var HI_ON = (T >= 13.7);
    vis(ctx, 'sceneHI', HI_ON);
    if (HI_ON) {
      // hatch lines inside the box (hbox-canvas coords = box-local 1150x385);
      // exact 26° CSS geometry, staggered 13.77 + k*0.04.
      var hCv = CK.MakeCanvasSurface('hbox-canvas').getCanvas();
      if (hCv) {
        hCv.clear();
        var hatchP = new CK.Paint(); hatchP.setStyle(CK.PaintStyle.Stroke); hatchP.setStrokeWidth(2);
        hatchP.setColor(CK.Color(74, 74, 74, 0.85));
        var HS = Math.sin(26 * Math.PI / 180), HC = Math.cos(26 * Math.PI / 180);
        for (var hk = 0; hk < 6; hk++) {
          var hOn = 13.77 + hk * 0.04;
          if (T < hOn) continue;
          var hbx = -40 + hk * 230 + 1;               // bottom anchor (left+1, 435)
          hCv.drawLine(hbx, 435, hbx + 540 * HS, 435 - 540 * HC, hatchP);
        }
      }
      // scramble dissolve: lambda drawRect over the field render target;
      // the draw lands at hcanvas-local (232, 230) = screen (232, 430)
      var hDCv = CK.MakeCanvasSurface('hcanvas').getCanvas();
      if (hDCv) {
        hDCv.clear();
        drawDissolve(ctx, hDCv, T);
      }
      // g2qmarks furniture (ref makeQmarks/makePlusses); opacity is declarative.
      var g2Cv = CK.MakeCanvasSurface('g2qmarks-canvas').getCanvas();
      if (g2Cv) {
        g2Cv.clear();
        drawQmarks(CK, g2Cv, [[343, 265, -1, -1], [1575, 265, 1, -1], [343, 813, -1, 1], [1575, 813, 1, 1]]);
        drawPlusses(CK, g2Cv, [[252, 178], [1668, 178], [252, 898], [1668, 898]], 22, 0.4);
      }
      // ifurn: stripIL/IR + iqmarks + plusses (opacity is declarative).
      var ifCv = CK.MakeCanvasSurface('ifurn-canvas').getCanvas();
      if (ifCv) {
        ifCv.clear();
        drawStrip(CK, ifCv, 306, 535, 109, 9, 15); drawStrip(CK, ifCv, 1504, 535, 109, 9, 15);
        drawQmarks(CK, ifCv, [[400, 290, -1, -1], [1520, 290, 1, -1], [400, 780, -1, 1], [1520, 780, 1, 1]]);
        drawPlusses(CK, ifCv, [[300, 205], [1615, 205], [300, 875], [1615, 875]], 25, 0.45);
      }
    }
  });
  return (
    <div id="sceneHI" class="absolute inset-0 z-[2] opacity-0">
      {/* g2qmarks live in #gfurn2, the FIRST child of sceneHI — must paint
          under hbox/iwordmark */}
      <StageCanvas id="g2qmarks-canvas" />
      {/* wireframe dissolve box */}
      <div id="hbox" class="absolute border-[1px] border-[#6a6a6a] left-[400px] top-[265px] w-[1150px] h-[385px] opacity-0">
        <Cdot id="d-558" s="12px" c="#e6e6e6" class="-left-[24px] -top-[24px]" />
        <Cdot id="d-559" s="12px" c="#e6e6e6" class="-right-[24px] -top-[24px]" />
        <Cdot id="d-560" s="12px" c="#e6e6e6" class="-left-[24px] -bottom-[24px]" />
        <Cdot id="d-561" s="12px" c="#e6e6e6" class="-right-[24px] -bottom-[24px]" />
        <Cdot id="d-721" s="7px" c="#cfcfcc" class="-left-[9px] -top-[9px]" />
        <Cdot id="d-722" s="7px" c="#cfcfcc" class="-right-[9px] -top-[9px]" />
        <Cdot id="d-723" s="7px" c="#cfcfcc" class="-left-[9px] -bottom-[9px]" />
        <Cdot id="d-724" s="7px" c="#cfcfcc" class="-right-[9px] -bottom-[9px]" />
        <canvas id="hbox-canvas" class="absolute inset-0 w-[1150px] h-[385px]"></canvas>
      </div>
      {/* dissolve canvas: full-bleed layer at screen (0,200) like #hcanvas */}
      <canvas id="hcanvas" class="absolute left-0 top-[200px] w-[1920px] h-[560px]"></canvas>
      {/* bloom glow layers: each is the lockup itself blurred. CSS
          blur(<length>) takes the STANDARD DEVIATION directly, and with
          scale applied after the filter σ_eff = σ*scale. */}
      <div id="iglow3" class="absolute left-[746.94px] top-[462.45px] w-[426.12px] h-[154.1px] opacity-0 blur-[69.6px] brightness-[2.2]">
        <div id="d-562" class="absolute left-[30.82px] top-0 w-[154.1px] h-[154.1px]">
          <path id="g3-p1" view-box="0 0 512 512" class="absolute inset-0 w-[154.1px] h-[154.1px] fill-[#ffffff]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="g3-p2" view-box="0 0 512 512" class="absolute inset-0 w-[154.1px] h-[154.1px] fill-[#4d9dff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="g3-p3" view-box="0 0 512 512" class="absolute inset-0 w-[154.1px] h-[154.1px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="g3-t" class="absolute left-[214.45px] top-[5.75px] font-[k3-sans] font-medium text-[#ffffff] leading-none text-[143.67px] tracking-[-3.35px]">K3</text>
      </div>
      <div id="iglow2" class="absolute left-[801px] top-[482px] w-[318px] h-[115px] opacity-0 blur-[24.6px] brightness-[2.1]">
        <div id="d-563" class="absolute left-[23px] top-0 w-[115px] h-[115px]">
          <path id="g2-p1" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#ffffff]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="g2-p2" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#4d9dff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="g2-p3" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="g2-t" class="absolute left-[160.0345px] top-[4.2938px] font-[k3-sans] font-medium text-[#ffffff] leading-none text-[107.2165px] tracking-[-2.5px]">K3</text>
      </div>
      <div id="iglow1" class="absolute left-[801px] top-[482px] w-[318px] h-[115px] opacity-0 blur-[4.1px] brightness-[1.5]">
        <div id="d-564" class="absolute left-[23px] top-0 w-[115px] h-[115px]">
          <path id="g1-p1" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#e6e6e9]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="g1-p2" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#2b8bff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="g1-p3" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="g1-t" class="absolute left-[160.0345px] top-[4.2938px] font-[k3-sans] font-medium text-[#e6e6e9] leading-none text-[107.2165px] tracking-[-2.5px]">K3</text>
      </div>
      {/* K3 wordmark lockup (318×115) */}
      <div id="iwordmark" class="absolute flex items-center gap-[18px] left-[801px] top-[482px] w-[318px] h-[115px] opacity-0">
        <div id="d-565" class="absolute left-[23px] top-0 w-[115px] h-[115px]">
          <path id="p-652" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#dadade]" d="M503 114.333v280c0 60.711-49.29 110-110 110H113c-60.711 0-110-49.289-110-110v-280c0-60.71 49.289-110 110-110h280c60.71 0 110 49.29 110 110z" />
          <path id="p-653" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#027aff]" d="M342.065 189.759c1.886-2.42 3.541-4.63 5.289-6.77.81-1.007.74-1.771-.046-2.824-7.58-9.965-8.298-21.028-3.935-32.254 3.275-8.448 10.52-12.406 19.373-13.25 5.52-.521 10.936.046 15.959 2.73 6.596 3.53 10.438 8.912 11.688 16.341.995 5.926.81 11.712-.868 17.452-2.974 10.161-10.277 15.427-20.287 16.758-8.31 1.11-16.734 1.25-25.113 1.817-.648.046-1.308 0-2.06 0z" />
          <path id="p-654" view-box="0 0 512 512" class="absolute inset-0 w-[115px] h-[115px] fill-[#010101]" d="M321.512 144.254h-50.064l-39.637 90.384h-56.036v-89.99H131v232.868h44.787v-98.103h78.973c13.598 0 26.015-7.927 31.744-20.252v118.355h44.787v-98.103c0-23.342-18.239-42.97-41.523-44.671v-.116h-24.593a45.577 45.577 0 0026.884-24.534l29.453-65.838z" />
        </div>
        <text id="t-651" class="absolute left-[160.0345px] top-[4.2938px] font-[k3-sans] font-medium text-[#dadade] leading-none text-[107.2165px] tracking-[-2.5px]">K3</text>
      </div>
      {/* flanking formation dashes */}
      <div id="idashL" class="absolute h-[3px] bg-[#a9a9ac] left-[718px] top-[538px] w-[30px] opacity-0"></div>
      <div id="idashR" class="absolute h-[3px] bg-[#a9a9ac] left-[1172px] top-[538px] w-[30px] opacity-0"></div>
      {/* ifurn: stripIL/IR + iqmarks + plusses, shown 15.3→15.46 */}
      <StageCanvas id="ifurn-canvas" />
    </div>
  );
}
