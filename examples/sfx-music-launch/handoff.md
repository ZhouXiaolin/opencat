# handoff — sfx-music-launch

> 2026-10-10 对齐工作流收工时整理。目标：`examples/sfx-music-launch/sfx-music-launch.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。

## 状态：未开始（从零移植，4 轮预算未动）

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/sfx-music-launch/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/sfx-music-launch/renders/sfx-music-launch_2026-10-06_swgl.mp4`（1920x1080 @30，1014 帧）
- 名字暗示音频/音乐驱动：音频轨不在 k3diff 像素对比范围内，但若画面有**音频驱动的可视化**（波形/节拍缩放），要从参考组合里找到驱动数据（数组/节拍表），用 useGSAP 按绝对秒还原，不要猜

## 下一步
1. 读参考 index.html，列场景清单；重点找音频驱动的参数来源
2. 写第 1 版 jsx：meta 导出 1920x1080 @30、时长 = 1014/30 秒
3. 渲染 → k3diff → 进入 4 轮循环

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/sfx-music-launch/sfx-music-launch.jsx out/sfx-music-launch.mp4`
- 编码失败退回逐帧：同上换 `--bin opencat-frames` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/sfx-music-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/sfx-music-launch.mp4 --out out/k3diff-sfx-music-launch --dump <最差帧号>`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
