// g2 真包入口：@antv/g2 主包 + 渲染器全家（scripts/
// build-example-bundle.mjs 与 vite 插件共用的 `g2` 模块段）。
//
// canvaskit-wasm 经 esbuild alias 指到本目录的 g2-canvaskit-shim.mjs：
// g-canvaskit 只 import 它的默认导出 CanvasKitInit，别名转发引擎内建
// 官方面（canvas_api.js 的 globalThis.CanvasKitInit）——库零 fork，
// 不加载 wasm，每个 draw 调用录制进目标 <canvas> 的 DrawOp 流。
//
// Renderer2D = 官方 @antv/g-canvas（G2 的内建默认渲染器）：绘制面是
// `canvas.getContext('2d')`，runtime 的 context2d.js 把每次 2D 调用
// lower 进同一条 DrawOp 流。
// RendererSVG = 官方 @antv/g-svg（G2 canvas 内建的第二渲染器）：绘制面是
// 容器下的一棵私有 <svg> 子树，每次 setAttribute 只是一次字符串写入——
// runtime 的 dom_api.js 给这棵子树做了影子元素，宿主再把它序列化成
// markup 交给 usvg 管线（与 <svg inline> 同一条 emit 路径）。
import { Chart } from '@antv/g2';
import { Renderer } from '@antv/g-canvaskit';
import { Renderer as Renderer2D } from '@antv/g-canvas';
import { Renderer as RendererSVG } from '@antv/g-svg';

export { Chart, Renderer, Renderer2D, RendererSVG };
