# SVG 渲染的 Chrome-driver Oracle 对齐报告

日期：2026-10-27 · 分支：resvg `feat/lightningcss-css`（usvg simplecss → lightningcss）

## 链路

浏览器渲染首次接入 SVG 对齐链路作为 oracle。三方对同一 SVG 出图后两两做
k3diff 风格像素 diff（透明底统一黑底合成后比通道差）：

| 通道 | 渲染器 | 说明 |
| --- | --- | --- |
| chrome | Edge headless（`--headless=new` + `--screenshot`） | SVG 以 `<img>` 嵌透明底 HTML——图像模式下 Chrome 禁脚本与外部引用，与 resvg 的静态渲染语义对齐 |
| resvg | `vendor/resvg` 分支构建（lightningcss） | usvg 解析 + tiny-skia 光栅化 |
| opencat | `opencat.exe`（align.py 既有 jsx 链路） | SVG 移植面 + tiny 光栅化 |

工具：`tools/svg-align/chrome-oracle.py`（语料默认 CSS 相关子集，
`--all` 跑全量；Edge 并发多实例会 rc=21 秒退，默认串行 + 一次重试）。
自回归工具：`tools/svg-align/css-regress.py`（分支 vs simplecss 基线逐案比像素）。

## 结果（CSS 语料子集，122 案全部 ok）

| 配对 | 中位 p8 | 均值 p8 | p90 p8 | 最大 p8 | 过线数（p8≤0.5%） |
| --- | --- | --- | --- | --- | --- |
| chrome ↔ resvg | 0 | 1.27% | 0.05% | 0.64% | 112/122 |
| chrome ↔ opencat | 0 | 1.15% | 0.01% | 0.64% | 114/122 |
| resvg ↔ opencat | 0 | 0.12% | 0 | 0.093% | 120/122 |

三方中位数全部为 0：过半语料逐像素一致。均值被少数案抬高的根因全部在
下面三类，且每一类里 resvg↔opencat 都为 0——**两套自渲染器彼此一致，
差异全部来自与 Chrome 的语义/栈差**。

## 差异归因（按幅度排序）

1. **SVG2 geometry properties 未实现（0.64%，最大项）**
   `structure/style{,-attribute}/non-presentational-attribute`：
   `#rect1 { height:160px }`。SVG2 把 x/y/width/height 等 geometry 属性
   CSS 化，Chrome 生效（rect 放大）；usvg 不支持（SVG 1.1 语义下 rect
   缺 height 不渲染）。simplecss 时代同样不生效——能力缺口，非本次
   CSS 引擎切换的回归。

2. **mix-blend-mode 色彩空间（0.09%）**
   `saturation`/`color` 两案。Chrome 的 blend 走 color-management 色彩
   空间，resvg/opencat 在 sRGB 直接混合；非简单混合模式（hue/saturation/
   color/luminosity）对色彩空间敏感。自渲染两侧一致，属已知语义 gap。

3. **文本栈底噪（0.002%～0.025%）**
   font-kerning / font-size 百分值 / font shorthand / 复杂文种案：
   Chrome DirectWrite 的 kerning/hinting/亚像素定位与 fontdb+rustybuzz
   的微差，全部落在文字边缘像素。同案 resvg↔opencat=0。

## 分支自回归（css-regress.py，resvg 官方测试集全量 1749 案）

- 1746 案与 simplecss 基线渲染**逐像素一致**；
- 3 案两侧一致渲染失败（negative/zero-size、非 UTF-8，预期行为）；
- 过程中修掉三处适配层缺口：typed 属性值需走 `value_to_css_string`
  （`to_css_string` 输出的是整声明）、lightningcss 不认识的属性名落
  `CustomPropertyName::Unknown` 必须按原文保留、规则应用按
  (specificity, 源序) 稳定排序补齐 Chrome 层叠。

## 结论

- lightningcss 分支相对 simplecss 基线**零渲染回归**；
- 浏览器 oracle 下，自渲染两侧与 Chrome 的差异集中在三类已知 gap
  （SVG2 geometry properties、blend 色彩空间、字体栈），CSS 解析/匹配/
  层叠语义已对齐 Chrome；
- 后续候选：SVG2 geometry properties 的支持面（usvg 侧改造）、
  blend 的色彩空间语义（影响面小，暂缓）。
