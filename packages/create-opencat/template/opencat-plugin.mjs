// OpenCat vite 插件（内联于模板，随项目分发）。
//
// 职责：
//   1. build / dev 时把 entry 所在目录的 className 编译成 tailwind.css sidecar
//      （引擎按 <jsx目录>/tailwind.css 约定路径读取，见 crates/opencat-engine/src/jsx.rs）。
//      语义与 scripts/build-css.sh 完全一致，只是换成跨平台 Node 实现。
//   2. render 前用 esbuild 把入口打包成标准 bundle（解析 npm 依赖——lucide-react
//      等真包在这里被折叠进产物；react 系 import alias 到 react-shim.mjs）。
//      bundle 落在 entry 同目录（<stem>.bundle.jsx），引擎的 base_dir（资产/
//      字体/sidecar 相对入口目录解析）约定原样成立。
//   3. vite build 结束后自动调用 opencat render 产出视频（渲染的是 bundle）。
//   4. dev 模式下 watch jsx/app.css 变化重编 sidecar（配合 opencat-see 预览）。
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

/// 垫片与插件同目录分发（模板/示例各自 cp 一份）。
const SHIM_PATH = join(dirname(fileURLToPath(import.meta.url)), 'react-shim.mjs');

export default function opencat(options = {}) {
    const entry = options.entry ?? 'src/index.jsx';
    const output = options.output ?? 'dist/video.mp4';

    let root = process.cwd();
    let command = 'build';
    let entryAbs;
    let entryDir;
    let outputAbs;
    let tailwindCli;
    let esbuild;
    let bundleAbs;
    // 串行化编译：dev 下连续保存只排队不并发 spawn。
    let chain = Promise.resolve();

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
            spawn(process.execPath, [tailwindCli, '-i', '.tailwind-input.css', '-o', 'tailwind.css', '--minify'], {
                cwd: entryDir,
                stdio: 'inherit',
            })
                .on('exit', (code) => {
                    rmSync(input, { force: true });
                    if (code === 0) {
                        console.log(`[opencat] wrote ${join(entryDir, 'tailwind.css')}`);
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

    const scheduleCompile = () => {
        chain = chain.then(compileTailwind, compileTailwind);
        return chain;
    };

    // 标准工程流：npm 组件树 → esbuild bundle → 引擎渲染 bundle。
    // react/react/jsx-runtime alias 到垫片（svg 子树在垫片里序列化成
    // inline markup）；opencat/runtime 保持 external，引擎内建解析。
    const bundle = () => {
        if (!esbuild) {
            const require = createRequire(join(root, 'package.json'));
            esbuild = require('esbuild');
        }
        const stem = basename(entryAbs).replace(/\.jsx$/, '');
        bundleAbs = join(entryDir, `${stem}.bundle.jsx`);
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
            external: ['opencat/runtime'],
            charset: 'utf8',
            logLevel: 'warning',
        });
        console.log(`[opencat] bundled ${entry} -> ${bundleAbs}`);
    };

    const render = () => {
        bundle();
        const bin = findOpencatBin();
        mkdirSync(dirname(outputAbs), { recursive: true });
        console.log(`[opencat] render ${entry} -> ${output}`);
        const result = spawnSync(bin, ['render', bundleAbs, outputAbs], { stdio: 'inherit' });
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
            tailwindCli = resolveTailwindCli(root);
        },
        buildStart() {
            if (command === 'build') {
                return scheduleCompile();
            }
        },
        closeBundle() {
            render();
        },
        configureServer(server) {
            scheduleCompile();
            const watched = (path) => path.endsWith('.jsx') || basename(path) === 'app.css';
            const onChange = (path) => {
                if (watched(path)) {
                    console.log(`[opencat] change: ${path}，重新编译 tailwind.css`);
                    scheduleCompile();
                }
            };
            server.watcher.on('change', onChange);
            server.watcher.on('add', onChange);
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
