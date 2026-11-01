// ref 侧 bundle：esbuild IIFE 单文件（真包浏览器原生语义）。shaders/core 从
// corpus/node_modules 解析（npm 发布的转译产物，无 'use gpu' 残留）。
// 用法：bun ref/build.mjs   → ref/ref.bundle.js
import { build } from 'esbuild';
import { resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));

await build({
    entryPoints: [resolve(here, 'ref-entry.mjs')],
    bundle: true,
    outfile: resolve(here, 'ref.bundle.js'),
    format: 'iife',
    platform: 'browser',
    target: 'es2020',
    charset: 'utf8',
    logLevel: 'info',
});
console.log('ref: built ref.bundle.js');
