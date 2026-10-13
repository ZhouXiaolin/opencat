# handoff — hyperframes-launch

> 2026-10-13 对齐更新。状态：**未达标，但首次有了有效 diff**。
> 全片 `avg_p8 = 0.0733`（目标 <0.02）。参考自噪声地板见下。

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
- 参考组合：`F:/hyperframes-launches/hyperframes-launch/index.html`
- 参考视频：`F:/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4`（1920x1080 @30，1494 帧）
- 素材已拷入 `examples/hyperframes-launch/assets/`（bg.mp4、beat4-clip.mp4、pill.mp4、figma-promo.mp4、Frame 1/3.mp4 等 + Impact/Consolas/SegoeUI/TimesNewRoman/ArialBlack 等系统字体）

## 证据与产物
- jsx：`examples/hyperframes-launch/hyperframes-launch.jsx`（已随 e0736a2 进 dev，附带 `jsx_runtime.js` +7 行小改动）
- 排查时重跑逐帧渲染，数最后一帧停在哪，看下一帧在 jsx 时间轴上对应什么内容

## 下一步
1. 合并后 `cargo build --release` 重链，单独复现渲染：先跑 opencat-frames PNG 序列，确认是否还死在 562 帧附近
2. 定位死因：对照 jsx 时间轴看 562 帧（≈18.7s）在渲染什么；若是视频素材问题，用 ffmpeg 单独解该素材验证；若是引擎崩溃（panic/挂起），修 crates——这是引擎缺陷外露的机会
3. 渲染通了之后才算第 1 轮，进入正常 4 轮 k3diff 循环

## 渲染与对比（在 F:/OpenCat 下，PATH 需含 C:/ffmpeg/bin）
- 直接渲染：`python -c "import os,subprocess,sys; os.environ['PATH']=r'C:/ffmpeg/bin'+os.pathsep+os.environ.get('PATH',''); sys.exit(subprocess.call(['target/release/opencat.exe']+sys.argv[1:]))" examples/hyperframes-launch/hyperframes-launch.jsx out/hyperframes-launch.mp4`
- 逐帧退路（本示例排障首选）：同上换 `target/release/opencat-frames.exe` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/hyperframes-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/hyperframes-launch.mp4 --out out/k3diff-hyperframes-launch --dump <最差帧号>`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
