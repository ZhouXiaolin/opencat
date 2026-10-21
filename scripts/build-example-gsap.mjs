// 为示例预打包项目侧 gsap bundle：`<example>/dist/gsap.bundle.mjs`。
//
// 引擎不再内嵌 gsap（core 纯化）：渲染期按约定读取该 sidecar 注入模块表。
// vite 工程（opencat-plugin.mjs）在 closeBundle 自动生成；本脚本服务
// 裸 `.jsx` 示例与手工补建，模式同 tools/build-example-css.mjs。
//
// 用法：
//   bun scripts/build-example-gsap.mjs examples/gsap-plugins [more…]
//   bun scripts/build-example-gsap.mjs --all      # examples/* 下所有目录
//
// 项目 node_modules 没装 gsap 时跳过——没有 gsap import 的示例不需要 sidecar。
// gsap 从项目自身依赖解析（nodePaths），版本跟项目走；esbuild 优先取项目
// 自己的，回退 k3-promo 的（仓库根无 package.json，同 build-example-css 的
// tailwind CLI 同源策略）。

import { existsSync, mkdirSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const ROOT = resolve(import.meta.dirname, '..');
const ENTRY = join(ROOT, 'scripts', 'gsap-entry.mjs');
const FALLBACK_NODE_MODULES = join(ROOT, 'examples', 'k3-promo', 'node_modules');

async function esbuildFor(projectNodeModules) {
    for (const nm of [projectNodeModules, FALLBACK_NODE_MODULES]) {
        const main = join(nm, 'esbuild', 'lib', 'main.js');
        if (existsSync(main)) return import(pathToFileURL(main).href);
    }
    throw new Error('[gsap] esbuild not found (project and k3-promo node_modules)');
}

function isDir(path) {
    try {
        return statSync(path).isDirectory();
    } catch {
        return false;
    }
}

function candidateDirs(root) {
    return readdirSync(root)
        .map((name) => join(root, name))
        .filter(isDir);
}

async function build(dir) {
    const abs = resolve(dir);
    const nm = join(abs, 'node_modules');
    if (!existsSync(join(nm, 'gsap'))) {
        console.error(`[gsap] ${dir}: no gsap in node_modules, skipped`);
        return true; // 不装 gsap 的示例不是失败
    }
    const { build } = await esbuildFor(nm);
    const outfile = join(abs, 'dist', 'gsap.bundle.mjs');
    mkdirSync(join(abs, 'dist'), { recursive: true });
    await build({
        entryPoints: [ENTRY],
        outfile,
        bundle: true,
        format: 'cjs',
        platform: 'browser',
        target: 'es2020',
        charset: 'utf8',
        logLevel: 'warning',
        nodePaths: [nm],
    });
    console.log(`[gsap] ${dir}/dist/gsap.bundle.mjs  ${statSync(outfile).size} bytes`);
    return true;
}

const args = process.argv.slice(2);
if (args.length === 0) {
    console.error('usage: bun scripts/build-example-gsap.mjs <example-dir>… | --all');
    process.exit(2);
}
const targets = args[0] === '--all' ? candidateDirs(join(ROOT, 'examples')) : args;
let failed = 0;
for (const dir of targets) {
    const ok = await build(dir);
    if (!ok) failed++;
}
process.exit(failed === 0 ? 0 : 1);
