# House Style

当没有 `design.md` 时的默认设计方向。任何不符合内容的都可覆盖。

---

## 写 JSONL 之前

1. **解读需求。** 生成真实内容。
2. **选色板。** 亮色还是暗色？声明 bg、fg、accent 再写代码。
3. **选定排版层级。** 使用 Tailwind text size 体系。

---

## 需要警惕的默认项

AI 设计的通用套路。如果是有意选择就用，否则暂停：

- 渐变文字（`bg-gradient-to-r` 用在 text 上）
- 左边缘强调条纹在卡片上
- 暗色背景上的青色/紫到蓝渐变/霓虹强调
- 纯 `#000` 或 `#fff`（向强调色偏移一点）
- 相同尺寸卡片重复排列的网格
- 所有元素居中相同权重

---

## 色彩

- 亮/暗匹配内容：食物、健康、儿童 → 亮。科技、电影、金融 → 暗。
- 一个强调色。所有场景使用同一背景色。
- 中性色向强调色偏移。

---

## 背景层

每个场景需要视觉深度 — 持续存在的装饰元素。

思路（混合搭配，每场景 2-5 个）：

- 径向发光（accent-tinted，低透明度，呼吸缩放）
- 幽灵文字（主题词 3-8% 透明度，大字号，缓慢漂浮）
- 强调线（细线 `border-t`、`w-full`、微弱脉冲动画）
- 杂色/噪点叠加、几何形状、网格图案
- 主题装饰（轨道的环、音乐的唱片纹、数据的网格线）

所有装饰应该有缓慢的 `ctx.fromTo()` 环境动画 — breathing、drift、pulse。

---

## 动效

动效不是装饰，是叙事语气。每个运动都在说话。

- **变化是底线。** 缓动、速度、入场方向、交错节奏——每一项在同一场景内不得重复超过两次。全部 `ease-out` + 全部 12 帧 + 全部从下方入场 = 机器人做的东西。
- **缓动是副词。** `ease-out` = 自信果断，`ease-in-out` = 梦幻流畅，`elastic-out` = 俏皮活泼。选错了语气就错了。入场永远减速停下（ease-out），退场永远加速甩走（ease-in）。
- **速度传达重量。** 快 = 能量紧迫，中 = 专业可靠，慢 = 沉稳奢华。同一视频里最慢应该比最快慢 3 倍，否则所有东西读起来一样重。
- **场景有呼吸。** 建场（元素交错入场）→ 呼吸（环境动效赋予生命力）→ 收场（退场或决断性结束）。不要把所有东西堆在前面然后静止。
- **入场比退场长。** 出现要 12 帧，消失只要 7-8 帧。人对事物消失的耐心比出现短。
- **最先动的最重要。** 按重要性顺序编排，不按 DOM 顺序。重叠入场，不要串行等待。
- **过渡是叙事选择。** 交叉淡入淡出 = 延续，硬切 = 中断/唤醒，慢速溶解 = 漂移。不要所有地方都用交叉淡入淡出。

### 图片

没有静态图片。每张图都必须有动效处理：轻微透视倾斜、慢速 Ken Burns 缩放、设备边框包裹、或视差浮动。

### 画面

- 每场景至少两个焦点，视线需要路径
- Hero 文字占画布 60-80%，不要网页尺寸
- 至少三层：背景装饰 + 前景内容 + 强调元素
- 背景不为空：发光、幽灵文字、边框面板
- 锚定到边缘，不要居中漂浮

---

## 排版

使用 Tailwind text size 体系：

| 元素 | 网页 | 视频 |
|------|------|------|
| 标题 | 16-20px | `text-[60px]-[80px]` |
| 正文 | 14-16px | `text-[24px]-[36px]` |
| 标签 | 12px | `text-[16px]-[20px]` |
| 装饰不透明度 | 3-8% | 12-25% |
| 边框 | `border`（1px） | `border-2` 或 `border-4` |
| 内边距 | 16-32px | `p-[60px]-[140px]` |

---

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