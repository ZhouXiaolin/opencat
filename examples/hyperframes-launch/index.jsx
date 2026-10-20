/* JSX frontend port of /Users/solaren/Projects/hyperframes-launches/hyperframes-launch/index.html
   (16 sub-compositions on a 49.77s master) — one continuous 1920x1080 stage.
   Structure mirrors the reference: per-composition components, each with one
   useGSAP hook (hook order = tree order = master timeline order). Every scene
   sits in a `<tl>`, so a scene's times are its OWN: `ctx.current.time` starts
   at 0 where the scene becomes visible and the scene tree is culled outside
   its window. Element reveals the scene window does not cover (the glass-intro
   typewriter, the caption chips) stay per-frame `vis()` gates.
   Engine substitutions (web fonts -> faces, CSS 3D -> projected canvas,
   WebGL shader -> ly.pixel lambda, Canvas2D ASCII -> canvas 2d, conic
   gradients -> canvas, backgroundPosition sweeps -> crossfading layers) are
   annotated inline with the reference line they replace. */

export const meta = { width: 1920, height: 1080, fps: 30, duration: 49.8 };

/* Reference loads Google web fonts (css2 API). Engine fonts are ttf/otf, so
   each face maps to the closest-metric system font:
   Instrument Serif is bundled (assets/fonts/InstrumentSerif-*.ttf);
   EB Garamond/Cormorant -> Times New Roman (+Italic),
   Geist/Space Grotesk -> Segoe UI (+Bold), Azeret Mono -> Consolas,
   Bebas Neue -> Impact, Inter 900 -> Arial Black; Inter/JetBrains Mono are real.
   `family` MUST be the ttf's own family name (fontdb reads the file, so a
   mismatched CSS name silently falls back to the default sans). */
export const fonts = {
  default: 'hf-inter',
  faces: [
    { id: 'hf-inter', family: 'Inter', path: 'assets/fonts/Inter-Regular.ttf' },
    { id: 'hf-inter-med', family: 'Inter', path: 'assets/fonts/Inter-Medium.ttf' },
    { id: 'hf-inter-black', family: 'Arial Black', path: 'assets/fonts/ArialBlack.ttf' },
    { id: 'hf-jbm', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Regular.ttf' },
    { id: 'hf-jbm-bold', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-Bold.ttf' },
    { id: 'hf-serif', family: 'Instrument Serif', path: 'assets/fonts/InstrumentSerif-Regular.ttf' },
    { id: 'hf-serif-it', family: 'Instrument Serif', path: 'assets/fonts/InstrumentSerif-Italic.ttf' },
    { id: 'hf-eb-it', family: 'Times New Roman', path: 'assets/fonts/TimesNewRoman-Italic.ttf' },
    { id: 'hf-cg-it', family: 'Cormorant Garamond Medium', path: 'assets/fonts/CormorantGaramond-MediumItalic.ttf' },
    { id: 'hf-geist', family: 'Segoe UI', path: 'assets/fonts/SegoeUI.ttf' },
    { id: 'hf-geist-semi', family: 'Segoe UI', path: 'assets/fonts/SegoeUI-Bold.ttf' },
    { id: 'hf-grotesk', family: 'Segoe UI', path: 'assets/fonts/SegoeUI.ttf' },
    { id: 'hf-grotesk-bold', family: 'Segoe UI', path: 'assets/fonts/SegoeUI-Bold.ttf' },
    { id: 'hf-azeret', family: 'Consolas', path: 'assets/fonts/Consolas.ttf' },
    { id: 'hf-bebas', family: 'Bebas Neue', path: 'assets/fonts/BebasNeue-Regular.ttf' },
  ],
};

import GlassIntro from './scenes/glass-intro/index.jsx';
import CaptionLayer from './scenes/captions.jsx';
import FlexCss from './scenes/flex-css.jsx';
import FlexGsap from './scenes/flex-gsap.jsx';
import FlexShader from './scenes/flex-shader.jsx';
import FlexThreejs from './scenes/flex-threejs.jsx';
import Thesis from './scenes/thesis.jsx';
import FlexDrop from './scenes/flex-drop.jsx';
import FlexMusic from './scenes/flex-music.jsx';
import FlexSfx from './scenes/flex-sfx.jsx';
import FlexFootage from './scenes/flex-footage.jsx';
import FlexIt from './scenes/flex-it.jsx';
import FlexAll from './scenes/flex-all.jsx';
import FlexCompose from './scenes/flex-compose.jsx';
import Engine from './scenes/engine.jsx';
import Cta from './scenes/cta.jsx';
import CanvasClose from './scenes/canvas-close.jsx';

/* ══ master — index.html: 16 sub-compositions on a 49.77s stage,
   voiceover.mp3 @0, 14 caption chips. Master cuts several template
   timelines short (flex-gsap, thesis, engine exits) — each component's
   window matches the master data-start/duration table, so the truncation
   falls out of the scene window. ═════════════════════════════ */
function HyperframesLaunch() {
  return (
    <div id="hf-root" class="relative w-[1920px] h-[1080px] bg-[#0a0a0a] overflow-hidden">
      {/* The master stage is a `<tl>`: 16 sub-compositions on a 49.77s timeline.
          Each scene declares its own segment length; `ctx.current` inside a
          scene is its local clock, starting at 0 where the scene becomes
          visible. glass-intro and flex-css share a 0.94s fade, which is why
          they overlap in the reference. The rest hard-cut. The 1.13s gap
          between flex-gsap and flex-shader is the reference's own empty beat —
          a blank spacer segment holds it. */}
      <tl id="hf-tl" class="absolute inset-0">
        <GlassIntro duration={17.30} />
        {/* The reference hard-cuts at 17.30 — glass-intro's tree is removed and
            flex-css is drawn over it. The overlap still counts as time, so the
            transition declares the span with no effect. */}
        <transition from="gi-root" to="fc-root" effect="cut" duration={0.94} />
        {/* flex-css shows for 1.71s: the 0.94s fade it shares with glass-intro
            plus its own 0.77s segment. */}
        <FlexCss duration={0.77} />
        <FlexGsap duration={0.67} />
        <Spacer duration={1.13} />
        <FlexShader duration={1.20} />
        <FlexThreejs duration={1.21} />
        <Thesis duration={3.82} />
        <FlexDrop duration={0.36} />
        <FlexMusic duration={0.58} />
        <FlexSfx duration={0.93} />
        <FlexFootage duration={0.83} />
        <FlexIt duration={0.15} />
        <FlexAll duration={0.33} />
        <FlexCompose duration={2.22} />
        <Engine duration={9.04} />
        <Cta duration={4.54} />
        <CanvasClose duration={3.75} />
      </tl>
      <CaptionLayer />
      <audio id="hf-vo" path="assets/voiceover.mp3" attach="hf-root" />
    </div>
  );
}

/* An empty beat. Holds timeline time without rendering anything — the
   reference leaves 1.13s of bare background between flex-gsap and
   flex-shader. */
function Spacer() {
  return <div id="spacer" class="absolute left-0 top-0 w-[1920px] h-[1080px]"></div>;
}

export function main() {
  return <HyperframesLaunch />;
}
