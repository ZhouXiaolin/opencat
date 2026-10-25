# Goal:三态架构收敛——壳子标准化,库纵切闭环

> 2026-10-22 重写。架构裁决线见 [ARCHITECTURE.md](./ARCHITECTURE.md)(两原则/壳子原则/三态接入模板/退役判据),本文只写目标、路线、批次落地件、验收纪律。
> 上一版(全官方 GSAP 路线,2026-10-21 grilling 定案)已收口,完成记录压缩在 §2。

## 1. 目标

examples/template 写**标准代码**:官方 npm 包(gsap / three / canvaskit-wasm / lucide-react)+ React 组件记法 + 领域 hooks(hooks 是记法不是语义)。所有平台特异性——时序装配、op 转换、文字排版、3D、绘制——沉到 core/engine/web。

三库按同一模板纵切闭环:**示例迁官方 import → core 承接面 → engine/web 两端逐位回归**。

| 目标 | 职责 |
|------|------|
| examples | 完整官方 API 代码(壳子) |
| core | 解析为协议:gsap=tween 求值;canvaskit=DrawOp 序列;three=Scene3 快照 |
| engine | 自研实现绘制:tiny-skia / cosmic-text / opencat-three(Rust) |
| web | 官方源码重放:真 canvaskit.wasm(draw-ir.ts)/ 真 three(three-facade.ts) |

验收总口径:**example 里那行代码变回官方写法,渲染逐位不变**。web 重放端是 engine 实现的对拍 oracle。

## 2. 已完成(基线,不再展开)

- **gsap 五批退役收口**(2026-10-22,详见 git log):
  - 方言退役批次 1(f2d716c1)= ctx.* facade + 4 方言插件出清;批次 2(4c935bd7)= Rust tween 链 9 绑定出清
  - SplitText/MorphSVG/DrawSVG 完整化五批:A 20b99a94(morph_svg/along_path 出清)、B 79749708(TreeOverlay+脚本侧 DOM 覆盖层)、C1/C2 e1d5e734(text_element_boxes+SplitText 渲染桥)、C3 b2112800(三示例迁官方 SplitText)、C4 47e085c5(ctx.splitText 退役)、D 99ddced9(属性袋 data-original+fill:none+查询返回元素面句柄,k3 回归 0.135%)
  - 插件收录收齐:15 项(npm 3.15.0 全部 22 个独立插件归类完毕,无第三个桶);交互类/桥/工具永久排除
- **宪法 + 方言反查上线**:ARCHITECTURE.md;`scripts/check-dialect.sh` 接入 regression-gate(STRICT 全绿;PENDING 39 处=§3 工作量清单)
- **保留面过堂**:parse/color.rs 已删(cargo test 497 全绿)
- **批次⑤ liangyi 退役收口**(2026-10-25):7 处内核迁官方 SKSL/CSS 面,crate 出工作空间,反查 STRICT,gate 10/10
- **批次② canvaskit 纵切收口**(2026-10-22):13 个示例文件迁官方 `CanvasKitInit` import(k3×7 / dribbble / nexus7 / inspector / vfx 三处死变量 / hyperframes×4),useCanvas/useCanvasKit/ctx.CanvasKit 全退役;core 假面合并=canvas_api.js 一处官方签名面(Embind 数字枚举+官方形态 RRectXY+Rust 绑定字符串翻译层+MakeCanvasSurface/CanvasKitInit 桩+drawParagraph 签名留位),g2_api.js 改 g2CanvasKit() 派生面(G2 特有 Paragraph/FakeFont/MakeDash 不污染全局);frame hooks 移到微任务 drain 后(then 回调先 resolve,hook 首帧即有 CK);方言反查四处转 STRICT。**验收数据**:k3/hyperframes/inspector/vfx 新旧代码自回归 mae=0 maxd=0 逐位;k3 对齐 F:\hyperframes-launches swgl 参考 avg_p8 1.29%(<2%);hyperframes 对齐 4.35%=存量移植差距(自回归归零,无黑帧);dribbble/nexus7 修复后首渲成功——此前被批次 D 误提交的 stub `examples/dist/gsap.bundle.mjs` 挡死(tl.fromTo 不存在),已用 esbuild 标准管线换真 bundle;gate 10/10 全绿(cargo test+clippy×3+web test/tsc/build+crate tsc/vite+方言反查;本环境补装 clippy、web 依赖,测试 vi.mock 工厂改命名空间 spy 兼容 bun)

## 3. 路线图(按库纵切,串行)

### 批次② canvaskit 纵切(✅ 2026-10-22 收口,记录压进 §2)

**目标态**:examples 写 `import CanvasKitInit from 'canvaskit-wasm'; CanvasKitInit({...}).then(CK => CK.MakeCanvasSurface('id'))` ——逐字官方写法,id 字符串直传(官方签名本就接受 id 或元素)。

落地件:
1. **示例迁移**:ctx.CanvasKit×3 文件(inspector-launch / nexus7-cyberpunk / vfx-heygen-combined)+ useCanvasKit×14 处(k3-promo 8 场景 / dribbble-logo-animated)迁官方 import;useCanvasKit 退役;useCanvas 按语料拆解(元素获取走 id 直传,assets 声明走 JSX 元素属性)。
2. **core 假面合并**:canvas_api.js recorder 面 + g2_api.js 枚举假面(extendCanvasKit)→ 一处官方签名面。CanvasKitInit 桩立即 resolve(不加载真 .wasm——官方 JS 层只是 API 糖+装载器,绘制语义在 DrawOp+两端重放器);Surface/Path/Paint/Shader/ColorFilter/PathEffect/MaskFilter/Image/Canvas 全家签名收录。
3. **DrawOp 对齐语料**:drawRect/clipPath/drawCircle/drawLine/save/restore/drawRRect/drawArc/drawPath/drawImage——每个 op 确认 engine(Rust)与 web(draw-ir.ts)两端重放器都有实现;新 op 进协议前两端必须齐,否则 fail-loud。
4. **文字线(约束非排期)**:drawParagraph 签名留位、实现挂起(零语料);任何文字路径一律数据化转 cosmic-text——排版在 core 出 glyph runs,重放端只光栅化不重排;Skia text(SkParagraph/SkFont/SkTypeface/SkFontMgr)不出场。builder.layout() 测量值由 binding 回 Rust cosmic-text 提供。
5. canvas 像素进合成沿用 G2 surface 管线(canvas target→surface→picture,已验证)。
6. 反查黑名单:ctx.CanvasKit / useCanvasKit 转 STRICT。

**验收**:k3 系列逐位回归不回退 + dribbble/nexus7/inspector/vfx 渲染对齐 + gate 全绿。

### 批次③ three 纵切

**目标态**:examples 写 `import * as THREE from 'three'` + `new THREE.WebGLRenderer({canvas})` ——逐字官方写法。

落地件:
1. **three-entry.mjs**(项目侧 sidecar,gsap-entry 同模板;three 纯 JS 可 bundle):官方包整体执行——场景图/矩阵/四元数/ColorManagement 全由官方跑(updateMatrixWorld 纯 JS 确定);唯一替换点 `WebGLRenderer` 整类(构造解析 canvas → Scene3 提交器,不碰真 getContext);Clock/AnimationMixer/Raycaster fail-loud;GLTFLoader/TextureLoader.loadAsync = 立即-resolve stub(宿主同步解码,首帧可见)。
2. **模块表接线**:'three' 从 ctx.threejs 自研面切到 sidecar;builtin 按目标供货(engine=承接面,web=真包)。
3. **示例迁移**:useThree×2(hyperframes-launch/flex-threejs、liquid-brand-refraction)迁官方 import;GLTFGeometry → GLTFLoader stub 写法;document.getElementById 垫片(g2 installDomShims 先例)。
4. **three_api.js 退役**(判据:零消费方+官方唯一路+逐位回归):自研矩阵数学全部出清;语料 17 类(Scene 22/ShaderMaterial 11/Mesh 7/PerspectiveCamera 6/Plane 5/Line 5/Group 5/Sphere 3/VideoTexture 2/物理材质 4/GLTF 2/灯光 3/CanvasTexture 1)逐类对齐 Scene3 协议;大几何走 scene3_geometry_register(快照只带 Asset{id});未映射材质提取时报错,不静默降级。
5. **web 对齐**:Scene3WebExecutor 按同语料逐项对齐(对拍 oracle);协议扩面判据=Rust 重放器能承接。
6. 反查黑名单:ctx.three / useThree 转 STRICT。

**验收**:flex-threejs / liquid-brand-refraction 两端逐位(engine vs web vs Chrome 参考)+ k3 回归。

### 批次④ gsap 三缺口

1. **CSS 字符串写入通道**(cssText/setProperty)——CSSPlugin 面;落点=DOM 覆盖层写通道泛化(write_style_value trait 面已有)。
2. **translate3d 分支**。
3. **Flip**:帧内 mutation-aware 按需重排——getBoundingClientRect 类查询把当前 mutation 栈喂给布局引擎增量重排(merkle 布局缓存已有增量基础);官方语义直通,不做声明式特化。语料=liquid-brand-refraction + gsap-plugins。

**验收**:上述示例逐位回归。

### 批次⑤ liangyi 退役(✅ 2026-10-25 收口)

1. **前置**:putImageData + 资源像素读回——早已落地(k3 溶解先行)。
2. **示例移植**(7 处 ly.pixel 全清,形态=官方 CanvasKit/Canvas2D 写法):flex-shader 与 vfx 四内核(CHROM/PORTAL/STING/STING_LOGOONLY)→ 手写 SKSL(RuntimeEffect.Make + makeShaderWithChildren + paint.setShader + drawRect;GLSL 直译保表达式序,noise2 抄 codegen 同式);canvas-close → ctx.surface + getImageData + putImageData(k3 同链);flex-css conic → 原始 CSS conic-gradient 写法(引擎补全原生 conic 链:lightningcss→SweepGradient→engine gradient_shader::sweep / web MakeSweepGradient);5 处 ly.surface(1,1) measure → ctx.surface。
3. **出清**:canvas_api.js ly 面全家(Kernel/flatten/ly 命名空间/apply/sweep)、bindings 三绑定(canvas_lambda_effect_draw/surface_apply_effect/surface_scan_pass)、helpers(auto_bake/cpu_child_images/uniform_f32s_to_vals/Surface child)、ablate LiangyiCompile、lambda_integration_tests、engine 分带光标(ExecutionPlan/draw_runtime_effect_banded/PreparedRuntimeEffect.plan)、opencat-liangyi crate 删除出工作空间;反查 ly.* 转 STRICT。
4. 效果此后只有官方 RuntimeEffect(SKSL) 一条路。验收:gate 10/10 全绿;hyperframes/vfx 新链路渲染成功;SKSL 与 swgl 参考的混沌 fbm 图案差用户拍板可接受(fbm 对浮点序敏感,非缺陷)。

### 批次⑥ 文档与桩

- `jsx-format.md` 补宿主面清单 + 桩语义总表(ResizeObserver observe 永不触发、document.fonts 永不发 loadingdone、getScreenCTM 恒 identity、定时器报错指路——确定性特性,不是欠账)。

## 4. 验收纪律

- **gate**:`scripts/regression-gate.sh` = cargo test + clippy + web test/tsc/build + 方言反查。
- **渲染回归**:k3diff;入口必须 `dist/index.bundle.jsx`(直渲 index.jsx 缺内嵌 CSS 塌成黑帧不报错);`opencat` 与 `opencat-frames` 是两个 bin,验收须 `--bins`;vfx 基线目录首帧差异大,不能跨目录比。
- **oracle 双参照**:Chrome 官方库截帧 + web 重放端。
- **无损比对**:PNG 序列或 `-qp 0` + yuv444p——同一张 PNG 编解码一次的偏差就有 ma_e≈3。当前基线:全片 ma_e 0.327 / avg_p8 0.764%(2026-10-21)。

## 5. 红线与已知坑(持续有效)

- **core 纯度判据不变**:core 生成 payload 字节,永不解释/绘制;排版例外见宪法 §2。
- **子 tween 注入 paused 会被父 timeline 跳过、duration 塌 0**——只有 timeline 工厂注入 paused。
- **两侧必须同字体同行高**:Chrome `sans-serif` 落系统 UI 字体、`line-height: normal` 取字体度量(Inter≈1.21);opencat 是内置 Noto Sans SC + 行高 1.5。不钉住就是在比字体替换与行盒算法。
- **随机必须确定性写法**:ScrambleText `chars:'X'` / `randomize:false`;RoughEase 缓动表由 Math.random 生成——两侧序列不同必然对不齐。
- **禁读回引擎已合成的表面**(资源像素读回只读 host 解码的字节,见批次⑤)。
- release 二进制改代码后必须重建;golden_c 基线已过期,对比对当前产物。
- **不写单测,验收走渲染 k3diff**(用户指令)。
- **工作流串行**(Solaren 要求)。
- realm console.log 不出 stdout,调试用 `throw new Error('DBG...')`。
- Windows:shell 不读 profile 需显式 source;FFMPEG_DIR 已过期(实际 `C:\ffmpeg-9.0.2\ffmpeg-9.0.2-full_build-shared\bin`,用前验证 swscale-10.dll);渲染用 `--release`;harness spawn 无 ffmpeg PATH 要注入。
- **k3-promo 必须 `--base-dir`**:bundle 在 `dist/`,资产/字体相对路径按入口目录解析;字体相对路径按入口文件目录算。
- **本机没有 node,只有 bun 1.4.2**;esbuild 从 `examples/k3-promo/node_modules` 取依赖。
- **JSX 元素白名单**:div/text/canvas/image/audio/video/transition/tl/path/lottie/svg——没有 `<group>`,容器用 `div`。
- **裸写 JSX 文本带缩进**:`<text>\n  HELLO\n</text>` 会给子节点带换行空格,用 `{'HELLO'}` 包住。
- **`gsap.fromTo(t, from, to)` 没有 position 参数**:要定位放进 timeline。
- **ffmpeg 新版没有 `-vsync`**:用 `-fps_mode passthrough`。
- **Python 读不到 `/tmp`**:Windows 临时目录是 `C:/Users/zhoux/AppData/Local/Temp/`。

## 6. 关键落点速查

| 事项 | 位置 |
|---|---|
| vite 插件 / external 面 | `packages/create-opencat/template/opencat-plugin.mjs`(external: opencat/runtime、gsap、three、canvaskit-wasm) |
| gsap sidecar 入口 / 供货 | `packages/create-opencat/template/gsap-entry.mjs`;插件 esbuild bundleGsap → `dist/gsap.bundle.mjs` |
| gsap 装配层 | `crates/opencat-core/src/script/runtime/jsx_runtime.js:556`(installGsapBuiltins:paused 协议/target 归一/hookGated/子路径视图) |
| 宿主 DOM 面 | `crates/opencat-core/src/script/runtime/dom_api.js`(getComputedStyle);`script/tree.rs`(TreeOverlay) |
| mutation 记录面 | `crates/opencat-core/src/script/recorder/mod.rs`(MutationRecorder trait;record_text_content/svg_path/scene3) |
| canvaskit 官方签名面 | `crates/opencat-core/src/script/runtime/canvas_api.js`(CanvasKitInit 桩+MakeCanvasSurface;批次②已合并枚举面);G2 派生面 `g2/g2_api.js` g2CanvasKit() |
| Scene3 协议 | `crates/opencat-core/src/ir/scene3.rs`;提交 `script/recorder/store.rs`(record_scene3/geometry registry) |
| three 自研面(待退役) | `crates/opencat-core/src/script/runtime/three_api.js`——批次③ |
| web 重放端 | `crates/opencat-web/web/src/draw-ir.ts`(真 canvaskit.wasm)、`three-facade.ts`(真 three) |
| 方言反查 | `scripts/check-dialect.sh`(STRICT/PENDING 两层,接 regression-gate.sh) |
