# House Style

当没有 `design.md` 时的默认设计方向。这些是起点 — 任何不符合内容的都可覆盖。当 `design.md` 存在时，其品牌值优先；house-style 填补空白。

## 写 JSONL 之前

1. **解读需求。** 生成真实内容。一个菜谱列出真实食材。一个 HUD 有真实读数。
2. **选色板。** 亮色还是暗色？声明 bg、fg、accent 再写代码。
3. **选定排版层级。** 使用 Tailwind text size 体系。

## 需要警惕的默认项

这些是 AI 设计的通用套路。如果你正要使用其中一个，暂停并问：这是为这个内容做出的**有意选择**，还是我在偷懒？

- 渐变文字（`bg-gradient-to-r` 用在 text 上）
- 左边缘强调条纹在卡片上
- 暗色背景上的青色 / 紫到蓝渐变 / 霓虹强调
- 纯 `#000` 或 `#fff`（向强调色偏移一点）
- 相同尺寸卡片重复排列的网格
- 所有元素居中相同权重（让视线有个去处）

如果内容确实需要其中之一 — 居中布局用于庄严结尾、卡片用于真实产品 UI 模拟、纯黑用于电影效果 — 就用。目标是**有意选择**，不是回避。

## 色彩

- 亮/暗匹配内容：食物、健康、儿童 → 亮。科技、电影、金融 → 暗。
- 一个强调色。所有场景使用同一背景色。
- 中性色向强调色偏移（即使是微弱的暖/冷也比死灰色好）。
- 不要在 className 中硬编码色值 — 使用 Tailwind token。

## 背景层

每个场景需要视觉深度 — 持续存在的装饰元素，在内容入场动效期间保持画面不空。没有这些，场景在入场 staggered 时会感觉空洞。

思路（混合搭配，每场景 2-5 个）：

- 径向发光（accent-tinted，低透明度，呼吸缩放） — 用 `div` + `bg-gradient-to-*` + `blur-*` 实现
- 幽灵文字（主题词 3-8% 透明度，大字号，缓慢漂浮）
- 强调线（细线 `border-t`、`w-full`、微弱脉冲动画）
- 杂色/噪点叠加、几何形状、网格图案
- 主题装饰（轨道的环、音乐的唱片纹、数据的网格线）

所有装饰应该有缓慢的 `ctx.fromTo()` 环境动画 — breathing、drift、pulse。静态装饰感觉死板。

**装饰计数 vs 动画计数。** "每场景 2-5 个"指的是装饰**元素**。如果 design.md 说"每场景单环境动效"，意思是一个循环动效应用于这些装饰（共享的 breath/drift/pulse）— 不是总共只有一个元素。4 个装饰共享一个 breathing 动画是正确的；1 个装饰是穿少了。

## 动效

参见 opencat.md §6 Animation System 了解完整 API。快速参考：duration 12-24 帧，变化 easing，在入场时组合变换。

## 排版

使用 Tailwind text size 体系。推荐：`text-[60px]-[80px]` 标题 / `text-[24px]-[36px]` 正文 / `text-[16px]-[20px]` 标签。

## 色板

在写 JSONL 之前声明一个背景、一个前景、一个强调色。

| 类别 | 适用 | 参考风格 | 色板 |
|------|------|---------|------|
| Bold / Energetic | 产品发布、社交、公告 | Maximalist Type | [palettes/bold-energetic.md](palettes/bold-energetic.md) |
| Warm / Editorial | 叙事、纪录片、案例 | Soft Signal | [palettes/warm-editorial.md](palettes/warm-editorial.md) |
| Dark / Premium | 科技、金融、奢侈品、电影 | Velvet Standard / Shadow Cut | [palettes/dark-premium.md](palettes/dark-premium.md) |
| Clean / Corporate | 解释视频、教程、演示 | Swiss Pulse | [palettes/clean-corporate.md](palettes/clean-corporate.md) |
| Nature / Earth | 可持续、户外、有机 | Soft Signal 变体 | [palettes/nature-earth.md](palettes/nature-earth.md) |
| Neon / Electric | 游戏、科技、夜生活 | Deconstructed | [palettes/neon-electric.md](palettes/neon-electric.md) |
| Pastel / Soft | 时尚、美妆、生活方式、健康 | Soft Signal | [palettes/pastel-soft.md](palettes/pastel-soft.md) |
| Jewel / Rich | 奢侈品、活动、精致 | Velvet Standard | [palettes/jewel-rich.md](palettes/jewel-rich.md) |
| Monochrome | 戏剧、排版为重点 | Shadow Cut | [palettes/monochrome.md](palettes/monochrome.md) |

或从头推导：选一个色相，在不同明度上构建 bg/fg/accent，将所有中性色向该色相偏移。
