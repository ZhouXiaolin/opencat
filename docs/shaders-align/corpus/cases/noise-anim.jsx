// shaders 对齐案 noise-anim：createRendererFromJSON + 逐帧 renderFrame。
// 挂载 = g2 同构（声明 canvas 只是容器，自建 canvas appendChild 冒用 id）。
// renderer 挂 globalThis（entry 模块每帧重评估，realm 全局持久）；delta
// 按帧号补齐——同帧的多次 effect 运行（analyze+render phase）幂等。
import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

export const meta = { width: W, height: H, fps: FPS, duration: CASES["noise-anim"].frames / FPS };

const CASE = CASES["noise-anim"];

export default function Composition() {
    useEffect(() => {
        const st = (globalThis.__ocShadersCase ??= { r: null, ready: false, last: -1 });
        if (!st.r) {
            const host = document.querySelector('#fx');
            const canvas = document.createElement('canvas');
            canvas.width = W;
            canvas.height = H;
            host.appendChild(canvas);
            st.r = createRendererFromJSON({ components: CASE.components });
            // initialize 是 async：录制桩立即 resolve，落定在本帧微任务
            // drain——then 里的首绘（补齐 delta + 重绘）同帧进流。
            st.r.initialize(canvas)
                .then(() => {
                    st.ready = true;
                    const f = globalThis.ctx.global.frame;
                    while (st.last < f) {
                        st.r.renderFrame({ deltaSeconds: 1 / FPS });
                        st.last++;
                    }
                    return st.r.renderFrame({ deltaSeconds: 0 });
                })
                .catch((e) => globalThis.__ocStderr?.(
                    '[shaders] initialize failed: ' + (e && (e.stack || e.message))));
            return;
        }
        if (!st.ready) return;
        const f = globalThis.ctx.global.frame;
        while (st.last < f) {
            st.r.renderFrame({ deltaSeconds: 1 / FPS });
            st.last++;
        }
        st.r.renderFrame({ deltaSeconds: 0 });
    });
    return (
        <div id="root" class="relative w-[960px] h-[540px] overflow-hidden bg-white">
            <canvas id="fx" class="absolute left-0 top-0 w-[960px] h-[540px]" width={W} height={H} />
        </div>
    );
}
