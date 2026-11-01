// 双侧同源的唯一事实：ref（Edge 真 WebGPU）与 engine（core 录制 → wgpu）
// 都从这份表取 preset。组件名与 props 与 react 版完全一致（官方跨框架
// 同一 props 面），只是走 shaders/js 的声明式组件数组形态。
//
// frames: 渲染帧数；anim: 每帧推进 delta=1/30（false = 每帧 delta 0 原样
// 重画，静态案）。首帧两侧协议一致：delta 0（ref 循环 i===0 ? 0 : DELTA；
// engine 侧 initialize 落定后的首绘同为 delta 0）。
export const W = 960;
export const H = 540;
export const FPS = 30;

export const CASES = {
    solid: {
        frames: 1,
        anim: false,
        components: [
            { type: 'SolidColor', id: 'c1', props: { color: '#3366cc' } },
        ],
    },
    gradient: {
        frames: 1,
        anim: false,
        components: [
            { type: 'LinearGradient', id: 'g1', props: { colorA: '#0f172a', colorB: '#7c3aed', angle: 45 } },
        ],
    },
    noise: {
        frames: 1,
        anim: false,
        components: [
            { type: 'PerlinNoise', id: 'n1', props: { scale: 2, seed: 42 } },
        ],
    },
    blend: {
        frames: 1,
        anim: false,
        components: [
            { type: 'LinearGradient', id: 'bg', props: { colorA: '#0f172a', colorB: '#7c3aed', angle: 45 } },
            { type: 'RadialGradient', id: 'fg', props: { colorA: '#f59e0b', colorB: '#ef4444cc', blendMode: 'overlay', opacity: 0.8 } },
        ],
    },
    // 动画案：30 帧噪声演化（seed 固定，speed 交由逐帧 delta 驱动）。
    'noise-anim': {
        frames: 30,
        anim: true,
        components: [
            { type: 'PerlinNoise', id: 'n1', props: { scale: 3, seed: 42, speed: 1 } },
        ],
    },
};
