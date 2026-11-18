import { applyTypeCalibration } from './shared';
import { Composition, COMPOSITION_DURATION, onFrame } from './runtime';
// import 序 = 旧工程的 hook 序 = 渲染树序（useTween 在模块顶层注册）。
import GhostCols from './scenes/ghost-cols';
import SceneA from './scenes/scene-a';
import SceneB from './scenes/scene-b';
import SceneC from './scenes/scene-c';
import Flash from './scenes/flash';
import SceneDE from './scenes/scene-de';
import SceneFG from './scenes/scene-fg';
import SceneHI from './scenes/scene-hi';
import OpenCard from './scenes/open-card';

export default function App() {
  // 共享每帧前导：k3 语料的字宽标定在 scene 钩之前跑（旧入口 useEffect 序）。
  onFrame(() => applyTypeCalibration());

  return (
    <Composition id="main" duration={COMPOSITION_DURATION} width={1920} height={1080} fps={30}
      class="relative w-[1920px] h-[1080px] bg-[#010101] overflow-hidden font-[k3-sans]">
      <GhostCols />
      <SceneA />
      <SceneB />
      <SceneC />
      <Flash />
      <SceneDE />
      <SceneFG />
      <SceneHI />
      <OpenCard />
      {/* 常开调色层（全片，无时间窗） */}
      <div id="grade-vig" class="absolute inset-0 z-[6] bg-[radial-gradient(ellipse_76%_80%_at_50%_50%,transparent_58%,rgba(0,0,0,0.28)_100%)]"></div>
      <div id="grade-lines" class="absolute inset-0 z-[10] bg-[repeating-linear-gradient(0deg,rgba(0,0,0,0.18)_0_1px,transparent_1px_3px)]"></div>
      <div id="grade-noise" class="absolute inset-0 z-[10] opacity-[0.012] bg-[fe-turbulence(fractalNoise,0.9,2,7)] bg-[length:240px_240px]"></div>
    </Composition>
  );
}
