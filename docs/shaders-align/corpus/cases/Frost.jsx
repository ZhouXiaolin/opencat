// shaders 对齐案 Frost（动画）：createRendererFromJSON + 逐帧 renderFrame（每帧恰一次）。
// 钟协议与 ref-entry.mjs 相同：帧 0 delta 0（首绘无推进），帧 i 的时钟 =
// 累加 i 次 delta 1/30；引擎帧号跳号时补齐中间帧（时钟不缺步）。
// 挂载 = g2 同构（声明 canvas 只是容器，自建 canvas appendChild 冒用 id）。
import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

// rAF 全程禁用：lib requestRender 的墙钟渲染会推进 animatedTime，
// corpus 协议时间只经 deltaSeconds 推进（ref-entry.mjs 同禁）。
globalThis.requestAnimationFrame = () => 0;

export const meta = { width: W, height: H, fps: FPS, duration: CASES["Frost"].frames / FPS };

const CASE = CASES["Frost"];

export default function Composition() {
    useEffect(() => {
        const st = (globalThis.__ocShadersCase ??= { r: null, ready: false, done: -1, pushed: 0 });
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
            // 注意 then 只设 ready/首绘，**不写 done**：真 WebGPU 端
            // （web 腿）onSubmittedWorkDone 是浏览器任务级异步，then 尾
            // 可能迟到数帧——若它把 done 打回 0，下一帧的补齐循环会重推
            // 已推过的 delta（钟快 N 帧 → 动画相位恒差）。delta 推进量
            // 由 pushed 绝对计数（帧 f 的钟恒 = f/FPS，同 ref-entry.mjs）。
            st.r.initialize(canvas)
                .then(() => {
                    st.ready = true;
                    return st.r.renderFrame({ deltaSeconds: 0 });
                })
                .catch((e) => globalThis.__ocStderr?.(
                    '[shaders] initialize failed: ' + (e && (e.stack || e.message))));
            return;
        }
        if (!st.ready) return;
        const f = globalThis.ctx.global.frame;
        if (st.done >= f) return; // 同帧的后续 effect 运行：不重复渲染
        // 绝对推进：帧 f 的钟 = f 次 delta（帧 0 的 delta 0 已在首绘里）。
        // 跳号（f 快进）天然由 while 补齐，与 done 无关。
        while (st.pushed < f) {
            st.r.renderFrame({ deltaSeconds: 1 / FPS });
            st.pushed++;
        }
        st.done = f;
    });
    return (
        <div id="root" class="relative w-[960px] h-[540px] overflow-hidden bg-white">
            <canvas id="fx" class="absolute left-0 top-0 w-[960px] h-[540px]" width={W} height={H} />
        </div>
    );
}
