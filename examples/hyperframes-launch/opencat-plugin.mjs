// OpenCat vite 插件（内联于模板，随项目分发）。
//
// 职责：
//   1. build / dev 时把 entry 所在目录的 className 编译成 tailwind.css sidecar
//      （引擎按 <jsx目录>/tailwind.css 约定路径读取，见 crates/opencat-engine/src/jsx.rs）。
//      语义与 scripts/build-css.sh 完全一致，只是换成跨平台 Node 实现。
//   2. vite build 结束后自动调用 opencat render 产出视频。
//   3. dev 模式下 watch jsx/app.css 变化重编 sidecar（配合 opencat-see 预览）。
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, delimiter, dirname, join, resolve } from 'node:path';

export default function opencat(options = {}) {
    const entry = options.entry ?? 'src/index.jsx';
    const output = options.output ?? 'dist/video.mp4';

    let root = process.cwd();
    let command = 'build';
    let entryAbs;
    let entryDir;
    let outputAbs;
    let tailwindCli;
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

    const render = () => {
        const bin = findOpencatBin();
        mkdirSync(dirname(outputAbs), { recursive: true });
        console.log(`[opencat] render ${entry} -> ${output}`);
        const result = spawnSync(bin, ['render', entryAbs, outputAbs], { stdio: 'inherit' });
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
