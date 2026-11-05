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
   `pause + seek(t, suppressEvents=true)`。页脚本每帧重跑，天然满足
   write-out IS seek；`window.__timelines` 注册值并入登记表。
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

## 已知缺口挂账

- 行内多行混排（真 IFC）= 既有 Paragraph 批次缺口，非本分支新增。
- `offsetWidth` 用布局探针预测链路（scope_boxes），合成盒（fit-content）
  首期近似：文本内在宽 + padding + border。
- k3 无 @font-face，字体走系统回退链（Arial），与 swgl 参考同机字体假设。
