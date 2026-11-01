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

## 后续批次（已拍板范围）

- S2 动画时钟全案（推钟协议全量验证）。
- S3 扩语料（compute 系出列）。
- S4 compute pass 协议（录制面协议级缺失）。
- S5 web 腿（原始代码直跑）。
- 长期约束：tie 敏感语料对拍必须双侧同 GPU；NVIDIA 驱动 fast-math 是平台事实，
  WGSL 的 IEEE 语义在 NVIDIA 上不可依赖。
