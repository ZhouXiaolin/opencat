// g2 真包入口：@antv/g2 主包 + 渲染器全家（build-example-bundle.mjs 与
// vite 插件共用的 `g2` 模块段）。
//
// canvaskit-wasm 经 esbuild alias 指到 g2-canvaskit-shim.mjs：g-canvaskit
// 只 import 它的默认导出 CanvasKitInit，别名转发引擎内建官方面
// （canvas_api.js 的 globalThis.CanvasKitInit）——库零 fork，不加载
// wasm，每个 draw 调用录制进目标 <canvas> 的 DrawOp 流。
//
// 引擎 realm 的对齐面（core 不嵌任何 g2 专属协议）：
//   - Chart 是作者唯一入口（`new Chart({ container, ... })`），renderer
//     兜底读 OC_G2_RENDERER：'2d' = 官方 @antv/g-canvas（G2 的内建默认
//     渲染器），绘制面是 `canvas.getContext('2d')`，runtime 的
//     context2d.js 把每次 2D 调用 lower 进同一条 DrawOp 流；'svg' = 官方
//     @antv/g-svg，绘制面是容器下的一棵私有 <svg> 子树，标记容器后由
//     宿主序列化成 markup 交给 usvg 管线（与 <svg inline> 同一条 emit
//     路径）；'webgl' = 官方 @antv/g-webgl，绘制面是
//     `canvas.getContext('webgl2')`，runtime 的 webgl_api.js 把每次 GL
//     调用录进 GpuStream（与 three 同一条路）；默认 = @antv/g-canvaskit。
//   - interaction 一律 no-op：离线帧没有交互，G2 否则安装指针监听和
//     tooltip DOM 层。
import { Chart as G2Chart, register } from '@antv/g2';
import { Renderer } from '@antv/g-canvaskit';
import { Renderer as Renderer2D } from '@antv/g-canvas';
import { Renderer as RendererSVG } from '@antv/g-svg';
import { Renderer as RendererWebGL } from '@antv/g-webgl';
import { Renderer as RendererWebGPU } from '@antv/g-webgpu';
import { Canvas as GLiteCanvas } from '@antv/g-lite';

// GLSL→WGSL 编译器（wasm-bindgen 产物）的本地路径：默认指 unpkg CDN，
// 离线必须覆盖。引擎侧 asset reader 以 bundle 目录为 base，此相对路径
// 按 corpus 布局（dist/ 与 node_modules/ 同父目录）写死；其它布局
// （scripts/template 的 examples/<dir>/dist）须按各自 base 自行供给
// wasm，否则 webgpu 渲染器初始化 fetch 失败（fail-loud 可见）。ref 腿
// （ref/entry.mjs）自行传页面绝对路径，不经此常量。
var G_WEBGPU_WASM_PATH = '../node_modules/@antv/g-device-api/dist/pkg/glsl_wgsl_compiler_bg.wasm';

// webgl 分支的绘制全部经过 g-lite Canvas.render。浏览器里空场景帧落在
// initAsync 未落定之前（inited=false 直接空转）；我们的 init 同步完成，
// g-webgl 会用空场景走真帧，RenderGraph 在 beginGraphBuilder 里断言
// currentGraph === null 后卡死，后续每帧都抛同一条 assert。跳过空场景
// 帧与浏览器观察到的顺序等价。
if (globalThis.OC_G2_RENDERER === 'webgl' || globalThis.OC_G2_RENDERER === 'webgpu') {
    const ocRender = GLiteCanvas.prototype.render;
    GLiteCanvas.prototype.render = function () {
        const root = this.document && this.document.documentElement;
        if (root && root.childNodes.length === 0) return;
        return ocRender.apply(this, arguments);
    };
}

class Chart extends G2Chart {
    constructor(config) {
        const next = Object.assign({}, config || {});
        // container/autoFit/renderer 注入必须全部在 super 之前完成：
        // G2Chart 构造器同步读 options，super 之后改 next 不生效；且
        // derived constructor 的 super 只允许调用一次（V8: "Super
        // constructor may only be called once"）。
        // 官方语义：container 可以是元素或 id 字符串。id 走
        // querySelector('#id')——声明 canvas 节点上会拿到宿主容器
        // （dom_api 的 ensureCanvasHost 语义）。
        if (typeof next.container === 'string') {
            next.container = document.querySelector('#' + next.container);
        }
        // autoFit 读的是浏览器容器 clientWidth；离线面没有真布局，统一
        // 固定尺寸（与参考侧 harness 的容器同大）。
        // webgl 不再强制 960x540：引擎侧 GL drawingBuffer 已改为读 canvas
        // 元素的 width/height 内容属性（dom_api/webgl_api 尺寸透传），
        // chart 的 720x720 由宿主 canvas 跟随。
        if (next.autoFit && !next.width) {
            next.width = 960;
            next.height = 540;
            next.autoFit = false;
        }
        // heatmap 一类 mark 需要真像素画布（模板/调色板/密度场 + toDataURL）。
        // 不传 createCanvas 时 G2 回落到 g-lite document.createElement——引擎里
        // 那是显示对象不是画布。统一注入：返回宿主 document 的 canvas 元素
        // （引擎侧是影子离屏画布，浏览器侧是真 DOM canvas，语义一致）。
        if (!next.createCanvas) {
            next.createCanvas = () => globalThis.document.createElement('canvas');
        }
        if (!next.renderer) {
            const mode = globalThis.OC_G2_RENDERER;
            const size = {
                width: Number(next.width) || 640,
                height: Number(next.height) || 480,
            };
            if (mode === '2d') {
                next.renderer = new Renderer2D();
            } else if (mode === 'svg') {
                if (typeof globalThis.__ocMarkSvgSubtree === 'function' && next.container) {
                    globalThis.__ocMarkSvgSubtree(next.container);
                }
                next.renderer = new RendererSVG(size);
            } else if (mode === 'webgl') {
                next.renderer = new RendererWebGL(size);
            } else if (mode === 'webgpu') {
                next.renderer = new RendererWebGPU({ shaderCompilerPath: G_WEBGPU_WASM_PATH });
            } else {
                next.renderer = new Renderer(size);
            }
        }
        // 离线帧只要终态：动画 tick 依赖宿主推钟，且 g-lite 动画 finished
        // 后的终值写入到不了渲染面（strokeOpacity 停在初值 0）。参考侧
        // harness 用 getAnimations 排空到终态，两侧终态同构——直接关动画，
        // 元素创建即终态。
        next.animate = false;
        super(next);
        // 批量 harness（g2-batch.jsx）的收尾强制帧靠它拿到 chart 引用。
        globalThis.__ocLastChart = this;
    }

    interaction() { return this; }

    // 离线帧推钟协议：render() 同步返回的 promise 依赖动画走完，而动画
    // tick 依赖宿主推钟——等 promise 再推钟会死锁，必须 render 内联推。
    // 动画的 startTime 在首次 raf tick 定格为当时的 timelineTime，而 G2 的
    // orchestration 是长 promise 链（dataflow/scale/layout/shape/animate 各
    // 阶段），play 的发生轮次不可预知——固定轮数 + 一次推钟会被"play 在
    // 推钟之后"打穿（动画 fill:'both' 停在 keyframe[0] 全透明）。所以分多
    // 轮循环：每轮 flush 后推钟 +10000s，无论 play 落在哪一轮，其后必有
    // 一次推钟把它推过 duration。纯浏览器（无 harness 全局）不推钟自然走。
    render() {
        // demo 的 chart.options(spec) 是整体替换（不是合并），constructor
        // 注入的 animate 会被冲掉——render 前最后注入一次。注意 options
        // 是方法不是属性：this.options() 读 spec、this.options(spec) 整体
        // 替换，直接改返回的 spec 对象即可。离线帧只要终态：动画 tick 依
        // 赖宿主推钟，且 g-lite 动画 finished 后的终值写入到不了渲染面
        // （strokeOpacity 停在初值 0）。参考侧 harness 用 getAnimations
        // 排空到终态，两侧终态同构。
        try {
            const spec = this.options();
            if (spec && typeof spec === 'object') spec.animate = false;
        } catch (e) { /* spec 未就绪则由默认动画路径兜底 */ }
        const promise = super.render();
        // 渲染失败必须可见：引擎里 console.error 是静默的，只有 __ocStderr
        // 能落到 stderr，批量脚本靠它把「出图但报错」的 demo 揪出来。
        promise.then(undefined, (e) => {
            globalThis.__ocStderr('[g2-render] rejected: ' + (e && (e.stack || e)));
        });

        if (typeof globalThis.__ocSetClock === 'function') {
            // 动画 finished 后 fill:'both' 的终值 style 写入不带渲染 dirty
            // 标记（g-lite 的 renderReasons 为空就跳过整帧），最后一次重绘
            // 会定格在动画中途。每轮推钟后手动置 CAMERA_CHANGED 再 flush，
            // 逼出终态重绘。必须用 CAMERA_CHANGED（0）而不是
            // DISPLAY_OBJECT_CHANGED（1）：后者走局部脏矩形重绘，只重画
            // 被标记的对象——离线帧把整帧 op 累积成一张图，中间那次整帧
            // clear 之后只剩「clear 后重画过的对象」，其余元素消失（图案
            // 图源异步到达时逐柱重绘，正是这种情形）。
            const forceRepaint = () => {
                try {
                    const rc = this.getContext().canvas.context.renderingContext;
                    if (rc && rc.renderReasons && rc.renderReasons.add) rc.renderReasons.add(0);
                    // webgl/webgpu 分支没有宿主 rAF 渲染循环，推钟/置脏后必须
                    // 手动驱动一帧，否则终态永远上不了 GPU 流。
                    if (globalThis.OC_G2_RENDERER === 'webgl' || globalThis.OC_G2_RENDERER === 'webgpu')
                        this.getContext().canvas.render();
                } catch (e) { /* canvas 未建好则由主循环自然重绘 */ }
            };
            // play 的时机不可预知（G2 orchestration 是长 promise 链）——把
            // stub 安装放进每轮重试：canvas/timeline 在头几轮才就绪（构造期
            // 安装会 throw 被 catch 吞掉，playCount 一直为 0，动画走真
            // timeline 且 tick 不推进，fadeIn 停在 opacity 0——heatmap 出白
            // 图的根因）。装上后 animate 的 play 直接 resolve 终态。
            const ensurePlayStub = () => {
                try {
                    const tl = this.getContext().canvas.document.timeline;
                    if (tl && !tl.__ocPlayStubbed) {
                        tl.__ocPlayStubbed = true;
                        tl.play = function () {
                            const finished = Promise.resolve();
                            return {
                                finished,
                                cancel() {},
                                currentTime: 0,
                                startTime: 0,
                                playState: 'finished',
                                effect: null,
                            };
                        };
                    }
                } catch (e) { /* canvas/timeline 未就绪，下一轮重试 */ }
            };
            let t = 0;
            for (let round = 0; round < 8; round++) {
                ensurePlayStub();
                for (let i = 0; i < 15; i++) globalThis.__ocFlushRaf();
                t += 10000;
                globalThis.__ocSetClock(t);
                forceRepaint();
            }
            for (let i = 0; i < 3; i++) globalThis.__ocFlushRaf();
            // Async data connectors (type:'fetch') stretch the orchestration
            // past these fixed rounds: the render promise only resolves once
            // every mark has its shapes, so push the clock one more stint
            // then. Harmless for sync charts (finished animations are
            // idempotent).
            promise.then(() => {
                for (let round = 0; round < 4; round++) {
                    ensurePlayStub();
                    for (let i = 0; i < 15; i++) globalThis.__ocFlushRaf();
                    t += 10000;
                    globalThis.__ocSetClock(t);
                    forceRepaint();
                }
                for (let i = 0; i < 3; i++) globalThis.__ocFlushRaf();
            }).catch(() => {});
            // （旧版一次性 tl.play stub 安装已并入上方 ensurePlayStub：构造
            // 期 canvas 未就绪会 throw 被吞，装不上还误报成功。）
        }
        return promise;
    }
}

// 离线 fetch shim：G2 的 data loader（esm/data/fetch.js）按扩展名调
// `fetch(value).json()/.text()`。批量对齐面把远程数据镜像到引擎 cwd 的
// `assets/`（download-assets.mjs），URL → 文件名映射由 demo 段注入
// `__ocAssetMap`。shim 只在有镜像表时安装（纯浏览器侧不安装）。
// 引擎 asset_api 已提供 fetch（three FileLoader 面见 asset_api.js）；
// 这里包一层 mirror 查表：demos 段注入的 `__ocAssetMap`（url → assets/
// 文件名）命中时走文本面（json/csv 数据），未命中落回引擎原 fetch。
if (typeof globalThis.fetch === 'function') {
    const baseFetch = globalThis.fetch;
    globalThis.fetch = function (url) {
        // 镜像表的键不带空白：demo 里的 URL 字面量可能带前导空格
        // （general/point/demo/point-label 的 fetch value 就是），浏览器
        // 会裁掉再请求，这里同样裁。
        url = String(url).trim();
        // The map comes from the demos segment, which evaluates after this
        // one — resolve it at call time, not install time.
        const ASSET_MAP = globalThis.__ocAssetMap;
        const name = ASSET_MAP && ASSET_MAP[String(url)];
        if (!name) {
            globalThis.__ocStderr('[g2-shim] miss: ' + url + ' map=' + (ASSET_MAP ? Object.keys(ASSET_MAP).length : 'none'));
            return baseFetch(url);
        }
        const text = __asset_fetch(name);
        return Promise.resolve({
            ok: true,
            status: 200,
            text: () => Promise.resolve(text),
            json: () => Promise.resolve(JSON.parse(text)),
        });
    };
} else {
    globalThis.fetch = function (url) {
        url = String(url).trim();
        const ASSET_MAP = globalThis.__ocAssetMap;
        const name = ASSET_MAP && ASSET_MAP[String(url)];
        if (!name) {
            return Promise.reject(new Error('fetch: no local mirror for ' + url));
        }
        const text = __asset_fetch(name);
        return Promise.resolve({
            ok: true,
            status: 200,
            text: () => Promise.resolve(text),
            json: () => Promise.resolve(JSON.parse(text)),
        });
    };
}

// 拦截面只覆盖 Chart 与渲染器配置；G2 的其余具名导出（ChartEvent、
// register、G2 常量/类型工具……）原样透传——demo 里 `chart.on(ChartEvent.X)`
// 之类的写法不能因为段入口只导出了 Chart 就拿到 undefined。
export * from '@antv/g2';
export { Chart, Renderer, Renderer2D, RendererSVG, RendererWebGL, register };
