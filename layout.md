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
rtk cargo test chromedriver_tailwind_layout_matches_taffy -- --nocapture
```

其中：

- `parser_` 负责 class -> style 映射
- `generated_layout_fixture_templates_cover_utilities_manifest` 负责检查自动生成模板能否承接已接入的 layout group
- `chromedriver_tailwind_extended_flex_layout_matches_taffy` 负责自动生成 fixture 的浏览器几何对齐
- `chromedriver_tailwind_layout_matches_taffy` 负责手动 fixtures + 集成 fixtures 的浏览器几何对齐

## 测试文件结构

浏览器布局测试分为三个层次，避免文件无限膨胀：

### 1. 自动生成测试（`GENERATED_LAYOUT_GROUP_SPECS`）

- 位置：`src/inspect/browser_layout_tests.rs`
- 数量：60+ 组
- 来源：从 `testsupport/utilities.test.ts` 自动抽取
- 职责：测试**单个 utility class** 的布局语义

### 2. 手动 fixtures（`browser_layout_fixtures()`）

- 位置：`src/inspect/browser_layout_tests.rs`
- 数量：11 个独特场景
- 职责：测试**不在自动生成覆盖范围内**的特殊组合
- 维护原则：只保留无法被其他测试替代的独特场景

### 3. 集成测试 fixtures（`browser_layout_integration_fixtures()`）

- 位置：`src/inspect/browser_layout_integration_tests.rs`
- 数量：49 个场景
- 职责：测试**多个 utility class 组合**的真实 UI 模式
- 为什么手动：这些测试验证的是 utility 之间的交互效应，而非单个 class 的语义

#### 为什么集成测试不自动生成？

`utilities.test.ts` 测试的是**单个 utility class 生成的 CSS 是否正确**，而集成测试验证的是：

- 多个 utility 组合后的布局效果
- 真实场景模式（卡片、导航栏、表单、文本换行等）
- 浏览器渲染与 Taffy 布局引擎的一致性

例如 `flex-row-justify-between` 涉及：
```
flex flex-row justify-between items-center w-full h-full px-[24px] py-[16px]
```

虽然 `utilities.test.ts` 分别测试了 `flex`, `flex-row`, `justify-between`, `items-center` 等 class，但**没有测试它们组合在一起时浏览器的实际渲染结果是否与 Taffy 一致**。

#### 集成测试覆盖的典型场景

- Flex 行/列布局与对齐（justify-between, items-center, gap 等）
- 文本在窄容器中的换行行为
- 绝对定位叠加层（badges, overlays）
- 嵌套 flex 布局（导航栏、侧边栏、卡片）
- 导航网格、标签页、表单等 UI 模式
- 中文文本排版

#### 维护规则

向集成测试添加新 fixture 时：

1. 确保场景测试有意义的 utility 组合
2. 避免与已有 fixture 重复
3. 使用真实的视口尺寸和合理的容差值
4. 优先覆盖实际开发中常见的布局模式

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
- `gap / gap-x / gap-y`
- `p / px / py / pt / pr / pb / pl`
- `margin / mx / my / mt / mr / mb / ml / ms / me / mbs / mbe`
- `self`
- `min-width / max-width`
- `min-height / max-height`
- `order`
- `translate-x / translate-y`
- `visibility`
- `box-sizing`
- `aspect-ratio`
- `place-self`
- `justify-items`
- `justify-self`

这批里，所有新增的 layout group 都已接入自动生成测试，不再依赖手工 fixture。

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
rtk cargo test chromedriver_tailwind_layout_matches_taffy -- --nocapture
rtk cargo check
```

### 文件重构完成

- `browser_layout_tests.rs`：从 3623 行精简到 2451 行
  - 60+ 个自动生成规格
  - 11 个独特手动 fixtures（无法被其他测试替代的场景）
- `browser_layout_integration_tests.rs`：新增 1140 行
  - 49 个集成测试 fixtures
  - 覆盖真实 UI 模式的组合场景
- 删除了 49 个冗余 fixtures（从主文件迁移到集成测试文件）

### 测试分层职责

| 层级 | 文件 | 数量 | 职责 |
|------|------|------|------|
| 自动生成 | `browser_layout_tests.rs` | 60+ | 单个 utility class 的布局语义 |
| 手动 fixtures | `browser_layout_tests.rs` | 11 | 无法被自动生成的特殊组合 |
| 集成测试 | `browser_layout_integration_tests.rs` | 49 | 多 utility 组合的真实 UI 模式 |

当前可以继续扩的方向很明确：

- 先补更多 `utilities.test.ts` 的 layout group 模板
- 用 browser suite 炸出真实差异
- 再按差异扩内部尺寸与 margin 模型
- 集成测试按需添加新的真实布局模式
