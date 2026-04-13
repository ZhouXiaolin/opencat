# Layout Alignment

## 目标

`opencat` 的布局对齐工作分成两层：

- Rust 单元测试只负责 parser 和语义映射，验证 Tailwind class 是否被正确解析进 `NodeStyle`
- 浏览器几何对齐只放在 `src/inspect/browser_layout_tests.rs`，用生成的 HTML + ChromeDriver 去对比我们自己的布局引擎

这条边界是强约束：

- 解析正确，不代表布局正确
- 布局正确，也不应该靠 parser 单测证明

## 当前测试入口

```bash
cd /Users/solaren/Projects/CatCut/opencat
rtk cargo test parser_ -- --nocapture
rtk cargo test generated_layout_fixture_templates_cover_utilities_manifest -- --nocapture
rtk cargo test chromedriver_tailwind_extended_flex_layout_matches_taffy -- --nocapture
```

其中：

- `parser_` 负责 class -> style 映射
- `generated_layout_fixture_templates_cover_utilities_manifest` 负责检查自动生成模板能否承接已接入的 layout group
- `chromedriver_tailwind_extended_flex_layout_matches_taffy` 负责真正的浏览器几何对齐

## 自动生成链路

浏览器布局测试不再手工一条条补 fixture，而是从 `testsupport/utilities.test.ts` 自动抽取。

流程如下：

1. 从 `utilities.test.ts` 定位 `test('...')`
2. 抽取 test body 中第一个候选 class 数组
3. 通过 `LayoutGroupSpec` 把这个测试组映射到一个 fixture 模板
4. 生成 HTML，编译 Tailwind CSS
5. 用 ChromeDriver 读取浏览器几何
6. 用 `RenderSession + Taffy + text measurement` 读取引擎几何
7. 逐节点比对

这意味着后续扩覆盖的主方式应该是：

- 优先补 `utilities.test.ts` 对应组的模板
- 再跑 browser parity
- 根据失败修 parser 或布局引擎

而不是继续手工添加离散 fixture。

## 当前已接入的 layout group

自动生成模板当前已覆盖：

- `position`
- `inset / inset-x / inset-y / inset-s / inset-e / inset-bs / inset-be`
- `top / right / bottom / left`
- `width`
- `height`
- `flex`
- `flex-shrink`
- `flex-grow`
- `flex-basis`
- `flex-direction`
- `flex-wrap`
- `justify`
- `align-content`
- `place-content`
- `items`
- `place-items`
- `gap`
- `p / px / py / pt / pr / pb / pl`
- `margin / mx / my / mt / mr / mb / ml / ms / me / mbs / mbe`
- `self`

这批里，margin 现在已经是自动生成测试的一部分，不再依赖手工 fixture。

## 当前明确不放进通用模板的项

### viewport units

`w-screen / w-svw / w-lvw / w-dvw` 与 `h-screen / h-svh / h-lvh / h-dvh` 目前不走通用模板。

原因不是 parser，而是测试模型本身：

- 浏览器里的真实 viewport 是 headless Chrome 的窗口尺寸
- `opencat` 的布局基准是 composition 宽高

这两个坐标系不一致时，会制造假失败。它们需要单独的 viewport 专项模板。

### 当前样式模型还没完整承载的能力

以下能力和浏览器语义还有结构性差距，因此还不能简单混进当前通用生成器：

- `size-*` 的完整集合
- `width/height` 的 `auto / min / max / fit`
- 百分比型 `width/height`
- `margin-auto`

根因是我们当前内部尺寸模型主要仍是：

- `width: Option<f32>`
- `height: Option<f32>`
- `width_full / height_full`
- margin 仍是数值边距，而不是可表达 `auto`

如果要把这些能力完整接进来，需要先扩内部数据模型，而不是只补 parser。

## 文本对齐规则

浏览器布局测试里，文本节点的几何容差现在固定为 `1px`。

实现规则是：

- 非文本节点继续使用 fixture 自己的 `tolerance_px`
- 文本节点无论 fixture 容差是多少，都强制收紧到 `1.0`

这样做的目的很直接：

- 盒模型可以允许少量模板级浮动
- 文本必须保持严格，否则真正的排版漂移会被大容差吞掉

## 文本未对齐的根因

当前文本偏差的根因，不在普通盒模型，而在“文本测量栈不是同一个实现”：

- 浏览器使用 Chrome 的字体选择、字形 shaping、line box 和 font metrics
- `opencat` 使用 Skia 的文本测量结果，再把结果送给 Taffy 参与布局

因此文本类失败的首要怀疑对象应该是：

- font metrics 不同
- line-height 解释不同
- letter-spacing 放大宽度误差
- uppercase 或换行策略改变测量结果
- 父容器对文本宽度约束的传播方式不同

结论上，文本错位通常不是“Flex 算法错了”，而是“文本测量输入或度量结果已经和浏览器分叉了”。

### 已经确认并修复过的一类根因

之前一批大幅文本错位来自 arbitrary line-height 的误读：

- `leading-[18px]`
- `leading-[24px]`

浏览器把它当成绝对像素行高，但我们之前把它当成 unitless multiplier，导致高度被成倍放大。

现在这类问题已经通过区分：

- `line_height`
- `line_height_px`

修掉了。

这说明文本问题要从“输入语义是否先错了”开始查，而不是一上来怪 Taffy。

## 实践规则

后续推进布局对齐时，按这个顺序处理：

1. 先看 parser 是否把 class 映射对了
2. 再看该组是否已经接入 `browser_layout_tests.rs` 的自动模板
3. 跑 browser parity，看是盒模型错位还是文本错位
4. 盒模型失败优先修布局引擎
5. 文本失败优先查测量输入、宽度约束、line-height、tracking、字体度量

不要再把布局几何测试散落到 parser 单测里。

## 本轮状态

本轮完成后，以下检查通过：

```bash
cd /Users/solaren/Projects/CatCut/opencat
rtk cargo test parser_ -- --nocapture
rtk cargo test generated_layout_fixture_templates_cover_utilities_manifest -- --nocapture
rtk cargo test chromedriver_tailwind_extended_flex_layout_matches_taffy -- --nocapture
rtk cargo check
```

当前可以继续扩的方向很明确：

- 先补更多 `utilities.test.ts` 的 layout group 模板
- 用 browser suite 炸出真实差异
- 再按差异扩内部尺寸与 margin 模型
