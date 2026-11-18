import { Composition, Timeline, Transition, Scene } from './runtime';
import Beat1 from './scenes/glass-intro/beat1';
import Beat2 from './scenes/glass-intro/beat2';
import Beat3 from './scenes/glass-intro/beat3';
import Beat4 from './scenes/glass-intro/beat4';
import CaptionLayer from './scenes/captions';
import FlexCss from './scenes/flex-css';
import FlexGsap from './scenes/flex-gsap';
import FlexShader from './scenes/flex-shader';
import FlexThreejs from './scenes/flex-threejs';
import Thesis from './scenes/thesis';
import FlexDrop from './scenes/flex-drop';
import FlexMusic from './scenes/flex-music';
import FlexSfx from './scenes/flex-sfx';
import FlexFootage from './scenes/flex-footage';
import FlexIt from './scenes/flex-it';
import FlexAll from './scenes/flex-all';
import FlexCompose from './scenes/flex-compose';
import Engine from './scenes/engine';
import Cta from './scenes/cta';
import CanvasClose from './scenes/canvas-close';

export default function App() {
  return (
    <Composition id="main" duration={49.8} width={1920} height={1080} fps={30}
      class="relative w-[1920px] h-[1080px] bg-[#0a0a0a] overflow-hidden">
      <Timeline id="hf-tl" class="absolute inset-0" duration={49.77}>
        <Beat1 duration={2.1} />
        <Beat2 duration={1.75} />
        <Beat3 duration={1.75} />
        <Beat4 duration={12.64} />
        <Transition from="gi-root" to="fc-root" effect="cut" duration={0.94} />
        <Scene start={18.24} duration={0.77}><FlexCss /></Scene>
        <Scene start={19.01} duration={0.67}><FlexGsap /></Scene>
        <Scene start={20.81} duration={1.2}><FlexShader /></Scene>
        <Scene start={22.01} duration={1.21}><FlexThreejs /></Scene>
        <Scene start={23.22} duration={3.82}><Thesis /></Scene>
        <Scene start={27.04} duration={0.36}><FlexDrop /></Scene>
        <Scene start={27.4} duration={0.58}><FlexMusic /></Scene>
        <Scene start={27.98} duration={0.93}><FlexSfx /></Scene>
        <Scene start={28.91} duration={0.83}><FlexFootage /></Scene>
        <Scene start={29.74} duration={0.15}><FlexIt /></Scene>
        <Scene start={29.89} duration={0.33}><FlexAll /></Scene>
        <Scene start={30.22} duration={2.22}><FlexCompose /></Scene>
        <Scene start={32.44} duration={9.04}><Engine /></Scene>
        <Scene start={41.48} duration={4.54}><Cta /></Scene>
        <Scene start={46.02} duration={3.75}><CanvasClose /></Scene>
      </Timeline>
      <CaptionLayer />
    </Composition>
  );
}
