# handoff — inspector-launch

> 2026-10-10 对齐工作流收工时整理。目标：`examples/inspector-launch/inspector-launch.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。工作在 lane2（worktree align-lane2 分支）完成，**已合并回 dev 主检出**（merge commit 95de42f）。

## 状态：进行中（jsx 已写完一版，尚未渲染验证过；4 轮预算一轮未用）
- `examples/inspector-launch/inspector-launch.jsx`（1475 行）已提交（WIP commit 随 1459589 合入 dev）
- 停工时移植员正在给引擎加 **GSAP CustomEase 采样缓动**支持，已落地部分：
  - `crates/opencat-core/src/parse/easing.rs`：`Easing::Samples(u32)` 变体 + 进程全局采样表注册表（`register_ease_table`）+ `points:<name>` 标签解析 + 分段线性采样语义
  - `crates/opencat-core/src/script/bindings.rs`：+6 行绑定
  - **需核查**：JS 侧的接线是否完整——`register_ease_table` 是否已暴露成 jsx/animation facade 可调用的入口、jsx_runtime 是否会在帧首注册采样点。若只有 Rust 侧，采样表永远是空的（退化为线性）

## 素材
- 参考组合：`/Users/solaren/Projects/hyperframes-launches/inspector-launch/index.html`
- 参考视频：`/Users/solaren/Projects/hyperframes-launches/inspector-launch/renders/inspector-launch_2026-10-06_swgl.mp4`（1920x1080 @30，1350 帧）
- `examples/inspector-launch/assets/` 已建（素材随 WIP 提交入库）

## 下一步
1. 核查 CustomEase 的 JS→Rust 全链路：jsx 里 `points:` 缓动怎么注册采样点（对照 easing.rs 注释：「JS 侧每帧注册一次、按名字幂等」），补齐 JS 入口后加一个最小验证用例
2. `cargo build --release --bin opencat --bin opencat-frames`（合并后首次全量重链），渲染 inspector-launch.jsx 完成真正的第 1 轮
3. k3diff 进 4 轮循环；渲染失败先看是不是 CustomEase 解析（`points:` 标签）路径崩溃

## 渲染与对比（在仓库根目录下）
- 直接渲染：`cargo run --release --bin opencat -- examples/inspector-launch/inspector-launch.jsx out/inspector-launch.mp4`
- 编码失败退回逐帧：同上换 `--bin opencat-frames` → PNG 序列，再 `ffmpeg -framerate 30 -i frame_%04d.png -crf 0 -pix_fmt yuv420p out/inspector-launch.mp4`
- 对比：`python tools/k3diff.py <参考视频> out/inspector-launch.mp4 --out out/k3diff-inspector-launch --dump <最差帧号>`

## 移植规则（Solaren 定，勿违反）
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（`ctx.timeline()`/to/fromTo/set，绝对秒定位）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
