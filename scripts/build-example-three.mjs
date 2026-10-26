// 为示例预打包项目侧 three bundle：`<example>/dist/three.bundle.mjs`。
//
// 引擎不内嵌 three（core 纯化）：渲染期按约定读取该 sidecar 注入模块表。
// vite 工程（opencat-plugin.mjs）在 closeBundle 自动生成；本脚本服务
// 裸 `.jsx` 示例与手工补建，模式同 build-example-gsap.mjs。
//
// 用法：
//   bun scripts/build-example-three.mjs examples/three-min [more…]
//   bun scripts/build-example-three.mjs --all   # examples/* 下所有目录
//
// 项目 node_modules 没装 three 时跳过——没有 three import 的示例不需要
// sidecar。three 从项目自身依赖解析（nodePaths），版本跟项目走。

import { existsSync, mkdirSync, readFileSync, readdirSync, statSync } from 'node:fs';
import { join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

// three 0.181 上游 bug：`} catch ( error ) { error( 'WebGLState:', … ) }` 等
// catch 参数遮蔽模块级 error 工具函数，运行到该分支即 TypeError；esbuild 的
// rename 忠实保留这个 bug。只能在打包前的源文本上修：catch 内遮蔽调用改
// console.error，catch 外的同名正常调用（如 Invalid blending 分支）不动。
// 遮蔽分布：three.module.js（WebGLState×10、WebGLRenderer 构造×1）、
// three.core.js（DataTextureLoader onError 缺省分支×1）；three.cjs 同款。
// esbuild 走 package.json 的 import 条件，入口是 three.module.js（引用
// three.core.js），不是 main 指的 three.cjs——三个文件都覆盖。
const THREE_SHADOWED_CALLS = [
    ["error( 'WebGLState:', error )", "console.error( 'WebGLState:', error )"],
    ["error( 'WebGLRenderer: ' + error.message )", "console.error( 'WebGLRenderer: ' + error.message )"],
    ["error( error )", "console.error( error )"],
];
const threeShadowPatch = {
    name: 'three-shadowed-error',
    setup(build) {
        build.onLoad({ filter: /three[\\/]build[\\/]three(\.core|\.module)?\.(cjs|js)$/ }, (args) => {
            let src = readFileSync(args.path, 'utf8');
            for (const [from, to] of THREE_SHADOWED_CALLS) src = src.split(from).join(to);
            return { contents: src, loader: 'js' };
        });
    },
};

const ROOT = resolve(import.meta.dirname, '..');
const ENTRY = join(ROOT, 'scripts', 'three-entry.mjs');
const FALLBACK_NODE_MODULES = join(ROOT, 'examples', 'k3-promo', 'node_modules');

async function esbuildFor(projectNodeModules) {
    for (const nm of [projectNodeModules, FALLBACK_NODE_MODULES]) {
        const main = join(nm, 'esbuild', 'lib', 'main.js');
        if (existsSync(main)) return import(pathToFileURL(main).href);
    }
    throw new Error('[three] esbuild not found (project and k3-promo node_modules)');
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
    if (!existsSync(join(nm, 'three'))) {
        console.error(`[three] ${dir}: no three in node_modules, skipped`);
        return true; // 不装 three 的示例不是失败
    }
    const { build } = await esbuildFor(nm);
    const outfile = join(abs, 'dist', 'three.bundle.mjs');
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
        plugins: [threeShadowPatch],
    });
    console.log(`[three] ${dir}/dist/three.bundle.mjs  ${statSync(outfile).size} bytes`);
    return true;
}

const args = process.argv.slice(2);
if (args.length === 0) {
    console.error('usage: bun scripts/build-example-three.mjs <example-dir>… | --all');
    process.exit(2);
}
const targets = args[0] === '--all' ? candidateDirs(join(ROOT, 'examples')) : args;
let failed = 0;
for (const dir of targets) {
    const ok = await build(dir);
    if (!ok) failed++;
}
process.exit(failed === 0 ? 0 : 1);
