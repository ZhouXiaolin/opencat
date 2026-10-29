// g-canvaskit 的 esm dist 顶部带着死的 node 内建导入（rollup interop 残留：
// `import require$$0 from 'path'`，全文零使用）。浏览器目标下 esbuild 解析
// 不到，用空对象 stub 掉即可。
export default {};
