# handoff — pr-to-video-launch

> 2026-10-10 对齐工作流收工时整理。目标：`examples/pr-to-video-launch/pr-to-video-launch.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。

## 状态：未达标（最好 2.675%，差门禁 0.675pp，全场最接近的一批）
- 四轮 avg_p8：2.793 → 2.686 → 2.688 → **2.675**（收敛变慢，进入平台期）
- 更早的 8 轮探索期也在 2.9–3.4% 平台震荡：差异偏**系统性**（字体度量 / 子像素定位 / 时机微移），不是结构性缺失
- 最差帧集中在 **226–233**（top5：228, 227, 226, 229, 233）

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/pr-to-video-launch/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/pr-to-video-launch/renders/pr-to-video-launch_2026-10-06_swgl.mp4`（1920x1080 @30，900 帧）

## 证据与产物
- jsx：`examples/pr-to-video-launch/pr-to-video-launch.jsx`（已提交，commit 2d08d06）
- 渲染：`out/pr-to-video-launch.mp4`；对比证据：`out/k3diff-pr-to-video-launch/`（summary.json、pixel.csv、最差帧对照图 f*-ref/-test/-diff.png）
- 本示例带出的引擎改动已随提交入库：`crates/opencat-core/src/layout/mod.rs`、`parse/jsonl/mod.rs`、`script/runtime/animation/core.js` 等

## 下一步
1. 重渲当前 jsx 复现 2.675%（确认引擎基线没变）
2. k3diff `--dump 228,227,226`，Read 对照图定位 226–233 这段动画的差异根因
3. 差异属系统性：优先排查字体度量（ fontFamily 解析 / 字号 / 行高 / letter-spacing 的子像素累积）与绝对秒定位的小偏移

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/pr-to-video-launch/pr-to-video-launch.jsx out/pr-to-video-launch.mp4`
- 编码失败退回逐帧：同上换 `--bin opencat-frames` → PNG 序列 `frame_%04d.png`，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/pr-to-video-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/pr-to-video-launch.mp4 --out out/k3diff-pr-to-video-launch --dump 228,227,226`（stdout 尾部 `=== aggregate ===` 给 avg_p8；summary.json 的 `agg.avg_p8` 是占比，0.02 = 2%）
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 素材复原
大体积素材不入库（见 `.gitignore`）。参考仓库 `/Users/solaren/Projects/hyperframes-launches/pr-to-video-launch`：
- 直接拷：`assets/launch-music.mp3`、`assets/hyperframes-logo-motion.mp4`、`assets/pet-typing.mp4`
- 派生件 `pet-typing-2x.mp4`：参考里 `.pet` 是同一文件的两个顺序实例（media `[0,5.27]` + `[0,1.73]`），JSX `<video>` 无法表达，故按媒体区间拼成单文件：
  ```bash
  ffmpeg -i pet-typing.mp4 -t 5.27 -c copy a.mp4
  ffmpeg -i pet-typing.mp4 -t 1.73 -c copy b.mp4
  printf "file '%s'\n" "$PWD/a.mp4" "$PWD/b.mp4" > list.txt
  ffmpeg -f concat -safe 0 -i list.txt -c copy assets/pet-typing-2x.mp4
  ```

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
