// 把裸 `.jsx` 示例预构建成**单文件多模块段**的 `<example>/dist/<name>.bundle.jsx`。
//
// 裸 `.jsx` 没有 vite 工程流：入口源码本身不做打包（引擎的 oxc 前端负责
// JSX → h、import → `__ocRequire`），本脚本只把它与项目侧 gsap/three 的
// CJS 产物拼进同一个文件，每段以 `/*@@oc-module:<spec>@@*/` 起头。引擎开
// 文件时按标记切段灌进模块表（crates/opencat-engine/src/jsx.rs）——
// `__ocResetFrame` 只让 gsap/three 的模块条目跨帧存活，段必须分开。
//
// 样式表也一并嵌进 entry 段（`export const css`），与 vite 插件的 CSS 路径同构：
// 渲染期不需要 node/bun，也不存在入口同目录的 `tailwind.css`。
//
// vite 工程（opencat-plugin.mjs）在 closeBundle 里自己产出同样格式的文件；
// 本脚本服务裸 `.jsx` 示例与手工补建。
//
// 用法：
//   bun scripts/build-example-bundle.mjs examples/figma-launch [more…]
//   bun scripts/build-example-bundle.mjs --all      # examples/* 下所有裸 jsx 目录
//
// 渲染：opencat render examples/<dir>/dist/<name>.bundle.jsx out.mp4 --base-dir examples/<dir>
//
// 包解析：先在示例自己的 node_modules 找 gsap/three/esbuild，再回退到同仓
// 已安装的示例（vfx-heygen-combined 有 gsap+three，k3-promo 有 gsap）。
// 仓库根没有 package.json，这是既有的同源策略。

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { basename, join, resolve } from 'node:path';
import { pathToFileURL } from 'node:url';

const ROOT = resolve(import.meta.dirname, '..');
const GSAP_ENTRY = join(ROOT, 'scripts', 'gsap-entry.mjs');
const THREE_ENTRY = join(ROOT, 'scripts', 'three-entry.mjs');
/// g2 图表入口（@antv/g2 + @antv/g-canvaskit）；canvaskit-wasm 别名到零
/// wasm 替身（g-canvaskit 只 import 它的默认导出 CanvasKitInit，转发
/// 引擎内建官方面）。
const G2_ENTRY = join(ROOT, 'scripts', 'g2-entry.mjs');
const G2_CK_SHIM = join(ROOT, 'scripts', 'g2-canvaskit-shim.mjs');
/// node 内建死导入的 stub（g-canvaskit esm 顶部的 rollup 残留，零使用）。
const NODE_STUB = join(ROOT, 'scripts', 'node-stub.mjs');
/// tailwind CLI 与 vite 插件同源：从 k3-promo 的依赖里取。
const TAILWIND_CLI = join(ROOT, 'examples', 'k3-promo', 'node_modules', '@tailwindcss', 'cli', 'dist', 'index.mjs');
/// 回退包根：顺序即优先级。
const FALLBACK_NODE_MODULES = [
    join(ROOT, 'examples', 'vfx-heygen-combined', 'node_modules'),
    join(ROOT, 'examples', 'k3-promo', 'node_modules'),
];

/// 段首标记，与 opencat-plugin.mjs / crates/opencat-engine/src/jsx.rs 一致。
const SEGMENT_PREFIX = '/*@@oc-module:';
const SEGMENT_SUFFIX = '@@*/';

/// three 0.181 上游 bug：`catch ( error )` 参数遮蔽模块级 error 工具函数。
/// 与 opencat-plugin.mjs 里的同名 plugin 保持一致。
const threeShadowedCalls = [
    ["error( 'WebGLState:', error )", "console.error( 'WebGLState:', error )"],
    ["error( 'WebGLRenderer: ' + error.message )", "console.error( 'WebGLRenderer: ' + error.message )"],
    ['error( error )', 'console.error( error )'],
];
const threeShadowPatch = {
    name: 'three-shadowed-error',
    setup(build) {
        build.onLoad({ filter: /three[\\/]build[\\/]three(\.core|\.module)?\.(cjs|js)$/ }, (args) => {
            let src = readFileSync(args.path, 'utf8');
            for (const [from, to] of threeShadowedCalls) {
                src = src.split(from).join(to);
            }
            return { contents: src, loader: 'js' };
        });
    },
};

// g-canvaskit 的 dist 内嵌整套 canvaskit wasm 加载器（不走 canvaskit-wasm
// 包导入，alias 够不着）：把 loadCanvaskit 的唯一调用点接到
// globalThis.CanvasKitInit——引擎 realm 由 canvas_api facade 应答（零 wasm）。
// 另带两个上游 bug 修复：Rect radius 缺省炸 `.map`；fill/stroke 的
// fillOpacity/opacity 无默认值，undefined 进乘法产出 NaN alpha。
const g2CanvaskitPatch = {
    name: 'g2-canvaskit-loadcanvaskit',
    setup(build) {
        build.onLoad({ filter: /@antv[\\/]g-canvaskit[\\/]dist[\\/]/ }, (args) => {
            let src = readFileSync(args.path, 'utf8');
            const patches = [
                [
                    'return CanvasKitInit({',
                    'return (globalThis.CanvasKitInit || CanvasKitInit)({',
                ],
                [
                    'radius = _ref.radius,',
                    'radius = _ref.radius || [0, 0, 0, 0],',
                ],
                [
                    'Number(fill.alpha) * fillOpacity * opacity',
                    'Number(fill.alpha) * (fillOpacity === undefined ? 1 : fillOpacity) * (opacity === undefined ? 1 : opacity)',
                ],
                [
                    'Number(stroke.alpha) * strokeOpacity * opacity',
                    'Number(stroke.alpha) * (strokeOpacity === undefined ? 1 : strokeOpacity) * (opacity === undefined ? 1 : opacity)',
                ],
                // 上游 API 漂移：现行 @antv/g 已把 TextNode 位置挪出
                // parsedStyle.x/y（绘制段落时 CTM 已带平移），g-canvaskit
                // 1.1.3 读到 undefined → NaN 坐标 → CK 静默丢弃整段文字。
                // 兜底 0（局部原点）即落位。
                [
                    'canvas.drawParagraph(paragraph, offsetX + x + dx, linePositionY + y + dy);',
                    'var _px = x == null ? 0 : x, _py = y == null ? 0 : y, _pdx = dx == null ? 0 : dx, _pdy = dy == null ? 0 : dy;\n        canvas.drawParagraph(paragraph, offsetX + _px + _pdx, linePositionY + _py + _pdy);',
                ],
                // G2 默认文字 family 是 'sans-serif'，而 FontMgr.FromData 按
                // 字体文件内部名注册且此路径无 family 回退——映射到注册字体
                // 的内部名（engine/ref 两侧 fonts 配置同给内部名）。
                [
                    "fontFamilies = _ref$fontFamily === void 0 ? '' : _ref$fontFamily,",
                    "fontFamilies = _ref$fontFamily === void 0 || _ref$fontFamily === 'sans-serif' ? 'Noto Sans SC' : _ref$fontFamily,",
                ],
            ];
            for (const [from, to] of patches) {
                if (!src.includes(from)) {
                    throw new Error(`g2-canvaskit patch: pattern not found in ${args.path}: "${from.slice(0, 60)}" — upstream dist changed, re-derive the patch site`);
                }
                src = src.split(from).join(to);
            }
            return { contents: src, loader: 'js' };
        });
    },
};

function isDir(path) {
    try {
        return statSync(path).isDirectory();
    } catch {
        return false;
    }
}

/// 目录里的入口 `.jsx`：与目录同名的那个文件。
function entryOf(dir) {
    const candidate = join(dir, `${basename(dir)}.jsx`);
    return existsSync(candidate) ? candidate : null;
}

function nodeModulesDirs(dir) {
    return [join(dir, 'node_modules'), ...FALLBACK_NODE_MODULES].filter(isDir);
}

async function esbuildFor(nodeModulesDirs) {
    for (const nm of nodeModulesDirs) {
        const main = join(nm, 'esbuild', 'lib', 'main.js');
        if (existsSync(main)) return import(pathToFileURL(main).href);
    }
    throw new Error('[bundle] esbuild not found (example and fallback node_modules)');
}

/// 入口是否 import 了该包（含子路径 `gsap/CustomEase`）。
function usesPackage(source, pkg) {
    return new RegExp(`from\\s+['"]${pkg}(/|['"])`).test(source);
}

function segment(spec, code) {
    return `${SEGMENT_PREFIX}${spec}${SEGMENT_SUFFIX}\n${code}\n`;
}

/// 样式表编译进 entry 段（`export const css`），bundle 单文件自包含——与
/// vite 插件的 CSS 路径同构。返回空串表示没有可用 class 扫描到（CLI 缺失）。
async function compileCss(dir) {
    if (!existsSync(TAILWIND_CLI)) {
        console.error(`[css] ${dir}: tailwind CLI not found (${TAILWIND_CLI}), css segment skipped`);
        return '';
    }
    const input = join(dir, '.tailwind-input.css');
    writeFileSync(input, ['@import "tailwindcss";', '@source "./**/*.jsx";', ''].join('\n'));
    const css = await new Promise((resolveJob, rejectJob) => {
        const child = spawn(process.execPath, [TAILWIND_CLI, '-i', '.tailwind-input.css', '-o', '-', '--minify'], {
            cwd: dir,
            stdio: ['ignore', 'pipe', 'inherit'],
        });
        let out = '';
        child.stdout.on('data', (chunk) => {
            out += chunk;
        });
        child
            .on('exit', (code) => {
                rmSync(input, { force: true });
                code === 0 ? resolveJob(out) : rejectJob(new Error(`[css] tailwind exited with ${code}`));
            })
            .on('error', (err) => {
                rmSync(input, { force: true });
                rejectJob(err);
            });
    });
    return css;
}

function cssExport(css) {
    if (!css) return '';
    const literal = JSON.stringify(css).replace(/\u2028/g, '\\u2028').replace(/\u2029/g, '\\u2029');
    return `\nexport const css = ${literal};\n`;
}

async function build(dir) {
    const abs = resolve(dir);
    const entry = entryOf(abs);
    if (!entry) {
        console.error(`[bundle] ${dir}: no ${basename(abs)}.jsx, skipped`);
        return true;
    }
    const nms = nodeModulesDirs(abs);
    // 入口源码原样进 entry 段：JSX 与 import 留给引擎的 oxc 前端。
    const source = readFileSync(entry, 'utf8');
    // 只带入口真正 import 的包：three 的类定义有 1.6MB，白带会让每次渲染
    // 多付一份解析成本。g2 的包名是 scoped（@antv/g2），模块段名是 `g2`。
    const wanted = [
        ['gsap', GSAP_ENTRY, null, 'gsap'],
        ['three', THREE_ENTRY, 'three-shadow-patch', 'three'],
        ['g2', G2_ENTRY, 'g2-canvaskit-patch', join('@antv', 'g2')],
    ].filter(([pkg, , , dir]) => nms.some((nm) => existsSync(join(nm, dir))) && usesPackage(source, pkg));
    if (wanted.length === 0) {
        console.error(`[bundle] ${dir}: no gsap/three/g2 import, only the entry is emitted`);
    }

    const { build: esbuild } = await esbuildFor(nms);
    const chunks = [];
    for (const [pkg, entryPath, patch] of wanted) {
        const result = await esbuild({
            entryPoints: [entryPath],
            bundle: true,
            write: false,
            format: 'cjs',
            platform: 'browser',
            target: 'es2020',
            charset: 'utf8',
            logLevel: 'warning',
            nodePaths: nms,
            plugins: patch === 'three-shadow-patch' ? [threeShadowPatch]
                : patch === 'g2-canvaskit-patch' ? [g2CanvaskitPatch] : [],
            alias: pkg === 'g2'
                ? { 'canvaskit-wasm': G2_CK_SHIM, path: NODE_STUB, fs: NODE_STUB, html2canvas: NODE_STUB }
                : undefined,
        });
        chunks.push(segment(pkg, result.outputFiles[0].text));
    }
    chunks.push(segment('<entry>', source + cssExport(await compileCss(abs))));

    const outDir = join(abs, 'dist');
    mkdirSync(outDir, { recursive: true });
    const outfile = join(outDir, `${basename(abs)}.bundle.jsx`);
    writeFileSync(outfile, chunks.join(''));
    console.log(`[bundle] ${dir}/dist/${basename(abs)}.bundle.jsx  ${statSync(outfile).size} bytes`);
    return true;
}

const args = process.argv.slice(2);
if (args.length === 0) {
    console.error('usage: bun scripts/build-example-bundle.mjs <example-dir>… | --all');
    process.exit(2);
}
const targets =
    args[0] === '--all'
        ? readdirSync(join(ROOT, 'examples'))
              .map((name) => join(ROOT, 'examples', name))
              // vite 工程自己产出同名文件（插件在 closeBundle 里跑），跳过。
              .filter((path) => isDir(path) && entryOf(path) && !existsSync(join(path, 'vite.config.mjs')))
        : args;
let failed = 0;
for (const dir of targets) {
    try {
        await build(dir);
    } catch (err) {
        console.error(`[bundle] ${dir}: ${err.message}`);
        failed++;
    }
}
process.exit(failed === 0 ? 0 : 1);
