# handoff — frame-md-launch-storyboard

> 2026-10-10 对齐工作流收工时整理。目标：`examples/frame-md-launch-storyboard/frame-md-launch-storyboard.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。工作在 lane2（worktree align-lane2 分支）完成，**已合并回 dev 主检出**（merge commit f2b8b6e）。

## 状态：未达标（最好 9.585%，但收敛最快）
- 四轮：r1 渲染失败 → r2 44.552% → r3 19.475% → **r4 9.585%**（每轮近乎砍半，势头最好）
- 最差帧分两簇：**1778–1780**（片尾）与 **587–588**（中段）——两处独立根因
- 最差帧对照图在 worktree：`F:/OpenCat-lane2/out/k3diff-frame-md-launch-storyboard/`

## 素材
- 参考组合：`F:/hyperframes-launches/frame-md-launch-storyboard/index.html`
- 参考视频：`F:/hyperframes-launches/frame-md-launch-storyboard/renders/frame-md-launch-storyboard_2026-10-06_swgl.mp4`（1920x1080 @30，1808 帧）
- 素材已拷入 `examples/frame-md-launch-storyboard/assets/`：Inter 全家桶 + JetBrainsMono + Nunito + JustAnotherHand 字体、A-roll 视频两版（`scene-03-aroll-cutout-alpha.mov` 带透明通道 + `scene-03-aroll-h264.mp4`）、`ssframe.mp4`

## 证据与产物
- jsx：`examples/frame-md-launch-storyboard/frame-md-launch-storyboard.jsx`（1168 行，已随 9e0c5e1 进 dev）
- 本示例移植带出的引擎能力（随 9e0c5e1 入库）：`tailwind.rs` +219 行（更多工具类解析）、`render/text.rs` 文本渲染修正、`style.rs`——对文字排版的示例都有正外部性
- 注意 out/ 不随 git 合并：渲染与 PNG 序列（仅 120 帧）在 `F:/OpenCat-lane2/out/`

## 下一步
1. 合并后先 `cargo build --release` 重链，复渲确认 9.585% 基线
2. `--dump 1779,587` 分别定位两簇根因：587–588 在 A-roll 场景附近（优先查视频节点时间轴/透明通道合成），1778–1780 在片尾（查收尾动画/定版帧）
3. 按每轮砍半的势头，2–3 轮内可能进 2%

## 渲染与对比（在 F:/OpenCat 下，PATH 需含 C:/ffmpeg/bin）
- 直接渲染：`python -c "import os,subprocess,sys; os.environ['PATH']=r'C:/ffmpeg/bin'+os.pathsep+os.environ.get('PATH',''); sys.exit(subprocess.call(['target/release/opencat.exe']+sys.argv[1:]))" examples/frame-md-launch-storyboard/frame-md-launch-storyboard.jsx out/frame-md-launch-storyboard.mp4`
- 编码失败退回逐帧：同上换 `target/release/opencat-frames.exe` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/frame-md-launch-storyboard.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/frame-md-launch-storyboard.mp4 --out out/k3diff-frame-md-launch-storyboard --dump 1779,587`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
