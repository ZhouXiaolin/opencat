# handoff — 全官方 GSAP 路线

> 2026-10-21 第 1 轮收工。目标见 [goal.md](./goal.md)：官方 gsap 与全部非交互插件原样跑在
> core 上，自研 `ctx.*` gsap 面整个退役。

## 本轮完成

### 阶段 1（bundle 重建）已交付

`crates/opencat-core/src/script/runtime/gsap/entry.js` 是新入口，收录 core + CSSPlugin +
goal.md §4 的全部 15 个非交互插件（eases 全家、SplitText/MorphSVG/DrawSVG/MotionPath/
ScrambleText/Text、Flip/Physics2D/PhysicsProps）。构建脚本
`scripts/build-gsap-bundle.mjs`（`bun scripts/build-gsap-bundle.mjs`）产出
`gsap.bundle.js`（371KB，原 171KB）。**官方源码一行没改。**

入口额外导出 `TimelineLite/TimelineMax/TweenLite` 与全部 eases：包装层
（`jsx_runtime.js:590`）靠 `gsapPackage.TimelineLite` 取 timeline 构造器来拦截
`to/from/fromTo/set` 做 target 归一。少了这个导出，数组 target 的字符串不会被归一，
会把裸字符串当 CSS 选择器送进 `querySelectorAll` —— 本轮踩过，报错是
`(scope || _doc).querySelectorAll is not a function`。

### 宿主面补齐（阶段 3 的前置）

1. **`gsap/<Plugin>` 子路径 builtins**（`jsx_runtime.js:614-651`）：
   `import { CustomEase } from 'gsap/CustomEase'` 这类官方写法现在能原样解析。
   EasePack 特例：三个 ease 是一个模块，不是单个插件。
2. **extract realm 加载 gsap**（`crates/opencat-engine/src/jsx.rs`）：JSX 提取期用的是
   一次性 realm，之前没 eval GSAP_BUNDLE，模块顶层 `import ... from 'gsap/CustomEase'`
   直接报 unknown module。
3. **`JS_PLATFORM_SHIM` 提为 pub**（`realm.rs:32`）并接进 extract realm：gsap 包在 eval
   时会 `_wake()` 排一个 tick，没有 rAF 队列就抛
   `requestAnimationFrame is not available in a deterministic renderer`。

### 统一 example + Chrome oracle（阶段 2）已跑通

- `examples/gsap-plugins/index.jsx`：多场景 timeline 示例（D6），当前接了 eases + physics
  两组；DOM 依赖插件等宿主面补齐后加场景。
- `tools/gsap-oracle.mjs`：Chrome CDP 截帧管线（Bun 自带 WebSocket，零新依赖）。
  参考页用官方 gsap UMD，时间轴 `paused`，只由页面里的 `__seek(t)` 驱动 —— 同一帧号
  永远给出同一张图，不受 rAF / 机器速度影响。
  `bun tools/gsap-oracle.mjs --all --fps 30` → `out/gsap-oracle/f0000.png…`
- `tools/gsap-oracle-measure.py`：同帧色块中心位置对比。把「同一时刻每个 ease 把方块
  推到哪」变成可读的数，比像素 diff 更能定位动画数值问题。

## 验收结果

| 项 | 结果 |
|---|---|
| k3-promo 回归 | **像素零变化**，`out/index.bundle.mp4` 与改前 `out/fixed-gsap.mp4` MD5 相同；对真 swgl 参考 avg_p8 = **1.7394%**（与 goal.md 记录一致） |
| 插件加载 | 15/15 全部注册；`gsap.plugins` 含 css/morphSVG/drawSVG/motionPath/scrambleText/text/physics2D/physicsProps |
| 全部场景数值 | 与 Chrome 逐帧色块中心位置差 **≤0.36px**（f0/15/30/45/60/90/150/165 采样） |
| eases 7/7 | linear、CustomEase、CustomWiggle、CustomBounce、SlowMo、ExpoScaleEase、RoughEase 全部对齐 |
| physics 2/2 | Physics2D、PhysicsProps 全部对齐 |
| 整片 k3diff | avg_p8 = **1.08%**（剩余差异是标题文字抗锯齿） |

## 已知缺口（下一轮）

### 1. 物理插件的写法约束（已澄清，需写进文档）

一度以为 ocTarget 缺 CSSPlugin harness 是缺口，实测证伪：Chrome 里用**普通对象**作
target，`pt=[physics2D>x]` 得 `x=50.00`，与 opencat 逐值相同——引擎行为是对的。

真正的原因是**示例写法违反官方语义**：物理插件自己驱动属性，`vars` 里再给同名终点
（`{x: 1400, physics2D: {...}}`）会生成第二条 PropTween 把它覆盖成线性插值。官方行为
就是如此（Chrome 普通对象同样得 50，只有真实 DOM 走 CSSPlugin harness 才不同）。
示例与参考页都已改成只给物理参数、不给同名终点。

已写进 `skills/opencat-creator/references/animations.md` 新增的「官方插件」一节。

### 2. RoughEase 字符串形式的坑（已绕开，需写进文档）

`rough(2,3,0.4,randomize:false)` 这种**字符串形式**会把参数按位置传给
`config()`，只有第一个（`points`）生效，`randomize` 仍是默认 `true`（缓动表由
`Math.random` 生成 → 两侧序列不同 → 永远对不齐）。确定性写法是对象形式：
`rough({strength:3, points:2, taper:"none", randomize:false})`。示例与参考页都已改用
对象形式。

同样已写进 `animations.md` 的「官方插件」一节。

### 3. 文字抗锯齿（纪律内）

标题/副标题的文字边缘有 AA 差异，属既有纪律内的正常差异（k3-promo 同样存在）。

## 下一轮建议顺序

1. **阶段 3 宿主最小子集**：`createElement/appendChild` 通道、样式继承链
   `getComputedStyle`、cosmic-text IFC 行盒、`setAttribute('d')` 写穿 svg_path、
   `getTotalLength` 接 `path_measure.rs`、`getBoundingClientRect`。这是
   ScrambleText/Text/MotionPath/DrawSVG/MorphSVG/SplitText/Flip 七個插件的前置。
2. **阶段 5 退役**：删 `ctx.timeline/to/from/fromTo/set`、五个原生插件、
   `bezier:`/`points:` 标签 + `Easing::Samples`；11 个 ctx.* 示例迁真包写法。

## 命令速查

```bash
# 重建 bundle（改 entry.js 后必跑）
bun scripts/build-gsap-bundle.mjs

# 渲染示例
cargo build --bin opencat --release --features profile
./target/release/opencat.exe examples/gsap-plugins/index.jsx out/gsap-plugins.mp4

# Chrome 参考帧（全帧 or 指定帧）
bun tools/gsap-oracle.mjs --all --fps 30
bun tools/gsap-oracle.mjs --frames 0,30,60

# 合成参考视频 + 对比
/c/ffmpeg-9.0.2/ffmpeg-9.0.2-full_build-shared/bin/ffmpeg.exe -y -framerate 30 \
  -i out/gsap-oracle/f%04d.png -c:v libx264 -pix_fmt yuv420p -crf 18 out/gsap-oracle.mp4
python3 tools/k3diff.py out/gsap-oracle.mp4 out/gsap-plugins.mp4 --out out/gsap-oracle-diff

# 同一帧色块中心位置对比
./target/release/opencat.exe render examples/gsap-plugins/index.jsx /tmp/f30.png 30
python3 tools/gsap-oracle-measure.py /tmp/f30.png out/gsap-oracle/f0030.png
```

## 坑记录（本轮踩到的）

- **k3-promo 必须用 `--base-dir`**：bundle 在 `dist/`，资产/字体相对路径按入口目录解析。
  `vite build` 的自动渲染步骤需要 `OPENCAT_BIN` 指向 target/release/opencat.exe，本机
  PATH 里没有，所以手动跑 `opencat render dist/index.bundle.jsx --base-dir examples/k3-promo`。
- **本机没有 node**，只有 bun 1.4.2。仓库根没有 package.json，esbuild 从
  `examples/k3-promo/node_modules` 取（构建脚本里用 `pathToFileURL` 直取）。
- **JSX 元素白名单**只有 div/text/canvas/image/audio/video/transition/tl/path/lottie/svg。
  没有 `<group>`，容器用 `div`（`SUPPORTED_TAGS`，`jsx_runtime.js:950`）。
- **`gsap.fromTo(t, from, to)` 没有 position 参数**：独立的 fromTo 不是 timeline 子项，
  第 4 个参数会被忽略。要定位就放进 timeline。
- **realm 里 `console.log` 不出 stdout**：调试用 `throw new Error('PROBE ...')` 冒出来。
