# handoff — hyperframes-launch

> 2026-10-13 对齐更新。状态：**未达标，但首次有了有效 diff**。
> 全片 `avg_p8 = 0.0733`（目标 <0.02）。参考自噪声地板见下。

## 2026-10-16 嵌套 `<tl>`：glass-intro 拆成 4 个场景

**结论：`avg_p8 0.0591 → 0.0569`，且与拆分前逐像素等价**（全片 1494 帧，
两版最大 p8 偏差 0.00056，无任何帧差 > 0.01）。`vis()` 门从 7 处降到 1 处。

### 为什么要拆

参考的 `compositions/glass-intro.html` 本身不是子 composition 列表，而是**一层
平铺的 stage**：4 个 beat 首尾相接，外加 4 个 `.clip` 覆盖层。

| beat | master 窗口 | 长度 | 参考层 |
|---|---|---|---|
| text-intro | 0.00 – 2.10 | 2.10 | `.clip` `data-start=0` `duration=2.1` |
| figma 卡 | 2.10 – 3.85 | 1.75 | `#frame` 的 GSAP opacity |
| spotify 卡 | 3.85 – 5.60 | 1.75 | `#frame-2` 的 GSAP opacity |
| beat4 | 5.60 – 18.24 | 12.64 | `#frame-3` 的 GSAP opacity |

cursor(6.4)、ripple(6.8)、drag-overlay(16.14)、text-ask(16.74) 四个覆盖层窗口
**全都 ≥ 5.60**，因此全部落在 beat4 内。整个 glass-intro 恰好是 4 个场景、
0 个 transition——参考在 2.10 / 3.85 / 5.60 硬切，而相邻场景不声明
`<transition>` 就是硬切。

拆分前这 4 个 beat 每帧都被解析、布局，再用 `vis()` 设成 opacity 0；现在只有
窗口命中 playhead 的那个 beat 会被解析（场景裁剪）。**保留的唯一 `vis()` 是
beat-5 打字机的逐字揭示**——那是场景内的揭示，不是场景窗口。

### 两处引擎缺口（都已补，在 `crates/`）

1. **`JsonLine::Tl` 没有 `duration` 字段**（`parse/elements.rs`）。JS 层早就
   发出了这个字段（`jsx_runtime.js` 的 `emitTimeline`），但 serde 静默丢弃
   （`JsonLine` 不是 `deny_unknown_fields`），构造处又写死 `duration: None`。
   于是 `<tl>` 直接嵌 `<tl>` 报
   `timeline sequence 'inner-tl' is missing a duration`。
   修法：给 `Tl` 变体加 `duration`，透传而非写死。

2. **嵌套 `<tl>` 读全局合成时钟**（`resolve.rs` + `jsx_runtime.js`）。
   `resolve_timeline` 传的是 `cx.frame_ctx`（全局），所以内层场景的时钟是
   合成相对而非外层场景相对。修法：给时间轴一个**原点**——顶层为 0，嵌套
   在场景里的就取外层场景的起点。Rust 侧新增 `TimelineOrigin` /
   `frame_state_for_root_at`，`ScriptFrameCtx` 记录 `scene_start_frame` /
   `scene_start_secs`（不能用 `frame - current_frame` 反推：`current_frame`
   在窗口末端被 clamp）。JS 侧 `emitTimeline` 从 `sceneStack` 栈顶取原点。

   第三个坑在 `emitTimeline` pass 3：原先 `if (lines) emitElement(...)`，
   而逐帧走查的 `lines` 是 `null`——嵌套 `<tl>` 的子树**从未被走查**，
   内层组件的钩子根本没注册。改成 emit 无条件执行（`lines` 为 null 时
   `emitElement` 不产出行，只有注册副作用），且场景时钟压栈范围覆盖到
   emit 结束。

### 复现

```sh
cargo build --release --bin opencat --bin opencat-frames
target/release/opencat-frames examples/hyperframes-launch/hyperframes-launch.jsx out/hf-nested
ffmpeg -v error -y -framerate 30 -i out/hf-nested/frame_%04d.png -crf 0 -pix_fmt yuv420p out/hf-nested.mp4
python3 -I tools/k3diff.py <ref>.mp4 out/hf-nested.mp4 --out out/k3-nested
```

回归对照（旧扁平 jsx + 新引擎 = 0.0569，与嵌套版一致）；已达标示例不退化：
k3-promo 0.0121、liquid-brand-refraction 0.0236、website-to-hyperframes 0.0102。

## 2026-10-15 脚本后端 rquickjs → V8（分支 `feat/v8-script-backend`）

**结论：脚本时间降 2.8×，输出逐像素不变。** 全片 1494 帧
`avg_p8 = 0.0591`、`mae_mean = 2.766`，与 rquickjs 版**逐像素完全相同**
（k3diff `mae 0.000 / maxd 0`）——这是一次纯性能替换，不移动任何一帧。

| 指标 | rquickjs | V8 | 变化 |
|---|---|---|---|
| script ms/帧 | 26.44 | **8.86** | 2.98× |
| script p95 ms | 211.25 | **66.53** | 3.18× |
| 全片渲染耗时 | 150 s | **118 s** | 1.27× |
| 峰值 RSS | 3992 MiB | 4310 MiB | +8% |

### 为什么只快 1.27×，而不是 3×

`profile` 的 `script` 只占单帧约 26%，采样确认同一分布：主线程样本里
`JS_CallInternal` ≈ 27%，其余主要是 **Skia 光栅化**（bicubic 缩放、
blur、gradient）。脚本已不再是瓶颈——继续提速要看渲染后端，不是 JS 引擎。
单帧 profile：script 8.86 / layout 0.08 / display 0.27 / backend 0.26 ms。

### 实现要点（`crates/opencat-engine/src/js_context.rs`）

- **隔离单元不变**：仍是一个 pipeline 一个 isolate。但 isolate 持有的是
  `SharedIsolate` 而非 `OwnedIsolate`——后者 drop 时断言自己是
  `Isolate::GetCurrent()`，即要求**严格按创建逆序销毁**；多 pipeline 共存时
  这个顺序不受控，实测单帧冒烟即 `Fatal error in v8::Isolate::Dispose()`。
  `SharedIsolate` 经 `v8::Locker` 串行化访问、最后一个守卫消失时自行 dispose，
  生命周期与创建顺序无关。测试
  `realms_tear_down_out_of_creation_order` 锁住这个语义。
- **JSON 编组走 V8 原生 `JSON.stringify` / `JSON.parse`**，不再手写 Value
  遍历（原 rquickjs 后端的 `rq_to_json` / `json_to_rq` 已删）。
- **原生函数是零捕获的 `fn` 项**（V8 回调要求 `Copy + Sized`），per-realm
  状态经 isolate data slot 取回，因此 `install_dispatcher` 把 dispatcher 存进
  `NativeState` 而不是闭包捕获。
- **binary fast path 保持**：`__opencatPutImageData` 用
  `ArrayBufferView::copy_contents` 拷 `Uint8Array`；
  `__opencatSurfaceReadInto` 仍「填充而非返回」，直写调用方缓冲。
- 依赖：`v8 = "152.2.0"`（features `use_custom_libcxx`），走预编译库，
  本机 `cargo build` 约 2.5 分钟，**不需要** V8 源码构建。
  `rquickjs` 已从 workspace 完全移除。

### 复现

```sh
cargo build --release --features profile -p opencat
OC_RENDER_GUARD_FOREGROUND=1 tools/render-guard.sh \
  examples/hyperframes-launch/hyperframes-launch.jsx out/bench/v8.mp4
python3 -I tools/k3diff.py <ref.mp4> out/bench/v8.mp4 --out out/bench/k3-v8
```

### 测试状态

`opencat-core` 613 passed；`opencat-engine` 101 passed / 3 failed——
3 个失败在 rquickjs 基线上**同样失败**，与本改动无关：
两个 chromedriver 测试（本机 ChromeDriver 150 vs Chrome 155），
一个缺 `assets/lbr/heygen-prism.glb`。clippy 警告数 150 → 150（无新增）。

## 2026-10-15 对齐点：font-style 通道缺失 + thesis 词距

全片 `mae_mean 3.096 → 2.766`，`avg_p8 0.0606 → 0.0591`。
本轮修两处，都是**引擎缺陷**（不在 jsx 里绕），jsx 侧只改了一个 gap 值。

### 1. 引擎不支持 `font-style`（italic 静默失效）

**症状**：`thesis` 第二行参考是**斜体**衬线，我们渲染成正体。
`flex-threejs` 的 "Three.js"、`flex-footage` 的 "footage"、
`engine` 的 "Identical output, every time." 同病。

**根因**：`italic` 在 `build.rs` 里注册成 `ExactClassAction::Noop`，
注释写着「渲染器目前不区分这些表现」。而 `text_attrs`（`text/mod.rs:687`）
只向 shaper 传 family/weight/letter_spacing，**没有 style 通道**：

```rust
Attrs::new().family(family).weight(...).letter_spacing(...)   // 旧
```

manifest 里 `hf-serif` 和 `hf-serif-it` 的 `family` 都是 `"Instrument Serif"`，
两个 face 落进同一 family。cosmic-text 按默认 style 匹配，**永远选中 Regular**——
Italic 字体文件加载了但取不到。`hf-eb-it` / `hf-cg-it` 同理。

**修法**（`crates/opencat-core/`）：
- `style.rs`：新增 `enum FontStyle { Normal, Italic }`；`NodeStyle.font_style`
  与 `ComputedTextStyle.font_style`；继承 `style.font_style.unwrap_or(parent.font_style)`；
  加进 `Hash`；builder 加 `.italic()` / `.not_italic()`。
- `build.rs`：`italic` / `not-italic` 从 `Noop` 改成
  `ExactClassAction::FontStyle(FontStyle::{Italic,Normal})`。
- `parse/tailwind.rs`：新 action 臂 + 枚举变体。
- `text/mod.rs::text_attrs`：`.style(match style.font_style { ... })`——**这是让
  italic 真正生效的一行**。
- 三处 fingerprint 哈希补 `font_style`（`semantic/fingerprint.rs` ×2、
  `analyze/fingerprint/display_item.rs`），否则只差斜体的两个节点会撞缓存。

**测试**（`text::tests`）：
- `italic_style_selects_the_italic_face_of_the_family`——同 family 挂
  Regular + Italic 两个 face，断言两种 style 的度量宽度不同。已验证它能捕获
  回归（去掉 `.style(...)` 即失败）。
- `italic_request_on_an_italic_only_family_stays_on_that_face`——家族只有
  一个 Italic face（`hf-eb-it` / `serifd` 这种）时，Normal 与 Italic 必须
  落到同一个 face，保证本改动不会移动这类文档。
- `italic_classes_write_the_font_style_channel`——`italic` / `not-italic`
  确实写入 `font_style`（旧的 `select_and_italic_classes_are_silently_accepted`
  断言它们无副作用，已随本次改动拆开）。

**家族无斜体 face 时怎么办？合成 oblique——这正是 Chrome 的行为。**
参考视频是 Chrome 截的，所以拿 headless Chrome 当 oracle：对同一份
`Inter-800`（无斜体 face）请求 `font-style: italic`，Chrome 合成一个
**13.95°** 的 oblique；我们出 **14.25°**，墨迹盒位移也一致（左沿 Chrome +21px /
我们 +22px，右沿 +4 / +3）。拿 Chrome 的合成斜体做掩码 mae：我们-斜体
**0.042**，我们-正体 **0.174**——4 倍改善。cosmic-text 的合成阈值是
「face 的 OS/2 没有 ITALIC 位」，与 Chrome 一致（本仓库所有真斜体 face 都带
该位，Inter-800 不带）。

### 2. thesis 词距：flex gap 少了一个空格

参考的 `.word` 是 `display: inline-block; margin: 0 0.16em`，且 HTML 源码里
span 之间**有换行**——inline-block 之间的空白渲染成一个空格（Instrument Serif
132px 下 ≈21px）。所以参考每处词间隙 = `0.16em×2 + 空格` ≈ **63px**。

我们的 JSX 把每个词做成 flex 子项，`gap-[42.24px]` 只有 `0.16em×2`，
**少了那个空格**。实测：第二行参考宽 1554px，我们 1433px，窄 121px。

**修法**：`gap-[42.24px]` → `gap-[62.2px]`（4 处，含两行各自的 gradient 副本层）。
复测：line1 1476 vs 参考 1474，line2 1554 vs 1554。

### 效果

| 场景 | 修前 mae | 修后 mae |
|---|---|---|
| thesis | 6.221 | **2.048** |
| flex-drop | 5.355 | 4.873 |
| 全片 | 3.096 | **2.766** |

`flex-threejs` / `flex-footage` **没变**：它们用的 Times New Roman 家族只声明了
Italic 一个 face，本来就被选中。真正受 font-style 影响的是有 Regular+Italic
两个 face 的 Instrument Serif。

### 仍归因于视频解码、暂不追的部分

`glass-intro` 的 `bg.mp4`（mae 9.71）/ `bg-2.mp4`（8.90）两段共 105 帧，
差异是全画面均匀细噪，而画面内确定性 UI 差异近零 → 解码/采样差异，已确认忽略。
`gi-beat4-video`（271 帧）mae 仅 2.26，属正常。

## 2026-10-13 本轮结论（r1-r6）

### 渲染门禁（必用）
- 一律用 `tools/render-guard.sh <jsx> <out.mp4> [max_rss_gb]` 启动渲染。它 nohup 渲染、
  每秒采样 RSS、超过上限即 SIGTERM/SIGKILL，并把曲线写进 `out/render-guard/*.rss.csv`。
- 该脚本已修一个真实缺陷：从**非 tty**（agent、脚本、被拆掉的终端）启动时，脚本会
  随启动它的进程组一起被杀，渲染就**无门禁裸奔**。现在检测到非 tty 会 `setsid` 自脱离，
  并写 `.status` 文件；`OC_RENDER_GUARD_FOREGROUND=1` 可强制前台阻塞运行。
- 实测：1494 帧全片 `exit=0`、耗时 ~450s、峰值 RSS 3.3GiB（上限 20GiB，从未触发熔断）。

### 逐轮 avg_p8
| run | avg_p8 | avg_p2 | mae |
|---|---|---|---|
| OOM 修复前基线 | 0.44335 | 0.63070 | 37.798 |
| r1 | 0.23392 | 0.52276 | 10.690 |
| r2 | 0.16813 | 0.47811 | 5.521 |
| r3 | 0.08173 | 0.33725 | 4.466 |
| r4 | 0.08039 | 0.33704 | 4.444 |
| r5 | 0.07385 | 0.33155 | 3.932 |
| r6（卡片级 blur 拆分） | 0.07889 | 0.33368 | 4.129 |
| **r6final（blur 留 wrapper）** | **0.07334** | **0.33117** | **3.903** |

### 关键测量：参考自噪声地板
- 两个参考渲染 `..._2026-10-06_swgl.mp4` 与 `..._2026-10-02_10-31-06.mp4` 互比：
  **avg_p8 = 0.0641**。它们不是同一版本。
- 90.6% 的帧两者一致，其编码噪声地板 = **0.0134**（median 0.0037）。
- 9.4% 的帧两者**完全不同**（mean 0.55）：2.1-5.6s 是同一画面但颗粒/相位不同；
  **20.8-22.0s 是 10-02 版的 shader 没渲出来（纯白）**。这些帧的"目标"本身有歧义。
- 因此 `<0.02` 在整片上物理不可达；可比口径是**一致帧**（0.0134 地板），
  当前 r6final 在一致帧上 0.0578。

### 下一步的真差距（按"净差距 = 我们 - 地板"排序，仅取参考一致的场景）
1. `flex-threejs`（22.01-23.22s）：我们 0.230，地板 0.003 → 净 0.227（**最大**）。
   参考 `.three-stage` 是 `perspective:900px` + `perspective-origin:50% 50%`，
   我们 JSX 用 `proj()` 手写透视、旋转中心 (960,453.6)。torus 段位置/朝向仍不对。
2. `engine`（32.44-41.48s）：我们 0.087，地板 0.002 → 净 0.085。
   字幕胶囊：垂直位置已对齐（bottom_gap 均 81px），但我们的胶囊**窄约 8%**
   （f1100 参考 856px vs 我们 791px）——疑似 `font-[hf-inter-med]` 没解析成 Inter-Medium。
3. 其余一致场景净差距都在 0.02-0.03 量级（thesis、flex-css、cta、canvas-close 等）。

### 工作区未提交改动（本轮的实验成果，尚未 commit）
- `examples/hyperframes-launch/hyperframes-launch.jsx`：Instrument Serif 真字体、
  真 `entryEase` 函数型 ease、beat2/3/4 的 cut 显隐、flex-shader 的 `uy` 翻转、
  flex-threejs 的 `uW` 修正、若干 `uppercase` 补正。**blur 留在 wrapper**（实测最优）。
- `crates/.../core.js` + `bindings.rs` + `store.rs`：支持 GSAP 的**函数型 ease**
  （host 按线性跑，JS 侧再映射；`animate_set_progress` 回写进度供颜色路径用）。
- `crates/opencat-engine/src/executor/replay.rs`：视频 1:1 整数对齐 blit 时跳过
  Mitchell cubic 重采样（只对真缩放用 cubic）。
- `crates/opencat-core/src/render/helpers.rs`：`radial-gradient` 椭圆
  `farthest-corner`/`closest-corner` 半径 ×√2；`render_rect` 不再重复发 backdrop-blur。
- `crates/opencat-core/src/pipeline/frame.rs`：`OC_DBG_STACK=1` 的 save/restore 平衡探针。
- `tools/cmp.py`：对比图工具（已存在于工作区）。
- 测试：`cargo test -p opencat-core` = 661 passed / 3 ignored。

### 已知的坑
- **单帧渲染 ≠ 连续渲染**：同一 JSX 单帧 PNG 与全片 mp4 对应帧会差 maxdiff 40-90
  （视频解码 seek 路径不同）。逐帧调试结论必须用全片渲染复核。

> 2026-10-10 对齐工作流收工时整理。目标：`examples/hyperframes-launch/hyperframes-launch.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。工作在 lane2（worktree align-lane2 分支）完成，**已合并回 dev 主检出**（merge commit 95de42f）。

## 状态：未达标（4 轮全部渲染失败，没有产生有效 diff——先修渲染再谈对齐）
- 四轮移植员都在和渲染失败搏斗，4 轮预算耗尽在渲染关
- 渲染证据（worktree 已清理，需重渲复现）：当时的 mp4 是**坏文件（moov atom 缺失，编码中途死）**；opencat-frames 退路的 PNG 序列只出到 **561 / 1494 帧**——第 562 帧附近有让渲染死掉的东西
- 素材里有 86MB 的 `bg.mp4` 和多个视频素材，优先怀疑：某帧触发的视频解码/合成崩溃、内存、或特定效果

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/hyperframes-launch/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4`（1920x1080 @30，1494 帧）
- 素材已拷入 `examples/hyperframes-launch/assets/`（bg.mp4、beat4-clip.mp4、pill.mp4、figma-promo.mp4、Frame 1/3.mp4 等 + Impact/Consolas/SegoeUI/TimesNewRoman/ArialBlack 等系统字体）

## 证据与产物
- jsx：`examples/hyperframes-launch/hyperframes-launch.jsx`（已随 e0736a2 进 dev，附带 `jsx_runtime.js` +7 行小改动）
- 排查时重跑逐帧渲染，数最后一帧停在哪，看下一帧在 jsx 时间轴上对应什么内容

## 下一步
1. 合并后 `cargo build --release` 重链，单独复现渲染：先跑 opencat-frames PNG 序列，确认是否还死在 562 帧附近
2. 定位死因：对照 jsx 时间轴看 562 帧（≈18.7s）在渲染什么；若是视频素材问题，用 ffmpeg 单独解该素材验证；若是引擎崩溃（panic/挂起），修 crates——这是引擎缺陷外露的机会
3. 渲染通了之后才算第 1 轮，进入正常 4 轮 k3diff 循环

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/hyperframes-launch/hyperframes-launch.jsx out/hyperframes-launch.mp4`
- 逐帧退路（本示例排障首选）：同上换 `--bin opencat-frames` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/hyperframes-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/hyperframes-launch.mp4 --out out/k3diff-hyperframes-launch --dump <最差帧号>`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
