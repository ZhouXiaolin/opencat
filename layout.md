# Layout Alignment Notes

## Purpose

这份文档记录 `opencat` 当前在浏览器布局对比中的测量敏感点，方便后续继续把
`Tailwind HTML + Chrome` 与 `Rust Node tree + Taffy + Skia text measurement`
对齐得更精确。

当前浏览器对比测试入口：

```bash
cd /Users/solaren/Projects/CatCut/opencat
CHROMEDRIVER_BIN=/Users/solaren/.local/bin/chromedriver \
CHROME_BIN='/Applications/Google Chrome.app/Contents/MacOS/Google Chrome' \
rtk cargo test chromedriver_tailwind_layout_matches_taffy -- --nocapture
```

## Current Coverage

当前浏览器回归套件已覆盖这些类别：

- `justify-*`
- `items-*`
- `gap / padding / padding-* / margin / margin-* / spacing scale`
- `relative / absolute / inset-0 / inset-x / inset-y / left / right / top / bottom`
- `grow / shrink / basis` 的长度型子集，以及 `grow-[n] / shrink-[n]`
- `flex-row / flex-col` 下主轴与交叉轴的对齐差异
- 文本的单行、换行、字号、`leading-*`、`tracking-*`、`uppercase`

## Known Sensitive Points

### 1. Text measurement is still the biggest source of browser mismatch

盒模型类 fixture 大多已经稳定；目前主要偏差集中在文本。

根因上，浏览器和 `opencat` 不是同一个文本栈：

- 浏览器使用 Chrome 的排版与 font metrics
- `opencat` 使用 Skia text measurement，再把结果喂给 Taffy

因此即便 class 一致，也可能出现这些差异：

- 文本宽度有少量偏差
- 文本高度与 baseline/line box 位置有偏差
- `tracking` 和 `uppercase` 叠加时，宽度偏差会放大
- `leading-*` 会放大多行高度差异

### 2. The previous large text divergence was caused by absolute line-height parsing

`text-leading-and-tracking-stack` 和 `fixed-width-multisize-copy` 之前出现的数百像素级
`height / y` 偏差，根因已经确认并修复：

- `leading-[18px]`
- `leading-[24px]`

这类 Tailwind arbitrary line-height 在浏览器里是“绝对像素行高”，但 `opencat` 之前把它当成
了 unitless multiplier。

结果就是：

- `text-[16px] leading-[18px]` 被算成 `16 * 18 = 288px`
- `text-[20px] leading-[24px]` 被算成 `20 * 24 = 480px`

这不是约束传递问题，而是 line-height 数据模型缺少“absolute px”分支。

现在修复方式是：

- 保留原来的 unitless `line_height`
- 新增 absolute `line_height_px`
- `leading-[18px]` 这类 class 走 `line_height_px`
- Skia paragraph layout 时把 absolute px 转回对应的 height override

修复后，这两个样本已经重新回到主 browser parity suite。

### 3. `basis-*` support is currently length-only

目前为了先把浏览器回归扩起来，`basis-*` 只支持长度型子集：

- `basis-<spacing>`
- `basis-px`
- `basis-[<px>]`

还没有覆盖：

- `basis-1/2`
- `basis-full`
- 其它百分比 / 分数型 basis

后续如果要更完整对齐 Tailwind，需要把 `flex-basis` 从 `Option<f32>` 升级成可表达
`length/percent/auto` 的模型。

## Interpretation Rules For The Test Suite

当前对浏览器对比结果的解释应遵循这几个原则：

- 盒模型 fixture 的失败，优先当成真实布局回归
- 文本 fixture 的小偏差，优先看是否属于 font metrics 差异
- 文本 fixture 的大偏差，优先检查约束传递、wrap 策略和父容器布局模式
- 不要把所有文本失败都简单归因为“字体不同”

## Next Alignment Work

后续建议按这个顺序继续收敛：

1. 给失败文本 fixture 输出更多中间信息：
   `wrap_text`、父容器宽度约束、Taffy measure 输入宽度、Skia 返回宽高。
2. 对比浏览器的：
   `font-size`、`line-height`、`letter-spacing`、`text-transform`、`white-space`、`width`.
3. 把 `flex-basis` 数据模型扩成：
   `auto / length / percent`.
4. 再逐步把更多来自 Tailwind `utilities.test.ts` 的布局类加入浏览器矩阵。
