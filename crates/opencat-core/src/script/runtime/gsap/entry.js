// gsap 真包入口：core + CSSPlugin + goal.md §4 全部非交互插件。
// 由 scripts/build-gsap-bundle.mjs 用 esbuild 打成 IIFE，产物是
// gsap.bundle.js（include_str! 进 core）。
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
