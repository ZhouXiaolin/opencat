// Chrome 参考帧（D7 oracle）：同一份插件场景在真 Chrome + 官方 gsap 里按帧截图。
//
// 为什么不用 `chrome --screenshot`：那条路只能截页面初始状态，无法把暂停的
// 时间轴 seek 到指定时刻。这里直连 Chrome DevTools Protocol（Bun 自带
// WebSocket，不引入依赖）：Page.navigate 加载参考页 → Runtime.evaluate 调
// 页面里的 __seek(t) → Page.captureScreenshot 取帧。
//
// 页面里时间轴是 paused 的，只能由 __seek 驱动，所以同一帧号永远给出同一张
// 图（不依赖 rAF、不依赖机器速度）。
//
// 用法：
//   bun tools/gsap-oracle.mjs                      # 默认采样帧
//   bun tools/gsap-oracle.mjs --frames 0,30,60,90
//   bun tools/gsap-oracle.mjs --all --fps 30       # 全帧（配 k3diff 用）
//
// 产物：out/gsap-oracle/index.html + f0000.png…
// 参考视频：ffmpeg -framerate 30 -i out/gsap-oracle/f%04d.png …

import { spawn } from 'node:child_process';
import { existsSync, mkdirSync, writeFileSync, rmSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

const ROOT = join(dirname(fileURLToPath(import.meta.url)), '..');
const OUT_DIR = join(ROOT, 'out', 'gsap-oracle');
const GSAP_DIST = join(ROOT, 'examples/k3-promo/node_modules/gsap/dist');
const DURATION = 12;
const FPS_DEFAULT = 30;

const CHROME_CANDIDATES = [
    'C:/Program Files/Google/Chrome/Application/chrome.exe',
    'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe',
    '/usr/bin/google-chrome',
    '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome',
];

// 与 examples/gsap-plugins/index.jsx 的 ROWS 一一对应：同一组 id、同一个
// 0→1560 位移、同一个 3s 时长，唯一变量是 ease 名字。
const ROWS = [
    { id: 'ease-linear', ease: 'none', color: '#3b82f6' },
    { id: 'ease-customease', ease: 'spring', color: '#22c55e' },
    { id: 'ease-wiggle', ease: 'wiggle', color: '#eab308' },
    { id: 'ease-bounce', ease: 'bounce', color: '#f97316' },
    { id: 'ease-slowmo', ease: 'slow(0.7,0.7)', color: '#a855f7' },
    { id: 'ease-expo', ease: 'expoScale(0.5,7,none)', color: '#06b6d4' },
    { id: 'ease-rough', ease: 'rough(2,3,0.4)', color: '#ec4899' },
];

const PLUGIN_SCRIPTS = [
    'CustomEase.min.js',
    'CustomWiggle.min.js',
    'CustomBounce.min.js',
    'EasePack.min.js',
    'Physics2DPlugin.min.js',
    'PhysicsPropsPlugin.min.js',
];

function pageHtml() {
    const rows = JSON.stringify(ROWS);
    const scripts = PLUGIN_SCRIPTS.map(
        (name) => `  <script src="${pathToFileURL(join(GSAP_DIST, name)).href}"></script>`
    ).join('\n');
    return `<!doctype html>
<html>
<head>
<meta charset="utf-8">
<title>GSAP plugin oracle</title>
<script src="${pathToFileURL(join(GSAP_DIST, 'gsap.min.js')).href}"></script>
${scripts}
<style>
  html, body { margin: 0; padding: 0; background: #0b1017; overflow: hidden; }
  #stage { position: absolute; left: 0; top: 0; width: 1920px; height: 1080px; background: #0b1017; }
  .row { position: absolute; left: 60px; width: 70px; height: 70px; border-radius: 10px; }
  .label { position: absolute; left: 60px; font: 20px sans-serif; color: #5c6b7f; }
</style>
</head>
<body>
<div id="stage">
  <div class="label" style="top:40px;font-size:34px;color:#e6edf6">GSAP plugins — official package, unmodified</div>
  <div class="label" style="top:86px">CustomEase · CustomWiggle · CustomBounce · SlowMo · ExpoScaleEase · RoughEase</div>
  <div class="label" style="top:880px">Physics2D · PhysicsProps</div>
  <div id="phys2d" class="row" style="top:930px;left:60px;background:#38bdf8"></div>
  <div id="physprops" class="row" style="top:930px;left:200px;background:#f472b6"></div>
</div>
<script>
var ROWS = ${rows};
var stage = document.getElementById('stage');
for (var i = 0; i < ROWS.length; i++) {
  var el = document.createElement('div');
  el.className = 'row';
  el.id = ROWS[i].id;
  el.style.top = (170 + i * 96) + 'px';
  el.style.backgroundColor = ROWS[i].color;
  stage.appendChild(el);
}
// 官方写法，与 index.jsx 完全一致。
CustomEase.create('spring', 'M0,0 C0.126,0.382 0.282,0.674 0.44,0.822 0.632,1.002 0.818,1.001 1,1');
CustomWiggle.create('wiggle', { wiggles: 8, type: 'anticipate' });
CustomBounce.create('bounce', { strength: 0.7, squash: 0.4 });
gsap.registerPlugin(CustomEase, CustomWiggle, CustomBounce, SlowMo, ExpoScaleEase, RoughEase,
  Physics2DPlugin, PhysicsPropsPlugin);
// 暂停时间轴，只由 __seek 驱动：同一帧号永远给出同一张图。
var tl = gsap.timeline({ paused: true });
for (var j = 0; j < ROWS.length; j++) {
  // rough 的缓动表默认随机生成，两侧都会拿到不同的表。确定性写法是对象
  // 参数（字符串形式只把第一个参数传给 config，randomize 不生效）。
  var ease = ROWS[j].id === 'ease-rough'
    ? 'rough({strength:3, points:2, taper:"none", randomize:false})'
    : ROWS[j].ease;
  tl.fromTo('#' + ROWS[j].id, { x: 0 }, { x: 1560, duration: 3, ease: ease }, 0);
}
tl.fromTo('#phys2d', { x: 0, y: 0, rotation: 0 },
  { x: 1400, y: 400, rotation: 720, duration: 3, ease: 'none',
    physics2D: { velocity: 700, angle: -50, gravity: 900 } }, 4.2);
tl.fromTo('#physprops', { x: 0, opacity: 1, scale: 1 },
  { x: 1400, duration: 3, ease: 'none',
    physicsProps: { x: { velocity: 620, acceleration: 260 } } }, 4.2);
window.__seek = function (t) { tl.time(t); };
window.__ready = true;
</script>
</body>
</html>
`;
}

function findChrome() {
    const env = process.env.CHROME_BIN;
    if (env && existsSync(env)) return env;
    for (const c of CHROME_CANDIDATES) if (existsSync(c)) return c;
    throw new Error('找不到 Chrome：设置 CHROME_BIN 指向可执行文件');
}

// 最小 CDP 客户端：请求/响应配对 + 事件分发。
class Cdp {
    constructor(ws) {
        this.ws = ws;
        this.nextId = 1;
        this.pending = new Map();
        ws.addEventListener('message', (ev) => {
            const msg = JSON.parse(typeof ev.data === 'string' ? ev.data : String(ev.data));
            if (msg.id && this.pending.has(msg.id)) {
                const { resolve, reject } = this.pending.get(msg.id);
                this.pending.delete(msg.id);
                if (msg.error) reject(new Error(`${msg.error.message} (${msg.error.code})`));
                else resolve(msg.result);
            }
        });
    }
    send(method, params, sessionId) {
        const id = this.nextId++;
        const payload = { id, method, params: params || {} };
        if (sessionId) payload.sessionId = sessionId;
        this.ws.send(JSON.stringify(payload));
        return new Promise((resolve, reject) => this.pending.set(id, { resolve, reject }));
    }
}

async function connect(wsUrl) {
    const ws = new WebSocket(wsUrl);
    await new Promise((resolve, reject) => {
        ws.addEventListener('open', resolve, { once: true });
        ws.addEventListener('error', () => reject(new Error('CDP WebSocket 连接失败')), { once: true });
    });
    return new Cdp(ws);
}

const sleep = (ms) => new Promise((r) => setTimeout(r, ms));

async function main() {
    const argv = process.argv.slice(2);
    const fpsIdx = argv.indexOf('--fps');
    const fps = fpsIdx >= 0 ? Number(argv[fpsIdx + 1]) : FPS_DEFAULT;
    const framesIdx = argv.indexOf('--frames');
    const total = Math.round(DURATION * fps);
    let frames;
    if (argv.includes('--all')) {
        frames = Array.from({ length: total }, (_, i) => i);
    } else if (framesIdx >= 0) {
        frames = argv[framesIdx + 1].split(',').map((x) => Number(x.trim())).filter((x) => Number.isFinite(x));
    } else {
        frames = [0, 15, 30, 45, 60, 90, 120];
    }

    mkdirSync(OUT_DIR, { recursive: true });
    const page = join(OUT_DIR, 'index.html');
    writeFileSync(page, pageHtml());
    console.log(`[oracle] page -> ${page}`);

    const chrome = findChrome();
    const profile = join(OUT_DIR, '.chrome-profile');
    rmSync(profile, { recursive: true, force: true });
    const child = spawn(chrome, [
        '--headless=new',
        '--disable-gpu',
        '--hide-scrollbars',
        '--no-first-run',
        '--no-default-browser-check',
        '--remote-debugging-port=0',
        `--user-data-dir=${profile}`,
        '--window-size=1920,1080',
        '--force-device-scale-factor=1',
        'about:blank',
    ], { stdio: ['ignore', 'ignore', 'pipe'] });

    // Chrome 把实际端口打在 stderr 的 DevTools listening 行上。
    const wsUrl = await new Promise((resolve, reject) => {
        let buf = '';
        const timer = setTimeout(() => reject(new Error('等待 Chrome DevTools 端口超时')), 20000);
        child.stderr.on('data', (chunk) => {
            buf += chunk.toString();
            const m = /ws:\/\/\S+/.exec(buf);
            if (m) {
                clearTimeout(timer);
                resolve(m[0].replace('ws://localhost', 'ws://127.0.0.1'));
            }
        });
        child.on('exit', (code) => {
            clearTimeout(timer);
            reject(new Error(`Chrome 提前退出（code ${code}）`));
        });
    });

    const browser = await connect(wsUrl);
    const { targetId } = await browser.send('Target.createTarget', { url: 'about:blank' });
    const { sessionId } = await browser.send('Target.attachToTarget', { targetId, flatten: true });
    const send = (method, params) => browser.send(method, params, sessionId);

    await send('Page.enable');
    await send('Runtime.enable');
    await send('Emulation.setDeviceMetricsOverride', {
        width: 1920, height: 1080, deviceScaleFactor: 1, mobile: false,
    });

    await send('Page.navigate', { url: pathToFileURL(page).href });
    // 等页面把 __ready 立起来（脚本是同步的，poll 几次足够）。
    let ready = false;
    for (let i = 0; i < 100 && !ready; i++) {
        const r = await send('Runtime.evaluate', { expression: '!!window.__ready', returnByValue: true });
        ready = r.result && r.result.value === true;
        if (!ready) await sleep(50);
    }
    if (!ready) throw new Error('参考页未就绪：__ready 一直为 false');
    console.log('[oracle] page ready (official gsap in Chrome)');

    for (const frame of frames) {
        const t = frame / fps;
        const seek = await send('Runtime.evaluate', {
            expression: `window.__seek(${t}); tl.progress()`,
            returnByValue: true,
        });
        if (seek.exceptionDetails) {
            throw new Error(`__seek(${t}) 抛错：${seek.exceptionDetails.text}`);
        }
        const shot = await send('Page.captureScreenshot', { format: 'png', fromSurface: true });
        const out = join(OUT_DIR, `f${String(frame).padStart(4, '0')}.png`);
        writeFileSync(out, Buffer.from(shot.data, 'base64'));
        if (frame % 30 === 0 || frames.length <= 20) {
            console.log(`[oracle] frame ${frame} (t=${t.toFixed(2)}s) -> ${out}`);
        }
    }

    await browser.send('Target.closeTarget', { targetId });
    child.kill();
    console.log(`\n[oracle] ${frames.length} 帧 -> ${OUT_DIR}`);
    console.log(`[oracle] 合成参考视频：ffmpeg -framerate ${fps} -i ${join(OUT_DIR, 'f%04d.png')} -crf 18 -pix_fmt yuv420p out/gsap-oracle.mp4`);
}

main().catch((e) => {
    console.error('[oracle] 失败：', e && e.message ? e.message : e);
    process.exit(1);
});
