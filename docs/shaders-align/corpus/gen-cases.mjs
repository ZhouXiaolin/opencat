// 案文件生成器：presets.mjs → cases/<name>.jsx（两模板：static/anim）。
// 用法：bun gen-cases.mjs [案名...]（缺省全量重生成）。
//
// 两模板共有的协议面：
// - rAF 全程禁用（`requestAnimationFrame = () => 0`）：lib 的
//   requestRender 在 initialize 期间挂 rAF 墙钟渲染，引擎 rAF 桩当帧
//   排空会给 animatedTime（_animTime）混入一个墙钟步长（实测 0.016，
//   worley/voronoi 特征点全位移）；真浏览器侧 rAF 16ms 后才触发逃过
//   抓帧但帧间仍污染。corpus 钟协议 = 时间只经 deltaSeconds 推进，
//   两侧同禁才是同一协议。
// - initialize 的 then 里首绘同帧进流（录制桩立即 resolve，微任务
//   drain）；失败 fail-loud（__ocStderr 桥）。
// - 每引擎帧恰一次 renderFrame（done 帧号幂等守卫）：引擎 realm 对
//   同一 effect 每帧跑多次（analyze+render phase），多调对状态机类
//   （ReactionDiffusion）即多演化。
import { writeFileSync, mkdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { CASES, W, H, FPS } from './cases/presets.mjs';

const here = dirname(fileURLToPath(import.meta.url));

const HEADER_STATIC = (name) => `// shaders 对齐案 ${name}：createRendererFromJSON + 静态重绘（delta 0）。
// 案帧数 presets.frames、每帧 delta 0——time 恒 0，两侧同钟。
// 挂载 = g2 同构（声明 canvas 只是容器，自建 canvas appendChild 冒用 id）。`;

const HEADER_ANIM = (name) => `// shaders 对齐案 ${name}（动画）：createRendererFromJSON + 逐帧 renderFrame（每帧恰一次）。
// 钟协议与 ref-entry.mjs 相同：帧 0 delta 0（首绘无推进），帧 i 的时钟 =
// 累加 i 次 delta 1/${FPS}；引擎帧号跳号时补齐中间帧（时钟不缺步）。
// 挂载 = g2 同构（声明 canvas 只是容器，自建 canvas appendChild 冒用 id）。`;

const STATIC_BODY = (name) => `import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

// rAF 全程禁用：lib requestRender 的墙钟渲染会推进 animatedTime，
// corpus 协议时间只经 deltaSeconds 推进（ref-entry.mjs 同禁）。
globalThis.requestAnimationFrame = () => 0;

export const meta = { width: W, height: H, fps: FPS, duration: CASES[${JSON.stringify(name)}].frames / FPS };

const CASE = CASES[${JSON.stringify(name)}];

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
`;

const ANIM_BODY = (name) => `import { useEffect } from 'opencat/runtime';
import { createRendererFromJSON } from 'shaders';
import { CASES, W, H, FPS } from './presets.mjs';

// rAF 全程禁用：lib requestRender 的墙钟渲染会推进 animatedTime，
// corpus 协议时间只经 deltaSeconds 推进（ref-entry.mjs 同禁）。
globalThis.requestAnimationFrame = () => 0;

export const meta = { width: W, height: H, fps: FPS, duration: CASES[${JSON.stringify(name)}].frames / FPS };

const CASE = CASES[${JSON.stringify(name)}];

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
`;

const argv = process.argv.slice(2);
const names = argv.length ? argv : Object.keys(CASES);
mkdirSync(join(here, 'cases'), { recursive: true });
for (const name of names) {
    const spec = CASES[name];
    if (!spec) { console.error(`gen-cases: unknown case ${name}`); process.exit(2); }
    const file = join(here, 'cases', `${name}.jsx`);
    const body = spec.anim
        ? HEADER_ANIM(name) + '\n' + ANIM_BODY(name)
        : HEADER_STATIC(name) + '\n' + STATIC_BODY(name);
    writeFileSync(file, body);
    console.log(`gen: cases/${name}.jsx (${spec.anim ? 'anim' : 'static'}, ${spec.frames}f)`);
}
