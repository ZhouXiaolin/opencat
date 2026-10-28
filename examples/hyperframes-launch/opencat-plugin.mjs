// OpenCat vite 插件（内联于模板，随项目分发）。
//
// 职责：
//   1. 构建期把 entry 所在目录的 className 编译成标准 CSS（tailwind 原语必须
//      在构建期编译掉，引擎只吃标准 CSS），产物留在内存，不落盘。
//   2. 用 esbuild 把入口、gsap、three 各打一份，拼成**单文件多模块段**的
//      `dist/<stem>.bundle.jsx`：每段以 `/*@@oc-module:<spec>@@*/` 起头，
//      引擎开文件时按标记切段灌进模块表（crates/opencat-engine/src/jsx.rs）。
//      切段是必须的：`__ocResetFrame` 只让 gsap/three 的模块条目跨帧存活，
//      内联进 entry 会让 1.6MB 的 three 类定义每帧重估。编译后的 CSS 以
//      `export const css` 嵌在 entry 段内——bundle 单文件自包含，是渲染的
//      唯一样式来源。bundle 与视频同放 `output` 所在目录（默认 dist/，构建
//      产物不混进源码目录）。
//   3. vite build 结束后自动调用 opencat render 产出视频（渲染的是 bundle）。
//      bundle 不在入口目录，所以渲染显式传 `--base-dir <入口目录>`：资产/字体
//      相对路径仍按入口目录解析。预览同理：build 产出 bundle 后
//      `opencat view dist/<stem>.bundle.jsx --base-dir .`。
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/// 垫片与插件同目录分发（模板/示例各自 cp 一份）。
const SHIM_PATH = join(dirname(fileURLToPath(import.meta.url)), 'react-shim.mjs');
/// gsap 收录面入口，同上分发。项目装了 gsap 时打成 `gsap` 模块段。
const GSAP_ENTRY = join(dirname(fileURLToPath(import.meta.url)), 'gsap-entry.mjs');
/// three 主包入口，同上分发。项目装了 three 时打成 `three` 模块段。
const THREE_ENTRY = join(dirname(fileURLToPath(import.meta.url)), 'three-entry.mjs');

/// 段首标记：引擎按它把单文件切回多个模块条目。
const SEGMENT_PREFIX = '/*@@oc-module:';
const SEGMENT_SUFFIX = '@@*/';
/// entry 段的 spec 名，与 `crates/opencat-core/src/parse/jsx.rs` 的 ENTRY_SPEC 一致。
const ENTRY_SPEC = '<entry>';

/// three 0.181 上游 bug：WebGLState/WebGLRenderer/DataTextureLoader 的
/// `catch ( error )` 参数遮蔽模块级 error 工具函数，esbuild rename 忠实保留
/// 该 bug，必须在打包前的源文本上把遮蔽调用改成 console.error（catch 外的
/// 同名正常调用不动）。分布：module.js×11、core.js×1。
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

    const requireEsbuild = () => {
        if (!esbuild) {
            esbuild = createRequire(join(root, 'package.json'))('esbuild');
        }
        return esbuild;
    };

    /// 一个真包 → 一段 CJS 源码。项目没装该包时跳过：源码里的
    /// `import 'gsap'` 会落到 JS 运行时的 resolveSpec 报缺包，不需要构建期
    /// 报错（gsap/three 都是可选的）。
    const buildSegment = async (spec, entryPath, plugins = []) => {
        const nm = join(root, 'node_modules');
        if (!existsSync(join(nm, spec)) || !existsSync(entryPath)) return null;
        const result = await requireEsbuild().build({
            entryPoints: [entryPath],
            bundle: true,
            write: false,
            format: 'cjs',
            platform: 'browser',
            target: 'es2020',
            charset: 'utf8',
            logLevel: 'warning',
            nodePaths: [nm],
            plugins,
        });
        return segment(spec, result.outputFiles[0].text);
    };

    // 标准工程流：npm 组件树 → 单文件多段 bundle → 引擎渲染 bundle。
    // react/react/jsx-runtime alias 到垫片（svg 子树在垫片里序列化成 inline
    // markup）；opencat/runtime 与 canvaskit-wasm 是引擎内建，保持 external。
    const bundle = async () => {
        const stem = basename(entryAbs).replace(/\.jsx$/, '');
        // 产物一律进 dist/（与视频同目录，一起被 gitignore）。bundle 不在入口
        // 同目录，所以渲染时必须显式把 base_dir 指回入口目录——资产/字体相对
        // 路径仍按入口目录解析。
        bundleAbs = join(distDir, `${stem}.bundle.jsx`);
        mkdirSync(distDir, { recursive: true });

        const entryResult = await requireEsbuild().build({
            entryPoints: [entryAbs],
            bundle: true,
            write: false,
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

        const chunks = [];
        for (const built of [
            await buildSegment('gsap', GSAP_ENTRY),
            await buildSegment('three', THREE_ENTRY, [threeShadowPatch]),
        ]) {
            if (built) chunks.push(built);
        }
        chunks.push(segment(ENTRY_SPEC, entryResult.outputFiles[0].text + cssExport()));

        writeFileSync(bundleAbs, chunks.join(''));
        console.log(`[opencat] bundled ${entry} -> ${bundleAbs}`);
    };

    // 编译后的样式表嵌进 entry 段（`export const css`）：bundle 单文件自包含。
    const cssExport = () => {
        if (!cssText) return '';
        const literal = JSON.stringify(cssText)
            .replace(/\u2028/g, '\\u2028')
            .replace(/\u2029/g, '\\u2029');
        return `\nexport const css = ${literal};\n`;
    };

    const render = async () => {
        await bundle();
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
        closeBundle() {
            return render();
        },
    };
}

/// 把一段源码包成带段首标记的模块段。标记必须独占一行。
function segment(spec, code) {
    return `${SEGMENT_PREFIX}${spec}${SEGMENT_SUFFIX}\n${code}\n`;
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
