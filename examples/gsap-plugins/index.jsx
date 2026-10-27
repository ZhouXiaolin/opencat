// GSAP 插件统一示例 —— 每个插件一个场景，全部用官方 npm 写法。
//
// 作者面只有真 gsap：`import { gsap } from 'gsap'` 加上各插件自己的
// 子路径导入（`import { CustomEase } from 'gsap/CustomEase'`）。插件由
// core 的 bundle（crates/opencat-core/src/script/runtime/gsap/entry.js）
// 在 realm 启动时注册，示例只负责用。
//
// 场景与 goal.md §6 的阶段一一对应：
//   1. eases      CustomEase / CustomWiggle / CustomBounce / SlowMo /
//                 ExpoScaleEase / RoughEase —— 纯数学，不碰 DOM
//   2. physics    Physics2D / PhysicsProps —— 纯数学
//   3. DOM 依赖   ScrambleText / Text / MotionPath / DrawSVG / MorphSVG ——
//                 目标必须像真元素（tagName/style/getBBox/textContent）
//
// 待补：SplitText（构造器形态，拆分要建真节点树，等宿主补齐
// createElement/appendChild 通道）与 Flip（也是构造器 + 布局盒读回）。
//
// 验收走 Chrome 参考帧比对。比对必须无损（PNG 序列，或 `-qp 0` 全无损
// 编码）：两侧各走一次有损 x264 时，同一张图编解码一次的偏差就有
// mae ≈ 3，比插件差异大一个数量级，量到的就成了编码器差异。
import { useGSAP } from 'opencat/runtime';
import { gsap } from 'gsap';
import { CustomEase } from 'gsap/CustomEase';
import { CustomWiggle } from 'gsap/CustomWiggle';
import { CustomBounce } from 'gsap/CustomBounce';
import { SlowMo, ExpoScaleEase, RoughEase } from 'gsap/EasePack';
import { Physics2DPlugin } from 'gsap/Physics2DPlugin';
import { PhysicsPropsPlugin } from 'gsap/PhysicsPropsPlugin';
import { SplitText } from 'gsap/SplitText';
import { MorphSVGPlugin } from 'gsap/MorphSVGPlugin';
import { DrawSVGPlugin } from 'gsap/DrawSVGPlugin';
import { MotionPathPlugin } from 'gsap/MotionPathPlugin';
import { ScrambleTextPlugin } from 'gsap/ScrambleTextPlugin';
import { TextPlugin } from 'gsap/TextPlugin';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 12 };

// 两侧必须落在同一个字体文件上，否则比的不是插件语义，是字体替换。
// Chrome 参考页（tools/gsap-oracle.mjs）用 @font-face 把同一个文件绑成
// `oracle-sans`；这里声明成 `Inter`，两个 family 名指向同一份字节。
export const fonts = {
    default: 'sans',
    faces: [
        { id: 'sans', family: 'Inter', path: '../../assets/Inter-Regular.ttf', role: 'sans' },
        { id: 'sans-medium', family: 'Inter', path: '../../assets/Inter-Medium.ttf' },
    ],
};

// 每个 ease 一行：同一条 0→1420 的横向位移，唯一变量是 ease。
var ROWS = [
    { id: 'ease-linear', name: 'none (linear)', ease: 'none', color: '#3b82f6' },
    { id: 'ease-customease', name: 'CustomEase spring', ease: 'spring', color: '#22c55e' },
    { id: 'ease-wiggle', name: 'CustomWiggle anticipate', ease: 'wiggle', color: '#eab308' },
    { id: 'ease-bounce', name: 'CustomBounce', ease: 'bounce', color: '#f97316' },
    { id: 'ease-slowmo', name: 'SlowMo 0.7/0.7', ease: 'slow(0.7,0.7)', color: '#a855f7' },
    { id: 'ease-expo', name: 'ExpoScaleEase', ease: 'expoScale(0.5,7,none)', color: '#06b6d4' },
    { id: 'ease-rough', name: 'RoughEase', ease: 'rough(2,3,0.4)', color: '#ec4899' },
];

// SplitText 场景的原文。三段各自 type 不同，共用同一条时间轴：
//   chars —— 逐字从下方升起并淡入
//   words —— 逐词淡入
//   lines —— 靠宽度自动换行，逐行从右侧滑入
// 三段都给了确定宽度：`lines` 的行检测读每个顶层子级的盒子，只有换行过
// 才有多个 top 值；确定宽度是 opencat 侧允许换行的条件。
var SPLIT_CHARS = 'SplitText';
var SPLIT_WORDS = 'one word at a time';
var SPLIT_LINES = 'three lines of split text stacked in a column';

function EaseRow(props) {
    return (
        <div
            id={props.id}
            style={{
                position: 'absolute',
                left: 60,
                top: props.y,
                width: 70,
                height: 70,
                'border-radius': 10,
                'background-color': props.color,
            }}
        />
    );
}

export default function PluginsShowcase() {
    useGSAP((ctx) => {
        // ── 1. eases 全家 ────────────────────────────────────────────
        // 官方写法：CustomEase 用一条 SVG 路径描述缓动曲线。
        CustomEase.create('spring', 'M0,0 C0.126,0.382 0.282,0.674 0.44,0.822 0.632,1.002 0.818,1.001 1,1');
        CustomWiggle.create('wiggle', { wiggles: 8, type: 'anticipate' });
        CustomBounce.create('bounce', { strength: 0.7, squash: 0.4 });

        // 一条 timeline 承载全部场景。定位只用 timeline 的 position 参数——
        // 独立的 gsap.fromTo(target, from, to) 没有第 4 个参数，写上去会被
        // 忽略（与官方行为一致）。
        var tl = gsap.timeline();
        for (var i = 0; i < ROWS.length; i++) {
            // rough 单独处理（见下）：它的缓动表默认随机生成。
            if (ROWS[i].id === 'ease-rough') continue;
            tl.fromTo(ROWS[i].id, { x: 0 }, { x: 1560, duration: 3, ease: ROWS[i].ease }, 0);
        }

        // RoughEase 默认 randomize:true，缓动表由 Math.random 生成——两侧
        // 的随机序列不同，像素就永远对不齐。确定性写法是对象参数
        // `rough({...})`：字符串形式 `rough(2,3,0.4,...)` 会把参数按位置
        // 传给 config()，只有第一个（points）生效，randomize 仍是默认 true。
        tl.fromTo(
            'ease-rough',
            { x: 0 },
            {
                x: 1560,
                duration: 3,
                ease: 'rough({strength:3, points:2, taper:"none", randomize:false})',
            },
            0
        );

        // ── 2. 物理插件 ──────────────────────────────────────────────
        // 物理插件自己驱动属性：再写一个同名的普通 tween（{x: 1400}）会生成
        // 第二条 PropTween 把它覆盖成线性插值。所以 vars 里只给物理参数，
        // 终点交给插件算——与 Chrome 里官方包的行为逐值一致。
        tl.fromTo(
            'phys2d',
            { x: 0, y: 0 },
            {
                duration: 3,
                ease: 'none',
                physics2D: { velocity: 700, angle: -50, gravity: 900 },
            },
            4.2
        );
        tl.fromTo(
            'physprops',
            { x: 0, opacity: 1 },
            {
                duration: 3,
                ease: 'none',
                physicsProps: { x: { velocity: 620, acceleration: 260 } },
            },
            4.2
        );

        // ── 3. DOM 依赖插件 ──────────────────────────────────────────
        // 每个都是一次「插件读目标」的调用：str/isSVG/tagName/getBBox/
        // getAttribute('d')/getTotalLength 全落在宿主面上。

        // DrawSVG：沿路径描出。插件把已画长度写成 stroke-dasharray 的
        // 实线段（"1200px,1400px"），剩下的是空白段。
        tl.fromTo('drawsvg-path', { drawSVG: '0%' }, { drawSVG: '100%', duration: 2.4, ease: 'none' }, 7.2);

        // MorphSVG：起点路径 → 终点路径。插件读目标的 getAttribute('d')
        // 作为起点，终点写进 vars。
        tl.to(
            'morph-path',
            {
                morphSVG: 'M 1040 740 C 1040 500 1380 500 1380 740 C 1380 980 1720 980 1720 740',
                duration: 2.4,
                ease: 'none',
            },
            7.2
        );

        // MotionPath：把方块沿着一条路径推。路径坐标是绝对像素，方块自己
        // 的 left/top 就是起点，所以不需要 align。
        tl.to(
            'motionpath-dot',
            {
                duration: 2.4,
                ease: 'none',
                motionPath: {
                    path: 'M 100 900 C 400 700 700 1000 1000 800 C 1200 660 1400 900 1600 760',
                },
            },
            7.2
        );

        // ScrambleText / Text 写目标的文本通道。
        //
        // `chars` 给单字符集，不是 `'upperCase'`：插件的字符表是
        // `Math.random` 洗出来的（CharSet 用随机索引从字符集里取字），
        // 两侧同一个 `'upperCase'` 会得到不同的乱码，像素永远对不齐。
        // 单字符集让未揭示位全部显示同一个字，揭示进度仍然逐帧可见，
        // 而两侧输出逐字节相同——和 RoughEase 用 `randomize:false` 同理。
        tl.to(
            'scramble-text',
            {
                duration: 2.4,
                ease: 'none',
                scrambleText: { text: 'DECODED PAYLOAD', chars: 'X', speed: 1, revealDelay: 0.2 },
            },
            7.2
        );
        tl.to('text-target', { duration: 2.4, ease: 'none', text: { value: 'typed by TextPlugin' } }, 7.2);

        // ── 4. SplitText ─────────────────────────────────────────────
        // 官方构造器形态：`new SplitText(el, {type})` 会真的拆节点树
        // （char/word/line wrapper 都是 createElement 出来的元素），返回的
        // `.chars/.words/.lines` 就是这些 wrapper，交给 timeline 正常 tween。
        //
        // 每帧重建是它的固有语义：hook 每帧重跑，构造器先 revert 上一次的
        // 拆分再重新拆，所以下面的 tween 拿到的永远是本帧的节点。
        var splitChars = new SplitText(ctx.getNode('split-chars'), { type: 'chars' });
        var splitWords = new SplitText(ctx.getNode('split-words'), { type: 'words' });
        var splitLines = new SplitText(ctx.getNode('split-lines'), { type: 'chars,words,lines' });

        // chars：逐字从下方 40px 升起。位移写在 char wrapper 上，渲染面按
        // 字节区间把它折成每个字素的位移。
        //
        // 三段都用 fromTo 而不是 from：hook 每帧重跑、timeline 每帧重建，
        // 而 from 的终点要读目标的当前值。上一帧 immediateRender 写下的值
        // 会留在 store 里成为下一帧的终点，动画就塌成静止。fromTo 两端
        // 都显式给出，与帧数无关——示例集里其它场景也是这个写法。
        tl.fromTo(splitChars.chars, { y: 40, opacity: 0 },
            { y: 0, opacity: 1, duration: 0.6, ease: 'power2.out', stagger: 0.06 }, 9.6);
        // words：逐词淡入。
        tl.fromTo(splitWords.words, { opacity: 0 },
            { opacity: 1, duration: 0.5, ease: 'none', stagger: 0.14 }, 9.9);
        // lines：逐行从右侧滑入。
        tl.fromTo(splitLines.lines, { x: 220, opacity: 0 },
            { x: 0, opacity: 1, duration: 0.55, ease: 'power3.out', stagger: 0.16 }, 10.2);
    });

    return (
        <div
            id="stage"
            style={{
                position: 'absolute',
                left: 0,
                top: 0,
                width: 1920,
                height: 1080,
                'background-color': '#0b1017',
            }}
        >
            <div
                id="title"
                style={{
                    position: 'absolute',
                    left: 60,
                    top: 40,
                    'font-size': 34,
                    color: '#e6edf6',
                }}
            >
                GSAP plugins — official package, unmodified
            </div>
            <div
                id="subtitle"
                style={{
                    position: 'absolute',
                    left: 60,
                    top: 86,
                    'font-size': 20,
                    color: '#5c6b7f',
                }}
            >
                CustomEase · CustomWiggle · CustomBounce · SlowMo · ExpoScaleEase · RoughEase
            </div>
            {ROWS.map(function (r, i) {
                return <EaseRow key={r.id} id={r.id} color={r.color} y={170 + i * 96} />;
            })}
            <div
                id="phys-label"
                style={{
                    position: 'absolute',
                    left: 60,
                    top: 880,
                    'font-size': 20,
                    color: '#5c6b7f',
                }}
            >
                Physics2D · PhysicsProps
            </div>
            <div
                id="phys2d"
                style={{
                    position: 'absolute',
                    left: 60,
                    top: 930,
                    width: 70,
                    height: 70,
                    'border-radius': 8,
                    'background-color': '#38bdf8',
                }}
            />
            <div
                id="physprops"
                style={{
                    position: 'absolute',
                    left: 200,
                    top: 930,
                    width: 70,
                    height: 70,
                    'border-radius': 8,
                    'background-color': '#f472b6',
                }}
            />
            <div
                id="dom-label"
                style={{ position: 'absolute', left: 60, top: 1000, 'font-size': 18, color: '#5c6b7f' }}
            >
                DrawSVG · MorphSVG · MotionPath · ScrambleText · Text
            </div>
            <path
                id="drawsvg-path"
                d="M 1040 340 C 1140 160 1280 160 1380 340 C 1480 520 1620 520 1720 340"
                view-box="0 0 1920 1080"
                style={{
                    position: 'absolute',
                    left: 0,
                    top: 0,
                    width: 1920,
                    height: 1080,
                    fill: 'none',
                    stroke: '#22d3ee',
                    'stroke-width': 6,
                }}
            />
            <path
                id="morph-path"
                d="M 1040 740 C 1140 560 1280 560 1380 740 C 1480 920 1620 920 1720 740"
                view-box="0 0 1920 1080"
                style={{
                    position: 'absolute',
                    left: 0,
                    top: 0,
                    width: 1920,
                    height: 1080,
                    fill: 'none',
                    stroke: '#a78bfa',
                    'stroke-width': 6,
                }}
            />
            <div
                id="motionpath-dot"
                style={{
                    position: 'absolute',
                    left: 0,
                    top: 0,
                    width: 26,
                    height: 26,
                    'border-radius': 13,
                    'background-color': '#fbbf24',
                }}
            />
            <p
                id="scramble-text"
                style={{
                    position: 'absolute',
                    left: 60,
                    top: 700,
                    'font-size': 40,
                    color: '#e2e8f0',
                }}
            >
                {/* 花括号包住：裸写的 JSX 文本会带上换行与缩进，
                    ScrambleText 的 lengthDif 按初始文本长度算，多一个
                    空格就少揭示一位。 */}
                {'DECODED PAYLOAD'}
            </p>
            <p
                id="text-target"
                style={{
                    position: 'absolute',
                    left: 60,
                    top: 780,
                    'font-size': 32,
                    color: '#94a3b8',
                }}
            >
                {'start'}
            </p>
            {/* SplitText 三段：同一块区域三行，各用不同 type 拆分。 */}
            <p
                id="split-chars"
                style={{
                    position: 'absolute',
                    left: 700,
                    top: 690,
                    'font-size': 56,
                    color: '#f8fafc',
                }}
            >
                {SPLIT_CHARS}
            </p>
            <p
                id="split-words"
                style={{
                    position: 'absolute',
                    left: 700,
                    top: 770,
                    'font-size': 32,
                    color: '#a5b4fc',
                }}
            >
                {SPLIT_WORDS}
            </p>
            <p
                id="split-lines"
                style={{
                    position: 'absolute',
                    left: 700,
                    top: 830,
                    width: 380,
                    'font-size': 28,
                    color: '#67e8f9',
                }}
            >
                {SPLIT_LINES}
            </p>
        </div>
    );
}
