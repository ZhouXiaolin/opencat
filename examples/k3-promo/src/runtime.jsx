// opencat 拓展面（标准 web 之外的全部语义，作者工程只 import 这一个文件）：
//   - Composition：组合根（data-composition-id / duration / width / height /
//     fps 协议属性，引擎据此选根、定时长分辨率）。
//   - Timeline / Scene / Transition：场景时间轴与转场标签组件。编译产物里
//     它们是标准 HTML 自定义标签，浏览器降级为普通容器；引擎识别其语义
//     （时间窗树移除 + 转场算子）。
//
// 驱动结构：时间与时长只从 ctx.current / ctx.global 读（引擎节点级 / 全局
// 钟）。单工程单组合（k3/liquid）用全局主 tl；多场景工程用 useTween(id,
// start, build) 注册场景局部 tl。onFrame 的回调参数是 ctx.current.time。

import { onMount } from 'solid-js';
import gsap from 'gsap';

export const CK = globalThis.CanvasKit;
export const COMPOSITION_DURATION = 16.4667;

export function Composition(props) {
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

// ── 单组合主时间轴 ───────────────────────────────────────
// k3 整片一个组合，ctx.current 即全局钟。所有场景 tween 挂这一条 tl，
// 用绝对时间（0-COMPOSITION_DURATION）。
export const tl = gsap.timeline({ paused: true });

const pendingTweens = [];
export function useTween(build) {
  pendingTweens.push(build);
}
function flushTweens() {
  if (!pendingTweens.length) return;
  var builds = pendingTweens.slice();
  pendingTweens.length = 0;
  builds.forEach(function (build) { build(tl); });
}

const frameHooks = [];
const globalFrameHooks = [];
export function onFrame(f) { frameHooks.push(f); }
export function onGlobalFrame(f) { globalFrameHooks.push(f); }

function currentTime() {
  return globalThis.ctx && globalThis.ctx.current ? globalThis.ctx.current.time : 0;
}
function globalTime() {
  return globalThis.ctx && globalThis.ctx.global ? globalThis.ctx.global.time : 0;
}

tl.to({ u: 0 }, {
  u: 1,
  duration: COMPOSITION_DURATION,
  ease: 'none',
  onUpdate: () => {
    flushTweens();
    var t = currentTime();
    for (const f of frameHooks) f(t);
    var g = globalTime();
    for (const f of globalFrameHooks) f(g);
  },
}, 0.001);
window.__timelines = window.__timelines || {};
window.__timelines['main'] = tl;

export { onMount };
