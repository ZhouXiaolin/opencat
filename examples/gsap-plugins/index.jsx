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
//   3. 之后是 DOM 依赖插件（ScrambleText/Text/MotionPath/DrawSVG/
//      MorphSVG/SplitText/Flip），等宿主面补齐后逐个接上。
import { useGSAP } from 'opencat/runtime';
import { gsap } from 'gsap';
import { CustomEase } from 'gsap/CustomEase';
import { CustomWiggle } from 'gsap/CustomWiggle';
import { CustomBounce } from 'gsap/CustomBounce';
import { SlowMo, ExpoScaleEase, RoughEase } from 'gsap/EasePack';
import { Physics2DPlugin } from 'gsap/Physics2DPlugin';
import { PhysicsPropsPlugin } from 'gsap/PhysicsPropsPlugin';

export const meta = { width: 1920, height: 1080, fps: 30, duration: 12 };

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
    useGSAP(() => {
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
        </div>
    );
}
