# handoff — 三态架构收敛,批次② canvaskit 纵切收工

> 2026-10-22 收工。目标见 [goal.md](./goal.md);架构裁决线见 [ARCHITECTURE.md](./ARCHITECTURE.md)。
> 批次①(宪法+方言反查+color.rs 过堂)记录已归档,git 历史可查;高频坑清单在 goal.md §5。

## 本轮完成:批次② canvaskit 纵切

### 核心改动

| 落点 | 改动 |
|---|---|
| `canvas_api.js` | 一处官方签名面:Embind 数字枚举(PaintStyle/StrokeCap/StrokeJoin/ClipOp/TileMode/BlendMode 29 项/PointMode/FontEdging/FontHinting,值=Skia C++ 常量)、官方形态 RRectXY(12 元素数组)、官方数字→Rust 绑定字符串翻译层(strokeCapName/strokeJoinName/fontEdgingName/drawPoints mode)、`MakeCanvasSurface/MakeWebGLCanvasSurface`(id 字符串直传→makeRecordingSurface: getCanvas/flush/dispose/width/height)、`CanvasKitInit()` 桩立即 resolve、drawParagraph 签名留位 fail-loud;`ctx.CanvasKit`/`ctx.canvas`/`ctx.getCanvas` 出清 |
| `g2_api.js` | extendCanvasKit 枚举假面 → `g2CanvasKit()` 派生面(Object.create 本体;G2 特有 Paragraph/FakeFont/FontMgr/MakeDash/makePathFromSVGString/makeG2Surface 只在派生面);`__ocCanvasKitInit` resolve 派生面;顶层 FontStyle 污染删除 |
| `jsx_runtime.js` | builtins 接桩:`canvaskit-wasm` → `{default: CanvasKitInit}`;useCanvas/useCanvasKit/augmentCanvas 三函数出清 |
| `realm.rs`/`jsx.rs` | frame hooks 从 build_driver 移到 run_frame 首个微任务 drain 之后——`CanvasKitInit().then(ck=>{CK=ck})` 在 drain 里 resolve,随后的 hooks 首帧即有 CK |
| 反查 | ctx.CanvasKit / ctx.getCanvasById / useCanvasKit / useCanvas 转 STRICT;PENDING 剩 ly.* / ctx.three / useThree |
| 示例×13 | k3-promo 7 文件(手工模板 ghost-cols + 批量 6)、dribbble、nexus7、inspector、vfx(三处死变量删除+三组件 CK 化)、hyperframes-launch 4 场景——迁移形态:`let CK=null; CanvasKitInit({}).then(ck=>{CK=ck;}); useFrame((ctx,t)=>{ if(!CK) return; var cv=CK.MakeCanvasSurface('id').getCanvas(); ... })` |

### DrawOp 两端对齐(确认,无新协议)

10 语料 op(drawRect/clipPath/drawCircle/drawLine/save/restore/drawRRect/drawArc/drawPath/drawImage)engine helpers.rs(46 分支)与 web draw-ir.ts(49 op)全齐。

### 验收(全过)

| 项 | 结果 |
|---|---|
| k3-promo 自回归 | mae=0 maxd=0 **逐位**(494 帧;基线=stash 后同管线渲染) |
| k3 vs swgl 参考 | avg_p8 **1.29%**(<2%;mae 1.13 为 crf18 编码噪声,无损口径需 PNG/qp0 管线) |
| hyperframes-launch 自回归 | mae=0 maxd=0 逐位(1494 帧) |
| hyperframes vs swgl | 4.35%=存量移植差距(main 拆分悬案,非本批次;无黑帧,仅 1 帧 mae>15) |
| inspector / vfx 自回归 | mae=0 maxd=0 逐位(1350/789 帧) |
| dribbble / nexus7 | 修复后首渲成功,抽帧目检画面完整 |
| gate | **10/10 全绿**(cargo test + clippy×3 + web test/tsc/build + crate tsc/vite + 方言反查) |

### 过程中挖出并修掉的存量问题

1. **`examples/dist/gsap.bundle.mjs` 是批次 D 误提交的 572 字节 stub**(timeline 只有 to,`tl.fromTo` 直接断)——裸根 jsx 示例(dribbble/nexus7)因此从未渲染成功过。已用 esbuild 标准管线(scripts/gsap-entry.mjs + k3-promo node_modules)换真 bundle。
2. **web 全套从未在本环境跑过**(无 clippy、web node_modules 缺失):补装后暴露——draw-ir.ts 三处 ImageRef/ImageFilterSpec 联合类型漂移(generated/encoded 取 .id、video 取 assetId、chain 形状与线格式不符)、draw-ir.test IR v6→v8 断言过期、两个测试文件用 vi.mock/vi.doMock 工厂在 bun 下被忽略——改命名空间 `vi.spyOn`(两端 vitest 行为一致,测试体逐字未动)。opencat.js 类型链接:`file:` 依赖建链时 dist 尚不存在,补 junction。
3. `opencat-gpu-native/win32.rs` clippy `not_unsafe_ptr_arg_deref`:from_window/Presenter::new/for_window 链标 unsafe(调用方 opencat-see 同步)。

### 环境事实(新)

- vite 工程示例的 `bun run build` 在 closeBundle 会做渲染校验,要 `OPENCAT_BIN=F:/OpenCat/target/release/opencat.exe`。
- Windows junction 补链:`cmd //c "mklink /j <link> <target>"`。
- bun 的 vitest 运行时忽略 vi.mock/vi.doMock 模块工厂——测试写命名空间 spyOn。

## 下一步

**批次③ three 纵切**:three-entry.mjs sidecar(官方包整体执行,WebGLRenderer 整类替换为 Scene3 提交器)→ 模块表接线 → useThree×2 示例迁官方 import → three_api.js 退役 → 反查 ctx.three/useThree 转 STRICT。验收=flex-threejs / liquid-brand-refraction 两端逐位 + k3 回归。
