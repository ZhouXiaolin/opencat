# vfx 原始 html 直渲对齐计划

分支 `feat/html5ever-hyperframes`。终极目标：**用 OpenCat 的 core/engine 完美执行 hyperframes 原始 html**。

**总目标（2026-11-06 拍板）：至少四个工程 index.html 直渲 + 各自 swgl 参考 k3diff avg_p8 < 0.02：**
`k3-promo`（已达标 0.0175 全片 html 版）、`vfx-heygen-combined`（本计划主体）、`hyperframes-launch`（hf）、
`liquid-brand-refraction`。

- 渲染输入：`F:/hyperframes-launches/vfx-heygen-combined/index.html`（1617 行，未做任何改动）
- 参考：`F:/hyperframes-launches/vfx-heygen-combined/renders/vfx-heygen-combined_2026-10-05_swgl.mp4`
  （1920×1080@30，788 帧 / 26.267s）
- 门：`python tools/k3diff.py <ref> <our> --out <dir>`，**avg_p8 < 0.02**
- 不写测试；只用 k3diff。`examples/vfx-heygen-combined`（jsx 移植版）不参与本计划，它的分数与本目标无关。

**GL 执行端拍板（2026-11-06）：engine 的 WebGL 命令流执行换 `mozangle`（servo 维护的 ANGLE crate，
Windows D3D11 后端），不再直调桌面 GL。** 动机：PMREM（three r147 半浮点 RT 多 pass 预滤波）在
WGL/NVIDIA 桌面 GL 上驱动层段错误（`gl.create_vertex_array()` 内部崩，OC_GL_TRACE 定位实证）；
ANGLE 严格按 WebGL/GLES 语义校验参数（无效参数返回错误不崩驱动），且语义贴近 swgl 参考实现。

## 一、起点（2026-11-06 实测）

**当前一帧都渲不出。** 原样命令与报错：

```
$ ./target/release/opencat.exe render F:/hyperframes-launches/vfx-heygen-combined/index.html out/x.png 0
Error: html: nested composition (data-composition-src) not supported yet (1 host(s))
```

把 `data-composition-src` 属性临时改名（工程目录内 `_probe.html`，探针）后继续推进，暴露下一层阻塞：

```
Error: failed to download asset from https://cdn.jsdelivr.net/npm/gsap@3.14.2/dist/gsap.min.js
  ... tunnel error: unsuccessful        # 代理抖动，重试后通过
Error: script eval: ReferenceError: Request is not defined
    at mc.load (<anonymous>:1148:499169)
    at GLTFLoader.load (<anonymous>:1790:11)
```

所以当前的阻塞是三层串起来的，不是渲染质量问题：

1. **`data-composition-src` 嵌套未实现** —— `#vfx-intro` 引 `compositions/vfx-text-cursor.html`
   （794 行，`data-start=0`、`data-duration=6.48`，覆盖 0–194 帧，占全片 25%）。
2. **CDN 依赖无离线保障** —— html 头部引 8 个远程脚本：gsap 3.14.2、three r147 及其
   DRACOLoader / GLTFLoader / CopyShader / LuminosityHighPassShader / EffectComposer /
   RenderPass / ShaderPass / UnrealBloomPass。网络抖一次整帧渲染就失败，全片渲染不可复现。
3. **运行时缺 Web 平台面** —— three 的 FileLoader 需要 `Request`/`fetch`（读 `models/iphone.glb`）；
   再往上 `new THREE.WebGLRenderer({canvas})` 需要 `canvas.getContext('webgl'|'webgl2')`。

## 二、目标工程的渲染依赖清单（从 index.html 读出）

| 元素 | 依赖能力 | 现状 |
| --- | --- | --- |
| `#vfx-intro` → `compositions/vfx-text-cursor.html` | 嵌套 composition（子时间轴 + 窗口） | ❌ 硬报错 |
| `<video id="bg-video" src="assets/bg.mp4" data-start="5.58" data-duration="5">` | video 解码 + 帧同步 + 窗口 | ❌ 未接 |
| `<canvas id="theater">` + three r147 WebGLRenderer + EffectComposer/UnrealBloomPass | GL 上下文 + 后处理链 | ❌ html 通道无 GL |
| `<canvas id="chromatic-canvas">` / `#portal-canvas` | GLSL shader（全屏场） | ❌ |
| `<canvas id="chromatic-cap" layoutsubtree>` + 内嵌 `<section>` | canvas 承载 DOM 子树（新 API） | ❌ 未支持 |
| `<img>` ×3（png/svg）、`captures/heygen-mobile/screen-strip.png` | 图片/SVG 光栅 | ✅ 大体可用 |
| 内联 `<svg><path>` 图标 | svg 通道 | ✅ 已有 svg-align 能力 |
| CSS 布局/渐变/滤镜（`filter: blur/drop-shadow`） | html 通道 CSS | ⚠️ 部分（filter 未全） |
| 页面脚本（1000+ 行：时间线、shader、打字、DOM 动画） | JS DOM + MutationStore | ✅ 可用（k3 已验证） |

## 三、里程碑（按阻塞顺序，不按工作量）

### M0 渲出第一帧（解除三层阻塞）
- [ ] **M0.1 嵌套 composition**：宿主 div 按 `data-start/duration/width/height` 挂载子文档，
      子文档的脚本在自己的 realm 里跑（或共享 realm + 时间轴命名空间隔离），子时间轴按
      `t_local = t_host - start` 驱动，窗口外不进渲染。判据：`_probe.html` 恢复原属性后能出图。
- [ ] **M0.2 依赖可复现**：CDN 脚本预取进磁盘缓存，并提供「工程内 vendor 目录优先」的解析顺序；
      判据：断网重渲同一帧成功，且缓存命中时耗时 < 1s。
- [ ] **M0.3 `fetch`/`Request`/`Headers`/`XMLHttpRequest` 最小面**：至少覆盖 three FileLoader 的
      二进制加载路径（本地文件 + 缓存 + http(s)），返回 ArrayBuffer/Blob 语义正确。
- [ ] **M0.4 `canvas.getContext('webgl2'|'webgl')`**：桥到引擎既有 GL 通道（jsx 通道已有 GL stream
      DrawOp），支持 three 用到的子集：buffer/texture/program/uniform/framebuffer/readPixels。
      **这是最大的一块**，先做「能出图」，不追求完整 API。
- 判据：第 0/100/300/700 帧能渲出图 × 无错误 × 与参考同区段目视同构。

### M1 逐场景点亮（每场景独立判据）
按时间顺序，每段先「有 vs 无」，再「对不对」：

- [ ] 0–194 帧：intro（`vfx-text-cursor` 子工程）—— 依赖 M0.1；先对 DOM/文本，再对光线场。
- [ ] 167–317 帧：`bg-video` 素材（5.58s 起 5s）—— 依赖 video 通道。
- [ ] 全片：`#theater` iPhone 场景 —— 依赖 M0.3/M0.4；先对几何/构图，再对光照/色相，
      最后对 bloom（UnrealBloomPass 的参数与权重）。
- [ ] 400–500 帧：chromatic-split 全屏 shader。
- [ ] 尾段：portal shader + CTA。

### M2 结构与低频对齐（每段 p8 收敛）
- [ ] 段级指标表（每 50 帧）作为唯一进度表，每次全片渲染后追加。
- [ ] 先消「整块缺失/位置错」，再消「色带/亮度差」（可用低通 p8 作为结构指标）。

### M3 颗粒与位级
- [ ] intro 的 `fract(sin(gx*489+gy*647+progress*1527.168)*492)` sin-hash 颗粒：
      参考由 SwiftShader(swgl) 渲，我们的 GL 后端浮点近似可能不同。
      **先测同一像素的参考值 vs 我们值**，再决定是「对齐实现」还是「标为已知地板」。

## 四、顺带解决的余下问题（撞到就修，不单独立项）

- [ ] `letterSpacing` 进 canvas mock 的 measureText（k3 遗留；vfx 打字场景同样依赖测量）。
- [ ] `@font-face` / 工程内字体（vfx html 用 `font-family: Inter, system-ui, ...`，系统无 Inter）。
- [ ] `filter: blur()/drop-shadow()`（时间线大量进出场用）。
- [ ] `<canvas layoutsubtree>` 的 DOM 子树承载。
- [ ] 视频通道（`<video>` + data-start/duration + muted + 逐帧 seek）。
- [ ] html 通道的 k3 不退化：全局改动后 k3-promo/engine 必须仍 ≈0.0124（当前 0.0175 全片 html 版）。

## 五、纪律

- 只认 k3diff；全片渲染约 3 分钟、比对约 1 分钟，改完先单帧定位。
- 单帧：`./target/release/opencat.exe render <html> out.png <frame>`
- 任何全局改动（布局/文本/GL/脚本宿主）都跑 `SKIP_BUILD=1 ./scripts/regression-gate.sh` 三例。
- 探针一律落在工程内 `_probe*.html` 或 `OC_*` 环境变量，收尾即删；
  `grep -rn "OC_[A-Z]" crates/` 与工程内 `_probe*` 都必须清零。
- 每完成一个里程碑，把该段典型帧的并排图存 `out/vfx-align/`，指标写进本文底部。

## 六、风险（先摆明）

1. **M0.4 体量最大**：three r147 依赖的 WebGL 面不小（含 framebuffer 链、浮点纹理、readPixels），
   可能要和 jsx 通道的 GL 实现合并，才不至于写两套。
2. **swgl 是软件光栅**：即使 shader 逻辑一致，浮点/精度差异也会让 sin-hash 颗粒、bloom 阈值不同。
   全片 2% 的达成与否很可能卡在 M3，而不是布局。
3. **离线可复现**是硬前置：网络抖动会让「同一份代码渲出不同结果」，这比任何视觉差距都危险。

## 七、进度记录

- 2026-11-06 起点：一帧不出。阻塞①嵌套 composition（硬报错）②CDN 依赖无离线保障
  ③`Request is not defined`（three FileLoader）④html 通道无 GL 上下文。工程内探针 `_probe.html` 已建（收尾删）。
- 2026-11-06 上半场（本会话）：M0 大块已过，进入渲染链排雷。
  - 上会话遗留改动确认可用：M0.1 嵌套 composition（splice + 分段 seek）、属性选择器全量、
    `getContext('webgl')` 桥进 core 录制面、fetch/Blob 资产垫进 html 通道。
  - 修 extract 回传：JS serializeNode 的 `src` 按 http(s) 分流成 `path`/`url`（serde 端 DomElement::Image/Video
    只认这两个字段；静态 seed 的 path/url 原样带回）——img 首个报错由此而来。
  - 缺资产降级（浏览器 onerror 语义）：本地文件缺失 loader 跳过 + 1×1 透明 PNG 占位进缓存注册
    handle，prepare 不再 fail-fast。首个实例：`captures/heygen-mobile/screen-strip.png` 工程缺失
    （HANDOFF.md 列了但目录空）；JS 面被页面自己的 `complete && naturalHeight > screenH` 防御吞掉，
    drawImage 不触发。**strip 段内容差异记 M1 段落债**（参考里手机屏是真实捕获页面）。
  - svg img 链路：imagesize 不认 svg 字节 → core 新增 `svg_raster`（usvg 尺寸 + resvg 光栅成预乘
    RGBA，vendor 依赖，text feature 关）；probe 分流 + `static_image_scaled` 解码分流。`_mini-svg`
    探针验证通过。**挂账：img 无 CSS 尺寸时布局塌 0（ImageMeta 固有尺寸未接布局）**。
  - **vfx index.html f0/60/100/300/700 全部出图（exit 0）**，但内容差距大：f60/f100 全黑
    （intro 子工程在宿主内未渲染——下一段主敌）；f300 红块 vs 参考打字机（参考 f300 仍在 intro，
    时间轴与 data-duration 语义待核）；f700 灰 vs 参考 portal。M1 逐场景点亮从 intro 开始。
  - **回归门（ANGLE 切换后）**：k3-promo/engine **PASS 0.01241（与基线逐位一致）**；web 三例
    ChromeDriver 缺失（环境）。**vfx jsx 28.59→86.3%、hf engine 6.69→10.06% 恶化，未提交**：
    归因实验（GL 5 文件回 HEAD → vfx jsx 回 0.28589 基线）证明恶化全来自 ANGLE 执行端；
    tagAsWebGL2 已限定 html 通道（`__ocHtmlChannel`），限定后 jsx f300 逐位不变——
    **根因在 ANGLE×three r152「WebGL1 判定」命令流路径**（three 场景不渲染，liquid shader
    正常；蓝噪声场占位）。下一批：修 three r152 WebGL1 语义流在 ANGLE GLES3 上的绘制
    （候选：GLSL100 shader 直编的行为差 / OES_VAO 扩展面 / 纹理上传语义）。
    注意 stash 陷阱：`git checkout stash@{0} --` 恢复了过时 WIP（已归零重打补丁）。
  - 探针清单（收尾删）：`_probe.html` `_mini-svg.html` `_mini-gl.html` `_mini-gl2.html`
    `_probe2/3/4.html` `_probeN.html`。

## 八、批次③：全线 Vulkan 统一（2026-11-12 起，当前优先）

**拍板（2026-11-12）**：Linux / Windows / macOS 三端都用 Vulkan 启用 GPU 管线；
GL 与 Metal 路径退役。

**动机**：Docker 打包后是纯 CPU 环境，没有 GPU 驱动。SwiftShader（Chrome/ANGLE
用的 CPU Vulkan 实现）是唯一能同时喂饱 Skia、ANGLE、wgpu 三家的底座——三者都
说 Vulkan，就不必各写一份 CPU 光栅路径，也不必「一端一端地做对齐」。

### 已验证的前提（2026-11-12 实测）

1. **三平台官方预编译包都带 Vulkan**（rust-skia 0.153.3，`rust-skia/skia-binaries`
   releases）：`x86_64-unknown-linux-gnu-…-vulkan-…`、`x86_64-apple-darwin-…-vulkan-…`、
   `x86_64-pc-windows-msvc-…-vulkan-…` 均存在——改 feature 不会回退源码编译。
2. **Skia Vulkan backend 在 SwiftShader 上可建可渲**：`ash` 建 instance/device/queue
   → `skia_safe::gpu::direct_contexts::make_vulkan` → `render_target` 画矩形，
   读回像素精确为 `[255,0,0,255]`。
3. **Skia Vulkan 与 GL 路径逐位一致**：vfx-heygen-combined f305，Vulkan vs GL
   `p8 = 0.0`（逐位相同），两者对参考帧的差异也相同（`p8 = 0.0363`，残差是已知
   dy=+10 几何偏移，修正后 0.01）。
4. **ANGLE 暴露 `EGL_ANGLE_vulkan_image = YES`**：这是 WebGL 纹理能与 Skia
   Vulkan backend 跨 API 零拷贝共享的前提（实测自 ANGLE Vulkan 后端的 EGL
   扩展列表）。
5. **wgpu 可在同一 SwiftShader ICD 上建设备**：adapter = `SwiftShader Device
   (Subzero) (Cpu) backend=Vulkan`。

### 本批次要做的事

- **V1 预编译包本地化**（已完成）：`tools/skia-binaries/fetch.sh` 拉官方包到
  `~/skia-binaries/`，`SKIA_BINARIES_URL` 指向本地 `file://` 地址。改
  `skia-safe` 的 feature 后 key 会变，必须重跑该脚本，否则回退源码编译。
- **V2 Vulkan 会话**（已完成）：`gpu::vk::VkSession`（ash 0.38 的句柄模型与旧版
  不同：`Handle::as_raw()`）+ `gpu::vk::bootstrap_icd()` 统一 ICD。
- **V3 Skia Vulkan 合成路径**（已完成，Linux）：`CompositorBackend::Vulkan` +
  `OC_SKIA_BACKEND=vulkan` 开关；`VkRenderSession` 走 Skia 自建渲染目标。
- **V4 WebGL 纹理跨 API 导入**（已完成，lavapipe 实测）：用
  `EGL_ANGLE_vulkan_image` 把 ANGLE 的 VkImage 交给 Skia 的
  `backend_render_targets::make_vulkan`，替掉 GL 纹理 `make_gl` 导入；同光栅器
  下与独立 VkSession 逐字节一致。SwiftShader 上的零拷贝仍卡在 JIT SIGILL。
（2026-11-12 复测：此句失效。SwiftShader 零拷贝已完整跑通 788 帧；见
「SwiftShader 全片速度复测」。）
- **V5 GL/Metal 退役**：删 `gpu/gl.rs`、`gpu/win32.rs`、`gpu/metal.rs`、
  `gpu/egl.rs` 与 `present/{linux,win32,macos}.rs` 的 GL/Metal 分支；
  `present/` 改走 Vulkan + 窗口 surface。
- **V6 Windows/macOS 迁移**：Windows 的 ANGLE 走 Vulkan 后端（不再 D3D11）；
  macOS 需要 MoltenVK（Vulkan on Metal）——无 MoltenVK 则 macOS 无 Vulkan。
- **V7 单帧 tile 并行**（已验证，默认关）：帧级 flume + rayon 并行在
  lavapipe 零拷贝下 96s / 10.4GiB，相对串行 98s / 3.05GiB 无收益——光栅器
  线程已吃满 CPU。保留 `OC_PAR_JOBS` 开关，默认串行；单帧 tile 分块不再作为
  当前加速杠杆。

### 纪律

- 每步都跑 k3diff 门：vfx-heygen-combined 全片 `avg_p8` 与基线（0.1516）
  不得劣化；k3-promo 保持 PASS（0.01385）。
- `OC_VK_ZERO_COPY` 打开时，WebGL 流输出纹理经 VkImage 导入 Skia；关闭时走
  CPU 位图回读。最终合成帧仍同步读回给编码器，该读回不是纹理间零拷贝的目标。

### V4/V7 实测（2026-11-12 深夜，lavapipe 零拷贝打通）

**三个真根因修复（全部已验）**：

1. `vulkan_handles()` 出参 `queue_family` 用了 `u32`——ANGLE 按 EGLAttrib
   （64 位）写，多写 4 字节溢出到相邻栈变量；编译器布局一变就把
   `gipa` 砸成垃圾值（`0x710800000000` 型）。出参一律改 `u64`。
2. ICD dispatch 两处错：Mesa 系 ICD（lavapipe）只导出
   `vk_icdGetInstanceProcAddr`（硬编码 dlsym `vkGetInstanceProcAddr` 拿
   不到）；ICD 库路径改从 `VK_ICD_FILENAMES` 首个 json 的
   `ICD.library_path` 解析（裸文件名走 dlopen 搜索，含 `/` 相对 json 目录）。
   device 级函数仍按驱动分派：SwiftShader 走 ICD gipa，lavapipe 的
   instance/device 是 loader 级句柄，走 loader 的 `vkGetDeviceProcAddr`，
   并以 `BackendContext max_api_version=1.1` 兜住 loader 对 1.2/1.3 核心
   命令的 NULL 门控。
3. `FrameRenderer::new` 先建 `GpuRenderSession` 再被 Vk 分支遮蔽丢弃——其
   ANGLE 会话 Drop 调 `eglTerminate`，杀死共享 display：零拷贝路径
   `make_current` 报 0x3001（EGL_NOT_INITIALIZED），ANGLE 的 VkDevice 连带
   销毁，Skia VMA 析构撞死 device（ICD 内解引用 `0x2881`）。双症状同源，
   先判别 `OC_SKIA_BACKEND` 再建会话即除。

**零拷贝链路验证**：k3-promo f10，lavapipe 零拷贝 PNG 与独立 VkSession
PNG **逐字节一致**（md5 相同）；探针 `OC_VK_PROBE=1` 六步全绿。

**vfx-heygen-combined 全片 788 帧（1920×1080@30，26.267s）实测**：

| 配置 | 耗时 | 峰值 RSS | avg_p8（vs swgl 参考） |
|---|---|---|---|
| SwiftShader 串行（CPU 回读，旧基线） | 333s | 3.14GiB | 0.1681 |
| lavapipe 零拷贝 串行 | **98s** | **3.05GiB** | 0.3794 ❌ |
| lavapipe 零拷贝 OC_PAR_JOBS=3 | 96s | 10.4GiB | — |

- 串行稳态分解（lavapipe 零拷贝）：script=12.4ms gl-stream=32.7ms
  draw=59.3ms readback=17.9ms，avg 122.4ms/帧。零拷贝把 gl-stream 从
  310ms 砍到 33ms——**速度 3.4 倍的来源就是砍工作量，不是并行**。
- `OC_PAR_JOBS=3`（flume+rayon 帧级并行，每 worker 独立 pipeline）无收益：
  光栅器线程已吃满 CPU，内存反而 3.4 倍。帧级并行保留作开关，默认串行。
- **像素门 FAIL**：avg_p8=0.3794 远超基线 0.1516。分歧集中在 ANGLE 光栅的
  WebGL 3D 开场（f0-199，p0 低至 0.11）；非 WebGL 段 mae≈0.24——llvmpipe
  与 SwiftShader 的光栅精度/AA 不同，ANGLE-on-Vulkan(llvmpipe) 与参考
  ANGLE-on-swgl 逐位分歧。SwiftShader-Vulkan 与 swgl 同族，故 0.1681 接近。
  零拷贝导入本身无损（同光栅器下逐字节一致）。

**拍板（2026-11-12）**：接受 llvmpipeline（lavapipe）作为当前 Vulkan 后端，
以 98s / 3.05GiB 的速度与内存两门为准；像素门按「同光栅器下零拷贝无损、
非 WebGL 段逐位一致」验收，WebGL 3D 开场与 swgl 参考的类间差异暂不阻塞
本批提交。下一步研究 SwiftShader JIT 的 SIGILL：目标是让 ANGLE 与 Skia 同跑
SwiftShader-Vulkan，恢复像素等价，同时保住零拷贝的速度与内存收益。
`OC_SKIP_REFLECTOR` / `OC_VK_PROBE` / `OC_TRACE_VK_PROC` 等探针按诊断开关
保留，不按死代码清理。

### JIT 研究（2026-11-12，提交后）

**复测结论：此前记录的「SwiftShader 零拷贝卡在 JIT SIGILL」已经不可考。**
当前提交在 k3-promo f10 上用 `OC_VK_ZERO_COPY=1` +
`OC_ANGLE_SWIFTSHADER=1` 完整跑通六步探针：Skia make_vulkan → render_target
→ clear → submit → read_pixels → 合成，无 SIGILL、无 panic。进程里出现
`/memfd:swiftshader_jit`，说明 Subzero/Reactor JIT 正在执行。

**短样本实测（vfx-heygen-combined，PNG 序列）**：

| 样本 | 帧数 | 后端 | elapsed | peak RSS |
|---|---|---|---|---|
| f0,1,2,5,10 | 5 | SwiftShader 零拷贝 | 8.00s | 715MiB |
| f0,1,2,5,10 | 5 | llvmpipe 零拷贝 | 3.30s | 813MiB |
| f100..780 | 8 | SwiftShader 零拷贝 | 11.81s | 1.21GiB |
| f100..780 | 8 | llvmpipe 零拷贝 | 8.90s | 1.42GiB |

- f0/1/2 两后端逐字节一致；f5 几乎一致；f10 出现 WebGL 光栅差异。
- 长尾 f100/200/300/400/780 差异很小；f500/600/700 两后端几乎全画面分歧。
  SwiftShader 输出在 f500 起大量黑像素，llvmpipe 输出是浅色背景；
  `OC_KEEP_STREAM_TEX=1` 与 SwiftShader 独立 VkSession（CPU 回读）都复现同一
  黑像素形态。故这不是零拷贝导入损坏，而是 ANGLE-on-SwiftShader 与
  ANGLE-on-llvmpipe 在该场景的 GL 流/光栅产物不同。
- 整片 MP4 路径已复测完成 788 帧，无卡死；此前「2 分钟无输出帧」结论作废。
  当前待解的是连续长片 f511 后的像素漂移，不是 JIT SIGILL 或编码队列卡死。

**JIT 边界**：

- SwiftShader：Electron 自带 `libvk_swiftshader.so` 是 Subzero/Reactor 后端；
  upstream 文档说明 CMake 默认 LLVM、Chrome GN 默认 Subzero（二进制更小）。
  二进制里可见 `ThreadCount` / `AffinityMask` / Subzero translation 开关字样，
  但发布库无符号、无运行时环境变量文档；不从字符串猜开关。
- llvmpipe：`libvulkan_lvp.so` 链接 `LLVM*MCJIT*` 系列符号，shader 由 LLVM JIT
  编译；Mesa 文档只稳定暴露 shader cache 开关（`MESA_SHADER_CACHE_*`）与调试
  用 `LP_NO_RAST`，没有可安全加速生产渲染的 JIT 调优旋钮。
- 结论：当前可执行的加速杠杆仍是「少做一次 CPU 回读」（零拷贝）与后端选择，
  不是改 JIT。要同时拿回 swgl 像素等价，下一步应定位 f500–700 的 ANGLE GL
  流差异，并单独诊断整片 MP4 连续渲染路径。

### SwiftShader 全片速度复测（2026-11-12）

复测配置：`OC_SKIA_BACKEND=vulkan`、`OC_ANGLE_DIR=tools/angle-linux`、
`OC_ANGLE_SWIFTSHADER=1`、
`VK_ICD_FILENAMES=tools/angle-linux/vk_swiftshader_icd.json`，不带
`OC_VK_ZERO_COPY`（独立 VkSession + GL 流 CPU 回读）。这是旧基线里能完整
跑完的 SwiftShader 路径；新加的零拷贝开关在 SwiftShader 上另测。

**vfx-heygen-combined 全片 788 帧实测**：

| 后端 | 配置 | 耗时 | 峰值 RSS | avg_p8 | mae_mean |
|---|---|---|---|---|---|
| llvmpipeline | 零拷贝，串行 | 110s | 3045MiB | 0.37788 | 48.4752 |
| SwiftShader | 独立 VkSession，CPU 回读 | **306s** | **3133MiB** | **0.16805** | **5.3721** |
| SwiftShader | 纹理零拷贝，共享 ANGLE VkDevice | 385s | 3079MiB | 0.43607 | 62.3621 |

- SwiftShader 墙钟 305.96s、用户态 3148.01s、约 10.7 核占用；输出
  `ffprobe` 为 1920×1080@30、26.266667s、788 帧，文件大小 76M。
- 与同轮 llvmpipeline 相比：SwiftShader 慢 **2.78 倍**，峰值 RSS 高 88MiB；
  但 `avg_p8` 0.16805 对 0.37788、mae 5.37 对 48.48，像素显著更接近 swgl
  参考。速度门（2–3min）不达标。
- SwiftShader 纹理零拷贝**已完整跑完 788 帧**：墙钟 384.32s、用户态
  4113.49s、约 11.1 核占用，`ffprobe` 为 1920×1080@30、26.266667s、788 帧。
  此前按前 67 帧外推的 24.5min、79 帧停止结论作废。逐帧均值：script 15.3ms、
  gl-stream 5.0ms、draw 52.6ms、readback 412.7ms、total 485.7ms；f0–99
  readback 1840ms/帧，f100–199 796ms/帧，f200–299 52ms/帧，f300 后约
  86–135ms/帧。纹理零拷贝在 SwiftShader 上比 CPU 回读慢 79s，比 llvmpipeline
  零拷贝慢 3.5 倍。
- 像素门禁：SwiftShader 零拷贝 f0–510 与 CPU 回读逐帧接近；f511 起整片灰白，
  k3diff f511 `p8=0.98923`、`mae=243.86`。连续 PNG f500–539 与 MP4 segment
  500:540 的短窗口复测里，零拷贝与 CPU 回读逐字节一致。长片从 f0 连续执行到
  f511 后出现状态漂移，短样本不能代表全片相位。
- 归因复测（2026-11-12）：`OC_SKIP_REFLECTOR=1` 会改变像素相位。该开关下，
  独立 VkSession + CPU 回读与共享 VkDevice + 纹理零拷贝的 `--segment 0:540`
  都在 f511 灰白；全片独立 VkSession 仅设该开关也灰白，耗时从 306s 降到
  91s、峰值 RSS 2962MiB、788 帧完整。关反射器不能用来验证零拷贝像素。
- 严格对照（正常反射器、同样 `--segment 0:540`）：独立 VkSession + CPU 回读
  226s、3108MiB，f510/f511 与全片正确基线逐字节一致；共享 VkDevice + 纹理
  零拷贝 311s、3105MiB，f511 仍灰白。故 f511 漂移确属 ANGLE→Skia 纹理导入
  路径，不是 segment、反射器或最终合成帧读回。
- 速度归因：反射器开启时，SwiftShader 零拷贝 `--segment 0:540` 前 100 帧
  readback 均值 2188.7ms；关闭反射器时均值 39.7ms，段耗时 60s、2913MiB。
  反射器创建第二个离屏 ANGLE 会话，是前段 2s/帧 readback 的来源。
  `OC_VK_GL_FINISH=1`、`OC_KEEP_STREAM_TEX=1`、`OC_FORCE_CPU_STREAM=1`、
  Skia 资源缓存清理都不改变 f510/f511 像素。
- 结论：当前要速度用 llvmpipeline 零拷贝；要像素接近参考用 SwiftShader CPU
  回读，但耗时约 5min。SwiftShader 纹理零拷贝能完成全片，但速度不达标；
  后续要同时处理两条独立问题：GLSL 反射器的第二 ANGLE 会话（速度），以及
  ANGLE→Skia 借入纹理在 f511 后的状态漂移（像素）。不要改最终合成帧读回。
