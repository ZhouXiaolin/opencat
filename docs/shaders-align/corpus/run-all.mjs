// 批量 runner：ref shoot（Edge headless WebGPU 真值 PNG）→ ffmpeg crf0
// 合 ref mp4 → CASE=<案> vite build（插件自动 opencat render 出 engine
// mp4，钉 dx12+low）→ k3diff 判定 avg_p8。
//
// 用法：bun run-all.mjs <案...>（缺省 = presets 全案）
// 已有 ref mp4 的案跳过 shoot+合成（--reshoot 强制重拍）。
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, readdirSync, readFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const CORPUS = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(CORPUS, '..', '..', '..');
const FFMPEG = 'C:/ffmpeg-9.0.2/ffmpeg-9.0.2-full_build-shared/bin/ffmpeg.exe';

const argv = process.argv.slice(2);
const reshoot = argv.includes('--reshoot');
const caseArgs = argv.filter((a) => !a.startsWith('--'));
import { CASES } from './cases/presets.mjs';
const caseNames = caseArgs.length ? caseArgs : Object.keys(CASES);

const run = (cmd, args, opts = {}) => {
    const r = spawnSync(cmd, args, { stdio: 'inherit', shell: false, ...opts });
    if (r.status !== 0) throw new Error(`${cmd} ${args.join(' ')} exit ${r.status}`);
};

const results = [];
for (const name of caseNames) {
    const spec = CASES[name];
    if (!spec) { console.error(`run-all: unknown case ${name}`); process.exit(2); }
    const frames = spec.frames;
    const refPngDir = join(CORPUS, 'ref', 'out', name);
    const refMp4 = join(CORPUS, 'out-k3', `ref-${name}.mp4`);
    const distMp4 = join(CORPUS, 'dist', `${name}.mp4`);
    const diffDir = join(CORPUS, 'out-k3', name);

    console.log(`\n=== [${name}] frames=${frames} anim=${spec.anim} ===`);
    const t0 = Date.now();

    // 1. ref shoot（PNG 直出；帧数校验在 shoot 内部）
    const havePngs = existsSync(refPngDir) &&
        readdirSync(refPngDir).filter((f) => f.endsWith('.png')).length === frames;
    if (!havePngs || reshoot) {
        run('bun', [join('ref', 'shoot.mjs'), name, resolve(refPngDir), String(frames)], { cwd: CORPUS });
    } else {
        console.log(`ref: reuse ${refPngDir}`);
    }

    // 2. ffmpeg crf0 → ref mp4（S1 配方：yuv420p，复现 0.0000% 已验证）
    mkdirSync(dirname(refMp4), { recursive: true });
    run(FFMPEG, ['-y', '-v', 'error', '-framerate', '30',
        '-i', join(resolve(refPngDir), 'frame_%04d.png'),
        '-c:v', 'libx264', '-crf', '0', '-pix_fmt', 'yuv420p', refMp4]);

    // 3. engine 腿：vite build（opencat 插件渲染，env 钉扎在插件内）。
    // vite 8/rolldown 在插件 closeBundle 渲染完成后会因缺 index.html
    // 报 UNRESOLVED_ENTRY 退出 1（S1 既有怪癖，vite 仅是钩子宿主）——
    // 成功判据 = 日志含 [opencat] Done 且 mp4 落盘。
    const vr = spawnSync('bun', ['vite', 'build'], {
        cwd: CORPUS,
        // OC_VIDEO_CRF=0：引擎腿无损编码（ref 侧 crf 0 同参数）。默认
        // crf 18 的量化在饱和色/细纹理上产生 ±8-20 的像素差，会淹没
        // 真实对拍信号（RD 实测 mp4 6.6% → 0.0000%）。
        env: {
            ...process.env,
            CASE: name,
            OPENCAT_BIN: process.env.OPENCAT_BIN || 'F:/OpenCat/target/release/opencat.exe',
            OC_VIDEO_CRF: '0',
        },
        encoding: 'utf8',
    });
    const out = (vr.stdout || '') + (vr.stderr || '');
    if (!out.includes('[opencat] Done') || !existsSync(distMp4)) {
        console.error(out.slice(-3000));
        throw new Error(`engine 腿失败（vite exit ${vr.status}）: ${name}`);
    }

    // 4. k3diff
    mkdirSync(diffDir, { recursive: true });
    run('python', [join(REPO, 'tools', 'k3diff.py'), refMp4, distMp4, '--out', diffDir]);

    const summary = JSON.parse(readFileSync(join(diffDir, 'summary.json'), 'utf8'));
    const p8 = summary.agg.avg_p8;
    results.push({ name, p8, pass: p8 <= 0.02, ms: Date.now() - t0 });
    console.log(`[${name}] avg_p8=${(p8 * 100).toFixed(4)}% ${p8 <= 0.02 ? 'PASS' : 'FAIL'}`);
}

console.log('\n=== summary ===');
let fails = 0;
for (const r of results) {
    console.log(`${r.pass ? 'PASS' : 'FAIL'}  ${r.name}  avg_p8=${(r.p8 * 100).toFixed(4)}%  (${(r.ms / 1000).toFixed(1)}s)`);
    if (!r.pass) fails++;
}
console.log(`\n${results.length - fails}/${results.length} PASS`);
process.exit(fails ? 1 : 0);
