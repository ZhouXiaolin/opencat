# handoff — codex-five-hour-limit-replica

> 2026-10-10 对齐工作流收工时整理。目标：`examples/codex-five-hour-limit-replica/codex-five-hour-limit-replica.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。

## 状态：未开始（从零移植，4 轮预算未动）
- `examples/codex-five-hour-limit-replica/` 目录尚不存在，第 1 轮就要写出可渲染、结构完整的 jsx

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/codex-five-hour-limit-replica/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/codex-five-hour-limit-replica/renders/codex-five-hour-limit-replica_2026-10-06_swgl.mp4`（**3840x2160 @60**，609 帧）
- 注意：4K@60 渲染开销大，每轮渲染时间会明显长于 1080p 示例，第 1 轮就把结构写对、少烧轮次

## 下一步
1. 读参考 index.html，列场景清单（图层/时刻/动画）
2. 写第 1 版 jsx：meta 导出 3840x2160 @60、时长 = 609/60 秒；字体从参考仓库 assets/fonts 拷进本示例 assets/fonts/ 并在 fonts 导出登记
3. 渲染 → k3diff → 进入 4 轮循环；每轮只修最大差异源

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/codex-five-hour-limit-replica/codex-five-hour-limit-replica.jsx out/codex-five-hour-limit-replica.mp4`
- 编码失败退回逐帧：同上换 `--bin opencat-frames` → PNG 序列，再 `ffmpeg -framerate 60 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/codex-five-hour-limit-replica.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/codex-five-hour-limit-replica.mp4 --out out/k3diff-codex-five-hour-limit-replica --dump <最差帧号>`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
