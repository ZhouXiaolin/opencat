// opencat 拓展面（标准 web 之外的全部语义，作者工程只 import 这一个文件）：
//   - Composition：组合根（data-composition-id / duration / width / height /
//     fps 协议属性，引擎据此选根、定时长分辨率）。
//   - Timeline / Scene / Transition：场景时间轴与转场标签组件。编译产物里
//     它们是标准 HTML 自定义标签（timeline/scene/transition），浏览器将其
//     降级为普通容器；引擎原生识别其语义（时间窗树移除 + 转场算子）。本例
//     无转场，未用到——API 先立，转场例（inspector/vfx）dogfood。
// 驱动结构：时间与时长只从 ctx.current / ctx.global 读。onFrame 的回调
// 参数是 ctx.current.time（单组合里等于全局钟）。

import { onMount } from 'solid-js';
import gsap from 'gsap';

// 引擎态：globalThis.CanvasKit 由引擎垫片预置（签名兼容 canvaskit-wasm 的
// 标准 API 面）。浏览器预览态：首次取用前动态加载真包（渐进增强，渲染门
// 不依赖它）。
export const CK = globalThis.CanvasKit;

export const COMPOSITION_DURATION = 6.0;

export function Composition(props) {
  // Composition 组件只管标准 DOM 和协议属性；时间轴在模块装载时已挂好，
  // 不依赖组件生命周期——引擎 driver 在 init 段之后就扫 window.__timelines。
  // duration/width/height/fps 是协议属性；其余（id/class/style）照常透传。
  const { duration, width, height, fps, ...rest } = props;
  return (
    <div
      {...rest}
      data-composition-id={props.id}
      data-start="0"
      data-duration={duration}
      data-width={width}
      data-height={height}
      data-fps={fps}
    >
      {props.children}
    </div>
  );
}

export function Timeline(props) {
  const { duration, ...rest } = props;
  return <timeline data-duration={duration} {...rest}>{props.children}</timeline>;
}

export function Scene(props) {
  const { duration, start, ...rest } = props;
  return <scene data-start={start} data-duration={duration} {...rest}>{props.children}</scene>;
}

export function Transition(props) {
  const { from, to, effect, duration, ...rest } = props;
  return <transition data-from={from} data-to={to} data-effect={effect} data-duration={duration} {...rest} />;
}

// ── 主时间轴 ─────────────────────────────────────────────
// paused 主 tl：引擎 driver 扫 window.__timelines 到点 play+seek(虚拟钟)，
// 之后 gsap ticker（lagSmoothing(0) 已由 driver 设）每帧走钟。proxy span
// 覆盖全片——任意时刻 seek 都会触发 onUpdate 重放每帧钩子（html 母本的
// frame driver 惯用法，纯函数 of tl.time()，seek-safe、抽帧安全）。
export const tl = gsap.timeline({ paused: true });

// 场景补间入口：全部补间挂到唯一注册的主 tl（引擎 driver 只 play/seek
// window.__timelines 里的 timeline）。绝对时间定位与参考语料的 masterTL
// 同构；不另建子 timeline（未注册的子 tl 走 gsap 全局 ticker，与虚拟钟
// 脱节，且创建早于 DOM 时选择器目标为空）。
// Solid 组件在模块装载期调用 useTween，此时模板还未挂载——延迟到首帧
// 钩子（DOM 已存在）构建一次，之后每帧 seek 到虚拟钟即可命中目标。
const pendingTweens = [];
export function useTween(build) {
  pendingTweens.push(build);
}
function flushTweens() {
  if (!pendingTweens.length) return;
  var builds = pendingTweens.slice();
  pendingTweens.length = 0;
  // 当前 tl.time() 可能已过幕起点（跳帧渲染）——tween 按绝对时间加入后，
  // 下一行 tl.time() 的读取会自动触发 gsap 对当前时刻的渲染。
  builds.forEach(function (build) { build(tl); });
}

const frameHooks = [];
const globalFrameHooks = [];
// 组件体（树序）里调用，注册序=执行序=老 hook 语义。
export function onFrame(f) {
  frameHooks.push(f);
}
export function onGlobalFrame(f) {
  globalFrameHooks.push(f);
}

// 主 tl 的 proxy span 覆盖全片：任意时刻 play+seek 都会触发 onUpdate，
// 重放每帧钩子（纯函数 of tl.time()，seek-safe、抽帧安全）。
export function mountTimeline(durationSecs) {
  tl.to(
    { u: 0 },
    {
      u: 1,
      duration: durationSecs,
      ease: 'none',
      onUpdate: () => {
        flushTweens();
        var t = globalThis.ctx && globalThis.ctx.current ? globalThis.ctx.current.time : 0;
        for (const f of frameHooks) f(t);
        var g = globalThis.ctx && globalThis.ctx.global ? globalThis.ctx.global.time : 0;
        for (const f of globalFrameHooks) f(g);
      },
    },
    0.001,
  );
  window.__timelines = window.__timelines || {};
  window.__timelines['main'] = tl;
}

mountTimeline(COMPOSITION_DURATION);

// 组件树外的兼容导出：旧生态插件通过 onMount 钩生命周期；本例的帧逻辑
// 全部走 onFrame（注册即生效，seek-safe），不依赖挂载时机。
export { onMount };
