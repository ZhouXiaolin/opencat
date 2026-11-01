// OpenCat vite 插件（shaders-align corpus 版，抄 g2-align corpus 精简）。
//
// 职责：tailwind 构建期编译 → esbuild 分段 bundle（entry 段 + shaders 段）
// → vite build 结束后自动 opencat render 出视频。分段标记
// /*@@oc-module:<spec>@@*/ 由引擎按行切段灌模块表；shaders 段跨帧存活
// （registry/pipeline 缓存不被 __ocResetFrame 清掉）。
import { spawn, spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import { basename, delimiter, dirname, join, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';

const SHIM_PATH = join(dirname(fileURLToPath(import.meta.url)), 'react-shim.mjs');
/// shaders 收录面入口（core + js），项目装了 shaders 时打成 `shaders` 模块段。
const SHADERS_ENTRY = join(dirname(fileURLToPath(import.meta.url)), 'shaders-entry.mjs');
const NODE_STUB = join(dirname(fileURLToPath(import.meta.url)), 'node-stub.mjs');

const SEGMENT_PREFIX = '/*@@oc-module:';
const SEGMENT_SUFFIX = '@@*/';
const ENTRY_SPEC = '<entry>';

export default function opencat(options = {}) {
    const entry = options.entry ?? 'src/index.jsx';
    const output = options.output ?? 'dist/video.mp4';

    let root = process.cwd();
    let entryAbs;
    let entryDir;
    let outputAbs;
    let distDir;
    let tailwindCli;
    let esbuild;
    let bundleAbs;
    let cssText = null;

    const compileTailwind = () => {
        if (!existsSync(entryAbs)) {
            throw new Error(`[opencat] 找不到入口文件：${entryAbs}`);
        }
        const input = join(entryDir, '.tailwind-input.css');
        const lines = ['@import "tailwindcss";', '@source "./**/*.jsx";'];
        writeFileSync(input, lines.join('\n') + '\n');
        return new Promise((resolveJob, rejectJob) => {
            const child = spawn(process.execPath, [tailwindCli, '-i', '.tailwind-input.css', '-o', '-', '--minify'], {
                cwd: entryDir,
                stdio: ['ignore', 'pipe', 'inherit'],
            });
            let out = '';
            child.stdout.on('data', (chunk) => { out += chunk; });
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

    const buildSegment = async (spec, entryPath, alias) => {
        const nm = join(root, 'node_modules');
        const pkgDir = spec === 'shaders' ? join(nm, 'shaders') : join(nm, spec);
        if (!existsSync(pkgDir) || !existsSync(entryPath)) return null;
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
            alias,
        });
        return segment(spec, result.outputFiles[0].text);
    };

    const bundle = async () => {
        const stem = basename(entryAbs).replace(/\.jsx$/, '');
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
            external: ['opencat/runtime', 'shaders'],
            charset: 'utf8',
            logLevel: 'warning',
        });

        const chunks = [];
        const shadersSeg = await buildSegment('shaders', SHADERS_ENTRY);
        if (shadersSeg) chunks.push(shadersSeg);
        chunks.push(segment(ENTRY_SPEC, entryResult.outputFiles[0].text + cssExport()));

        writeFileSync(bundleAbs, chunks.join(''));
        console.log(`[opencat] bundled ${entry} -> ${bundleAbs}`);
    };

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
            {
                stdio: 'inherit',
                // 钉到与 ref 腿同 GPU（shaders-align S1c：headless Edge 的
                // WebGPU 固定落核显 D3D12；engine 缺省 HighPerformance 拿独显，
                // 驱动 fast-math 在 f32 tie 节点偏离 IEEE，hash 噪声类整格翻转）。
                env: { ...process.env, OC_WGPU_BACKEND: 'dx12', OC_WGPU_POWER: 'low' },
            }
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
            entryAbs = resolve(root, entry);
            entryDir = dirname(entryAbs);
            outputAbs = resolve(root, output);
            distDir = dirname(outputAbs);
            tailwindCli = resolveTailwindCli(root);
        },
        buildStart() {
            return compileTailwind();
        },
        closeBundle() {
            return render();
        },
    };
}

function segment(spec, code) {
    return `${SEGMENT_PREFIX}${spec}${SEGMENT_SUFFIX}\n${code}\n`;
}

function resolveTailwindCli(root) {
    try {
        const require = createRequire(join(root, 'package.json'));
        const pkgPath = require.resolve('@tailwindcss/cli/package.json');
        const pkg = require(pkgPath);
        return join(dirname(pkgPath), pkg.bin.tailwindcss);
    } catch {
        throw new Error('[opencat] 解析不到 @tailwindcss/cli，请先在项目里 bun install');
    }
}

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
