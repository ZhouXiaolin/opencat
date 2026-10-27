// gsap 真包入口：core + CSSPlugin + 全部官方插件。
// 由 opencat-plugin.mjs（vite 工程流）或 scripts/build-example-bundle.mjs
// （裸 .jsx）用 esbuild 打成 CJS，作为单文件 bundle 里的 `gsap` 模块段
// ——渲染期由引擎按段标记注入模块表（core 不再内嵌 gsap 源码）。
//
// 收录面 = npm `gsap` 包的全部插件。非交互插件在入口预注册；交互类
// （ScrollTrigger/Draggable/Observer 等）与外部库桥、devtools 只收录导出，
// 不预注册——它们要真实浏览器宿主，作者显式 registerPlugin 时才激活，
// 在渲染环境里激活会得到明确报错而不是隐式错动画。
//
// 导出面 = npm `gsap` 主入口的完整记录（jsx_runtime.js 的包装层从
// gsapPackage.TimelineLite 取 timeline 构造器来拦截 to/from/fromTo/set，
// 缺这个导出数组 target 的字符串就不会归一）。
import {
    gsap,
    TweenLite,
    TimelineLite,
    TimelineMax,
    Power0,
    Power1,
    Power2,
    Power3,
    Power4,
    Linear,
    Quad,
    Cubic,
    Quart,
    Quint,
    Strong,
    Elastic,
    Back,
    SteppedEase,
    Bounce,
    Sine,
    Expo,
    Circ,
} from 'gsap';
import { CSSPlugin } from 'gsap/CSSPlugin';
import { CustomEase } from 'gsap/CustomEase';
import { CustomWiggle } from 'gsap/CustomWiggle';
import { CustomBounce } from 'gsap/CustomBounce';
import { SlowMo, ExpoScaleEase, RoughEase } from 'gsap/EasePack';
import { SplitText } from 'gsap/SplitText';
import { MorphSVGPlugin } from 'gsap/MorphSVGPlugin';
import { DrawSVGPlugin } from 'gsap/DrawSVGPlugin';
import { MotionPathPlugin } from 'gsap/MotionPathPlugin';
import { ScrambleTextPlugin } from 'gsap/ScrambleTextPlugin';
import { TextPlugin } from 'gsap/TextPlugin';
import { Flip } from 'gsap/Flip';
import { Physics2DPlugin } from 'gsap/Physics2DPlugin';
import { PhysicsPropsPlugin } from 'gsap/PhysicsPropsPlugin';
// 收录但不预注册：交互类、外部库桥、devtools、样式表对象依赖。
import { ScrollTrigger } from 'gsap/ScrollTrigger';
import { ScrollToPlugin } from 'gsap/ScrollToPlugin';
import { Draggable } from 'gsap/Draggable';
import { Observer } from 'gsap/Observer';
import { InertiaPlugin } from 'gsap/InertiaPlugin';
import { ScrollSmoother } from 'gsap/ScrollSmoother';
import { PixiPlugin } from 'gsap/PixiPlugin';
import { EaselPlugin } from 'gsap/EaselPlugin';
import { CSSRulePlugin } from 'gsap/CSSRulePlugin';
import { MotionPathHelper } from 'gsap/MotionPathHelper';
import { GSDevTools } from 'gsap/GSDevTools';

gsap.registerPlugin(
    CustomEase,
    CustomWiggle,
    CustomBounce,
    SlowMo,
    ExpoScaleEase,
    RoughEase,
    SplitText,
    MorphSVGPlugin,
    DrawSVGPlugin,
    MotionPathPlugin,
    ScrambleTextPlugin,
    TextPlugin,
    Flip,
    Physics2DPlugin,
    PhysicsPropsPlugin
);

export { gsap, gsap as default };
export { TweenLite, TimelineLite, TimelineMax, CSSPlugin };
export { Power0, Power1, Power2, Power3, Power4, Linear, Quad, Cubic, Quart, Quint, Strong };
export { Elastic, Back, SteppedEase, Bounce, Sine, Expo, Circ };
export { CustomEase, CustomWiggle, CustomBounce, SlowMo, ExpoScaleEase, RoughEase };
export { SplitText, MorphSVGPlugin, DrawSVGPlugin, MotionPathPlugin, ScrambleTextPlugin, TextPlugin };
export { Flip, Physics2DPlugin, PhysicsPropsPlugin };
// 收录不预注册面：import 可用，激活由作者显式 registerPlugin 触发。
export {
    ScrollTrigger, ScrollToPlugin, Draggable, Observer, InertiaPlugin, ScrollSmoother,
    PixiPlugin, EaselPlugin, CSSRulePlugin, MotionPathHelper, GSDevTools,
};
