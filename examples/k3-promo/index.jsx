// JSX frontend port of examples/k3-promo/k3-promo.xml — one continuous stage;
// every layer is gated by its time window via opacity, driven by the frame clock.
// The XML <template>s become components (props replace $param), and the
// <script> sections split into per-scene components: gsap tweens live in
// useGSAP, CanvasKit drawing in useEffect, so hook order (tree order)
// mirrors the XML script order. Shared prologue work (type calibration) runs
// in the entry component's own hook, which registers before all children.

import { useEffect } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 16.4667 };

export const fonts = {
  default: 'k3-sans',
  faces: [
    { id: 'k3-sans', family: 'Inter', path: 'assets/fonts/Inter-Regular.ttf', role: 'sans' },
    { id: 'k3-sans-light', family: 'Inter', path: 'assets/fonts/Inter-Light.ttf' },
    { id: 'k3-sans-medium', family: 'Inter', path: 'assets/fonts/Inter-Medium.ttf' },
    { id: 'k3-mono', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Regular.ttf', role: 'mono' },
    { id: 'k3-mono-bold', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Bold.ttf', role: 'mono' },
  ],
};


import GhostCols from './scenes/ghost-cols.jsx';
import SceneA from './scenes/scene-a.jsx';
import SceneB from './scenes/scene-b.jsx';
import SceneC from './scenes/scene-c.jsx';
import Flash from './scenes/flash.jsx';
import SceneDE from './scenes/scene-de.jsx';
import SceneFG from './scenes/scene-fg.jsx';
import SceneHI from './scenes/scene-hi.jsx';
import OpenCard from './scenes/open-card.jsx';
import { applyTypeCalibration } from './scenes/shared.jsx';

/* k3-promo: single continuous stage (no tl/transition); layer z mirrors the
   source data-track-index: ghostcols(1) < sceneA/DE/HI(2) < sceneB/FG(3) <
   sceneC(4) < flash(5â8) < opencard(9) < grade(10). Components appear in
   tree order = hook order = XML script order (OPEN card last: it only
   touches its own subtree). */
function K3Promo() {
  useEffect(() => {
    // per-frame prologue: the type calibration runs before every scene hook,
    // exactly like the XML script top-level.
    applyTypeCalibration(ctx);
  });
  return (
<div id="root" class="relative w-[1920px] h-[1080px] bg-[#010101] overflow-hidden font-[k3-sans]">
    <GhostCols />
    <SceneA />
    <SceneB />
    <SceneC />
    <Flash />
    <SceneDE />
    <SceneFG />
    <SceneHI />
    <OpenCard />

    {/* film-grade layers (always on, full duration) */}
    <div id="grade-vig" class="absolute inset-0 z-[6] bg-[radial-gradient(ellipse_76%_80%_at_50%_50%,transparent_58%,rgba(0,0,0,0.28)_100%)]"></div>
    <div id="grade-lines" class="absolute inset-0 z-[10] bg-[repeating-linear-gradient(0deg,rgba(0,0,0,0.18)_0_1px,transparent_1px_3px)]"></div>
    {/* index.html #gradenoise: data-URL SVG feTurbulence(fractalNoise,0.9,2,7)
       tiled 240×240 at opacity 0.012 — Skia renders the same noise Chrome does */}
    <div id="grade-noise" class="absolute inset-0 z-[10] opacity-[0.012] bg-[fe-turbulence(fractalNoise,0.9,2,7)] bg-[length:240px_240px]"></div>
</div>
  );
}
export function main() {
  return <K3Promo />;
}
