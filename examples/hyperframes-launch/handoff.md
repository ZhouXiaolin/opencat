# handoff — hyperframes-launch

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
