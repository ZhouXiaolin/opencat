# html5ever 直渲 · 后续计划

分支 `feat/html5ever-hyperframes`（改动尚未提交）。目标：hyperframes 的 `.html` 在 OpenCat 里直接渲出片，
唯一验收标准是 k3diff（只吃视频）——不写单元测试。

## 一、现状（2026-11-06）

k3-promo 全片 494 帧，对参考 `k3-promo_2026-10-06_swgl.mp4`：

| 指标 | 值 |
| --- | --- |
| avg_p8 | **0.0175**（门限 0.02 ✔） |
| mae_mean | 1.88 |
| 分桶 p8 最差 | 50–99 → 0.0307；250–299 → 0.0295 |
| 最差单帧 | f278 0.0585、f49 0.0584、f277/f279 0.0579 |

回归门（`SKIP_BUILD=1 ./scripts/regression-gate.sh`）：k3-promo/engine **PASS** 0.01241（与改动前逐位一致）；
vfx 0.28589 / hf 0.10055 属**既有失败**（已用 HEAD 基线二进制核对）；三例 web 侧因缺 ChromeDriver 失败（环境）。

本批（批次 A）已落地：html5ever 解析骨架、`data-*` 可见窗、级联组合器、JS DOM + gsap seek、html 入口与网络缓存、
行内格式化 pass、绝对定位包含块、背景层 position/size、canvas 2D 通道、族栈选面、合成 id 两阶段一致。

### 已知剩余差距（按收益排序）

1. **文字度量宽度**（50–99、250–299 两桶的主因）：k3 用 `fitW()` 以 `mctx.measureText(textContent).width` 算
   `scaleX`，再据宽度反推 `left`。我们的 canvas mock 尚未支持 `letterSpacing`（负字距），且字重（300/400）参与
   选面的路径需复核 → nat 偏小 → `scaleX` 偏大 → 大字整体偏右、偏大（f278 可见）。
2. **chip 标签定位**：`.tickbox .txt` 的 flex 居中已实现，剩余 10px 级偏差同样来自文本宽度预测。
3. **字形/压缩噪声地板**：单帧墨迹 bbox 与参考差 1–2%（字体回退面 + 视频压缩），这部分不应无限追。

## 二、纪律（每步都适用）

- 验收只认 k3diff；全片渲染 ≈ 2.5 分钟，`tools/k3diff.py` 出 `avg_p8` 与分桶。
- 全局行为改动（选择器、布局、字体、录制器、运行时）必须跑三例回归门，并把数字写进提交信息。
- 探针（`OC_*` 环境变量、临时 `eprintln!`）**进代码即登记，收尾即删**；`grep -rn "OC_[A-Z]" crates/` 必须为空。
- 参考噪声地板在 1–2%：单帧 p8 已到 0.03 的区块，先判断是否还有结构差异，没有就停手。

## 三、阶段计划

### P0 收尾与固化（0.5 天）

- [ ] 拆分提交（4 个）：① core 解析层 + `data-*` 可见窗；② CSS 能力（组合器/calc/背景层/族栈/圆角）；
      ③ 布局语义（行内 pass、绝对定位包含块、canvas 固有尺寸）；④ html 运行时与 engine 接线（canvas 2D、id 两阶段一致）。
- [ ] `docs/html-align/DESIGN.md` 增补四条新契约：合成 id 两阶段一致（`ext:` 命名空间）、canvas 2D 通道与
      生成图键含帧时刻、绝对定位包含块用 `effective_position`、行内 pass 的 flex 居中/不收缩规则。
- [ ] 新增 `scripts/k3-html-diff.sh`：一条命令 = 渲染 k3 全片 → k3diff → 打印分桶与最差 10 帧。
- [ ] `grep` 确认探针清零，临时文件（`out/*.html`、`__dbg.html`、baseline exe）删除。

### P1 文字度量对齐（1–1.5 天，吃掉两个 0.03 桶）

- [ ] `crates/opencat-core/src/script/runtime/canvas_api.js`：`measureText` 支持 `letterSpacing`（含负值，
      按「字数 × 字距」计；末尾不补），并在 `fillText` 侧同步，保证测量与绘制一致。
- [ ] `crates/opencat-core/src/text/surface.rs`：`measure_text` 增加 letterSpacing 参数（或由 `TextStyle` 携带），
      字重 300/400 的选面走 `fontdb::Query{ weight }` 且与 `text/mod.rs::text_attrs` 同一套族栈解析。
- [ ] 校准核对：用 `out/baseline*.html` 式探针（Arial 150px "H"、"Every modality" 135px）比对
      canvas mock 与真实排版墨迹宽高，误差目标 ≤ 0.5%。
- [ ] 复测 f49 / f52 / f278 / f279 单帧 p8 < 0.03；全片目标 `avg_p8 ≤ 0.015`。
- 影响面：canvas mock 与文本测量是 jsx 通道共用面 → 必须跑三例回归门（vfx 例高度依赖 canvas 文本）。

### P2 批次 B：CSS 视觉能力（2–3 天，为下一个工程做准备）

k3 已用到的：`mask-image: radial-gradient`（`.afield`）尚未支持；`::before/::after` 与 `@keyframes` 在
`crates/opencat-core/src/css/stylesheet.rs:130` 处被显式跳过。多工程语料一定会撞上。

- [ ] `@font-face` + fontdb 载入：`stylesheet.rs` 收集 `FontFace`，按工程独立 fontdb 注册（不污染全局）；
      验收：一个自建 `@font-face` 样例能正确选面。
- [ ] `mask-image`（radial/linear）：绘制端加 alpha mask 层（`crates/opencat-core/src/render/items.rs` + `gradient.rs`），
      验收：k3 `.afield` 与参考同形。
- [ ] `@keyframes` + `animation-*` 简写：按帧时刻解析关键帧插值，注入计算样式（不进脚本通道）。
- [ ] `::before/::after`：`content` + 继承样式物化成真实节点（与「不造近似节点」原则一致）。

### P3 批次 C：多 composition 与扩量（3–5 天）

- [ ] `data-composition-src` 嵌套：`crates/opencat-engine/src/script/html.rs:94` 目前只记注释；
      `parse/html.rs:495` 已收集宿主导航映射。按宿主可见窗挂载子工程，独立时间轴 + `clip_window`。
- [ ] `video`/`audio` 通道深化（帧同步、首尾裁切）。
- [ ] **扩量验证（最重要）**：再选 3–5 个 hyperframes 官方 `.html` 直渲对照。k3 单例存在过拟合风险，
      只有第二个、第三个工程通过，批次 A 才算真正完成。每个工程记录 `avg_p8` 与不可达原因。

### P4 批次 D（按需）

- [ ] three/WebGL/lottie 通道（先确认语料里是否真的出现，再评估成本）。

## 四、挂账（已在文档里，未排期）

- 行内多行混排（既有 Paragraph 缺口）。
- `offsetWidth` 预测：当前接的是布局探针回读（渲染遍有效、首遍为 0），若遇到首遍就要尺寸的页面再补 `scope_boxes` 预测。
- `html` 祖先级标签别名（`html .x`）未支持；k3 不涉及。
- 生成图按内容寻址，同键异像素会硬错误——新工程若出现，键需继续携带帧时刻/序号。

## 五、第一件要做的事

P0 的四个提交 + `scripts/k3-html-diff.sh`，然后立刻进 P1 的 `letterSpacing` 测量（收益最确定）。
