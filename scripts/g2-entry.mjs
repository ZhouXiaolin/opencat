// g2 真包入口：@antv/g2 主包 + @antv/g-canvaskit 渲染器（scripts/
// build-example-bundle.mjs 与 vite 插件共用的 `g2` 模块段）。
//
// canvaskit-wasm 经 esbuild alias 指到本目录的 g2-canvaskit-shim.mjs：
// g-canvaskit 只 import 它的默认导出 CanvasKitInit，别名转发引擎内建
// 官方面（canvas_api.js 的 globalThis.CanvasKitInit）——库零 fork，
// 不加载 wasm，每个 draw 调用录制进目标 <canvas> 的 DrawOp 流。
import { Chart } from '@antv/g2';
import { Renderer } from '@antv/g-canvaskit';

export { Chart, Renderer };
