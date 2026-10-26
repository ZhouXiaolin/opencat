// OpenCat vite 插件（内联于模板，随项目分发）。
//
// 职责：
//   1. 构建期把 entry 所在目录的 className 编译成标准 CSS（tailwind 原语必须
//      在构建期编译掉，引擎只吃标准 CSS），产物留在内存，不落盘。
//   2. 用 esbuild 把入口打包成标准 bundle（解析 npm 依赖——lucide-react
//      等真包在这里被折叠进产物；react 系 import alias 到 react-shim.mjs），
//      并把编译后的 CSS 以 `export const css` 嵌进 bundle——bundle 单文件
//      自包含，是渲染的唯一样式来源。bundle 与视频同放 `output` 所在目录
//      （默认 dist/，构建产物不混进源码目录）。
//   3. vite build 结束后自动调用 opencat render 产出视频（渲染的是 bundle）。
//      bundle 不在入口目录，所以渲染显式传 `--base-dir <入口目录>`：资产/字体
//      相对路径仍按入口目录解析。预览同理：build 产出 bundle 后
//      `opencat-see dist/<stem>.bundle.jsx --base-dir .`。
import { spawn, spawnSync } from 'node:child_process';
import { appendFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/// 垫片与插件同目录分发（模板/示例各自 cp 一份）。
const SHIM_PATH = join(dirname(fileURLToPath(import.meta.url)), 'react-shim.mjs');
/// gsap 收录面入口，同上分发。项目装了 gsap 时预打包成 dist/gsap.bundle.mjs
/// ——引擎不再内嵌 gsap，渲染期按约定读该 sidecar 注入模块表。
const GSAP_ENTRY = join(dirname(fileURLToPath(import.meta.url)), 'gsap-entry.mjs');
/// three 主包入口，同上分发。装了 three 时预打包成 dist/three.bundle.mjs。
const THREE_ENTRY = join(dirname(fileURLToPath(import.meta.url)), 'three-entry.mjs');

export default function opencat(options = {}) {
    const entry = options.entry ?? 'src/index.jsx';
    const output = options.output ?? 'dist/video.mp4';

    let root = process.cwd();
    let command = 'build';
    let entryAbs;
    let entryDir;
    let outputAbs;
    let distDir;
    let tailwindCli;
    let esbuild;
    let bundleAbs;
    // tailwind 编译产物（内存）：唯一去向是 bundle 里的 `export const css`。
    let cssText = null;

    const compileTailwind = () => {
        if (!existsSync(entryAbs)) {
            throw new Error(`[opencat] 找不到入口文件：${entryAbs}`);
        }
        const input = join(entryDir, '.tailwind-input.css');
        const lines = ['@import "tailwindcss";', '@source "./**/*.jsx";'];
        if (existsSync(join(entryDir, 'app.css'))) {
            lines.push('@import "./app.css";');
        }
        writeFileSync(input, lines.join('\n') + '\n');
        return new Promise((resolveJob, rejectJob) => {
            const child = spawn(process.execPath, [tailwindCli, '-i', '.tailwind-input.css', '-o', '-', '--minify'], {
                cwd: entryDir,
                stdio: ['ignore', 'pipe', 'inherit'],
            });
            let out = '';
            child.stdout.on('data', (chunk) => {
                out += chunk;
            });
            child
                .on('exit', (code) => {
                    rmSync(input, { force: true });
                    if (code === 0) {
                        cssText = out;
                        console.log(`[opencat] tailwind compiled (${out.length} bytes)`);
                        resolveJob();
                    } else {
                        rejectJob(new Error(`[opencat] tailwindcss 编译失败（exit ${code}）`));
                    }
                })
                .on('error', (err) => {
                    rmSync(input, { force: true });
                    rejectJob(err);
                });
        });
    };

    // 标准工程流：npm 组件树 → esbuild bundle → 引擎渲染 bundle。
    // react/react/jsx-runtime alias 到垫片（svg 子树在垫片里序列化成
    // inline markup）；opencat/runtime 与三个真包（gsap/three/canvaskit-wasm）
    // 保持 external——引擎内建或项目 sidecar 解析（gsap 见 bundleGsap）。
    const bundle = () => {
        if (!esbuild) {
            const require = createRequire(join(root, 'package.json'));
            esbuild = require('esbuild');
        }
        const stem = basename(entryAbs).replace(/\.jsx$/, '');
        // 产物一律进 dist/（与视频同目录，一起被 gitignore）。bundle 不在入口
        // 同目录，所以渲染时必须显式把 base_dir 指回入口目录——资产/字体相对
        // 路径仍按入口目录解析。
        bundleAbs = join(distDir, `${stem}.bundle.jsx`);
        mkdirSync(distDir, { recursive: true });
        esbuild.buildSync({
            entryPoints: [entryAbs],
            outfile: bundleAbs,
            bundle: true,
            format: 'esm',
            jsx: 'automatic',
            jsxImportSource: 'react',
            alias: {
                react: SHIM_PATH,
                'react/jsx-runtime': SHIM_PATH,
            },
            external: ['opencat/runtime', 'gsap', 'three', 'canvaskit-wasm'],
            charset: 'utf8',
            logLevel: 'warning',
        });
        embedCss();
        console.log(`[opencat] bundled ${entry} -> ${bundleAbs}`);
    };

    // 编译后的样式表嵌进 bundle（`export const css`）：bundle 单文件自包含。
    const embedCss = () => {
        if (!cssText) return;
        const literal = JSON.stringify(cssText)
            .replace(/\u2028/g, '\\u2028')
            .replace(/\u2029/g, '\\u2029');
        appendFileSync(bundleAbs, `\nexport const css = ${literal};\n`);
    };

    // 项目侧 gsap bundle：`gsap` 在主 bundle 里保持 external，渲染期由引擎
    // 从 dist/gsap.bundle.mjs 注入。gsap 从项目自身依赖解析（nodePaths），
    // 版本跟项目；没装 gsap 的项目跳过。CJS 产物：引擎模块表按 CJS 语义
    // new Function 执行，无 import/export 需要降级。
    const bundleGsap = () => {
        const nm = join(root, 'node_modules');
        if (!existsSync(join(nm, 'gsap'))) return;
        if (!esbuild) {
            const require = createRequire(join(root, 'package.json'));
            esbuild = require('esbuild');
        }
        const outfile = join(distDir, 'gsap.bundle.mjs');
        mkdirSync(distDir, { recursive: true });
        esbuild.buildSync({
            entryPoints: [GSAP_ENTRY],
            outfile,
            bundle: true,
            format: 'cjs',
            platform: 'browser',
            target: 'es2020',
            charset: 'utf8',
            logLevel: 'warning',
            nodePaths: [nm],
        });
        console.log(`[opencat] bundled gsap -> ${outfile}`);
    };

    // 项目侧 three bundle：`three` 在主 bundle 里保持 external，渲染期由引擎
    // 从 dist/three.bundle.mjs 注入。没装 three 的项目跳过。
    // three 0.181 上游 bug：WebGLState/WebGLRenderer/DataTextureLoader 的
    // `catch ( error )` 参数遮蔽模块级 error 工具函数，esbuild rename 忠实
    // 保留该 bug，必须在打包前的源文本上把遮蔽调用改成 console.error
    // （catch 外的同名正常调用不动）。分布：module.js×11、core.js×1。
    const threeShadowPatch = {
        name: 'three-shadowed-error',
        setup(build) {
            build.onLoad({ filter: /three[\\/]build[\\/]three(\.core|\.module)?\.(cjs|js)$/ }, (args) => {
                let src = readFileSync(args.path, 'utf8');
                for (const [from, to] of [
                    ["error( 'WebGLState:', error )", "console.error( 'WebGLState:', error )"],
                    ["error( 'WebGLRenderer: ' + error.message )", "console.error( 'WebGLRenderer: ' + error.message )"],
                    ["error( error )", "console.error( error )"],
                ]) {
                    src = src.split(from).join(to);
                }
                return { contents: src, loader: 'js' };
            });
        },
    };
    // three 遮蔽 patch 要走 esbuild plugin（onLoad 改源文本），plugin 只在
    // 异步 API 里可用——这里是工程流里唯一不能 buildSync 的打包步骤。
    const bundleThree = async () => {
        const nm = join(root, 'node_modules');
        if (!existsSync(join(nm, 'three'))) return;
        if (!esbuild) {
            const require = createRequire(join(root, 'package.json'));
            esbuild = require('esbuild');
        }
        const outfile = join(distDir, 'three.bundle.mjs');
        mkdirSync(distDir, { recursive: true });
        await esbuild.build({
            entryPoints: [THREE_ENTRY],
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
        console.log(`[opencat] bundled three -> ${outfile}`);
    };

    const render = async () => {
        bundle();
        bundleGsap();
        await bundleThree();
        const bin = findOpencatBin();
        mkdirSync(dirname(outputAbs), { recursive: true });
        console.log(`[opencat] render ${entry} -> ${output}`);
        const result = spawnSync(
            bin,
            ['render', bundleAbs, outputAbs, '--base-dir', entryDir],
            { stdio: 'inherit' }
        );
        if (result.status !== 0) {
            throw new Error(`[opencat] 渲染失败（exit ${result.status}）`);
        }
        console.log(`[opencat] Done: ${outputAbs}`);
    };

    return {
        name: 'opencat',
        configResolved(config) {
            root = config.root;
            command = config.command;
            entryAbs = resolve(root, entry);
            entryDir = dirname(entryAbs);
            outputAbs = resolve(root, output);
            distDir = dirname(outputAbs);
            tailwindCli = resolveTailwindCli(root);
        },
        buildStart() {
            // 先编译 CSS，closeBundle 才 bundle+render，时序天然成立。
            if (command === 'build') {
                return compileTailwind();
            }
        },
        async closeBundle() {
            await render();
        },
    };
}


/// 从项目 node_modules 解析 @tailwindcss/cli 的 bin 入口（dist/index.mjs）。
function resolveTailwindCli(root) {
    try {
        const require = createRequire(join(root, 'package.json'));
        const pkgPath = require.resolve('@tailwindcss/cli/package.json');
        const pkg = require(pkgPath);
        return join(dirname(pkgPath), pkg.bin.tailwindcss);
    } catch {
        throw new Error('[opencat] 解析不到 @tailwindcss/cli，请先在项目里 npm install');
    }
}

/// opencat 二进制查找顺序：OPENCAT_BIN → PATH 上的 opencat.exe / opencat。
function findOpencatBin() {
    if (process.env.OPENCAT_BIN) {
        return process.env.OPENCAT_BIN;
    }
    const exe = process.platform === 'win32' ? 'opencat.exe' : 'opencat';
    for (const dir of process.env.PATH?.split(delimiter) ?? []) {
        if (!dir) continue;
        const candidate = join(dir, exe);
        if (existsSync(candidate)) {
            return candidate;
        }
    }
    throw new Error(
        '[opencat] 找不到 opencat 可执行文件。请先 `cargo build --release --bin opencat`，' +
            '然后设置 OPENCAT_BIN 指向它，或把 target/release 加入 PATH。'
    );
}
