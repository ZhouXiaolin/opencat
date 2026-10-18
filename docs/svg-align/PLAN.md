# SVG 支持对齐计划（feat/svg）

状态：P0 完成（2026-10-18）。P1 完成（2026-10-18，shapes 133/133=100%、structure
262/262=100%，含 9 例遗留 fail 的清零与 harness 类名编码缺陷的修复）。同日全量
`--force` 首次真实渲染 1746 例：1319/1746=75.5% pass、render-error 0——此前"P2+
渲染失败是预期"是误判：共享 tailwind.css 被未编码 `.` 的非法选择器污染（`bias-0.5`
类名），所有渲染统一死于 CSS 解析错误。真实基线：text 377/386=97.7%、paint-servers
139/155=89.7%、painting 276/308=89.6%、masking 42/94=44.7%、filters 90/408=22.1%。
执行载体：goal（长会话），非 workflow，决策见 §8。

## 1. 目标与非目标

目标：
- JSX 获得 `<svg>` 元素，完整渲染 SVG 1.1/2.0 常用子集（resvg 支持面）。
- 与 resvg CLI 的渲染输出做像素级 diff 对齐，作为**唯一**测试手段（不写单测）。
- `<svg>` 全量落地后，退役 `icon`（lucide）与 `path` 元素：二者改写为 svg 的语法糖，能力单一化。

非目标：
- 不引入 tiny-skia 光栅化。绘制全部落到本引擎 DrawOp。
- 不追求 SMIL 动画（resvg 同样只取首帧语义）。

## 2. 架构

```
SVG 文件 --usvg 解析--> usvg::Tree --新后端(重写 resvg renderer)--> DrawOp 序列 --> 引擎 GPU 执行
              ^保留                      ^本仓库新代码                    ^已有
```

- **解析：保留 usvg**，不自己写 XML/SVG 语义解析。依赖方式：path dependency 指向本地
  `F:/resvg/crates/usvg`（版本锚点 054ccf39743179eee7d3a32e5b8bcf59941081da，2026-10-17 master）。
  稳定后 vendor 进 `third_party/` 或改 crates.io 固定版本。
  usvg 仅依赖 `tiny-skia-path`（纯路径数据类型，无光栅化），不把 tiny-skia 拉进依赖图。
- **绘制：重写 `crates/resvg` 的 renderer 部分**。以 `crates/resvg/src/*` 为移植参考
  （它把 Tree 翻译成 tiny-skia canvas 调用；我们做同样的树遍历，翻译成 DrawOp）。
  参考读源码，不依赖 resvg crate 本身。
- DrawOp 是 Skia 语义（SaveLayer/ClipPath/DrawPath/Paint/渐变/混合/滤镜），完全有能力
  顶替 tiny-skia 的角色；缺口（pattern 等）见 §3。

关键源码对照（移植时读）：
| resvg 侧 | 职责 | 我们的落点 |
|---|---|---|
| `crates/resvg/src/render.rs` | 树遍历、组语义 | 后端主循环 |
| `crates/resvg/src/path.rs` | fill/stroke 绘制 | DrawPath + PaintSpec |
| `crates/resvg/src/clip.rs` `mask.rs` | 裁剪/遮罩 | ClipPath / blend（§3 规则） |
| `crates/resvg/src/image.rs` | 图片解码与摆放 | ImageRect（引擎已有解码器） |
| `crates/resvg/src/filter/` | 滤镜图 | ImageFilterSpec + SKSL |

P1 定稿的两条架构决策：

1. **core 是纯协议层，不解码不采样图片**。`<image>` 光栅图走新协议变体
   `ImageRef::Encoded { data: BytesRangeId }`：编码字节原样进 frame bytes 表
   （wire 自包含，TS 侧 tag 3 已解），engine 侧 `MediaContext::encoded_image`
   复用 Static 通道的 skia `Image::from_encoded`；采样与 resvg 同构
   （1:1 blit→Nearest，缩放→Mitchell；core 不做预缩放）。SVG 嵌套树仍走
   DrawOp 内联。
2. **形状边缘走 tiny-skia 光栅化**（`OC_PATH_RASTERIZER=tiny`，engine
   `executor/path_tiny.rs`）。Skia 解析式 AA 几何精确，但 resvg=tiny-skia
   扫描线 AA（hairline 特判 + 4 子扫描线，细斜线边缘覆盖率与几何差 ±20%），
   每个边缘像素差 9~75/255——这是 P1 曾有 54 例 fail 的唯一根因。svg 路径
   离屏 tiny-skia 渲染后按原 blend 合成回 skia canvas（同 crate 同版本
   0.12，逐像素同源）；默认关闭，JSX/Chrome 对齐不受影响。渐变/pattern
   路径 P1 仍走 Skia（P2 接 tiny shader）。

P1 清零 9 例时新增的三条：

3. **隔离组 Save+SaveLayer 必须配平两个 Restore**（SaveLayer 隐含一次
   save）。泄漏会平移/裁剪后续兄弟子树（nested-svg stroke 框被平移的教训）。
4. **tiny 后端的 image/clip/pattern 覆盖图**：光栅图解码换 `image` crate
   （与 resvg 同源，顺带补上 skia codec 缺失的 webp），premultiply 后
   `Pattern(Pad, Bicubic)` 平铺；clip 走离屏 AA coverage 图 + `to_shader`
   （localMatrix 抵消图原点偏移）+ `clip_shader`；pattern tile 内容按
   "pattern 就是 picture recording" 录成 subtree（`DrawOp::FillPattern`，
   opcode 50，pattern_transform 存 asAffine 序 [sx ky kx sy tx ty] 对接
   tiny 0.12 `from_row`），tile 像素尺寸与 patt_ts 逐式对齐 resvg
   `render_pattern_pixmap`。

## 3. usvg::Tree → DrawOp 映射

| usvg 节点/属性 | DrawOp |
|---|---|
| Group（transform） | Concat |
| Group opacity<1 / blend / isolate | SaveLayer { alpha, paint(blend) } |
| fill/stroke 纯色 | PaintSpec 纯色 |
| linearGradient / radialGradient | ShaderSpec::LinearGradient / RadialGradient（userSpaceOnUse 与 objectBoundingBox 换算到 paint space） |
| pattern | **缺口**：方案 = SaveLayer + 平铺 RuntimeEffect（SKSL，liangyi 通道）；P2 定稿 |
| path fill | DrawPath(path, fill paint) |
| path stroke | DrawPath(path, stroke paint)（cap/join/dash/miter 走 PaintSpec/状态 ops） |
| clipPath | ClipPath（引擎已有），clip-rule 由 usvg 折算进路径 fill-rule |
| mask（内容为矢量形状） | **折算为 ClipPath**（既定规则：形状遮罩走 clip） |
| mask（含 image/亮度语义强） | **blend**：遮罩内容离屏成图，SaveLayer + luminosity 混合 |
| image | ImageRect { src, dst }，preserveAspectRatio 由 usvg 归一后预裁剪 |
| text | usvg 已把 text 排版成 glyph path，按 path 走；字体对齐是 P5 专项 |
| filter | SaveLayer + ImageFilterSpec：Blur/ColorFilter/DropShadow/Compose 直配；其余（feTurbulence、feDisplacementMap、feMorphology、feLighting、feComposite…）走 RuntimeEffect SKSL |
| switch / use / nested svg / viewBox fit | usvg 已归一，只剩 viewport 映射（Concat + ClipPath） |

mask 规则为既定决策：**形状遮罩 → ClipPath；图片遮罩 → blend**。masking/ 目录 94 例
逐例对齐时细化边界（luminance mask、maskContentUnits 等）。

## 4. JSX 接入与退役路线

新增（P1）：
- JS 运行时 shim：`emitElement` 白名单当前为 `div, text, canvas, image, audio,
  video, icon, transition, tl, caption, path, lottie`——`svg` 需加入此处与
  `parse/elements.rs`、`resolve/tree.rs`、display、dispatch 五处。
- `parse/elements.rs`：`type: "svg"`，props `src`（相对 jsx base_dir 或绝对路径）、
  可选 `width/height` 覆盖 intrinsic、`preserveAspectRatio` 透传。
- `resolve/tree.rs`：`ElementKind::Svg(ElementSvg)`。
- display/render：`SvgDisplayItem` → dispatch 处调用后端 `emit_svg(tree, viewport)` 展开为
  DrawOp 段；usvg::Tree 按 asset 路径缓存。
- 相对引用（`<image href="x.png">`）相对 **svg 文件自身目录** 解析，不是 jsx 目录。

退役（终态）：`icon`、`path` 在 parse 层降级为等价 `<svg>`，对外写法保留、内部能力
只剩一条。验收 = 现有 examples 全量回归不劣化（hyperframes-launch 等对齐指标不回退）。

## 5. 对齐测试（唯一测试手段）

```bash
# 全量：resvg 参考 → 生成 case → opencat 渲染 → diff 报表
python tools/svg-align/align.py --stage all
# 子集（迭代时按目录收敛）
python tools/svg-align/align.py --stage all --filter shapes/rect
# harness 自检
python tools/svg-align/align.py --probe
```

- 语料：`F:/resvg/crates/resvg/tests/tests/` 七类 1748 例（filters 408 / text 386 /
  painting 308 / structure 264 / paint-servers 155 / shapes 133 / masking 94），
  另有 `tests/extra/` 21 例。锚点：F:/resvg@054ccf39。
- case 生成：meta 尺寸 = resvg 参考 PNG 实际尺寸；`<svg src>` 绝对路径；生成物在
  `out/svg-align/`（gitignored），生成器是唯一需要随 `<svg>` API 演进同步的文件。
- 案例样式走 sidecar 约定：`<jsx目录>/tailwind.css`（标准 CSS 引擎，tailwind 原生
  解析已退役）。全部案例共享 `out/svg-align/cases/tailwind.css`，由 gen 阶段整体
  重写。注意引擎 CSS 子集：用 `background-color`，`background` 简写不生效；
  位置/尺寸/overflow 等简单属性可用。类名由 `css_class()` 生成：非
  `[A-Za-z0-9-]` 字节一律转义 `_XX`（`_`→`_5f` 保证双射）——曾有语料 `bias-0.5`
  的 `.` 未转义，污染共享 css 使全部渲染死于同一 CSS 解析错误。
- diff：双边 PNG 先合成到黑底（resvg 输出透明底），k3diff 语义：mae/maxd/
  p0..p128（max 通道差超阈值像素占比）/bbox@16。**通过线：p8 ≤ 0.5% 且渲染无错**，
  初值，按阶段实况校准。
- 产物：`out/svg-align/pixel.csv`、`summary.json`（分类通过率 + worst-20）、
  `errors/<id>.txt`（渲染失败尾部日志）。
- 迭代循环：跑报表 → 取 worst → 修引擎/后端 → 重跑该类 → 收敛。不写 Rust 测试代码。

## 6. 阶段路线（每阶段验收 = 该类通过率达标）

| 阶段 | 范围 | 语料 | 验收线(初值) |
|---|---|---|---|
| P0 | harness + 分支 + resvg CLI | probe | harness 自检 p0=0 |
| P1 | `<svg>` 元素 + 后端骨架 + shapes + structure | 397 | ≥95% |
| P2 | painting + paint-servers（渐变、pattern） | 463 | ≥90% |
| P3 | masking（clip 34 / mask 60） | 94 | ≥90% |
| P4 | filters（SKSL/liangyi 为主战场） | 408 | ≥70%（按子滤波器推进） |
| P5 | text（字体库对齐）+ icon/path 退役 | 386 | ≥85%，退役回归零劣化 |

顺序理由：shape/structure 是其余一切的地基；mask 规则依赖 clip 已稳；filters 最大且
独立；text 最后（字体匹配噪音大，避免早期污染指标）。

## 7. 本次提交内容

- `docs/svg-align/PLAN.md`（本文档）
- `tools/svg-align/align.py` + `tools/svg-align/fixture/`（probe 语料）
- 分支 `feat/svg`（自 dev@162e698a）

## 8. 执行载体：goal，不用 workflow

- 工作必须串行（既定约束），workflow 的并行优势不存在。
- pr-to-video 已证明：迭代式深度对齐在 workflow 子代理里 8 轮不达标——上下文割裂，
  无法跨轮积累"哪类特征是哪个 bug"的经验。
- 引擎移植需要单一连贯的架构心智（映射表、缓存、PaintSpec 抽象是跨文件决策）。
- 批量渲染/diff/报表是机械步骤，由 align.py 零模型成本完成；模型只消费 worst-N。
- goal 长会话保持上下文；断点由 PLAN.md + pixel.csv + summary.json 承载，随时可续。
