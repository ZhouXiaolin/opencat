# shaders 对齐线 · goal

对齐目标：F:\shaders（shaders.com 官方 npm 包 `shaders@4.0.6`，TypeGPU 封装的纯
WebGPU 效果库）。链路：Edge 真 WebGPU（chromedriver/msedgedriver 通路）出参考
真值 → opencat core 录制面（webgpu_api.js）成流 → engine wgpu_executor 执行 →
k3diff 判定。先对齐 engine 腿，web 腿（原始代码直跑）后置。

判据：每案 avg_p8 ≤ 2%（与主 gate 同门限）。双侧 PNG → crf0 MP4 →
`python tools/k3diff.py ref.mp4 test.mp4 --out <dir>`。

## 不变式

1. ref 侧真值可复现：同案重拍逐位一致（确定性协议：`renderSyntheticFrame(delta)`
   / 静态案 delta=0；页面内 `Object.defineProperty(window,'devicePixelRatio',
   {get:()=>1})` 钉 dpr）。
2. engine 侧失败必须 fail-loud 定位到具体 opcode/资源，不留静默替身。
3. 对拍口径：双侧同 GPU。headless 浏览器的 WebGPU adapter 是本机核显，engine
   必须钉同一块（见批次 S1 根因②）。
4. 探针/对拍一律 PNG 直出（`opencat render <in> <out.png> <frame>`）。mp4 腿
   4:2:0 色度半采样会伪造逐像素结论（S1c 实锤：16×2 随机色块探针 32/32 假分歧）。

## corpus 运行方式

```
cd docs/shaders-align/corpus
CASE=<案名> bun vite build          # 插件自动 opencat render 出 dist/<案>.mp4
python tools/k3diff.py out-k3/ref-<案>.mp4 dist/<案>.mp4 --out out-k3/<案>
```

ref 侧（一次性）：`bun ref/shoot.mjs <案>`（写 ref/out/<案>/frame_*.png）→
ffmpeg crf0 合成 out-k3/ref-<案>.mp4。corpus 插件给 render 子进程预设
`OC_WGPU_BACKEND=dx12 OC_WGPU_POWER=low`（与 ref 腿同 GPU，原因见下）。

## 批次 S1（2026-11-02）：基建 + 五案 pilot，全 PASS

语料：solid / gradient / noise / blend / noise-anim（全纯 fragment，
`createShader(canvas,{components:[...]})` js 入口；静态 4 案单帧本质，noise-anim
30 帧 delta=1/30 双侧同钟）。

结果：五案 avg_p8 全部 **0.0000%**（mae ≤ 0.99/255，编码底噪级）。

- solid 0.0000% / gradient 0.0000% / blend 0.0000% / noise 0.0000% /
  noise-anim 0.0000%

### 根因①（ref 侧，已修）：Windows 显示缩放泄入 headless Edge

dpr=1.25 → shaders lib 把 canvas buffer 配成 clientWidth×dpr=1200×675，ref.html
的 `drawImage(1200×675 → 960×540)` 回缩是非线性变换，平滑渐变整体压暗 ~10%
（solid 全平场幸存，属假阴性掩护）。修复：页面内 defineProperty 钉 dpr=1
（`--force-device-scale-factor=1` chromedriver flag 实测无效）。

### 根因②（engine 侧，已修）：GPU adapter 不一致 + 驱动 fast-math

**真结论**：headless Edge/Chrome 的 WebGPU 固定落核显（本机 Intel UHD630，
D3D12，驱动 31.0.101.2140，算术逐位 IEEE）；engine 缺省
`PowerPreference::HighPerformance` 拿独显（NVIDIA MX250，驱动 582.78），其
驱动 JIT 在 f32 tie 节点（如 48×141421360=6,788,225,280 恰为 f32 网格中点）
重结合浮点表达式 → hash33 品格哈希整格翻转 → Perlin 晶格错 → 噪声场大块分歧。
NVIDIA 侧 DXBC（FXC）/DXIL（DXC）/SPIR-V（Vulkan）**三路同病**——是驱动级
fast-math，与 shader 编译器链无关。核显侧逐位 IEEE（16×2 hash33 微探针
browser=IEEE 32/32）。

修复：corpus 插件给 render 子进程钉 `OC_WGPU_BACKEND=dx12 OC_WGPU_POWER=low`
（engine 侧新增 OC_WGPU_POWER env，缺省仍 HighPerformance）。两个段错误教训：

- 核显 **Vulkan 路 `create_device` 段错误**（31.0.101.2140），必须走 dx12；
- 不能用 `enumerate_adapters` 子串过滤选 adapter——其 adapter 未经 hal 初始化，
  `request_device` 直接段错误；必须走 `request_adapter` 正路。

### 误诊记录（防再犯）

1. 「naga HLSL 后端不发 `precise` → 驱动 FMA 契约偏离 IEEE」——被 dxc.exe 静态
   实验证伪：DXC 对普通 fadd/fmul **根本不发** `dx.precise`（局部变量级
   `precise` 只标记 round 族与 alloca；函数级 `precise` 语法接受但无标记）；
   且 engine FXC 腿与 DXC 腿输出逐位相同，编译器选型彻底出局。vendor-naga
   打 precise patch 的方案未实施即作废。
2. 探针对比走了 mp4（crf18+4:2:0）→ 16×2 随机色图 32/32 假分歧。改 PNG 直出后
   才看到真实的 9/32。此后探针一律 PNG 直出（不变式 4）。
3. 「WGPU_DX12_COMPILER env 对照实验」无效——wgpu 从不读该 env，两跑同路，属
   伪对照。任何 A/B 结论前先确认开关真的生效（adapter/编译器都以日志或产物
   差异为证）。

### 诊断基建（保留）

- `OC_DUMP_WGPU_STREAM=<dir>`：WRITE_BUFFER 字节 dump（buf-N.bin）。
- wgpu 30 `Instance`/`Adapter` 日志可达（log crate 非可选依赖），装任意 log
  实现即可 bisect 设备创建崩溃（本批临时加过 OC_WGPU_LOG，已随收口删除）。

## 批次 S3+S4（2026-11-02）：扩语料 + compute pass 协议，16/16 PASS

S3：语料 5→16 案（Shapes/Textures/gradient 家族/Simplex/Worley/child 协议/
Plasma 动画）；compute 系 19 案按拍板出列（Glass/PixelSort 等）。
S4：compute pass 协议补齐——core opcode 37-39（BEGIN_COMPUTE_PASS/
DISPATCH_WORKGROUPS/DISPATCH_INDIRECT）、录制面（webgpu_api.js）、执行器
段回放（SegKind::Compute + storage read-write 绑定）。验收案
ReactionDiffusion（TypeGPU compute 管线，32 次 dispatch 覆盖 ping-pong
双向 bind group 交替）**avg_p8=0.0000%**（mp4-vs-mp4，双侧 crf0）。

结果：16/16 PASS。12 案 0.0000%（含 noise-anim/plasma-anim 两个 30 帧动画案，
S1 的推钟协议全量验证由此完成）；亚像素 AA 底噪案 star 0.0613% / conic
0.2508% / radial 0.4259% / truchet 1.0482%（gate 内预期级）。worley-noise
以 `jitter: 0` 收录（见内在极限②）。

### 根因①（engine 侧，已修）：extent_of 只认 dict 形态

GPUExtent3D 双形：sequence `[w,h,d?]` 或 dict `{width,...}`。录制面 size 走
数组，执行器只解析 dict → 缺省 1×1——所有经 createTexture 的纹理（S4 前无
消费方）实际建成 1×1，RD 显示链全黑。修复 = 双形解析（origin_of 同一自由度）。
教训：协议面补齐时对每个 W3C 类型先查 IDL 联合形态，录制端用什么形态就测
什么形态。

### 根因②（对拍口径，已修）：引擎腿 mp4 默认 crf 18 量化淹没信号

ref 腿 crf 0 无损；engine 腿 libx264 crf 18 在饱和色/细纹理上产生 ±8-20 的
量化噪声，RD 实测 6.6% → 钉 `OC_VIDEO_CRF=0` 后 0.0000%（engine 新增 env，
产品默认 18 不动；run-all.mjs 构建 env 固定注入）。诊断链条：先 PNG 直出
（不变式 4）确认真 0 差 → 再回头查编码链。

### 根因③（corpus 协议，已修）：rAF 墙钟渲染泄入 animatedTime

shaders lib 的 requestRender 在 initialize 期间挂
`requestAnimationFrame(frameTime => render(frameTime))`；engine rAF 桩当帧
排空 → 墙钟 delta 0.016 进 animatedTime（worley/voronoi 特征点全位移）。
corpus 钟协议拍板：**时间只经 deltaSeconds 推进，两侧全程禁用 rAF**
（ref-entry.mjs 与两模板同禁）。次生坑：anim 模板首绘必须放在
initialize 的 then 里——微任务 drain 时机在本帧最后一次 effect 之后时，
靠 effect 铸帧 0 会整帧落空（f0 黑帧，noise-anim/plasma-anim 曾 3.33%=
1/30）。静态模板（then 首绘）零此病，solid 已证。

### 内在极限两类（出列判据，跨编译器不可对拍）

1. **FMA 收缩分歧**：同一 WGSL 经 Dawn(Tint)→HLSL→DXC 与
   wgpu(naga)→HLSL→DXC，mul+add 链的 FMA 收缩决策不同 → 每次运算末位差。
   Gray-Scott 反应扩散对末位差指数放大（speed 6×30 帧 19.8%，分歧集中在
   成斑期）。RD 约束为单帧 + speed 32：种子核纯 IEEE 单步运算逐位一致，
   32 次迭代同时覆盖 ping-pong 两向；团斑形态对 4:2:0 色度可行（稀疏 1px
   点是色度最劣形态，speed 2 单帧 mp4 14.6% vs PNG 0.0000% 的教训）。
2. **数位提取散列**：`fract(sin(dot(p,(127.1,311.7)))*43758.5453)`——
   sin 参数 1 ulp × 43758 → fract 相位跳变 → 特征点全场重排。voronoi
   （无 jitter 面）/worley-anim（漂移即抖动路径）出列；worley-noise 保留
   `jitter: 0`（mix(0.5,…,0)=0.5 精确关断，实测 0.0000%）。

### 基建（保留）

- `run-all.mjs`：批量 runner（ref shoot → crf0 合 ref mp4 → vite build
  渲 engine mp4 → k3diff）。vite 成功判据 = 日志 `[opencat] Done` 且 mp4
  落盘（UNRESOLVED_ENTRY exit 1 是 S1 既有怪癖）。
- `gen-cases.mjs`：presets.mjs → cases/*.jsx 生成器（static/anim 双模板，
  协议面注释在模板头）。
- 流级诊断：`python -I <tmp>/parse_stream.py <streamDir>` 全 opcode 解包
  （QUEUE_SUBMIT 变长、SHADER_MODULE 原文 range）；buffer readback 必须
  clamp 到 buf.size()（64KiB 探针拷 192B buffer = 非法拷贝拿零）。
- 运维：改 presets.mjs 必须 `bun ref/build.mjs`（ref 腿吃预构建 bundle）；
  reshoot 前清 `ref/out/<案>`（stale PNG 污染计数）。

## 后续批次（已拍板范围）

- S2 动画时钟全案（推钟协议全量验证）——S3 动画案已部分覆盖，余量待估。
- S3 扩语料（compute 系出列）✅ 2026-11-02。
- S4 compute pass 协议（录制面协议级缺失）✅ 2026-11-02。
- S5 web 腿（原始代码直跑）。
- 长期约束：tie 敏感语料对拍必须双侧同 GPU；NVIDIA 驱动 fast-math 是平台事实，
  WGSL 的 IEEE 语义在 NVIDIA 上不可依赖；**跨编译器内在极限两类（FMA 收缩/
  数位提取散列）语料选型时先查 WGSL 源，命中即出列或找关闭面**。
