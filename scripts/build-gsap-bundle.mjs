// 重建 core 里嵌入的 gsap bundle（crates/opencat-core/src/script/runtime/gsap/）。
//
// 官方 npm gsap 3.15 原样打包，源码不改。入口是 runtime/gsap/entry.js，
// 收录 core + CSSPlugin + goal.md §4 的全部非交互插件；交互类
// （ScrollTrigger/Draggable/Observer/InertiaPlugin/ScrollSmoother）、
// 外部库桥（Easel/Pixi）、开发工具与 CSSRulePlugin 不收录。
//
// 产物是 esbuild 的 CJS 构建包进一层 IIFE，把包命名空间挂在
// globalThis.__ocGsapModule 上（jsx_runtime.js 把它装配成 builtins['gsap']）。
//
// 用法： bun scripts/build-gsap-bundle.mjs
//   依赖 examples/k3-promo/node_modules（npm gsap 3.15.0 + esbuild）。

import { readFileSync, writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const ENTRY = join(ROOT, 'crates/opencat-core/src/script/runtime/gsap/entry.js');
const OUTPUT = join(ROOT, 'crates/opencat-core/src/script/runtime/gsap/gsap.bundle.js');
const NODE_MODULES = join(ROOT, 'examples/k3-promo/node_modules');
// 仓库根没有 package.json，esbuild 从示例工程的 node_modules 取。
const { build } = await import(pathToFileURL(join(NODE_MODULES, 'esbuild/lib/main.js')).href);

const HEADER = `// Real gsap (npm \`gsap\` 3.15.0) bundled by esbuild (ESM->CJS) and wrapped as
// an IIFE that parks the package namespace on globalThis.__ocGsapModule.
// Regenerate: bun scripts/build-gsap-bundle.mjs  (entry: entry.js)
// Contents: core + CSSPlugin + every non-interactive plugin (goal.md §4).
// The bundle self-detects the missing window and never engages rAF/ticker;
// timelines are driven deterministically: paused + tl.time(ctx.global.time).
`;

const result = await build({
    entryPoints: [ENTRY],
    bundle: true,
    format: 'cjs',
    platform: 'browser',
    target: 'es2020',
    charset: 'utf8',
    write: false,
    nodePaths: [NODE_MODULES],
    logLevel: 'warning',
});

const code = readFileSync(ENTRY, 'utf8');
const body = result.outputFiles[0].text;
const wrapped = `${HEADER}(function (module) {\n${body}\n})(globalThis.__ocGsapModule = { exports: {} });\n`;
writeFileSync(OUTPUT, wrapped);
console.log(`[gsap] ${OUTPUT} (${wrapped.length} bytes) from ${code.split('\n').length}-line entry`);
