# handoff — heygen-apple-motion

> 2026-10-10 对齐工作流收工时整理。本示例在盘点阶段被跳过：根级无 index.html、无 renders/，是多子项目集合，需按子项目逐个移植。

## 状态：跳过（多子项目，需逐个盘点）
- 子项目（各有 index.html + renders，含 _swgl 渲染）：
  - `01-ui-sting/`
  - `02-bouncy-ui/`
  - `03-message-sting/`
  - `04-generate-reel/`
  - `hero/`
- 另有 `examples/instagram`、`examples/spotify` 两个子目录**无渲染**（同 claude-paper-launch 的问题：要先补参考基线才能对齐）

## 建议做法
每个子项目当作独立示例移植：`examples/heygen-apple-motion-<子项目>/`（或按 Solaren 喜好的命名），各自 4 轮预算、各自 k3diff：
1. 对每个子项目探参考视频：`ffprobe -v error -select_streams v:0 -count_frames -show_entries stream=nb_read_frames,r_frame_rate,width,height -of json <renders 里的 _swgl mp4>`
2. 读子项目 index.html 列场景清单，写第 1 版 jsx（meta 导出对齐 ffprobe 规格）
3. 渲染 → k3diff → 4 轮循环

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/<子项目>/<子项目>.jsx out/<子项目>.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/<子项目>.mp4 --out out/k3diff-<子项目> --dump <最差帧号>`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
