# Goal：全官方 GSAP 路线——core 宿主化、插件原样跑、ctx.* 退役

> 2026-10-21 三轮 grilling 定案，全部为用户拍板决策。
> 一句话：官方 gsap 与它的全部非交互插件，不改一行源码，原样跑在 core 上；自研 gsap 面（ctx.*）整个退役。

## 1. 目标

1. 补齐 core 宿主面，让官方 gsap 插件源码**原样运行**：`createElement`、`appendChild`、`getComputedStyle`、`getAttribute/setAttribute`、`getTotalLength`、`getBoundingClientRect`。
2. 插件收录范围 = **全部非交互插件**；交互类明确出清。
3. **ctx.* gsap 面一次性退役**，作者面只剩真 gsap 写法（`import { gsap } from 'gsap'`）。
4. 文字无论 text 还是 span，**一律 cosmic-text**；移除原生 SplitText/MorphSVG 设计。

## 2. 事实基线（侦察已核实）

- 嵌入 bundle = gsap **3.15.0 core+CSSPlugin only**（171KB，`crates/opencat-core/src/script/runtime/gsap/gsap.bundle.js`，include_str! 于 `mod.rs:46`，eval 于 `realm.rs:263`）。
- 完整含全部插件的 npm 3.15.0 在 `examples/k3-promo/node_modules/gsap`，物理可取；3.13 起全插件免费，无授权障碍。
- **语料事实**：21 个源工程插件需求仅 CustomEase（6 个工程且全带降级写法）；SplitText/MorphSVG/DrawSVG 等 15 个插件零代码命中。分裂感大于实际需求，趁需求小时定干净架构。
- realm 现状缺口：`createElement` 直接报错（`dom_api.js:71-73`）、无 `getComputedStyle`、无 `getAttribute/setAttribute`、无 `getTotalLength`、`appendChild` 等抛错（`node_style.js:94-100`）、`getBoundingClientRect` 仅显式盒。
- 真包通道已跑通：包装层 target 归一（`jsx_runtime.js:535-549`）、`__ocGsapPending` 注册、hook runner 帧末统一 `.time(ctx.current.time)`（`jsx_runtime.js:312-319`）、hook-gated。
- 双轨现状：真包只顶替补间引擎半边；五个原生插件（split_text/morph_svg/motion_path/scramble_text/text）仍在 ctx.* facade 服役。

## 3. 已定决策

| # | 决策 | 内容 |
|---|------|------|
| D1 | 根路线 | 全官方源码。不做 API 兼容的原生插件，不补浏览器全家桶之外的东西。 |
| D2 | 插件范围 | 全部非交互（见 §4 枚举）。 |
| D3 | 交互出清 | ScrollTrigger/Draggable/Observer/InertiaPlugin/ScrollSmoother 声明在 frame=f(t) 离线渲染中不支持。 |
| D4 | ctx.* 退役 | 一次性删除 + 11 个 ctx.* 风格示例同批迁真包写法，逐个过 k3diff 回归。 |
| D5 | 文字路线 | text/span 一律 cosmic-text；SplitText lines 挂 cosmic-text IFC 行盒，不在 JS 里补字体度量。 |
| D6 | 交付方式 | 一个统一 example（timeline 语义、多场景）把插件一起做完。 |
| D7 | 验收 oracle | 同一份场景代码在真实 Chrome 用官方 gsap 截帧作参考图 + k3diff 量化对齐。 |
| D8 | HTML 依赖 | 最小文本子集先行，全量 HTML（UA 表/组合器/级联）并行推进；子集是全量的组成部分，无弃工。 |

## 4. 插件清单

**收录（全部非交互，eases 全家 + 效果插件 + 布局/物理）：**
- eases：CustomEase、CustomWiggle、CustomBounce、RoughEase、SlowMo、ExpoScaleEase
- 效果：SplitText、MorphSVG、DrawSVG、MotionPath、ScrambleText、Text
- 布局/物理：Flip、Physics2D、PhysicsProps

**排除（硬理由，非交互性无关）：**
- 外部库桥：EaselPlugin、PixiPlugin（宿主库在 realm 不存在）
- 开发工具：GSDevTools、MotionPathHelper
- CSSRulePlugin（realm 无样式表对象）
- 交互类（D3 出清）

## 5. 退役清单

- `ctx.timeline/to/from/fromTo/set`（`animation/facade.js:88-399`）
- 五个原生插件：`animation/plugins/{split_text,morph_svg,motion_path,scramble_text,text}.js`
- `bezier:`/`points:` ease 标签 + Rust 侧 `Easing::Samples`（`parse/easing.rs`）
- 自研补间引擎余部（`animation/core.js`、`bootstrap.js` 相应注册面）
- 附带红利：inspector-launch 弹簧采样表回归官方 `CustomEase.create()`（源工程原样写法）

**不退役**：useGSAP hook 本体、`ctx.current.time`、`ctx.getNode`、ocTarget 桥、StyleMutations 写入面——这些是生命周期与桥接服务，不是 gsap 方言。

## 6. 实施阶段（串行）

1. **Bundle 重建**：esbuild 从 `examples/k3-promo/node_modules/gsap` 重打 bundle 收录 §4 全部插件；realm eval 序、builtins 导出面、`gsap.registerPlugin` 桥接通。验收：k3-promo 回归（PSNR 49.65 不回退）。
2. **统一 example 脚手架 + 截帧管线**：多场景 example（timeline 语义，每插件一场景 + 组合场景）；Chrome（真实浏览器 + 官方 gsap）逐帧截图 → k3diff 参考图。
3. **宿主最小子集**：`createElement/appendChild` 通道（recorder store 插节点 + 每帧 walk 拾取）、样式继承链 `getComputedStyle`、cosmic-text IFC 行盒、`setAttribute('d')` 写穿 svg_path、`getTotalLength` 接 `path_measure.rs`、显式盒 + 行盒 `getBoundingClientRect`。
4. **逐插件打通**：纯数学先（eases 全家、Physics2D/PhysicsProps），DOM 依赖后（ScrambleText/Text/MotionPath/DrawSVG/MorphSVG/SplitText/Flip）；语义以官方行为为 oracle。
5. **退役 + 迁移**：删 §5 全部；11 个示例迁真包写法；全量 k3diff 回归；文档更新支持面清单（含 D3 出清声明）。

## 7. 全局验收

- 统一 example：Chrome 参考帧 vs opencat 渲染帧，k3diff avg_p8 达标（对齐既有纪律）。
- k3-promo 真包通道回归不回退。
- 11 个迁移示例逐个 k3diff 回归通过。
- 仓库内 grep 无 `ctx.timeline/ctx.fromTo/split_text.js` 等退役面残留引用。

## 8. 红线与已知坑

- **纯度判据不变**：core 可生成 payload 字节但永不解释/绘制。宿主 API 是 realm 服务，不违反判据；`createElement` 通道只动 recorder store，不动绘制。
- **大坑**：子 tween 注入 `paused` 会被父 timeline 跳过、duration 塌 0——只有 timeline 工厂注入 paused。
- 调试：hook 里 `throw new Error('DBG...')` 冒错误信息（realm console.log 不出 stdout）。
- release 二进制改代码后必须重建；golden_c 基线已过期，对比应对当前产物。
- 不写单测，验收走渲染 k3diff（用户指令）。
- 工作流串行（Solaren 要求）。

## 9. 关键落点速查

| 事项 | 位置 |
|---|---|
| bundle 嵌入 | `crates/opencat-core/src/script/runtime/gsap/gsap.bundle.js`；`mod.rs:46`；`realm.rs:263` |
| 包装层 | `jsx_runtime.js:519-669`（检测/target 归一/注册/帧末 seek/ocTarget） |
| 宿主 DOM | `dom_api.js:61-76`、`node_style.js:59-121` |
| svg_path 写穿 | `script/recorder/store.rs:936` |
| path 度量绑定 | `script/bindings.rs:719-768`（`path_measure.rs`） |
| 完整 npm 包 | `examples/k3-promo/node_modules/gsap`（3.15.0） |
| 既有插件文档（待退役） | `skills/opencat-creator/references/animations.md:109-171` |
