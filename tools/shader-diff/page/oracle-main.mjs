// oracle-main.mjs — oracle 渲染页。
//
// 绝对路径 import shaders 库 dist：esbuild 打包时从该文件位置向上解析
// bare deps（typegpu 等），产出自包含单文件 bundle。
//
// 协议（render.mjs 经 page.evaluate 调用）：
//   __oracle.boot()                 → { adapter } 适配器信息（无 WebGPU 时 null）
//   __oracle.start(job)             → 初始化 renderer + t=0 预热帧（不进时钟）
//   __oracle.step({capture})        → 推进 1/fps 一帧；capture 时同一任务内回读，
//                                     返回 { frame, shot?: {w,h,b64} }
//                                     （headless 下 present 会回收纹理，回读必须
//                                      与渲染同任务，隔一次 evaluate 就读到清空帧）
//   job = { width, height, preset: {type, props, children?} }
//
// 确定性：首个 deltaSeconds 调用进入 frame-locked，时钟归零，此后每帧
// 时间 = 精确 Σdelta（库保证 "same frame rendered in two tabs is
// pixel-identical"）。仿真类组件（ping-pong compute）按输出帧率逐帧步进。

import * as core from '/home/solaren/Projects/shaders/packages/core/dist/index.js'
import { getAllShaders } from '/home/solaren/Projects/shaders/packages/core/dist/registry.js'

const canvas = document.getElementById('stage')
const FPS = 30

let sharedAdapter = null
let sharedDevice = null

async function boot() {
  if (!('gpu' in navigator)) return { adapter: null, reason: 'navigator.gpu missing' }
  core.setShadersDebug(true)
  sharedAdapter = await navigator.gpu.requestAdapter()
  if (!sharedAdapter) return { adapter: null, reason: 'no adapter' }
  sharedDevice = await sharedAdapter.requestDevice()
  sharedDevice.addEventListener('uncapturederror', (e) =>
    console.error('[gpuerror]', e.error.message.slice(0, 400)))
  const info = sharedAdapter.info ?? {}
  return {
    adapter: {
      vendor: info.vendor ?? '',
      architecture: info.architecture ?? '',
      device: info.device ?? '',
      description: info.description ?? '',
    },
    // 自检：side-effect import 被摇树会导致 registry 空（--ignore-annotations 防住）。
    registry: getAllShaders().length,
  }
}

let renderer = null
let frame = 0

async function start(job) {
  if (renderer) {
    try { renderer.cleanup() } catch { /* 已失败渲染器清理可忽略 */ }
    renderer = null
  }
  canvas.width = job.width
  canvas.height = job.height
  frame = 0
  renderer = core.createRendererFromJSON(
    { components: [job.preset] },
    { forceFullFrameRate: true, gpu: { device: sharedDevice, adapter: sharedAdapter } },
  )
  await renderer.initialize(canvas)
  // 重配置 context 补 COPY_SRC（库走 WebGPU 默认 usage，画布纹理不可回读）。
  // alphaMode/format 与库保持一致（dist: configureCanvasContext → premultiplied）。
  canvas.getContext('webgpu').configure({
    device: sharedDevice,
    format: navigator.gpu.getPreferredCanvasFormat(),
    usage: GPUTextureUsage.RENDER_ATTACHMENT | GPUTextureUsage.COPY_SRC,
    alphaMode: 'premultiplied',
  })
  // 预热帧：不推进时钟，把组合/管线编译/首帧上传从计时段里摘出去。
  await renderer.renderFrame({ deltaSeconds: 0 })
}

async function step(opts = {}) {
  // waitForGpu:false：默认的 fence await 会产生任务边界，Chrome 恰在此 present
  // 并轮换 swapchain 纹理（回读必拿到清空帧）。同设备采样画布纹理时队列顺序
  // 已保证一致（presetRenderer 文档明示此用法），mapAsync 是真正的同步点。
  await renderer.renderFrame({ deltaSeconds: 1 / FPS, waitForGpu: false })
  frame += 1
  const out = { frame }
  if (opts.capture) {
    const cap = await captureNow()
    const u8 = new Uint8Array(cap.bytes)
    let s = ''
    const CH = 0x8000
    for (let i = 0; i < u8.length; i += CH) {
      s += String.fromCharCode.apply(null, u8.subarray(i, i + CH))
    }
    out.shot = { w: cap.width, h: cap.height, b64: btoa(s) }
  }
  return out
}

// 帧采集：canvas 纹理（start 里重 configure 补了 COPY_SRC）→ buffer → 字节。
// 只允许在渲染的同一任务里调用（见 step 注释）。
async function captureNow() {
  const W = canvas.width
  const H = canvas.height
  const bytesPerRow = Math.ceil((W * 4) / 256) * 256
  const buf = sharedDevice.createBuffer({
    size: bytesPerRow * H,
    usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ,
  })
  const enc = sharedDevice.createCommandEncoder()
  enc.copyTextureToBuffer(
    { texture: canvas.getContext('webgpu').getCurrentTexture() },
    { buffer: buf, bytesPerRow, rowsPerImage: H },
    [W, H],
  )
  sharedDevice.queue.submit([enc.finish()])
  await buf.mapAsync(GPUMapMode.READ)
  const src = new Uint8Array(buf.getMappedRange())
  // 去掉 bpr padding，交给 Node 端编码 PNG。
  const out = new Uint8Array(W * H * 4)
  for (let y = 0; y < H; y++) out.set(src.subarray(y * bytesPerRow, y * bytesPerRow + W * 4), y * W * 4)
  buf.unmap()
  buf.destroy()
  return { width: W, height: H, bytes: out.buffer }
}

window.__oracle = {
  boot, start, step, fps: FPS,
  __oracleDevice: () => sharedDevice,
  failureReason: () => renderer?.getFailureReason?.() ?? null,
}
