# handoff — vfx-heygen-combined

> 2026-10-10 对齐工作流收工时整理。目标：`examples/vfx-heygen-combined/vfx-heygen-combined.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。

## 状态：未达标（最好 29.897%，离门禁很远）
- 四轮平均差异仍在 30% 量级：存在**结构性差异**（整层缺失 / 时机错位 / 遮挡层级不对），不是精修问题
- 最差帧集中在 **68–95**（top5：95, 92, 93, 69, 68）——看这段是什么场景
- 本轮 4 轮把 jsx 重写过一遍（248 行，-143/+105），方向未突破；旧探索期的引擎修复（markup canvas 子树支持）反而把 pr-to 带进了 2% 门槛附近，说明 vfx 的差异源还没找对

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/vfx-heygen-combined/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/vfx-heygen-combined/renders/vfx-heygen-combined_2026-10-05_swgl.mp4`（1920x1080 @30，788 帧）

## 证据与产物
- jsx：`examples/vfx-heygen-combined/vfx-heygen-combined.jsx`（已提交，commit 40cf11b）
- 渲染：`out/vfx-heygen-combined.mp4`（77MB）；对比证据：`out/k3diff-vfx-heygen-combined/`（summary.json、pixel.csv、最差帧对照图）
- PNG 序列退路可用：`out/vfx-heygen-combined-frames/`

## 下一步
1. `--dump 95,92,68` Read 对照图：先判断差异是「整段动画缺失/提前/滞后」还是「效果实现不同」（这是 canvas/vfx 重头示例，优先怀疑 useCanvas 层与参考的自定义着色器行为不一致）
2. 对照参考 index.html 逐场景列清单（每个特效层的出现时刻/持续/参数），和 jsx 的 timeline 逐项核对
3. 特效内核用 liangyi（ly.pixel/ly.scan）实现，别在 jsx 里手撸逐帧

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/vfx-heygen-combined/vfx-heygen-combined.jsx out/vfx-heygen-combined.mp4`
- 编码失败退回逐帧：同上换 `--bin opencat-frames` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/vfx-heygen-combined.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/vfx-heygen-combined.mp4 --out out/k3diff-vfx-heygen-combined --dump 95,92,68`
- 引擎改动后先 `cargo build --release --bin opencat --bin opencat-frames`

## 素材复原
大体积素材不入库（见 `.gitignore`），参考仓库 `/Users/solaren/Projects/hyperframes-launches`。
- 直接拷：`assets/bg.mp4`、`assets/bg-2.mp4`、`assets/pointing-finger-extended.png`（同名同路径）
- 派生件 `bg-pad.mp4` / `bg-2-pad.mp4`（JSX `<video>` 无 `data-start`，故把内容起点前移）：
  ```bash
  ffmpeg -i bg.mp4   -vf tpad=start_duration=5.6:start_mode=clone -an -c:v libx264 -pix_fmt yuv420p assets/bg-pad.mp4
  ffmpeg -i bg-2.mp4 -vf tpad=start_duration=3.1:start_mode=clone -an -c:v libx264 -pix_fmt yuv420p assets/bg-2-pad.mp4
  ```

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
