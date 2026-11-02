// ref 侧驱动：?case=<name> 从 presets 取组件表，frame-locked 确定性逐帧。
// createRendererFromJSON + renderFrame({deltaSeconds}) 是官方的
// "frame-locked video export" 通路——时钟精确等于 delta 之和，与 RAF 无关。
//
// 确定性三件套（与 engine 侧同款，抄 g2）：mulberry32(42) 种子、Date 冻结
// 纪元 80000000。首帧 delta 0（与 engine 侧 initialize 落定后的首绘一致），
// 动画案后续帧 delta = 1/30。
import { createRendererFromJSON } from 'shaders/core';
import { CASES } from '../cases/presets.mjs';

// rAF 全程禁用：lib requestRender 的墙钟渲染会推进 animatedTime
// （worley/voronoi 特征点位移），corpus 钟协议 = 时间只经 deltaSeconds
// 推进——与 engine 侧 jsx 模板同禁才是同一协议（gen-cases.mjs 注释）。
window.requestAnimationFrame = () => 0;

const q = new URLSearchParams(location.search);
const name = q.get('case');
const spec = CASES[name];
if (!spec) {
    window.__ocRefError = 'unknown case: ' + name;
    document.title = 'FAIL:' + name;
    throw new Error('unknown case: ' + name);
}

let seed = 42 >>> 0;
Math.random = () => {
    seed = (seed + 0x6D2B79F5) | 0;
    let t = Math.imul(seed ^ (seed >>> 15), 1 | seed);
    t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
};
const EPOCH = 80000000;
class FrozenDate extends Date {
    constructor(...a) { if (a.length === 0) super(EPOCH); else super(...a); }
    static now() { return EPOCH; }
}
window.Date = FrozenDate;
// dpr 钉 1（S1c）：--force-device-scale-factor 在 headless=new 下不生效，
// Windows 显示缩放（1.25）泄入 → lib 把 canvas buffer 配成 1200×675，
// ref.html 的 drawImage 回缩 960 是非线性变换，把渐变压暗 ~10%。
// 引擎腿 realm canvas 恒 960×540，两侧必须同分辨率。
Object.defineProperty(window, 'devicePixelRatio', { get: () => 1, configurable: true });

const DELTA = 1 / 30;

async function waitFrame(i) {
    const deadline = performance.now() + 15000;
    while (!(window.__ocFrames && window.__ocFrames.length > i)) {
        if (performance.now() > deadline) {
            throw new Error('frame ' + i + ' readback timeout (rbErr=' + (window.__ocRbErr || 'null') + ')');
        }
        await new Promise((r) => setTimeout(r, 8));
    }
}

(async () => {
    try {
        if (window.__ocWgHookMissing) throw new Error('WebGPU hook missing (navigator.gpu unavailable?)');
        const canvas = document.querySelector('#fx');
        const renderer = createRendererFromJSON({ components: spec.components });
        await renderer.initialize(canvas);
        for (let i = 0; i < spec.frames; i++) {
            const delta = i === 0 ? 0 : (spec.anim ? DELTA : 0);
            await renderer.renderFrame({ deltaSeconds: delta, waitForGpu: true });
            await waitFrame(i);
        }
        renderer.dispose();
        window.__ocRefDone = true;
    } catch (e) {
        window.__ocRefError = (e && (e.stack || e.message)) || String(e);
        document.title = 'FAIL:' + name;
    }
})();
