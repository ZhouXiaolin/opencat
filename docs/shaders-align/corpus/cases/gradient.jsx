// shaders 对齐案 gradient：createRendererFromJSON + 静态重绘（delta 0）。
// 案帧数 presets.frames、每帧 delta 0——time 恒 0，两侧同钟。
// 挂载 = g2 同构（声明 canvas 只是容器，自建 canvas appendChild 冒用 id）。
import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

// rAF 全程禁用：lib requestRender 的墙钟渲染会推进 animatedTime，
// corpus 协议时间只经 deltaSeconds 推进（ref-entry.mjs 同禁）。
globalThis.requestAnimationFrame = () => 0;

export const meta = { width: W, height: H, fps: FPS, duration: CASES["gradient"].frames / FPS };

const CASE = CASES["gradient"];

let renderer = null;
let inited = false;
let done = -1;

export default function Composition() {
    useEffect(() => {
        const f = globalThis.ctx.global.frame;
        if (done >= f) return; // 同帧的后续 effect 运行：不重复渲染
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
                .then(() => { done = f; })
                .catch((e) => globalThis.__ocStderr?.(
                    '[shaders] initialize failed: ' + (e && (e.stack || e.message))));
            return;
        }
        if (!inited) return;
        renderer.renderFrame({ deltaSeconds: 0 });
        done = f;
    });
    return (
        <div id="root" class="relative w-[960px] h-[540px] overflow-hidden bg-white">
            <canvas id="fx" class="absolute left-0 top-0 w-[960px] h-[540px]" width={W} height={H} />
        </div>
    );
}
