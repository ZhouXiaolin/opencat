// S5 web 腿批量 runner：dist/<案>.bundle.jsx → opencat-web-render（web wasm
// 腿：真 Dawn 在 headless Edge 里直跑原始代码——native OffscreenCanvas
// 'webgpu' 上下文 + 帧末同步回读进生成图通道）→ k3diff vs ref mp4。
//
// bundle 由 run-all 的 vite build 产出（shaders 已整体打包，`/*@@oc-module:
// shaders@@*/`）；ref mp4 复用 run-all 产物 out-k3/ref-<name>.mp4。
//
// 用法：bun run-web.mjs <案...>（缺省 = presets 全案）
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const CORPUS = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(CORPUS, '..', '..', '..');
const WEB_EXE = process.env.OPENCAT_WEB_BIN || 'F:/OpenCat/target/release/opencat-web-render.exe';
const FFMPEG_DIR = 'C:/ffmpeg-9.0.2/ffmpeg-9.0.2-full_build-shared/bin';

// opencat-web-render 探测（CHROMEDRIVER_BIN/CHROME_BIN）；驱动同 ref 腿 shoot.mjs。
const WEB_ENV = {
    ...process.env,
    CHROMEDRIVER_BIN: 'C:/Users/zhoux/AppData/Local/Temp/ed156/msedgedriver.exe',
    CHROME_BIN: 'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    // web-render 出口过 ffmpeg（VUI tag / concat），spawn 走 PATH——注入。
    PATH: `${FFMPEG_DIR};${process.env.PATH}`,
};

const run = (cmd, args, opts = {}) => {
    const r = spawnSync(cmd, args, { stdio: 'inherit', shell: false, ...opts });
    if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} exit ${r.status}`);
};

const argv = process.argv.slice(2);
import { CASES } from './cases/presets.mjs';
const caseNames = argv.length ? argv : Object.keys(CASES);

const results = [];
for (const name of caseNames) {
    const spec = CASES[name];
    if (!spec) { console.error(`run-web: unknown case ${name}`); process.exit(2); }
    const bundle = join(CORPUS, 'dist', `${name}.bundle.jsx`);
    const refMp4 = join(CORPUS, 'out-k3', `ref-${name}.mp4`);
    if (!existsSync(bundle)) { console.error(`run-web: missing bundle ${bundle} — 先跑 bun run-all.mjs`); process.exit(2); }
    if (!existsSync(refMp4)) { console.error(`run-web: missing ${refMp4} — 先跑 bun run-all.mjs`); process.exit(2); }
    const webMp4 = join(CORPUS, 'out-k3', `web-${name}.mp4`);
    const diffDir = join(CORPUS, 'out-k3', `web-${name}`);

    console.log(`\n=== [web:${name}] frames=${spec.frames} ===`);
    const t0 = Date.now();

    // web wasm 腿出 mp4（页内 WebCodecs 编码 + VUI tag）。
    run(WEB_EXE, [
        bundle,
        '--base-dir', CORPUS,
        '--out', webMp4,
    ], { env: WEB_ENV });

    // k3diff：web vs ref（双侧 headless Edge 真 Dawn，同 adapter）。
    mkdirSync(diffDir, { recursive: true });
    run('python', [join(REPO, 'tools', 'k3diff.py'), refMp4, webMp4, '--out', diffDir]);

    const summary = JSON.parse(readFileSync(join(diffDir, 'summary.json'), 'utf8'));
    const p8 = summary.agg.avg_p8;
    results.push({ name, p8, pass: p8 <= 0.02, ms: Date.now() - t0 });
    console.log(`[web:${name}] avg_p8=${(p8 * 100).toFixed(4)}% ${p8 <= 0.02 ? 'PASS' : 'FAIL'}`);
}

console.log('\n=== web summary ===');
let fails = 0;
for (const r of results) {
    console.log(`${r.pass ? 'PASS' : 'FAIL'}  ${r.name}  avg_p8=${(r.p8 * 100).toFixed(4)}%  (${(r.ms / 1000).toFixed(1)}s)`);
    if (!r.pass) fails++;
}
console.log(`\n${results.length - fails}/${results.length} PASS`);
process.exit(fails ? 1 : 0);
