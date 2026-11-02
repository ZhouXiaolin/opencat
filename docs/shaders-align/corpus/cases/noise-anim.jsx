// shaders 对齐案 noise-anim（动画）：createRendererFromJSON + 逐帧 renderFrame（每帧恰一次）。
// 钟协议与 ref-entry.mjs 相同：帧 0 delta 0（首绘无推进），帧 i 的时钟 =
// 累加 i 次 delta 1/30；引擎帧号跳号时补齐中间帧（时钟不缺步）。
// 挂载 = g2 同构（声明 canvas 只是容器，自建 canvas appendChild 冒用 id）。
import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

// rAF 全程禁用：lib requestRender 的墙钟渲染会推进 animatedTime，
// corpus 协议时间只经 deltaSeconds 推进（ref-entry.mjs 同禁）。
globalThis.requestAnimationFrame = () => 0;

export const meta = { width: W, height: H, fps: FPS, duration: CASES["noise-anim"].frames / FPS };

const CASE = CASES["noise-anim"];

export default function Composition() {
    useEffect(() => {
        const st = (globalThis.__ocShadersCase ??= { r: null, ready: false, done: -1 });
        if (!st.r) {
            const host = document.querySelector('#fx');
            const canvas = document.createElement('canvas');
            canvas.width = W;
            canvas.height = H;
            host.appendChild(canvas);
            st.r = createRendererFromJSON({ components: CASE.components });
            // initialize 是 async：录制桩立即 resolve，落定在本帧微任务
            // drain——then 里的首绘（delta 0）同帧进流。首绘必须在 then
            // 里做：微任务 drain 时机在本帧最后一次 effect 之后时，靠
            // effect 铸帧 0 会整帧落空（f0 黑帧）。失败 fail-loud。
            st.r.initialize(canvas)
                .then(() => {
                    st.ready = true;
                    return st.r.renderFrame({ deltaSeconds: 0 });
                })
                .then(() => { st.done = 0; })
                .catch((e) => globalThis.__ocStderr?.(
                    '[shaders] initialize failed: ' + (e && (e.stack || e.message))));
            return;
        }
        if (!st.ready) return;
        const f = globalThis.ctx.global.frame;
        if (st.done >= f) return; // 同帧的后续 effect 运行：不重复渲染
        // 补齐跳号（f=0 已在 then 里绘过，done=0 守卫直接跳过）。
        while (st.done < f - 1) {
            st.r.renderFrame({ deltaSeconds: 1 / FPS });
            st.done++;
        }
        if (f > 0) {
            st.r.renderFrame({ deltaSeconds: 1 / FPS });
            st.done = f;
        }
    });
    return (
        <div id="root" class="relative w-[960px] h-[540px] overflow-hidden bg-white">
            <canvas id="fx" class="absolute left-0 top-0 w-[960px] h-[540px]" width={W} height={H} />
        </div>
    );
}
