// Edge headless 参考侧逐帧截帧（ChromeDriver 通路，与 opencat-web-render
// 同源）：spawn msedgedriver → POST /session（goog:chromeOptions.binary 指
// Edge + webgpu flag 组）→ /url 打开 ref.html?case=<c> → 轮询 __ocRefDone
// → 逐帧 execute/sync 拉 __ocFrames[i] dataURL 写 PNG 序列。
//
// 用法：bun ref/shoot.mjs <case> <outdir> [frames]
// 环境变量：EDGE_BIN / CHROMEDRIVER_BIN（默认探标准安装路径）。
import { spawn } from 'node:child_process';
import { createServer } from 'node:http';
import { createReadStream, existsSync, mkdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import { join, resolve, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const CORPUS = resolve(dirname(fileURLToPath(import.meta.url)), '..');

const MIME = {
    '.html': 'text/html', '.js': 'text/javascript', '.mjs': 'text/javascript',
    '.wasm': 'application/wasm', '.json': 'application/json', '.png': 'image/png',
};
const server = createServer((req, res) => {
    const url = new URL(req.url, 'http://x');
    const path = join(CORPUS, decodeURIComponent(url.pathname));
    if (!path.startsWith(CORPUS) || !existsSync(path) || statSync(path).isDirectory()) {
        res.writeHead(404).end(); return;
    }
    res.writeHead(200, { 'content-type': MIME[path.slice(path.lastIndexOf('.'))] || 'application/octet-stream' });
    createReadStream(path).pipe(res);
});
await new Promise((r) => server.listen(0, '127.0.0.1', r));
const servePort = server.address().port;

const shutdown = async (code, msg) => {
    if (msg) console.error(msg);
    try { if (typeof sessionId === 'string') await fetch(`${driverUrl}/session/${sessionId}`, { method: 'DELETE' }); } catch { }
    try { driver.kill(); } catch { }
    try { rmSync(profile, { recursive: true, force: true }); } catch { }
    server.close();
    process.exit(code);
};

const [caseName, outDir, framesArg] = process.argv.slice(2);
if (!caseName || !outDir) {
    console.error('usage: bun ref/shoot.mjs <case> <outdir> [frames]');
    process.exit(2);
}
const GLOBAL_BUDGET_MS = 180000;
setTimeout(() => {
    console.error('shoot: global budget exceeded');
    process.exit(1);
}, GLOBAL_BUDGET_MS).unref?.();

const EDGE_CANDIDATES = [
    process.env.EDGE_BIN,
    'C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe',
    'C:/Program Files/Microsoft/Edge/Application/msedge.exe',
].filter(Boolean);
const edge = EDGE_CANDIDATES.find((p) => existsSync(p));
if (!edge) { console.error(`shoot: no Edge found (tried ${EDGE_CANDIDATES.join(', ')})`); process.exit(1); }
const DRIVER_CANDIDATES = [
    process.env.CHROMEDRIVER_BIN,
    'C:/Users/zhoux/AppData/Local/Temp/ed156/msedgedriver.exe',
].filter(Boolean);
const driverBin = DRIVER_CANDIDATES.find((p) => existsSync(p));
if (!driverBin) { console.error(`shoot: no msedgedriver found (tried ${DRIVER_CANDIDATES.join(', ')})`); process.exit(1); }

const profile = join(CORPUS, 'ref', 'out', `.edge-profile-shaders-${Date.now()}`);
mkdirSync(profile, { recursive: true });

// msedgedriver：普通 console 程序直接 spawn（无 launcher 转交问题），从
// stderr 解析就绪端口（--port=0 动态分配）。
const driver = spawn(driverBin, ['--port=0'], { stdio: ['ignore', 'pipe', 'pipe'] });
let driverUrl = null;
const driverOutChunks = [];
const onDriverOut = (c) => {
    driverOutChunks.push(String(c));
    // msedgedriver 的就绪行在 stdout（chromedriver 才打 stderr）：
    // "msedgedriver was started successfully on port 56267."
    const m = String(c).match(/started successfully on port (\d+)/);
    if (m) driverUrl = `http://127.0.0.1:${m[1]}`;
};
driver.stdout.on('data', onDriverOut);
driver.stderr.on('data', onDriverOut);
for (let i = 0; i < 40 && !driverUrl; i++) await new Promise((r) => setTimeout(r, 250));
if (!driverUrl) {
    console.error('shoot: msedgedriver start timeout\n' + driverOutChunks.join(''));
    process.exit(1);
}

const W = 960, H = 540;
const caps = {
    capabilities: {
        alwaysMatch: {
            // browserName 不设：msedgedriver 隐含 msedge（写 'chrome' 会被
            // 拒 No matching capabilities；写 'msedge' chromedriver 拒——
            // 与 opencat-web-oracle create_session 同一结论）。
            'goog:loggingPrefs': { browser: 'ALL' },
            'goog:chromeOptions': {
                binary: edge,
                args: [
                    '--headless=new',
                    '--no-first-run',
                    '--hide-scrollbars',
                    // dpr 钉 1：Windows 显示缩放会泄入 headless（dpr=1.25 时
                    // lib 把 canvas buffer 配成 1200×675，drawImage 回缩 960
                    // 的重采样把渐变压暗 ~10%）——引擎腿 realm 是 960×540，
                    // 两侧必须同分辨率。
                    '--force-device-scale-factor=1',
                    '--enable-unsafe-webgpu',
                    '--enable-unsafe-swiftshader',
                    `--window-size=${W},${H}`,
                    `--user-data-dir=${profile}`,
                ],
            },
        },
    },
};
const sessionRes = await fetch(`${driverUrl}/session`, { method: 'POST', headers: { 'content-type': 'application/json' }, body: JSON.stringify(caps) });
const sessionBody = await sessionRes.json();
const sessionId = sessionBody.sessionId || sessionBody?.value?.sessionId;
if (!sessionId) {
    console.error('shoot: webdriver session failed: ' + JSON.stringify(sessionBody).slice(0, 500));
    driver.kill();
    process.exit(1);
}

const wd = async (method, path, body) => {
    const res = await fetch(`${driverUrl}/session/${sessionId}${path}`, {
        method,
        headers: { 'content-type': 'application/json' },
        body: body === undefined ? undefined : JSON.stringify(body),
    });
    const out = await res.json();
    if (out.value && out.value.error) throw new Error(`webdriver ${path}: ${out.value.error} ${out.value.message || ''}`.slice(0, 600));
    return out.value;
};
const evaluate = async (script) => {
    const v = await wd('POST', '/execute/sync', { script, args: [] });
    return v;
};

// WebGPU 真值腿与 WebGL 不同：adapter 走 D3D，不强制 swiftshader 软光栅；
// viewport 由 --window-size 定（headless 下 window 即视口）。
await wd('POST', '/url', { url: `http://127.0.0.1:${servePort}/ref/ref.html?case=${caseName}` });

// done 轮询。
let done = false, lastError = null;
for (let i = 0; i < 300 && !done; i++) {
    await new Promise((r) => setTimeout(r, 200));
    try {
        const state = await evaluate('return JSON.stringify({ done: window.__ocRefDone === true, error: window.__ocRefError || null })');
        if (state) { const s = JSON.parse(state); done = s.done; lastError = s.error; }
    } catch { /* page still loading */ }
}
if (!done) {
    const diag = await evaluate('return JSON.stringify({ err: (window.__ocRefError||"").slice(0,400), rbErr: window.__ocRbErr||null, firstErr: window.__ocWgFirstError||null, verifyErr: (window.__ocWgVerifyErr||[]).slice(0,6), hookMissing: !!window.__ocWgHookMissing, frames: (window.__ocFrames||[]).length })').catch(() => 'diag-failed');
    await shutdown(1, 'shoot: __ocRefDone timeout; diag=' + diag);
}
if (lastError) {
    await shutdown(1, 'shoot: ref page error:\n' + lastError);
}

// 逐帧拉取（dataURL 单条几百 KB，逐条 execute/sync）。
const n = await evaluate('return window.__ocFrames.length');
if (framesArg && Number(framesArg) !== n) {
    console.error(`shoot: frame count mismatch (page=${n}, arg=${framesArg})`);
}
mkdirSync(resolve(outDir), { recursive: true });
for (let i = 0; i < n; i++) {
    const dataURL = await evaluate(`return window.__ocFrames[${i}]`);
    const p = resolve(outDir, `frame_${String(i).padStart(4, '0')}.png`);
    writeFileSync(p, Buffer.from(String(dataURL).replace(/^data:image\/png;base64,/, ''), 'base64'));
}
const diag = await evaluate('return JSON.stringify({ firstErr: window.__ocWgFirstError || null, errCnt: window.__ocWgErrCount || 0, rbErr: window.__ocRbErr || null, verifyErr: (window.__ocWgVerifyErr || []).slice(0, 6) })').catch(() => '?');
console.error('[webgpu] ' + diag);
console.log(`shoot: ${n} frames -> ${outDir}`);
await shutdown(0);

