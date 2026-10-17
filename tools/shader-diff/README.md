# shader-diff — shaders 原库 vs opencat-shaders 逐组件对拍

oracle（shaders 原库，headless Chrome WebGPU）与 ours（opencat-shaders，Rust wgpu）
逐组件、逐采样帧的像素对拍系统。定位等价 k3 时代的 k3diff：渲染路径不同，
验收指标同一套（mae / maxd / p\<T\> / bbox + 热图）。

## 目录

```
catalog.mjs          从 shaders 库 dist 提取全组件默认 props → catalog.json
catalog.json         195 组件 × 预设（512×512、fps 30、采样帧 10/45/90）
page/oracle.html     oracle 页面（黑底合成契约：对比双方都在不透明黑底上合成；
                     engine 侧 wgpu 目标同样清黑叠加。改这里必须同步 engine 契约）
page/oracle-main.mjs oracle 协议（boot/start/step + 同任务 GPU readback）
render.mjs           puppeteer 驱动：选组件、步进帧、采集 PNG 落盘
diff.py              两棵结果树逐对比较 → rows.json / 热图 / report.md / 退出码
```

## 产物契约（ours 槽位必须同构）

```
<out>/<Component>/default/f<frame>.png     RGB PNG（512×512）
<out>/<Component>/default/manifest.json    props / preset / adapter / fps / sampleFrames
```

ours（opencat-shaders）按同一契约落盘后，`diff.py out-oracle out-ours` 即为对拍。

## 用法

```bash
# 一次性环境准备（shaders 库已 clone 并构建）
cd ~/Projects/shaders && pnpm install && pnpm lib:build

# 打 bundle（oracle-main.mjs 改动后重跑）
npx esbuild page/oracle-main.mjs --bundle --format=esm \
  --outfile=page/oracle.bundle.mjs --ignore-annotations --log-level=error

# 渲染 oracle
node render.mjs --only Aurora,LinearGradient --out out-oracle
node render.mjs --all --out out-oracle            # 全量 ~190 组件

# 复现性自检（期望 r1 vs r2 逐字节为零）
node render.mjs --only Aurora --runs 2 --out out-oracle

# 对拍 + 门禁
python3 diff.py out-oracle out-ours               # 退出码 0=过 1=超门 2=缺失
python3 diff.py out-oracle out-ours --gate-mae 0.5 --gate-p8 0.001
```

## 环境关键点（每一条都踩过坑，勿删）

1. **lavapipe 软件 Vulkan**：Chrome 进程内 NVIDIA ICD `vkAllocateMemory` OOM，
   GL/SwiftShader 路径毁于 headless shared-image 缺失。render.mjs 以
   `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json` 启动（adapter 报
   `architecture: swiftshader` 是正常的——ANGLE 走 Vulkan→lavapipe）。
2. **Chrome 旗标**：`--enable-unsafe-webgpu --enable-features=Vulkan --use-vulkan
   --use-angle=vulkan --disable-dev-shm-usage`。
3. **headless 无呈现通路**：canvas 呈现/CDP 截屏/2D drawImage 全部拿到 0 或失败
   （`SharedImageBackingFactory` 缺 WebgpuSwapChainTexture 支持）。唯一可靠采集 =
   GPU readback：canvas 纹理 COPY_SRC → copyTextureToBuffer → mapAsync。
4. **COPY_SRC 补丁**：库 `configureContext` 走 WebGPU 默认 usage（仅 RENDER_ATTACHMENT）。
   `start()` 里 initialize 之后重 `configure` 同一 context，补 COPY_SRC；
   alphaMode/format 与库一致（premultiplied / preferred）。
5. **waitForGpu:false 是黑图问题的最终修复**：默认 renderFrame 会 await GPU fence，
   该异步边界恰让 Chrome present 并**轮换 swapchain 纹理**（实测 8 次
   getCurrentTexture 返回 5 个不同实例），回读必拿到清空帧。同设备采样画布纹理
   时队列顺序已保证一致（presetRenderer 文档明示此用法），mapAsync 是同步点。
   采集必须与渲染在**同一次 evaluate、同一任务**内完成。
6. **确定性**：`createRendererFromJSON(..., {forceFullFrameRate: true, gpu:{device,adapter}})`
   + frame-locked `renderFrame({deltaSeconds})`（首调进入锁定，时钟 = 精确 Σdelta）。
   首帧 `{deltaSeconds: 0}` 仅预热（组合/管线编译不计入）。实测同机两次 run
   逐字节为零（见下）。
7. **esbuild `--ignore-annotations` 必须**：库 package.json `sideEffects:false`，
   否则组件注册的 side-effect import 被摇树（boot 输出 `registry` 数量即自检，应 200）。

## 复现性地板（2026-10-17 实测）

Aurora / LinearGradient × f10/f45/f90，两次 run：maxdiff=0（逐字节）。
动画差异可测性：Aurora f10 vs f45 mae=8.11、maxd=59。

## 门禁

`--gate-mae 0.5 --gate-p8 0.001` 为暂定值（oracle 自身噪声地板为 0，
门禁语义 = 「ours 对 oracle 的允许偏差」）。等 ours 侧 opencat-shaders 首批
组件落地后，按跨实现的真实差异分布收紧/放宽并在提交里记录依据。

## ours 槽位（待实现）

opencat-shaders（Rust wgpu）按同一 catalog 预设、同一帧步进（frame=f(t)，
Δt = 1/30）、同一黑底合成契约落盘 `<out>/<Component>/default/f<frame>.png`，
即可直接进入 diff.py 门禁。media 类组件（VideoTexture 等）在 catalog.json 的
EXCLUDE 里，双方都跳过。

## ours 槽位实现与首轮全量结果（2026-10-17）

`export/export-wgsl.test.ts`（驻留 shaders 包 `src/__tests__/gpu/zz-export-wgsl.test.ts`
同步副本，`pnpm vitest run` 执行）把全部 195 组件导出为 ours 槽位输入：

```
wgsl/<Component>/shader-N.wgsl   每 pass 一份（final 恒为最后一个）
wgsl/<Component>/passes.json     rtt 表 + finalReads + computeSteps
wgsl/<Component>/values.json     n_<id> → 字段 → 已 transform 的 uniform 值
wgsl/<Component>/extra.json      采样帧 f10/f45/f90 的 extraField CPU 回写快照
wgsl/<Component>/layout.json     预设树 + sysInitial（_sys 真初值）
```

`ours/`（Rust wgpu，lavapipe）读上述产物渲染 `out-ours/`：naga 反射 combined
struct 展平表打 uniform；帧循环 time_acc/anim_time 累计（Δt=1/30）；采样帧前写
extra.json 的 extraField（key `n<id>\0<field>`）。同步副本更新后必须重导出
（`cd ~/Projects/shaders/packages/core && pnpm vitest run src/__tests__/gpu/zz-export-wgsl.test.ts`）。

### 首轮全量门禁（mae≤0.5 / p8≤0.001）：181/195 过

FAIL 14 个，按根因分三档：

- **sin/大参数 hash ULP 档（9）**：`fract(sin(x*43758...))` 类 hash 在 naga/wgpu 与
  Dawn 编译下 ULP 不同，被 `fract`×大常数放大成完全不同噪声相位，架构固有差异，
  逐像素不可修复：WorleyNoise Voronoi Scratches Aurora Truchet FallingLines
  BarShift ConcentricSpin ObjectTracker。
- **旋转/采样边界 1-ULP 档（3）**：sin/cos(小角度) 1 ULP → 旋转坐标在条带边界
  相位差（1px 边带）：Stripes(mae 0.049) Engraving(2.48) Halftone(0.89)。
- **待深挖（2）**：MeshGradient(83.4) DiffuseBlur(1.54，疑模糊核 rtt 迭代精度)。
  MeshGradient 的 anchors 通路已验证正确（哨兵实验生效、CPU 理论 t 场合理），
  剩余差异在 scatterField+mxPerlin+warp 组合。

### extraField CPU 回写链路（本轮关键修复）

真 renderer 帧序 = updateAnimatedTime → onBeforeRender → flush → render。
组件经 `params.setExtraField` 写 uniform（scatteredAnchors/quadBlend conv*/
Form3D 参数等）。导出器注入 `writeExtraField`（捕获写入）+ `resolveDriverCpuValue`
（供回调读 `_animTime`）+ 90 帧模拟循环。**两个语义坑**：

1. `setExtraField` 是持久写入（dirty-keyed 组件只首帧写一次，值持续有效到下次
   覆盖）——pendingExtras 不能逐帧清空，snapshot 取当前累积状态。
2. ours 采样帧整块写 ubuf 前必须把 time/_animTime 同步进 ubuf（这两个平时直接
   写 GPU buffer，整块覆盖会把时间打回 initial 0）。

修复后收敛：StudioBackground mae=0（逐字节）、Form3D 0.0049、Blob 0.0032、
FlowingGradient 0.011。
