// shaders-launch — shaders 库（WebGPU 效果组件）对接 demo。
//
// 这是目标 API 形状（Phase 0 前）：树声明 shader 节点，逐帧 uniforms 走
// 既有 ctx 变更通道；烘焙管线（Node 侧 defineStd → WGSL）与 IR
// ShaderEffect 节点接上后即可双端渲染——engine = opencat-shaders(wgpu)，
// web oracle = npm `shaders` 真库。props 名与库组件文档一一对应，双端共用。
//
// 与原库 `<Shader>` React 绑定的映射规则：
//   1. <Shader> = 库的 <Shader> 画布：children 是 GPU 层，自上而下求值、
//      混合。generator（Aurora/MeshGradient）产像素；filter（VHS）吃
//      child；warp（ZoomBlur）弯 UV。层序约定：filter 的输入 = 它之前
//      兄弟层的合成结果（运行时自动 RTT 边界，jsx 不出现纹理概念）。
//   2. filter 包我们的 DOM 子树（img/text/div）时同样成立：子树先 RTT
//      成纹理再进 wgpu。这一步在 IR 边界完成，与原库 HTMLInCanvas 对应。
//   3. 时间显式化（关键偏离）：原库 animatedTime 用实时挂钟逐帧累积；
//      视频渲染必须逐帧确定，所以统一以 shaderProps({ time: t * k })
//      驱动——引擎把它绑到节点 _animTime uniform，web oracle 侧喂同一 t。
//   4. 静态 props 写在元素上（编译期分支如 curtainCount 参与 WGSL 烘焙
//      变体枚举）；逐帧 props 全部走 shaderProps，不混用。
//
// 渲染通路：每个 <Shader> 是一个 wgpu 离屏目标，产物以零拷贝 GL 纹理
// 进 Skia 画面（gpu_compositor，同 ctx.three 的 3D 通路）。

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
   一个 <Shader> 即一个 wgpu 渲染目标。Aurora 是 generator（不吃 child），
   ZoomBlur 是 warp，作用于其下全部层的合成结果——出场时拉出径向拖影。 */
function AuroraScene() {
  useGSAP((ctx, t) => {
    var ON = t < 4.2;
    vis(ctx, 'aurora-stage', ON);
    if (ON) {
      // 逐帧 uniforms：time 显式驱动（确定性），强度/湍流是内容动画。
      var pulse = 0.5 + 0.5 * Math.sin(t * Math.PI * 2 / 3);
      ctx.getNode('aurora-fx').shaderProps({
        time: t,
        intensity: 62 + 18 * pulse,
        waviness: 90 + 30 * pulse,
      });
      // 出场：ZoomBlur 强度 0→1.6，与标题退场同时收。
      var exit = smooth((t - 3.4) / 0.8);
      ctx.getNode('aurora-fx').warp('ZoomBlur', { time: t, strength: 1.6 * exit });
      ctx.timeline()
        .fromTo('s1-title', { opacity: 0, y: 40 }, { opacity: 1, y: 0, duration: 0.8, ease: 'power2.out' }, 0.4)
        .to('s1-title', { opacity: 0, y: -18, duration: 0.6, ease: 'power1.in' }, 3.5);
    }
  });
  return (
    <div id="aurora-stage" class="absolute inset-0 z-[1] opacity-0">
      <Shader id="aurora-fx" class="absolute inset-0 w-[1920px] h-[1080px]">
        {/* generator：静态 props 全真名（shaders/Aurora 组件文档）。
            curtainCount 是 compileTime prop → 参与 WGSL 烘焙变体。 */}
        <Aurora
          colorA="#a533f8" colorB="#22ee88" colorC="#1694e8" colorSpace="linear"
          curtainCount={4} speed={4.5} rayDensity={24} height={130}
          center={{ x: 0.5, y: 0 }} seed={7}
        />
        {/* warp：作用于下方合成结果，初始 strength 0（逐帧覆盖）。 */}
        <ZoomBlur strength={0} />
      </Shader>
      <text id="s1-title" class="absolute left-[160px] top-[420px] font-[launch-sans-medium] text-[96px] text-[#eef1f6] tracking-[-2px] opacity-0">
        Aurora
      </text>
    </div>
  );
}

/* ════════ 场景 2（3.2→8s）：媒体上 VHS filter + Glitch 抖动 ════════
   filter 吃 child：<img> 子树经 RTT 进 wgpu，VHS 作用于它——原库
   requiresChild 语义。规则线（scanline）强度用 t 抖动，5s 处一次故障脉冲。 */
function VhsScene() {
  useGSAP((ctx, t) => {
    var ON = t >= 3.2;
    vis(ctx, 'vhs-stage', ON);
    if (ON) {
      var glitch = t < 5.0 ? 0 : Math.min(1, (t - 5.0) / 0.25) * Math.max(0, 1 - (t - 5.0) / 0.55);
      ctx.getNode('hero-vhs').shaderProps({
        time: t,
        distortion: 0.18 + 0.55 * glitch,
        noise: 0.12 + 0.4 * glitch,
      });
      ctx.timeline()
        .fromTo('vhs-stage', { opacity: 0 }, { opacity: 1, duration: 0.6, ease: 'power1.inOut' }, 3.2)
        .fromTo('vhs-caption', { opacity: 0, y: 24 }, { opacity: 1, y: 0, duration: 0.5, ease: 'power2.out' }, 3.9);
    }
  });
  return (
    <div id="vhs-stage" class="absolute inset-0 z-[2] opacity-0">
      <Shader id="hero-vhs" class="absolute left-[360px] top-[150px] w-[1200px] h-[675px]">
        {/* 媒体层：引擎现有 image draw op 烘成 RTT，作 VHS 的 child。 */}
        <img src="assets/hero.jpg" class="absolute inset-0 w-full h-full object-cover" />
        <VHS distortion={0.18} noise={0.12} />
      </Shader>
      <text id="vhs-caption" class="absolute left-[360px] top-[852px] font-[launch-sans] text-[34px] text-[#dcdcde] tracking-[0.4px] opacity-0">
        Real WebGPU effects · deterministic frames
      </text>
    </div>
  );
}

/* ════════ 场景 3（6→8s）：MeshGradient 纯 generator 出板 ════════
   最小形态：单 generator、无 child、无 warp——Phase 0 的第一批烘焙对象
   就是这一类（一个 uniform struct + 一段 WGSL，无多趟）。 */
function OutroScene() {
  useGSAP((ctx, t) => {
    var ON = t >= 6.0;
    vis(ctx, 'outro-stage', ON);
    if (ON) {
      ctx.getNode('outro-fx').shaderProps({
        time: t - 6.0,
        speed: 1.2,
      });
      ctx.timeline()
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

export default function Launch() {
  return (
    <div id="root" class="relative w-[1920px] h-[1080px] overflow-hidden bg-[#050507]">
      <AuroraScene />
      <VhsScene />
      <OutroScene />
    </div>
  );
}
