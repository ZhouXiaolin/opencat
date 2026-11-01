// shaders 对齐案：createRendererFromJSON + 逐帧 renderFrame({deltaSeconds})
// 是官方 frame-locked 确定性通路（时钟精确等于 delta 之和），与 ref 侧
// ref-entry.mjs 同序列（静态案每帧 delta 0 原样重画）。
import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H } from './presets.mjs';

export const meta = { width: W, height: H, fps: 30, duration: 1 / 30 };

const CASE = CASES.solid;
const CASE_W = W;
const CASE_H = H;

let renderer = null;
let inited = false;

export default function Composition() {
    useEffect(() => {
        if (!renderer) {
            // g2 同构挂载：声明 canvas 只是容器（声明节点 handle 没有
            // getContext），自建 canvas 经 appendChild 钩子冒用声明 id 进
            // visual registry，对自建 canvas 拿 'webgpu' context。
            const host = document.querySelector('#fx');
            const canvas = document.createElement('canvas');
            canvas.width = CASE_W;
            canvas.height = CASE_H;
            host.appendChild(canvas);
            renderer = createRendererFromJSON({ components: CASE.components });
            // initialize 是 async：录制桩的 requestAdapter/requestDevice 立即
            // resolve，落定发生在本帧的微任务 drain——then 里的首绘（delta 0）
            // 同帧进流，帧 0 即有内容。失败必须 fail-loud（__ocStderr 上抛）。
            renderer.initialize(canvas)
                .then(() => {
                    inited = true;
                    return renderer.renderFrame({ deltaSeconds: 0 });
                })
                .catch((e) => {
                    globalThis.__ocStderr?.('[shaders] initialize failed: ' + (e && (e.stack || e.message)));
                });
            return;
        }
        if (!inited) return;
        renderer.renderFrame({ deltaSeconds: 0 });
    });
    return (
        <div id="root" class="relative w-[960px] h-[540px] overflow-hidden bg-white">
            <canvas id="fx" class="absolute left-0 top-0 w-[960px] h-[540px]" width={960} height={540} />
        </div>
    );
}
