// 为裸 `.jsx` 示例生成 sidecar 样式表：`<example>/tailwind.css`。
//
// 背景：tailwind 原生类名解析已退役，class → 声明一律走标准 CSS 引擎；
// 标准工程流把 css 打进 bundle（`export const css`），裸 jsx 没有 bundle，
// 于是走宿主约定的同目录 sidecar（crates/opencat-engine/src/jsx.rs:
// read_sidecar_css）。本脚本只做「预编译」，渲染期不需要 node/bun。
//
// 用法：
//   bun tools/build-example-css.mjs examples/vfx-heygen-combined [more…]
//   bun tools/build-example-css.mjs --all        # examples/* 下所有含 .jsx 的目录
//
// 类名扫描范围是该目录下所有 `.jsx`（`_` 前缀的调试副本除外），所以跑之前
// 先清掉临时文件，避免把调试类的样式带进产物。

import { spawnSync } from 'node:child_process';
import { existsSync, readdirSync, statSync, writeFileSync, rmSync } from 'node:fs';

import { join, resolve } from 'node:path';
// tailwind CLI 与 core 的 bundle 同源：从 k3-promo 的依赖里取（本机没有
// node_modules 缓存时唯一可靠的一份）。
const TAILWIND_CLI = resolve(
    'examples/k3-promo/node_modules/@tailwindcss/cli/dist/index.mjs',
);

function isDir(path) {
    try {
        return statSync(path).isDirectory();
    } catch {
        return false;
    }
}

function jsxDirs(root) {
    return readdirSync(root)
        .map((name) => join(root, name))
        .filter((path) => isDir(path) && readdirSync(path).some((f) => f.endsWith('.jsx')));
}

function build(dir) {
    const abs = resolve(dir);
    const jsxFiles = readdirSync(abs).filter((f) => f.endsWith('.jsx') && !f.startsWith('_'));
    if (jsxFiles.length === 0) {
        console.error(`[css] ${dir}: no .jsx, skipped`);
        return false;
    }
    // 输入文件必须落在示例目录里：tailwind CLI 按它的位置解析
    // `@import "tailwindcss"` 与 `@source` 的 glob（临时目录里解析不到依赖）。
    const input = join(abs, '.tailwind-input.css');
    const output = join(abs, 'tailwind.css');
    writeFileSync(
        input,
        ['@import "tailwindcss";', ...jsxFiles.map((f) => `@source "./${f}";`), ''].join('\n'),
    );
    const run = spawnSync(process.execPath, [TAILWIND_CLI, '-i', input, '-o', output, '--minify'], {
        stdio: ['ignore', 'ignore', 'pipe'],
        encoding: 'utf8',
    });
    rmSync(input, { force: true });
    if (run.status !== 0) {
        console.error(`[css] ${dir}: tailwind failed\n${run.stderr || ''}`);
        return false;
    }
    const size = statSync(output).size;
    console.log(`[css] ${dir}/tailwind.css  ${size} bytes  (${jsxFiles.length} jsx)`);
    return true;
}

const args = process.argv.slice(2);
if (args.length === 0) {
    console.error('usage: bun tools/build-example-css.mjs <example-dir>… | --all');
    process.exit(2);
}
if (!existsSync(TAILWIND_CLI)) {
    console.error(`[css] tailwind CLI not found: ${TAILWIND_CLI}`);
    process.exit(2);
}
const targets = args[0] === '--all' ? jsxDirs('examples') : args;
let failed = 0;
for (const dir of targets) {
    if (!build(dir)) failed++;
}
process.exit(failed === 0 ? 0 : 1);
