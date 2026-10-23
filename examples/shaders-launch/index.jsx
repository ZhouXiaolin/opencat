// shaders-launch — shaders 库（WebGPU 效果组件）对接 demo。
//
// <Shader> 就是一个普通的 JSX 标签，children 是 GPU 层，自上而下求值。
// generator（Aurora / MeshGradient）整帧产像素；filter（VHS）吃它下面的
// 合成结果；warp（ZoomBlur）弯 UV。层序 = 声明顺序，运行期自动在层边界
// 做 RTT，jsx 里不出现纹理概念。
//
// 双端同一份源码：engine = opencat-shaders（Rust 把 props 烘成 WGSL 跑
// wgpu），web = npm `shaders` 真库。props 名与库组件文档一一对应。
//
// 两条使用规则：
//   1. 时间显式化。视频必须逐帧确定，所以动画统一由 shaderProps({ time })
//      驱动——引擎把它绑到 _animTime，web 侧喂同一个 t。
//   2. 静态 props 写在元素上，逐帧 props 全部走 shaderProps，不混用。
//
// <Shader> 必须有确定的盒子：宿主的样式表解析出 class 里的尺寸（vite
// 工程走 opencat-plugin 注入的 `css`），裸 .jsx 请写内联 style。

import { gsap } from 'gsap';
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 8 };

export const fonts = {
  default: 'launch-sans',
  faces: [
    { id: 'launch-sans', family: 'Inter', path: 'assets/fonts/Inter-Regular.ttf', role: 'sans' },
    { id: 'launch-sans-medium', family: 'Inter', path: 'assets/fonts/Inter-Medium.ttf' },
  ],
};

/* ════════ 每帧工具（模块体每帧重估，纯函数） ════════ */
function vis(ctx, id, on) { ctx.getNode(id).opacity(on ? 1 : 0); }
function clamp01(v) { return Math.max(0, Math.min(1, v)); }
function smooth(v) { var p = clamp01(v); return p * p * (3 - 2 * p); }

/* ════════ 场景 1（0→4.2s）：Aurora 生成器背景 + ZoomBlur 出场 ════════
   Aurora 是 generator（不吃 child），ZoomBlur 是 warp，作用于下面全部层的
   合成结果——出场时拉出径向拖影。 */
function AuroraScene() {
  useGSAP((ctx, t) => {
    var ON = t < 4.2;
    vis(ctx, 'aurora-stage', ON);
    if (ON) {
      var pulse = 0.5 + 0.5 * Math.sin(t * Math.PI * 2 / 3);
      // 逐帧 props：时间显式驱动（确定性），强度/湍流是内容动画。
      // 栈里有多层时，第二个参数点名那一层。
      ctx.getNode('aurora-fx').shaderProps({
        time: t,
        intensity: 62 + 18 * pulse,
        waviness: 90 + 30 * pulse,
      }, 'Aurora');
      // 出场：ZoomBlur 强度 0→1.6，与标题退场同时收。
      var exit = smooth((t - 3.4) / 0.8);
      ctx.getNode('aurora-fx').shaderProps({ time: t, intensity: 1.6 * exit }, 'ZoomBlur');
      gsap.timeline()
        .fromTo('s1-title', { opacity: 0, y: 40 }, { opacity: 1, y: 0, duration: 0.8, ease: 'power2.out' }, 0.4)
        .to('s1-title', { opacity: 0, y: -18, duration: 0.6, ease: 'power1.in' }, 3.5);
    }
  });
  return (
    <div id="aurora-stage" class="absolute inset-0 z-[1] opacity-0">
      <Shader id="aurora-fx" class="absolute inset-0 w-[1920px] h-[1080px]">
        {/* generator：静态 props 全用库里的真名。curtainCount 是
            compileTime prop（参与 WGSL 变体枚举）。 */}
        <Aurora
          colorA="#a533f8" colorB="#22ee88" colorC="#1694e8" colorSpace="linear"
          curtainCount={4} speed={4.5} rayDensity={24} height={130}
          center={{ x: 0.5, y: 0 }} seed={7}
        />
        {/* warp：作用于下面层的合成结果，初始强度 0（逐帧覆盖）。 */}
        <ZoomBlur intensity={0} />
      </Shader>
      <text id="s1-title" class="absolute left-[160px] top-[420px] font-[launch-sans-medium] text-[96px] text-[#eef1f6] tracking-[-2px] opacity-0">
        Aurora
      </text>
    </div>
  );
}

/* ════════ 场景 2（3.2→8s）：生成器上套 VHS filter ════════
   filter 吃它下面的合成结果：MeshGradient 先 RTT 成纹理，VHS 作用于它——
   库的 requiresChild 语义。5s 处一次磁带故障脉冲。
   （`<img>` 作 filter 的 child 需要先把子树光栅化成纹理，engine 侧还没有这条
   通路，所以这里用 generator 当输入。） */
function VhsScene() {
  useGSAP((ctx, t) => {
    var ON = t >= 3.2;
    vis(ctx, 'vhs-stage', ON);
    if (ON) {
      var glitch = t < 5.0 ? 0 : Math.min(1, (t - 5.0) / 0.25) * Math.max(0, 1 - (t - 5.0) / 0.55);
      ctx.getNode('hero-vhs').shaderProps({
        time: t,
        smear: 0.2 + 0.55 * glitch,
        scanlineNoise: 0.6 + 0.4 * glitch,
      }, 'VHS');
      gsap.timeline()
        .fromTo('vhs-stage', { opacity: 0 }, { opacity: 1, duration: 0.6, ease: 'power1.inOut' }, 3.2)
        .fromTo('vhs-caption', { opacity: 0, y: 24 }, { opacity: 1, y: 0, duration: 0.5, ease: 'power2.out' }, 3.9);
    }
  });
  return (
    <div id="vhs-stage" class="absolute inset-0 z-[2] opacity-0">
      <Shader id="hero-vhs" class="absolute left-[360px] top-[150px] w-[1200px] h-[675px]">
        <MeshGradient colorA="#14233a" colorB="#f0a020" colorSpace="linear" />
        <VHS wobble={1} scanlineNoise={0.6} smear={0.2} speed={1} />
      </Shader>
      <text id="vhs-caption" class="absolute left-[360px] top-[852px] font-[launch-sans] text-[34px] text-[#dcdcde] tracking-[0.4px] opacity-0">
        Real WebGPU effects · deterministic frames
      </text>
    </div>
  );
}

/* ════════ 场景 3（6→8s）：MeshGradient 纯 generator 出板 ════════
   最小形态：单 generator、无 child、无 warp——第一批烘焙对象就是这一类。 */
function OutroScene() {
  useGSAP((ctx, t) => {
    var ON = t >= 6.0;
    vis(ctx, 'outro-stage', ON);
    if (ON) {
      ctx.getNode('outro-fx').shaderProps({
        time: t - 6.0,
        speed: 1.2,
      }, 'MeshGradient');
      gsap.timeline()
        .fromTo('outro-title', { opacity: 0, scale: 0.96 }, { opacity: 1, scale: 1, duration: 0.7, ease: 'power2.out' }, 6.3);
    }
  });
  return (
    <div id="outro-stage" class="absolute inset-0 z-[3] opacity-0">
      <Shader id="outro-fx" class="absolute inset-0 w-[1920px] h-[1080px]">
        <MeshGradient colorA="#0f172a" colorB="#7c3aed" />
      </Shader>
      <text id="outro-title" class="absolute left-0 top-[492px] w-full text-center font-[launch-sans-medium] text-[72px] text-[#f4f4f5] tracking-[-1px] opacity-0">
        opencat × shaders
      </text>
    </div>
  );
}

export default function ShadersLaunch() {
  return (
    <div id="root" class="relative w-[1920px] h-[1080px] overflow-hidden bg-[#050507]">
      <AuroraScene />
      <VhsScene />
      <OutroScene />
    </div>
  );
}
