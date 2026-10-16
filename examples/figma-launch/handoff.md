# handoff — figma-launch

> 2026-10-10 对齐工作流收工时整理。目标：`examples/figma-launch/figma-launch.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。工作在 lane2（worktree align-lane2 分支）完成，**已合并回 dev 主检出**（merge commit 95de42f）。

## 状态：未达标（最好 11.270%）
- 四轮：r1 渲染失败 → r2 渲染失败 → r3 11.584% → **r4 11.270%**（后两轮才开始有效对比）
- 最差帧集中在 **432–436**（top5：436, 433, 434, 432, 435）——同一段动画连续最差，是单点根因不是弥散差异

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/figma-launch/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/figma-launch/renders/figma-launch_2026-10-06_swgl.mp4`（1080x1080 @30，1922 帧，方版长片）

## 证据与产物
- jsx：`examples/figma-launch/figma-launch.jsx`（已随 72d1ee9 进 dev）
- 对比证据已归档到主检出：`out/lane2-k3diff/k3diff-figma-launch/`（summary.json、pixel.csv、最差帧对照图）；渲染 mp4 与 PNG 序列（1922 帧全）随 worktree 清理移除，按下方命令重渲
- 本示例移植贡献了大量引擎能力（随 72d1ee9 入库）：`style_props.js`（CSS 样式属性动画）、`script/mutations.rs`、`tailwind.rs`、`render/dispatch.rs`/`helpers.rs`（含 backdrop 能力）、`media/types.rs` 等——后续示例可直接受益

## 下一步
1. 合并后引擎有变动（video 计时字段 kebab/camel 双拼写、backdrop 双实现），先 `cargo build --release --bin opencat --bin opencat-frames` 再复渲，确认 11.27% 基线
2. `--dump 436,433,432` Read 对照图，定位 432–436 段的根因
3. r1/r2 的渲染失败已不再复现（后两轮成功），若复渲再遇编码失败，退回 opencat-frames PNG 序列封装

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/figma-launch/figma-launch.jsx out/figma-launch.mp4`
- 编码失败退回逐帧：同上换 `--bin opencat-frames` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/figma-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/figma-launch.mp4 --out out/k3diff-figma-launch --dump 436,433,432`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
