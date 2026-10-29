// canvaskit-wasm 的零 wasm 替身（esbuild alias 'canvaskit-wasm' → 本文件，
// 仅 g2 段使用）。g-canvaskit 只 import 默认导出 CanvasKitInit；官方面
// （canvas_api.js）是唯一 CanvasKit 源——签名一处，g2 无派生面。
export default function CanvasKitInit(options) {
    if (typeof globalThis.CanvasKitInit !== 'function') {
        throw new Error('g2: the CanvasKit facade is not installed (canvas_api.js must run first)');
    }
    return globalThis.CanvasKitInit(options);
}
