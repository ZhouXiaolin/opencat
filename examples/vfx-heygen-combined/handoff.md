# handoff — vfx-heygen-combined

> 2026-10-10 对齐工作流收工时整理。目标：`examples/vfx-heygen-combined/vfx-heygen-combined.jsx` 渲染结果与参考视频 k3diff **avg_p8 < 2%**，每示例最多 4 轮「改 jsx → 渲染 → 对比」。

## 状态：未达标（2026-10-21 最好 24.24%，门禁 avg_p8 < 2%）
分段（8x8 池化 p8，参考 = swgl 视频直解码，含编码噪声）：

```
  0- 49 28.0%   50- 99 55.1%   100-149 44.5%   150-199 52.5%
200-249 27.1%  250-299 44.9%   300-349  5.6%   350-399  1.4%
400-449 26.2%  450-499 19.8%   500-549  7.0%   550-599 10.9%
600-649 18.3%  650-699 18.2%   700-749 17.3%   750-799  6.8%
```

### 已修（本轮，avg_p8 28.94% → 24.24%）
- **裸 `.jsx` 无样式**：tailwind 原生解析退役后，没有 `export const css` 的示例整片透明。
  引擎改为读入口同目录的 `tailwind.css` sidecar（bundle 自带 css 优先），
  `bun tools/build-example-css.mjs examples/vfx-heygen-combined` 预编译。
  这一条是历史 29.897% 的真正原因，不是特效实现问题。
- **`transformOrigin: '174px 0'`**：gsap 写 px 原点被静默丢弃（store 只存盒子分数）。
  store 写入前按盒子尺寸折成百分比（`origin_px_to_fractions`）。
- **8 处 `ctx.timeline()` → 真 `gsap.timeline()`**（阶段 4 遗留）。
- **`#next-bg` 补 `saturate(1.1) contrast(1.03)`**、**`#next-card` 的 `backdrop-filter`
  改走 class**（CSSPlugin 不认识 `backdropBlur`，原来那条 tween 从未落库）。
- **`.availability-supported` 改左对齐 block**：源里 `.availability > span{display:block}`
  特异性压过 `.availability-supported{display:flex}`，原来横向偏 105px；现在逐字形对齐到 ±2px。

### 已知残差（按收益排序）
1. **100–199 玻璃卡内部右侧**：参考该区域均亮 142，视频同位置只有 26。任何 sigma 的模糊、
   任意视频帧、缩放/镜像采样都对不上；`screen` 合成能解释亮度但会把卡片顶部（顶条
   `mae 11/255`，与纯模糊完全吻合）弄坏。判断为参考侧 `backdrop-filter` 的提亮/泛光artifact，
   我们的实现是规范语义（模糊+卡片渐变），没有再往这个方向对齐。
2. **250–299 / 400–499 手机段**：参考手机是 `models/iphone.glb` 的 THREE 渲染（镜头模组、
   侧边倒角、透视都是模型自带），jsx 用 div 近似，结构差在摄像头模组和机身倒角。
3. **400–499 色散段**：三平面 `AdditiveBlending` + UnrealBloom 已有对应 SKSL（CHROM），
   但红/绿/蓝平面的合成结果偏"三份彩色"，参考偏"白字+细边"。zSplit/camZ 反算一致，
   差异集中在 bloom 权重与平面 alpha 的乘积语义。

## 证据与产物
- jsx：`examples/vfx-heygen-combined/vfx-heygen-combined.jsx`
- 逐帧 PNG：`out/vfx-fix3-frames/`（789 帧）；分段表脚本：`out/blurdiff.py`
- 参考帧抽取：`ffmpeg -i <swgl.mp4> -vf "select='eq(n,170)'" -fps_mode passthrough ref.png`
- 注意：单帧对比请用 PNG/无损，编码一次就有 `ma_e ≈ 2.97` 的编码器差异。

## 下一步
1. 接 1 号残差前先确认：要不要为参考的 backdrop 提亮做非规范 emulation（会偏离 CSS 语义，
   且只对这一支有收益）
2. 手机段若要继续，需要 GLTF 渲染能力（useThree + glb），不是 jsx 层能补的
3. 色散段先核对 `#chromatic-still-frame span` 的字号/位置，再调 bloom

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
- 与参考 HTML+Tailwind+GSAP 结构一一对应：静态层 OpenCat 节点 + tailwindcss；动画 useGSAP（真包 `gsap.timeline()`/to/fromTo/set，绝对秒定位；`ctx.timeline` 已退役）；文字动画用 GSAP 插件（scramble-text/splitText）；3D 用 useThree；自由绘制 useCanvas/useCanvasKit
- 禁止烘焙序列帧模仿动画；转场用 timeline 节点
- SKSL 绘制优先 liangyi 内核（ly.pixel/ly.scan）
- 引擎缺陷先补 opencat 本身（crates/），不要在 jsx 里绕；行为只能增强，已达标示例（k3-promo、liquid-brand-refraction、website-to-hyperframes）不能退化
- 黄金范例：`examples/k3-promo/k3-promo.jsx`；JSX 坑：JSXText 前导空格写 `{" ..."}`、path 必须带 view-box、prop 不能叫 h
