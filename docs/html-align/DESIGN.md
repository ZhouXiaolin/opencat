# HTML 直渲对齐设计（hyperframes .html → OpenCat 管线）

分支 `feat/html5ever-hyperframes`。目标：core 引入 html5ever，结合 engine，
直接渲染 F:/hyperframes-launches 的原始 .html 工程；验收 =
release 渲染视频 vs 同工程 `renders/*_swgl.mp4` 的 k3diff，gate 不退化。
首期打样：k3-promo html 原版（单文件、零伪元素、零 @keyframes）。

## hyperframes 语义结论（侦察汇总）

- 不改 HTML 本体，而是「编译期打包 + 页面内确定性 runtime」：所有动画对象
  paused，`t = floor(frame)/fps` 绝对 seek。与我们 frame=f(t) 同构。
- composition 协议长在 data-* 属性上：`data-composition-id/src/start/
  duration/width/height/track-index`；`data-start` 支持引用表达式；
  可见窗半开 `[start, end)`，首帧前 hidden。
- 嵌套 composition = DOM 内联（编译期 inline 或运行时 fetch+mount），
  子文件内容在 `<template>` 里。语料嵌套深度全部为 1。
- 动画 100% GSAP timeline（21/21）；CSS 动画几乎为零，CSS 只做静态造型。
  伪元素 14/21（k3-promo 为 0）；组合器极冷门但后代选择器遍地。
- 字体：@font-face 本地 woff2 15/21，Google Fonts CDN 6/21（拍板：补网络
  支持，抓取+磁盘缓存保确定性）。
- 媒体：video 16/21；渲染时引擎把 video 抽帧换 `<img>`。

## 我们管线的关键机制（对齐锚点）

- 提取期（一次）：V8 realm 重估模块图 → `__ocExtract` 取回
  `{meta, fonts, tree, css, audio}` payload → `from_extract` →
  DomDocument → `composition_from_dom` → ParsedComposition（Rust 静态树）。
- 每帧：`run_frame` 重跑 driver（`__ocResetFrame` fresh globals，gsap/
  three/canvaskit/g2 模块表保活）→ hook 体内声明 tween → write-out IS
  seek（`tween.time(t)`）→ MutationStore.take_mutations → resolve 期
  apply_mutation_stack 落到 ElementNode → taffy 布局 → DrawOp。
- 脚本面：`document`/`ctx.getNode(id)`/style proxy → store（gsap
  CSSPlugin 写 `style.transform` 直解 transform 列表，store.rs）。

## 架构决策

1. **JS 拥有最终树（单通道镜像 jsx）**：Rust html5ever 解析 .html →
   种子 JSON（DomElement 形）+ css + scripts → JS 物化成带 DOM API 的
   元素面 → 页面 `<script>` 按序执行（动态建 DOM：createElement/
   appendChild/innerHTML 全部落在 JS 树上）→ `__ocHtmlExtract()` 把
   最终树序列化回 payload → 复用 `from_extract` 全下游。每帧重跑同一段
   脚本（确定性保证结构逐帧一致），样式经 store 变更落 Rust。
2. **html5ever 落 core**（parse/html.rs）：符合 core 纯化判据（core 只
   产字节与树，不解释绘制）。解析器 = html5ever + markup5ever_rcdom。
3. **seek 协议 = 包裹 gsap 工厂**：driver 在页面脚本执行前 patch
   `gsap.timeline/to/from/fromTo/set`，登记所有产物；脚本跑完后统一
   `pause + seekOne(t)`（`suppressEvents=false`——页面把整帧重绘挂在
   timeline 的 onUpdate 上，抑制事件会让逐帧函数永不执行）。播放头已≈t
   时（首帧 t=0、同 t 重复 seek）正向 nudge 到 t+1e-4 强制一次渲染、
   不回退——反向 seek 会把同位 zero-duration set 重置成 init 捕获值；
   正常逐帧推进恰好单次渲染 = swgl 的 stale-1frame 语义（master 连续
   补间的 onUpdate 消费上一帧终值）。`window.__timelines` 注册值并入
   登记表。
4. **CSS 级联升级**：`compute_node_style` 加 `SelectorContext`
   （祖先链 + 前兄链 + tag 别名），支持后代/子/相邻/通用组合器与 `*`；
   伪元素/伪类继续丢弃（k3 无伪元素；14/21 伪元素推后续批次）。
   装配期 build_node DFS 天然携带上下文。
5. **可见窗 = NodeStyle.clip_window**：DomElement::Div 等扩
   `data-start/data-duration` 字段；resolve 期窗外 → taffy
   `Display::None`（整子树退出布局与绘制）。
6. **文本降级规则（语料近似，批次 A）**：
   - 仅文本子节点 → 该元素直接承载 text（P/Span 按标签映射）；
   - 混合内容 → 文本 run 包成合成子元素（块父 → 合成 P，行内父 → 合并
     进既有行内文本流）；空白按 CSS 折叠（pre 例外）；
   - 行内 span 兄弟 + `white-space:nowrap`（k3 的 .aline）→ 容器转
     taffy row（近似浏览器行内流；多行行内混排仍是已知缺口=Paragraph 批次）。
7. **网络支持层**（engine）：远程 `<script src>`、GF css2、woff2 走
   reqwest 抓取 + `base_dir/.oc-cache/net/<sha256>` 磁盘缓存；缓存命中
   则零网络，渲染确定性不受损。本地路径照旧 base_dir 解析。
8. **html5ever 树上的 id**：id 属性优先；缺省合成 `hf-<文档序>`（静态）
   / `s<序>`（脚本建）。两阶段必须逐位一致——脚本确定性保证。

## 批次

- **A（本批）**：解析骨架 + data-* 协议 + 级联组合器 + JS DOM 面 +
  gsap seek + 可见窗 + .html 入口 + 网络层。验收 k3-promo。
- B：伪元素 ::before/::after 绘制面、mask-image、@keyframes。
- C：video/audio 时间轴语义深化、多工程扩量（codex → HF-heygen-stripe）。
- D：three/WebGL shader/lottie 工程与重字体工程。

## 分层收束（2026-11-10 用户口径，待分批迁移）

用户口径：**core = 与浏览器标准一致的 HTML / JS / CSS**，
**自有格式（jsx + hyperframes 扩展）= 单独一层**（适配层 / engine）。
当前主体已合规：子组合作用域（`wrapScopedCompositionScript`）、
`data-composition-src` 嵌套展开、CSS 作用域前缀、脚本时间基准、
`hf-seek`、`__timelines` 全在 `crates/opencat-engine/src/script/html.rs`
（适配层），core 不解释 HF 语义。

**存量分层债**（core 里仍残留 HF 专有语义，迁移需全量门禁回归，分批做）：

| 位置 | 残留语义 | 迁移目标 |
|---|---|---|
| `parse/html.rs` 根选择（≈685、703、728、777） | `data-composition-id` 决定合成根、`<template>` 透明展开、子文档裸选择器改指子根 | engine `splice_nested_hosts` / `scope_composition_css` 调用侧 |
| `parse/dom.rs` `data-start` / `data-duration` 字段（≈48–51、132–141） | 可见窗半开 `[start,end)` = hyperframes 时间窗协议 | 适配层产出的 IR 字段；core 只认通用「可见窗」 |
| `resolve/resolve.rs` 末帧不隐藏（≈140） | hyperframes「终止瞬间」语义 | 同上 |
| `media/types.rs` 注释（≈133） | `data-start` 语义说明 | 随字段迁移 |

迁移判据（沿用 ARCHITECTURE.md §2）：core 可**生成**协议字节但永不**解释**
领域语义。`data-*` 属性本身是标准 HTML 属性（应留在 core 的属性表里），
要迁出的是「读到 `data-composition-*` 就改变树结构 / 可见窗」的**解释逻辑**。

**不改变渲染结果的重构，必须与修复批次分开提交**，避免回归归因困难。
每批迁移后跑 s28 同口径全量门禁（24 例 k3diff + 耗时/内存榜）确认逐位一致。

## 已知缺口挂账

- 行内多行混排（真 IFC）= 既有 Paragraph 批次缺口，非本分支新增。
- `offsetWidth` 用布局探针预测链路（scope_boxes），合成盒（fit-content）
  首期近似：文本内在宽 + padding + border。
- k3 无 @font-face，字体走系统回退链（Arial），与 swgl 参考同机字体假设。

## GL 执行端：ANGLE 落地（2026-11-11）

拍板（用户口径）：**WebGL 标准路径走 ANGLE**，不按端各做一遍对齐。
core 的 GL stream → engine 的桌面 GL 转换层因此可以整体省掉——
ANGLE 严格按 WebGL/GLES 语义校验参数，与 web 端（Chrome）同源。

### 各端来源

| 端 | ANGLE 来源 | 设备 | 说明 |
|---|---|---|---|
| Windows | 静态 mozangle | D3D11 | 原有路径，无运行时 DLL 依赖 |
| Windows | 动态 `OC_ANGLE_DIR` | Vulkan+SwiftShader | 与 swgl 参考同渲染器 |
| Linux | 动态 `OC_ANGLE_DIR` | Vulkan+硬件 GPU | **日常渲染**（GPU 优先） |
| Linux | 动态 `OC_ANGLE_DIR` | Vulkan+SwiftShader | 与 swgl 参考同渲染器（`OC_ANGLE_SWIFTSHADER=1`） |

mozangle 的 `egl` feature **只支持 Windows**（其 build.rs 硬断言非 Windows
平台不支持 EGL 构建），且它打包的是 D3D11 后端——Linux 走官方 ANGLE 动态库
（取自 Electron 发行包，`tools/angle-linux/fetch.sh`）。两端共用同一个 ANGLE
实现，GL 流语义不随平台漂移。

### Linux 上的两个真问题

1. **原生 GL 不认 GLSL ES 300**：Mesa 桌面 GL 编译 three 的 ES 300 shader
   直接失败（`syntax error, unexpected NEW_IDENTIFIER`）。ANGLE 是 GLES3
   上下文，ES 300 原文直接编译——这正是 mozangle 当初被选中的原因。
2. **帧内状态机与跨帧客户端影子脱节**：three 的流是增量的（程序没变就不重发
   `useProgram`），而执行器每帧把帧内 `program_key` 重置为 `None` → 流首的
   uniform 找不到程序（`uniform 命令出现在 USE_PROGRAM 之前`）。修复：帧内
   状态机继承 `ex.client.cur_program_key`（program 是跨帧的 GL 全局状态，
   `restore_default_state` 已把真 GL 绑回同一程序）。

### 实测归因（vfx 1920×1080，SwiftShader，MP4 路径）

| 阶段 | 耗时/帧 | 占比 |
|---|---|---|
| script（V8 + HTML 布局） | ~40ms | 4% |
| gl-stream（GL 命令流执行） | ~1.5ms | 0.15% |
| **compose（Skia 合成）** | **~930ms** | **94%** |
| readback（PBO 双缓冲读回） | ~1.3ms | 0.1% |

结论：**Skia 合成占 94%**——它是 ANGLE 上的光栅化成本，GL 命令流本身
只占 0.15%。换硬件 ANGLE 直接砍掉这 94%（软件→硬件光栅）。

### 当前分数（vfx-heygen-combined，788 帧）

- 全片 **avg_p8 = 0.1516**（门限 0.02）——Linux 上从「一帧渲不出」到
  全片可比的第一个基线。
- 分布：f0-249 严重（p8 0.29~0.54，3D 折射段）、f300+ 良好（p8 0.004~0.07，
  2D 段基本达标）。
- 剩余主要差异：3D 段文字几何位置（左亮边缘差 145px），非 GL 语义——
  折射/色散/bloom/字形本身逐位对齐。

### 已知环境限制

ANGLE + **NVIDIA** Vulkan 驱动在编译 shader 时崩（`NVVM compilation failed`
后 SIGSEGV，驱动 bug，非本仓代码）。AMD RADV 与 lavapipe 正常。
