// hyperframes-launch 的 opencat 扩展面。特色节点是原生 <timeline> 与
// <transition>：timeline 把直接子场景排成时间轴，transition 声明相邻场景间的转场段。
// 作者侧其余代码是标准 web：Solid JSX + Vite dist + gsap + three。
//
// 驱动结构：ctx.current 是节点级钟（场景起点归零），ctx.global 是全局钟。
// 每个场景用 useTween(id, start, build) 注册自己的局部 tl 到 __timelines[id]，
// 锚点写进 __ocCompOffsets——driver 到点 play + seek(ctx.current.time - start)。
// tween 用场景局部时间（0 起），onFrame 的 T = ctx.current.time - start。

import gsap from 'gsap';

export const CK = globalThis.CanvasKit;
export const COMPOSITION_DURATION = 49.8;

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

export function Transition(props) {
  const { from, to, effect, duration, ...rest } = props;
  return <transition data-from={from} data-to={to} data-effect={effect} data-duration={duration} {...rest} />;
}

export function Scene(props) {
  const { duration, start, ...rest } = props;
  return <div data-start={start} data-duration={duration} {...rest}>{props.children}</div>;
}

// ── 场景时间轴注册 ───────────────────────────────────────
// pending 按 id 分组；build(t) 的 t 是场景局部时间，tween 参数从 0 起。
const pending = {};
const registered = {};
export function useTween(id, start, build) {
  var t = registered[id];
  if (!t) {
    t = gsap.timeline({ paused: true });
    registered[id] = t;
    globalThis.__ocCompOffsets = globalThis.__ocCompOffsets || {};
    globalThis.__ocCompOffsets[id] = start;
    window.__timelines = window.__timelines || {};
    window.__timelines[id] = t;
  }
  (pending[id] = pending[id] || []).push({ tl: t, build: build, start: start });
  return t;
}

const frameHooks = [];
// onFrame 的 T = ctx.global.time（整片钟）。单文件 dist 中引擎每帧只设一次
// ctx（global），场景局部时钟由 useTween 的独立 tl 表达；需要局部时间的
// Canvas 绘制回调自行减场景 anchor（T - start）。
export function onFrame(f) { frameHooks.push(f); }

function currentTime() {
  return globalThis.ctx && globalThis.ctx.current ? globalThis.ctx.current.time : 0;
}
function globalTime() {
  return globalThis.ctx && globalThis.ctx.global ? globalThis.ctx.global.time : 0;
}

// proxy tl：全片 duration 的 span，每帧 onUpdate 触发一次。所有场景 tl
// 由 driver 按 __ocCompOffsets 锚点 play+seek；proxy 只负责驱动帧钩子。
export const tl = gsap.timeline({ paused: true });
tl.to({ u: 0 }, {
  u: 1,
  duration: COMPOSITION_DURATION,
  ease: 'none',
  onUpdate: () => {
    // 首帧落地所有场景补间（DOM 已挂载），再读 ctx 时间跑帧钩子。
    for (var id in pending) {
      var items = pending[id];
      delete pending[id];
      items.forEach(function (item) { item.build(item.tl); });
    }
    var gt = globalTime();
    for (const f of frameHooks) f(gt);
  },
}, 0.001);
window.__timelines = window.__timelines || {};
window.__timelines['main'] = tl;
