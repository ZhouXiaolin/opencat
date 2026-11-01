// shaders 对齐案 gradient：createRendererFromJSON + 静态重绘（delta 0）。
// 案本质单帧（presets frames=1 anim=false），ref 侧协议所有帧 delta 0
// ——time 恒 0，两侧必须同钟。挂载 = g2 同构（声明 canvas 只是容器，
// 自建 canvas appendChild 冒用 id）。
import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

export const meta = { width: W, height: H, fps: FPS, duration: CASES["gradient"].frames / FPS };

const CASE = CASES["gradient"];

let renderer = null;
let inited = false;

export default function Composition() {
    useEffect(() => {
        if (!renderer) {
            const host = document.querySelector('#fx');
            const canvas = document.createElement('canvas');
            canvas.width = W;
            canvas.height = H;
            host.appendChild(canvas);
            renderer = createRendererFromJSON({ components: CASE.components });
            // initialize 是 async：录制桩立即 resolve，落定在本帧微任务
            // drain——then 里的首绘（delta 0）同帧进流。失败 fail-loud。
            renderer.initialize(canvas)
                .then(() => {
                    inited = true;
                    return renderer.renderFrame({ deltaSeconds: 0 });
                })
                .catch((e) => globalThis.__ocStderr?.(
                    '[shaders] initialize failed: ' + (e && (e.stack || e.message))));
            return;
        }
        if (!inited) return;
        renderer.renderFrame({ deltaSeconds: 0 });
    });
    return (
        <div id="root" class="relative w-[960px] h-[540px] overflow-hidden bg-white">
            <canvas id="fx" class="absolute left-0 top-0 w-[960px] h-[540px]" width={W} height={H} />
        </div>
    );
}
