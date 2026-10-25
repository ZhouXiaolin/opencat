# OpenCat 架构宪法

> 本文是架构裁决线（constitution）。所有"这个功能放哪一层"的争议以本文为准。
> 专项路线见 `goal.md`（gsap）。每条判据标注拍板来源；修改本文须用户拍板。

## 1. 两原则

1. **frame = f(t)**：每一帧是时间 t 的纯函数。渲染离线逐帧进行，无墙钟、无 rAF、无事件循环。`ctx.global.time` / `script_time_secs` 隐式传钟即 seek 模型。
2. **animate = g(t)**：动画是时间的函数。tween 在帧末统一 `.time(t)` 求值（hook runner），真 gsap 的 ticker 永不出场（paused 协议）。

## 2. core 纯化判据

core 允许 = 解析 → V8 求值 → 产出 DrawOp IR + 声明式字节 payload。
core 可以**生成**字节但永不**解释**、永不绘制到表面、不解码资源、无墙钟。
既有先例：emoji/canvas 文字烤图的 `ImageRef::Generated` + `FrameMediaPlan::generated_images`。

**排版例外**（2026-10-22 拍板）：文字排版（shaping/断行/定位出 glyph runs）发生在 core（cosmic-text），重放端只光栅化已定位的 glyph runs、不重新排版——这是两端逐位一致的关键。排版=解释文字生成字节，不是绘制。Skia 的 text 能力（SkParagraph/SkFont/SkTypeface/SkFontMgr）在 core 全部数据化，不经 Skia。

## 3. 壳子原则（examples/template）

- examples 只写**标准代码**：标准 React 组件记法 + npm 官方包 import（gsap/three/canvaskit-wasm/lucide-react 全真包）+ 领域 hooks（useGSAP/useFrame/useCanvas）。
- **hooks 是记法不是语义**（2026-10-22 用户拍板）："只表示这个地方应该写 xxx"，不承载 React 状态/副作用语义。标准 React hooks（useState/useEffect 等）语料零命中，不补；遇到时 fail-loud 报错指路。
- 壳子里 opencat 的合法存在面只有两处：`import 'opencat/runtime'`（领域 hooks）+ vite 插件配置。
- **反查判据**：examples 里出现方言 API / `ctx.*` 残留 / workaround 注释 = 承接面泄漏信号。正确动作是把能力沉进 core/engine/web，让例子回归标准——不是改例子迁就引擎。

## 4. 三态接入模板（每库同构）

| 目标 | 职责 |
|------|------|
| examples | 完整官方 API 代码（壳子） |
| core | 解析为协议：gsap=tween 求值；canvaskit=DrawOp 序列；three=Scene3 快照 |
| engine | 自研实现绘制：tiny-skia / cosmic-text / opencat-three（Rust） |
| web | 官方源码重放：真 canvaskit.wasm（draw-ir.ts）/ 真 three（three-facade.ts）——"直接执行源码"=渲染端用官方源码执行，非绕过协议 |

- web 重放端天然是 engine 实现的对拍 oracle（k3diff 口径的第二个参照）。
- 集成面沿用现状链路（G2 surface 管线 / Scene3WebExecutor 已可用），工作是**对齐**，不是新建。
- 新 op 进协议前，engine 与 web 两端重放器都必须有实现，否则 fail-loud。

## 5. 接入形态（供货机制）

1. **esbuild external**：`opencat/runtime`、`gsap`、`three`、`canvaskit-wasm` 在主 bundle 保持 external（`packages/create-opencat/template/opencat-plugin.mjs`）。
2. **模块表按目标供货**：core 模块表把包名解析到承接面；web 目标解析到官方包。
3. **sidecar**：gsap 由项目侧预打包 `dist/gsap.bundle.mjs`（esbuild cjs，入口 `scripts/gsap-entry.mjs`），引擎 BFS 遇 `'gsap'` 读 sidecar 注入 `__ocModuleSources`，缺失报错指路。three 同模板（纯 JS 可 bundle）。**canvaskit-wasm 例外：不打官方 bundle、不加载真 .wasm**——官方 JS 层只是 API 糖 + WASM 装载器，绘制语义在 DrawOp + 两端重放器；模块表把 `'canvaskit-wasm'` 解析到官方签名 facade，`CanvasKitInit({locateFile})` 桩直接 resolve。
4. **惰性装配 + 常驻豁免**：包模块首次 require 时装配（patch/gate/子路径视图），`__ocResetFrame` 豁免常驻，防每帧重执行。异步初始化桩立即 resolve（确定性增强：首帧资源可见）。
5. **出口拦截**：拦截点在出口不在面内。gsap=tween 工厂包装（paused 协议 + target 归一）；three=`WebGLRenderer` 构造/render（场景图/矩阵/ColorManagement 全由官方包跑，updateMatrixWorld 纯 JS 确定）；canvaskit=API 调用即记录。

## 6. 各库拍板（2026-10-22 grilling 十项收口）

### gsap（专项见 goal.md，已完成五批退役）
- 插件收录已收齐（15 项，对照 npm 3.15.0 全部 22 个独立插件文件归类完毕）；attr/snap/数组 tween 随 core 主文件。
- 永久排除：交互类（ScrollTrigger/ScrollTo/Observer/ScrollSmoother/Draggable/Inertia——输入事件与真实时钟不可能写成 t 的纯函数）、外部库桥（Pixi/Easel）、工具（GSDevTools/MotionPathHelper）、CSSRule。
- 剩三缺口：CSS 字符串写入通道（cssText/setProperty）、translate3d 分支、Flip（见下）。

### canvaskit-wasm
- 承诺面：**签名面全量收录，实现语料驱动，未实现的调用 fail-loud**。
- 壳子写法：`CanvasKitInit(...).then(CK => CK.MakeCanvasSurface('id'))`——id 字符串直传（官方签名本就接受 id 或元素）；`useCanvasKit()` / `ctx.CanvasKit` 退役。
- canvas 像素进合成沿用 G2 surface 管线（canvas target → surface → picture）。
- 文字硬约束：见 §2 排版例外。drawParagraph/drawPicture 当前零语料：签名留位、实现挂起、调用 fail-loud。

### three
- engine = 官方包整体执行 + `WebGLRenderer` 构造/render 出口拦截 → Scene3 commit（`record_scene3`）→ `crates/opencat-three` 光栅化。自研矩阵数学（three_api.js）全部退役。
- `GLTFLoader/TextureLoader.loadAsync` = 立即-resolve stub（宿主同步解码，首帧可见）；Clock/AnimationMixer/Raycaster 沿用 fail-loud；未映射材质特性提取时报错，不静默降级。
- 大几何走 `scene3_geometry_register`（快照只带 `Asset{id}`，不逐帧过 JSON 桥）。
- web = 同一 Scene3 协议、官方 three 源码渲染（Scene3WebExecutor）。
- 壳子写法：`import * as THREE from 'three'`；canvas 经 `document.getElementById` 垫片（g2 installDomShims 先例）。

### Flip
- 帧内 mutation-aware **按需重排**：布局查询把当前 mutation 栈喂给布局引擎增量重排（merkle 布局缓存已有增量基础），官方语义直通，不做声明式特化。

### web 端预载
- 渲染启动前 warmup 轮：先跑一轮组件函数并 flush 微任务（loadAsync/init 全部就位），再从帧 0 正式逐帧渲染。与 engine 侧立即-resolve 语义拉齐。

## 7. 完备判据（语料驱动）

- 完备 = 21 个源工程需求全命中为验收线；官方全家桶跑通当压力测试，不当代办清单。
- 宿主面桩（ResizeObserver observe 永不触发、document.fonts 永不发 loadingdone、getScreenCTM 恒 identity、定时器报错指路）= **确定性特性，不是欠账**。
- API 承诺分两层：签名面全量收录（方法存在、签名正确）、实现按语料驱动，未实现调用时报错指路。

## 8. 双句柄架构

- **作者面**：`ctx.getNode(id)` 全捕获 Proxy（声明式风格兼容）。
- **元素面**：`__domTarget(id)` 返回元素面句柄——官方插件按 target 自身类型分支（CSSPlugin.targetTest / MorphSVG 读 nodeName / DrawSVG 读 tagName），所以查询返回元素面而非 numeric proxy；写穿透到同一 mutation store。

## 9. 宿主面交付记录

| 批次 | 内容 | commit |
|------|------|--------|
| A | morph_svg/along_path 方言出清 | 20b99a94 |
| B | TreeOverlay + 脚本侧 DOM 覆盖层（官方插件可建树搬子级） | 79749708 |
| C1/C2 | text_element_boxes 绑定 + SplitText 子级视图渲染桥 | e1d5e734 |
| C3 | 三处示例迁官方 SplitText | b2112800 |
| C4 | ctx.splitText 三绑定退役 | 47e085c5 |
| D | 属性袋 data-original + fill:none 语义 + 查询返回元素面句柄 | 99ddced9 |

宿主面清单详表与桩语义：`jsx-format.md`（待补）。

## 10. 退役判据（常设架构判据）

一个自研面可退役，须同时满足：
1. **零 JS 消费方**（全仓 grep）；
2. **官方/唯一路径**存在且已接通；
3. **渲染逐位回归**通过（k3diff 与基线一致）。

先例：ctx.splitText（C4）、方言插件四件（批次 1 f2d716c1）、Rust tween 链 9 绑定（批次 2 4c935bd7）、parse/color.rs（保留面过堂决议：全仓零消费、与 animate/color.rs 重复）、**liangyi 整体退役**（批次 ⑤ 2026-10-25：效果只留官方 RuntimeEffect(SKSL) 一条路，ly.* 全家 + opencat-liangyi crate 出清，反查转 STRICT）。

## 11. 验收构成

1. `scripts/regression-gate.sh`：cargo test + clippy + web test/tsc/build + **方言反查**（`scripts/check-dialect.sh`，黑名单随退役批次收紧）。
2. k3diff 渲染回归（入口必须用 `dist/index.bundle.jsx`；`opencat` 与 `opencat-frames` 是两个 bin，验收须 `--bins`）。
3. oracle：Chrome 官方 gsap/three/canvaskit 截帧参考 + web 重放端。

## 12. 路线图（按库纵切，2026-10-22 拍板）

① 宪法 + 方言反查（✅） → ② canvaskit 纵切（✅ 2026-10-22：13 示例迁官方 CanvasKitInit import、core 假面合并进 canvas_api.js 官方签名面、g2 改派生面、hooks 移微任务 drain 后、反查转 STRICT；k3/hyperframes/inspector/vfx 自回归 mae=0，k3 对齐 swgl avg_p8 1.29%）→ ③ three 纵切：three-entry 拦截 → 示例迁 `import * as THREE from 'three'` → three_api 退役 → 语料对齐 → ④ gsap 三缺口：CSS 字符串写入 + translate3d → Flip 按需重排 → ⑤ liangyi 退役（✅ 2026-10-25：7 处 ly.pixel 内核迁官方 RuntimeEffect(SKSL)——hyperframes-launch 的 flex-shader 与 vfx-heygen-combined 四内核手写 SKSL 直译原 WebGL 数学、canvas-close 走 ctx.surface + getImageData/putImageData 官方 Canvas2D 链、flex-css 的 conic-gradient 引擎补全原生 CSS conic 链路（lightningcss 解析 → SweepGradient spec → engine skia sweep / web MakeSweepGradient）——ly.* 出 canvas_api.js/bindings/helpers，opencat-liangyi 出工作空间删除，分带光栅（ExecutionPlan）随编译器失去来源一并退役，反查黑名单 ly.* 转 STRICT；gate 10/10，两示例新链路渲染成功，SKSL 与 swgl 参考的混沌 fbm 差异用户拍板可接受）。

每库闭环 = "example 里那行代码变回官方写法，渲染逐位不变"。
