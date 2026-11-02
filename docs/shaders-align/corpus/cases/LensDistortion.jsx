// shaders 对齐案 LensDistortion：createRendererFromJSON + 静态重绘（delta 0）。
// 案帧数 presets.frames、每帧 delta 0——time 恒 0，两侧同钟。
// 挂载 = g2 同构（声明 canvas 只是容器，自建 canvas appendChild 冒用 id）。
import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

// rAF 全程禁用：lib requestRender 的墙钟渲染会推进 animatedTime，
// corpus 协议时间只经 deltaSeconds 推进（ref-entry.mjs 同禁）。
globalThis.requestAnimationFrame = () => 0;

export const meta = { width: W, height: H, fps: FPS, duration: CASES["LensDistortion"].frames / FPS };

const CASE = CASES["LensDistortion"];

// renderer 挂 globalThis（跨 driver eval 持久）：web 腿每帧重建 renderer
// 会对同一 OffscreenCanvas 重新 configure——真 IPC 快于帧末回读时交换链
// 被清空，帧末读到空白帧（间歇白帧）。anim 模板同理（__ocShadersCase）。
const st = (globalThis.__ocShadersCaseStatic ??= { r: null, inited: false, done: -1 });

export default function Composition() {
    useEffect(() => {
        const f = globalThis.ctx.global.frame;
        if (st.done >= f) return; // 同帧的后续 effect 运行：不重复渲染
        if (!st.r) {
            const host = document.querySelector('#fx');
            const canvas = document.createElement('canvas');
            canvas.width = W;
            canvas.height = H;
            host.appendChild(canvas);
            st.r = createRendererFromJSON({ components: CASE.components });
            // initialize 是 async：录制桩立即 resolve，落定在本帧微任务
            // drain——then 里的首绘（delta 0）同帧进流。失败 fail-loud。
            // then 尾必须写 done（与 anim 模板不同）：static 案无补齐
            // 循环，done 只当幂等守卫——web 腿 warmup（test-oracle
            // warmupNativeAsync）在首绘落定后会重绘同一帧，若 then 首绘
            // 不写 done，重绘会再调一次 renderFrame；状态机类案
            // （ReactionDiffusion 的 compute 迭代）renderFrame 非幂等，
            // 多调一次 = 演化时间翻倍，对 ref（32 迭代）成混沌分叉假分歧。
            st.r.initialize(canvas)
                .then(() => {
                    st.inited = true;
                    return st.r.renderFrame({ deltaSeconds: 0 });
                })
                .then(() => { st.done = f; })
                .catch((e) => globalThis.__ocStderr?.(
                    '[shaders] initialize failed: ' + (e && (e.stack || e.message))));
            return;
        }
        if (!st.inited) return;
        st.r.renderFrame({ deltaSeconds: 0 });
        st.done = f;
    });
    return (
        <div id="root" class="relative w-[960px] h-[540px] overflow-hidden bg-white">
            <canvas id="fx" class="absolute left-0 top-0 w-[960px] h-[540px]" width={W} height={H} />
        </div>
    );
}
