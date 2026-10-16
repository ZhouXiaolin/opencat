# handoff — cloud-render-launch

> 2026-10-10 对齐工作流收工时整理。目标：`examples/cloud-render-launch/cloud-render-launch.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。

## 状态：未开始（从零移植，4 轮预算未动）
- `examples/cloud-render-launch/` 目录尚不存在，第 1 轮就要写出可渲染、结构完整的 jsx

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/cloud-render-launch/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/cloud-render-launch/renders/cloud-render-launch_2026-10-06_swgl.mp4`（1920x1080 @30，1140 帧）

## 下一步
1. 读参考 index.html，列场景清单（图层/时刻/动画）
2. 写第 1 版 jsx：meta 导出 1920x1080 @30、时长 = 1140/30 秒；字体从参考仓库 assets/fonts 拷进本示例 assets/fonts/ 并在 fonts 导出登记
3. 渲染 → k3diff → 进入 4 轮循环；每轮只修最大差异源

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/cloud-render-launch/cloud-render-launch.jsx out/cloud-render-launch.mp4`
- 编码失败退回逐帧：同上换 `--bin opencat-frames` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/cloud-render-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/cloud-render-launch.mp4 --out out/k3diff-cloud-render-launch --dump <最差帧号>`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
