# opencat 架构

本文描述 opencat 的分层、数据契约与渲染后端。事实来源是代码本身；
每节给出锚点文件。写作日期 2026-11-19，对应 dev 工作树。

## 1. 总览

三个主 crate 分层。职责单向：core 推导，engine/web 执行。

- **opencat-core**：HTML 实现 + hyperframes 扩展。纯数据 + trait + 算法，
  零 IO、零平台依赖（`crates/opencat-core/src/lib.rs`）。输入是宿主备好的
  资源元数据，输出是逐帧 op stream。core 不做任何解码。
- **opencat-engine**：桌面渲染引擎。承载 IO / ffmpeg / V8 / Skia / 系统字体
  （`crates/opencat-engine/src/lib.rs`）。
- **opencat-web**：浏览器渲染目标。core 编译成 wasm，宿主 JS 消费 op stream
  并驱动浏览器原生图形 API（`crates/opencat-web/src/wasm_bridge.rs`）。

数据流：

```
                 HTML + hyperframes 设计稿（输入）
                              │
                              ▼
        ┌────────────────── opencat-core ──────────────────┐
        │  parse → layout → script(V8/JS realm) → render   │
        │  每帧产出 RenderFrame：                           │
        │    draw  = 2D 绘制 op stream（OCIR）              │
        │    media = 媒体请求计划（不含像素）                │
        │    gpu   = GL / wgpu op stream（WebGL2 / WebGPU）│
        └──────────────┬───────────────────┬───────────────┘
                       │                   │
             RenderFrame（内存）     OCIR envelope（跨边界，可 OCZ1 压缩）
                       │                   │
                       ▼                   ▼
               opencat-engine         opencat-web（wasm32）
               桌面离线渲染            浏览器渲染
               skia + mozangle + wgpu CanvasKit + 原生 WebGL2/WebGPU
               backend 固定 Vulkan    GPU 指令由浏览器原生执行
                       │                   │
                       ▼                   ▼
                    RGBA 帧 / MP4       RGBA 帧 / MP4（WebCodecs）
```

两条腿共享同一 core、同一 op stream 契约。同一帧在两条腿上必须产出
一致的像素；一致性由对拍门验收，不靠实现巧合。

## 2. core：HTML 标准 + hyperframes 扩展

### 2.1 生命周期

core 的开放路径是显式生命周期（`crates/opencat-core/src/lifecycle/mod.rs`）：

```
parse → CompositionDraft → HostRequirements → HostInputs
      → prepare → PreparedComposition → open_pipeline
```

宿主负责 fetch / cache / decode / seek / prefetch / 导出。core 只校验
宿主送进来的元数据，然后打开一条纯推导管线。生产打开路径只走
`PreparedComposition::open_pipeline`。

### 2.2 frame = f(t)：RenderFrame 契约

管线的唯一契约（`crates/opencat-core/src/pipeline/mod.rs`）：

```rust
fn render_frame(&mut self, idx: u32) -> Result<RenderFrame>;
```

`RenderFrame` 是帧的纯函数（`crates/opencat-core/src/ir/draw_frame.rs`）：
同一帧——无论直接渲染、乱序渲染还是重复渲染——输出逐字段一致，与
调用历史无关。这是全部确定性要求的落点。

时间面是整数与有理数（`crates/opencat-core/src/time/`）：帧号、fps、
微秒时间戳、有理帧率。脚本面拿 `FrameCtx`（`frame_ctx.rs`）；媒体契约
只用微秒/有理数，宿主在最后一步才换算成平台 API 的单位。

动画算法是纯函数（`crates/opencat-core/src/script/animate/`）：
animate = g(t)，无副作用，入口状态存 `MutationStore`。随机数走
种子化 PRNG（`random_from_seed`），不吃全局熵。

### 2.3 op stream：三层

core 不渲染像素。它把一帧的全部视觉内容编码成三种 op stream，
都装在 `RenderFrame` 里：

1. **2D 绘制流**：`DrawOpFrame`（`ir/draw_frame.rs`、`ir/draw_op.rs`）。
   Display Item 经 `render/dispatch.rs` 派发，`render/canvas_lower.rs`
   降低成 `DrawOp` 序列，旁表挂 paints / paths / strings / bytes /
   f32_pool / effects。线格式是 OCIR envelope（magic `"OCIR"`，
   version 9，section 1–13，`ir/draw_encoding.rs`）。
2. **GL op stream**：`GpuSection` / `GpuStream`，backend = WebGL2
   （`ir/gpu_stream.rs`）。core 的 HTML 协议面
   `script/runtime/webgl_api.js` 把 `getContext('webgl2')` 实现成
   **录制器**：每次 GL 调用编码成 `[opcode: u8][payload]` 追加进目标
   canvas 的流。同步查询（getUniformLocation 等）由 core 侧确定性
   客户端状态机回答，不进流。变长数据（bufferData 像素、shader 源码）
   走流内 intern 池。绘制流每帧自足：帧首状态由帧内命令显式建立。
3. **wgpu op stream**：同一 `GpuStream`，backend = WebGPU。录制面是
   `script/runtime/webgpu_api.js`，opcode 表独立。

GL 流与 WebGL2 调用序列一一对应——它不是中间表示。执行宿主
（engine 的 mozangle，经 glow 调用面 / web 的浏览器原生 WebGL2）
逐 op 重放，零翻译。GPU 流的产出
以占位生成图（`ImageRef::Generated`，内容寻址 `GeneratedImageId`）
挂在 2D 流的正确 z 序上；宿主渲染流后把像素回填 media plan 的同名
条目。2D 流因此不需要知道 GPU 内容是什么。

### 2.4 core 不解码

core 的媒体面只有语义与请求（`crates/opencat-core/src/media/`）：

- `AudioPlan` / `AudioSegment`：全片音频时间线计划。
- `VideoFrameRequest` / `VideoFrameTiming` / `VideoColorMatrix`：
  逐帧视频采样请求与色彩矩阵契约。

解码、缓存、寻址全部由宿主实现。engine 用 ffmpeg；web 用 WebCodecs。
谁也不许在 core 里加解码路径。

### 2.5 HTML 标准实现

- 解析：html5ever（`parse/html.rs`）。
- 布局：taffy（flexbox / grid / block，`layout/`）。
- 文本：cosmic-text + fontdb，断行 UAX#14（`text/`）。
- CSS：`css/`，calc() 在求值层全部化简成字面值。
- 脚本：`script/`，宿主注入 JsContext（engine = V8，web = 浏览器 realm）。
  DOM / canvas / HTML 标准 API 垫片在 `script/runtime/*.js`。

### 2.6 hyperframes 扩展

HTML 词汇之外，core 实现两类扩展：

- **共有协议**（`parse/shared.rs`）：组合标识、可见时间窗（`data-start`）、
  尺寸、媒体相位、音量、`__timelines`。`<audio data-volume="0.30">`
  这类属性对齐 hyperframes-launch 语料。
- **HF 私有词汇**（`parse/hyperframes.rs`）：`data-track-index`、
  `<transition>` 协议属性（data-from/to/effect/direction/timing/
  damping/stiffness/mass/seed 等）。播放器 seek 事件的派发在 JS 扩展层
  `script/runtime/hyperframes_api.js`（`__ocHyperframes.dispatchSeek`）。

词表同时生成进 `protocol-vocab.generated.js`——新增只改 Rust 表。

### 2.7 跨边界传输与 schema 单源

- OCIR envelope 可包一层 OCZ1 容器过带宽敏感链路（server → browser）：
  raw DEFLATE（RFC 1951），浏览器用 `DecompressionStream('deflate-raw')`
  解（`ir/transport.rs`）。压缩是确定性的，纯函数性不受影响。
- 所有 wire 常量（opcode、section、词表、TypedArray kind 表）由 Rust
  单源生成 TypeScript：`ir/schema_gen.rs` → `cargo run -p opencat-core
  --bin ocir-schema-gen`，产物 checked in 在
  `crates/opencat-web/web/src/generated/`，并挂在 web build 链第一步。
  漂移不可能积累。

## 3. engine：桌面渲染引擎

engine 消费 `RenderFrame`，并将合成像素交给编码器。

### 3.1 渲染基础：skia / mozangle / wgpu（+ glow 绑定层）

三个执行模块：

- **Skia**（skia-safe）：执行 2D 绘制流。`EngineDrawExecutor`
  （`render/executor/`）把 `DrawOpFrame` 重放到 Skia canvas；
  Lottie 走 Skottie。
- **mozangle**（我们的 ANGLE 构建）：GL op stream 的**执行者**
  （`gpu/angle.rs`）。WebGL2 命令 ≈ OpenGL ES 调用序列——执行是
  直调，不是翻译（`gpu/stream_executor.rs`）。mozangle 严格按
  WebGL/GLES 语义校验参数：无效参数返回错误，不崩驱动（桌面 GL
  直调路径已退役，PMREM 在 WGL 上段错误的教训）。来源唯一：动态
  加载（`OC_ANGLE_DIR` 指向 ANGLE 目录；仓库预编译件在
  `tools/angle-linux`，Docker 镜像直接 COPY）。静态 mozangle
  （D3D11）已退出离线渲染路径。
- **wgpu**：wgpu op stream 的执行后端（`gpu/wgpu_executor.rs`）。
  opcode 到 wgpu crate 一对一映射；render pass 命令录到 PASS_END
  再重放（wgpu 的 RenderPass 借用 encoder，流序保证 pass 连续成段）。

**glow 不是后端，没有任何执行语义。**它是 Rust 的 GL 函数装载/
绑定层（crates.io glow，GL on Whatever）：会话建立时在 current
mozangle 上下文里加载函数入口（`eglGetProcAddress`），包成类型化的
`glow::Context`（`HasContext` trait）——就是函数指针表 + 类型安全
API。`stream_executor` 经 glow 调用 ANGLE。离屏会话
（`gpu/session.rs`）提供这个函数表。Skia Vulkan 合成器不用这个函数表。

离线工厂创建 Vulkan 执行目标或 CPU 光栅目标（`render/target.rs`）。Vulkan
合成器（`gpu/compositor.rs`）持有 Skia Vulkan surface 和会话。每帧合成后
同步读回 RGBA。当 ANGLE 能与 Skia 共享 Vulkan 设备时，WebGL 输出经
`TextureRegistry`（`gpu/registry.rs`）直接导入 Skia。设备不共享时，ANGLE
读回像素后再合成。WebGPU 输出由 wgpu 读回像素。桌面 presenter 各自管理
窗口 surface。

### 3.2 图形后端：Vulkan

离线渲染的 backend 固定为 Vulkan（`gpu/mod.rs`）。mozangle 落 Vulkan
device，wgpu 落 Vulkan，Skia 在 Vulkan session 上。硬件 GPU 与
SwiftShader 是 device 选择，不是两个 backend：

- 硬件 Vulkan：系统 ICD。
- 软渲染：`OC_ANGLE_SWIFTSHADER` 时从 `OC_ANGLE_DIR` 推
  `vk_swiftshader_icd.json`（`gpu/vk.rs::bootstrap_icd`）。
  `VK_ICD_FILENAMES` 是进程级变量，Skia / mozangle / wgpu 三家必须
  看到同一份 ICD。

平台现状矩阵（2026-11-19）：

| 平台 | 离线渲染 | 桌面预览 |
|------|----------|----------|
| Linux | 固定 Vulkan（mozangle/wgpu/Skia） | EGL(X11) + Skia GL（`present/linux.rs`） |
| Windows | 固定 Vulkan | Skia Vulkan + Win32 presenter |
| macOS | **Vulkan 未接入**（挂账） | CAMetalLayer + Skia Metal（`present/macos.rs`） |

### 3.3 媒体：ffmpeg + faac

- 解码：ffmpeg-next（`media/decode.rs`、`media/video.rs`）。
  `AudioPlan` 是全片唯一规范输出——engine 只读
  `pipeline.info().audio_plan`，不得重走组合树造第二套语义
  （`media/mod.rs` 头注释）。
- 编码：`media/encode.rs`（RGBA 帧序列 → MP4）；AAC 编码由
  内嵌 `crates/faac` 提供。
- 渲染驱动：`render/frame.rs::execute_render_frame` 是单帧执行的
  收口——executor + MediaContext + 生成图缓存束成 `FrameExec`。

### 3.4 脚本：V8

engine 的脚本后端是 V8（预编译，v8 crate 152）。JS realm 由
`EngineJsContext` 提供。CLI 入口在 `crates/opencat/src/bin/`
（`opencat`、`opencat-frames`），`crates/opencat` 是 App/CLI 门面。

## 4. web：浏览器渲染

### 4.1 wasm 桥

core 以 wasm32 编译进 `crates/opencat-web`。`WebRenderer`
（`src/wasm_bridge.rs`）持有打开一次的 `DefaultPipeline<WebJsContext>`；
每次 `build_frame_ir(frame)` 跑 `pipeline.render_frame` 并同时返回
自足 OCIR envelope 与帧 media plan——没有单独的 prepare 帧、没有
pending 缓存。web 侧在 `open_design` 时抓取全部声明资源，经显式
生命周期喂给 core——与 engine 的 host-owned 打开路径镜像。

### 4.2 渲染基础：CanvasKit + 浏览器原生 GPU

TS 侧（`crates/opencat-web/web/src/`）：

- **2D**：OCIR 解码（`draw-ir/decode.ts` → `interpret.ts`）后经
  CanvasKit（Skia wasm，`draw-ir/ck-adapter.ts`）重放。
- **GL 流**：浏览器真 WebGL2（`gpu/gl-executor.ts`）。
- **wgpu 流**：浏览器真 WebGPU（`gpu/wgpu-executor.ts`）。
- GLSL 反射 binding（core 的 glsl_reflector）：engine 腿由 mozangle
  离屏会话实现，web 腿由浏览器真 WebGL2 实现（`gpu/glsl-reflect.ts`）。
- 帧收口：`render-session.ts`——open-once 管线、可复用 CanvasKit
  surface、帧预算/堆/wasm 内存三道门（URL query 可覆盖）。

编码出 MP4 走 WebCodecs；视频解码同样 WebCodecs（`media/`）。

## 5. js-openmp：@omp 并行扩展

core 扩展了 JS：作者给热循环标 `@omp`，运行时把循环体提取成
bundle 派发给并行团队。文档化目标：语义照 OpenMP，但确定性优先。

### 5.1 链路

1. **提取期**：`@omp` 标注定位、自由标识符分析、span 级重写
   （`script/omp/transform.rs`，oxc 前端，版本与 core 锁定同批）。
2. **协议面**：worker 适配协议是引擎无关的
   （`script/omp/protocol.rs`）：`{__ocHeader, __ocRow, __ocSetCaps,
   __ocSetPrng, __ocNext}` IIFE 合同。不 import 任何具体引擎类型。
3. **数据面**：bundle 缓存 → args 解码 → 团队行派发 → 写回按行序
   落地（`script/omp/mod.rs`）。引擎只注入 `OmpTeam` trait 实现。
4. **执行腿**：
   - engine：rayon 池 + 每线程一个独立 V8 isolate
     （`engine/src/script/omp/team.rs` + `v8_team.rs`）。schedule
     映射 TR15 §19.6.3：static = 连续区间预分，dynamic(chunk) =
     chunk 行粒度认领，guided/auto = 自适应。线程亲和进类型系统
     （OwnedIsolate 是 !Send）。
   - web：worker 池 + SAB 任务板 + work-stealing
     （`crates/opencat-web/web/src/omp/omp-runner.ts`）。worker 是
     Blob 起的纯 JS realm；Rust 侧 `WebOmpTeam` 经全局
     `__ocOmpExecute` 派发。就绪/派发走 SAB 原子量（store-release），
     不走消息（本环境消息面会被主线程事件循环饿死）。

### 5.2 子集面：支持的构造与语义

事实源：`script/omp/transform.rs`（识别）、`script/omp/mod.rs`
（应用）、`script/omp/protocol.rs`（wire 面）。

**标注文法**：`// @omp parallel for`。Line 注释、独立一行、后跟
canonical for 巢（各层 init/test/update 齐全、body 是 block、非
for..in/of）。标注是零语义注释（asm.js / `@__PURE__` 同族）——浏览器
直跑语料行为不变，守卫只存在于 opencat 变换后的 driver。simd 变体
（`parallel for simd` / `for simd`）不采纳。任何解析失败都原样返回：
不变换 = 原循环照跑，即 OpenMP「pragma 未被编译器处理」的降级语义。

**子句支持表**：

| 子句 | 语义 |
|------|------|
| `shared(list)` / `private(list)` / `firstprivate(list)` | 数据共享类别；capture 名单按 OpenMP 规则合成，shadow 语义由 oxc ScopeTree 精确判定 |
| `default(shared)` / `default(none)` | 缺省类别 |
| `schedule(static \| dynamic \| guided \| auto \| runtime[, chunk])` | 调度原文进 ir，数据面解析：static=区间预分，dynamic[,chunk]=chunk 认领，其余=自适应。修饰符（monotonic: 等）不支持 |
| `reduction(+:list)` / `reduction(*:list)` | 归约。变量是行私有副本（TR15 §8.10），不算 capture；同 op 子句合并；v1 值域限 f64 |
| `collapse(n)` / `num_threads(n)` | 请求非承诺：结果与调度无关。num_threads 优先于 ICV 环境量 `OC_OMP_NUM_THREADS`（TR15 §7.7） |
| `lastprivate` `linear` `ordered` `if` `allocate` `order` `proc_bind` `copyin` `nowait` | 已认识但不支持 → 整条标注拒绝；未知子句同拒绝 |

**执行模型**：行 = 外层 header 的一次枚举（collapse 合并多层时行值
= 合并后的外层索引）。行函数的槽位类（`SlotKind`）五类：

- `Plain`：JSON 值直进 worker。
- `Prng`：零参随机函数槽，绑到预采池游标。循环内随机数不进行体
  bundle：发射的骨架（采集 IIFE）两遍跑——count 遍数内层迭代数，
  fill 遍按原序批量预采。行 i 消费池段 `[i*stride, (i+1)*stride)`。
  页面自己的 PRNG 原样跑，fallback 与并行逐位一致。
- `Array`：Set/Acc 写回槽。行产出 `IsolateWrite`（槽位 + 下标 +
  acc 标志 + f64 值），按行序应用到真数组。
- `ArrayRw`：读写数组槽。实参 = 帧初快照，每行从快照切片起步
  （行间零污染），Set/Acc 写穿工作副本（行内 read-after-write =
  串行语义）。
- `Ctx`：canvas 2D 上下文槽，绑到垫片 recorder。行内记
  fillStyle/满圈 arc，fill 时量化进 ovals 段（7 f64：cx, cy, r +
  RGBA，色已是 0-255 量化终值）。

**TypedArray binary 旁路**：shared 类别的 TypedArray 参数不走 JSON
逐元素串化——host 一次 memcpy 进快照（`SharedBuf`），args_json 只留
描述符，worker 按描述符重建真 TypedArray。九类支持
（F64/F32/U8Clamped/U8/I8/U16/I16/U32/I32），kind 码与字节宽表由
schema 单源生成（`omp-schema.generated.ts`）。v1 快照语义：行函数对
TA 的写落行私有缓冲，写不出本次执行——TA 实为只读面。

**reduction 应用**：行产出的终值槽按行序左折叠副本们，initializer
不重复参与（确定性收紧）。零行 = 域空：变量保持原值 → 不接管
（守卫 else 跑原循环，零次迭代幂等）。

**worker bundle 合同**：自包含 IIFE，五个入口
`{__ocHeader, __ocRow, __ocNext, __ocSetPrng, __ocSetCaps}`，
一次 eval 多帧复用（按 bundle.key 缓存）。

**诊断开关**：路由决定打一行日志（`isolate` / `degraded` /
`explain`），`OC_OMP_ROUTE=0` 静音；`OC_OMP_TRACE=1` 挂计时；
`OC_OMP_DISABLE=1` 全关走串行。

### 5.3 确定性三保证

1. 行产出按行号槽位合并——调度只影响负载分布，不影响字节。
2. f64 运算无归约、无 race——行间零共享可变状态（累加是对帧初值的
   有序累加，v1 值域限 f64）。
3. 同参数两跑逐位一致——帧 = f(t) 在并行腿同样成立。

### 5.4 降级链

任何一环失败（bundle 构建失败、解码失败、行体抛错、caps 漂移超界、
自旋看门狗触发）→ 毒化缓存条目 → 该 @omp 骨架不再跑 → fallback 到
主线程串行循环。输出不变；PRNG 不被无谓消耗。web 腿的降级一律
terminate 池再重建——宁杀勿猜（stale worker 会把完成计数永久打坏）。

## 6. 辅助 crate 与对拍

- **opencat-web-oracle**：对拍 harness 与 headless 渲染入口
  （`bin/opencat-web-render.rs`）。静态服务设 COOP/COEP（SAB 可用）。
- **原始 HTML 对齐**：`scripts/regression-gate.sh` 从 `REF_ROOT` 指定的
  `hyperframes-launches` 目录读取 `k3-promo`、`vfx-heygen-combined` 和
  `inspector-launch` 的 `index.html`，并与各自的 SWGL 录像比较。
  仓库内 `examples/` 还包含作者工程，经构建后的 `dist/index.html` 走同一
  HTML 入口。
- **scripts**：`walksheet.sh`（全语料逐例 walk_sheet）、
  `render-guard.sh`、`dist-full-align.sh`。
- SWGL 参考为 MP4。全片用 k3diff 与 FFmpeg SSIM 比较。检查重构时，两次
  engine 渲染使用同一输入、设备和编码参数。定位单帧像素差时，优先使用
  PNG，以排除 H.264 编码带来的差异。
- 唯一存留的对齐数据文档：`docs/html-align/
  VULKAN-SWEEP-PROFILE-2026-11-15.md`。

## 7. 设计不变量

1. core 零 IO、零解码、零平台依赖。往 core 加 fetch/decode 都是违约。
2. `RenderFrame` 是帧的纯函数。任何使输出依赖调用历史的改动都是违约。
3. op stream 是唯一跨边界视觉契约。执行宿主不得回读 core 内部资源表。
4. 离线 backend 只有 Vulkan 一个。新增 backend = 分叉渲染语义，禁止。
5. wire 常量单源生成（schema_gen）。手抄常量表禁止进 generated 目录。
6. @omp 并行与串行输出逐字节一致。调度改动不得影响字节。
