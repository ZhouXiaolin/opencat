// render.mjs — oracle 渲染驱动。
//
// 用法：
//   node render.mjs --only Aurora,MeshGradient --out out-oracle
//   node render.mjs --all --out out-oracle
//   node render.mjs --only Aurora --runs 2 --out out-oracle   # 自检（复现性地板）
//
// 产物契约（ours 槽位必须同构，见 README）：
//   <out>/<Component>/default/f<frame>.png
//   <out>/<Component>/default/manifest.json
//
// 采集：页面内 GPU readback（canvas 纹理 → COPY_SRC → MAP_READ），base64
// 回传 Node 编码 PNG。headless 的 canvas 呈现/截屏路径不可用（合成器
// shared-image 限制），readback 是唯一可靠通路。环境旗标见 CHROME_ARGS。

import { createServer } from 'node:http'
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'
import { fileURLToPath } from 'node:url'
import puppeteer from 'puppeteer-core'
import { PNG } from 'pngjs'

const HERE = dirname(fileURLToPath(import.meta.url))
const CHROME = '/usr/bin/google-chrome'
// lavapipe 软件 Vulkan：NVIDIA ICD 在 Chrome 沙箱 GPU 进程里
// vkAllocateMemory OOM；GL/SwiftShader 路径则毁于 shared-image 缺失。
const CHROME_ARGS = [
  '--enable-unsafe-webgpu',
  '--disable-dev-shm-usage',
  '--enable-features=Vulkan',
  '--use-vulkan',
  '--use-angle=vulkan',
]
const VK_ICD = '/usr/share/vulkan/icd.d/lvp_icd.json'

const args = process.argv.slice(2)
const flag = (name, dflt) => {
  const i = args.indexOf(`--${name}`)
  return i >= 0 ? args[i + 1] : dflt
}
const has = (name) => args.includes(`--${name}`)

const OUT = join(HERE, flag('out', 'out-oracle'))
const ONLY = flag('only', null)
const RUNS = Number(flag('runs', '1'))
const ALL = has('all')

const catalog = JSON.parse(readFileSync(join(HERE, 'catalog.json'), 'utf8'))
let components = catalog.components
if (ONLY) {
  const names = ONLY.split(',')
  components = components.filter((c) => names.includes(c.name))
  const missing = names.filter((n) => !catalog.components.some((c) => c.name === n))
  if (missing.length) {
    console.error(`catalog 中找不到: ${missing.join(', ')}`)
    process.exit(1)
  }
} else if (!ALL) {
  console.error('需要 --only A,B,C 或 --all（全量约 190 组件，耗时长）')
  process.exit(1)
}

// ── 静态服务器（WebGPU 需要安全上下文，file:// 不行） ──────────────────────
const server = createServer((req, res) => {
  const path = req.url === '/' ? '/oracle.html' : req.url.split('?')[0]
  try {
    const body = readFileSync(join(HERE, 'page', path))
    res.writeHead(200, {
      'content-type': path.endsWith('.html') ? 'text/html' : 'text/javascript',
    })
    res.end(body)
  } catch {
    res.writeHead(404)
    res.end()
  }
})
await new Promise((r) => server.listen(0, '127.0.0.1', r))
const PORT = server.address().port

const browser = await puppeteer.launch({
  executablePath: CHROME,
  headless: true,
  env: { ...process.env, VK_ICD_FILENAMES: VK_ICD },
  args: CHROME_ARGS,
})
const page = await browser.newPage()
await page.setViewport({ width: catalog.canvas.width, height: catalog.canvas.height })
page.on('console', (m) => {
  if (m.type() === 'error') console.error('  [page]', m.text().slice(0, 300))
})
await page.goto(`http://127.0.0.1:${PORT}/`, { waitUntil: 'load' })

const bootInfo = await page.evaluate(() => window.__oracle.boot())
if (!bootInfo.adapter) {
  console.error('WebGPU 不可用:', bootInfo.reason)
  await browser.close()
  process.exit(1)
}
const FORMAT = await page.evaluate(() => navigator.gpu.getPreferredCanvasFormat())
console.log(`adapter: ${JSON.stringify(bootInfo.adapter)} · registry=${bootInfo.registry} · format=${FORMAT}`)

const { width, height } = catalog.canvas
const maxFrame = Math.max(...catalog.sampleFrames)

// BGRA(readback) → RGBA(png)
function toPngBytes(u8) {
  if (FORMAT !== 'bgra8unorm') return u8
  for (let i = 0; i < u8.length; i += 4) {
    const b = u8[i]
    u8[i] = u8[i + 2]
    u8[i + 2] = b
  }
  return u8
}

for (const comp of components) {
  const dir = join(OUT, comp.name, 'default')
  mkdirSync(dir, { recursive: true })
  const job = { width, height, preset: comp.preset }
  for (let run = 1; run <= RUNS; run++) {
    const runSuffix = RUNS > 1 ? `-r${run}` : ''
    try {
      await page.evaluate((j) => window.__oracle.start(j), job)
      for (let f = 1; f <= maxFrame; f++) {
        const want = catalog.sampleFrames.includes(f)
        // 采样帧必须与渲染同一任务回读（跨 evaluate 读到的是被回收的纹理）。
        const res = await page.evaluate(async (c) => window.__oracle.step({ capture: c }), want)
        if (want && res.shot) {
          const u8 = toPngBytes(Buffer.from(res.shot.b64, 'base64'))
          const png = new PNG({ width: res.shot.w, height: res.shot.h })
          png.data.set(u8)
          const file = join(dir, runSuffix ? `f${f}${runSuffix}.png` : `f${f}.png`)
          writeFileSync(file, PNG.sync.write(png))
        }
      }
      writeFileSync(
        join(dir, runSuffix ? `manifest${runSuffix}.json` : 'manifest.json'),
        JSON.stringify({
          name: comp.name,
          props: comp.props,
          preset: comp.preset,
          adapter: bootInfo.adapter,
          format: FORMAT,
          fps: catalog.fps,
          sampleFrames: catalog.sampleFrames,
          run,
        }, null, 2),
      )
      console.log(`✓ ${comp.name}${runSuffix}`)
    } catch (e) {
      console.error(`✗ ${comp.name}${runSuffix}: ${String(e).slice(0, 200)}`)
      writeFileSync(join(dir, `ERROR${runSuffix}.txt`), String(e?.stack ?? e))
    }
  }
}

await browser.close()
server.close()
