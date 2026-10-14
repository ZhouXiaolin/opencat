---
name: opencat-creator
description: 用 OpenCat JSX 设计并生成有质感的视频合成。只要用户要做一个视频、动效、标题卡、产品演示、品牌短片、社交广告、发布预告、数据故事、片头，或要写/改 .jsx 合成文件，就用这个 skill。覆盖意图澄清、视觉设计、节奏规划、动画编排、CanvasKit 绘制、ly 效果内核、three.js 3D 场景、字幕和转场。输出为一个可渲染的 .jsx 文件。
license: MIT
---

# OpenCat Creator

你的任务是把用户的想法变成一个 `.jsx` 合成。这个合成能渲染成视频。

能跑起来只是底线。有质感的合成让观众记住一句话，并且每一帧都经得起看。所以先想清楚要说什么、给谁看、什么语气，再动手写代码。

## 第一步：揣摩意图

先推断，再确认。用户很少一次说清全部。你要补全这几项：

- **Message**：观众最终要记住的一句话。只能有一个主张。
- **Audience**：谁在看。开发者、消费者、投资人、内部团队。
- **Platform**：社交短视频、官网 hero、产品演示、发布预告。
- **Duration**：任意时长，由 message 的复杂度决定。时长决定 beat 数量，不是内容数量。
- **Tone**：克制、精密、热烈、奢华、温暖、实验、戏剧化。

用户的话通常很短。你要读出话外之意：

- "做个酷炫的发布视频" —— 缺受众、缺 message、缺平台。先补全，再给方向。
- 用户给了一个参考 —— 提取它的节奏、构图逻辑和色彩关系，不要照抄。
- 用户没给时长 —— 按 message 的复杂度定，不要默认塞满。

**不要拿着模糊需求直接开写。** 先说出你的理解，再给 2-3 个具体方向让用户选。方向要具体到能想象出画面，例如"深色控制台 + 逐条校准的数据条 + 结尾锁定 logo"，而不是"现代简约风"。

判断顺序对不对，见 `references/design-principles.md` 的「叙事顺序」：hook 说观众的语言，价值在第二个 beat 之前落地，画面指向源材料。

用户只是小改现有文件时，跳过这一步。

## 第二步：定设计方向

读 `references/design-principles.md` 的「设计身份」。按它确定视觉身份。

优先读取项目里的 `frame.md` → `design.md` → `DESIGN.md`，继承品牌颜色、字体、禁用项。设计文件定义品牌，不定义视频构图——颜色和字体是神圣的，字号和间距要按视频尺度重新判断。

没有设计文件时，自己声明最小设计身份：背景色 / 前景色 / 强调色 / 字体倾向 / 圆角与边框语言 / 动效性格 / 禁用项。

选字体前跑 `scripts/font-discover.py`，不要凭记忆伸手拿那几个默认字体。它会列出候选，并直接产出可用的 `fonts` 导出。

开放式任务，把设计方向告诉用户，确认后再继续。

## 第三步：规划节奏，把计划当提案交出去

每个 scene 是一个 beat。先规划节奏，再规划内容。哪个 beat 快、哪个慢、峰值在哪，先定下来，并给节奏起个名字（例如 `hook-hit → hold → peak → CTA settle`）。

每个 beat 写清 concept、mood、depth layers、motion verbs。做法见 `references/design-principles.md`。

**节奏基准**：约每 2-3 秒一个 beat。6 秒的片子 2-3 个 beat，10 秒约 3-4 个，30 秒约 8-12 个。开头 1.5-2s 是 hook，峰值放在全片 50-60% 处，尾部留 1-2s 收束。beat 数量跟着时长走，不要为了塞满而堆观点。

然后**把 beat 列表作为提案交出去**，不要闷头开写。开头回显策略（"这条片子告诉 [受众]：[message]"），再用表格一行一个 beat：

| Beat | 起止 | 画面 | 为什么 |
| --- | --- | --- | --- |
| 01 — 不再是 | 0-2.4s | 说出旧的痛，并在同一口气里解掉它 | 在第一个 beat 落地价值主张 |

「为什么」写这个 beat 在故事里的职责，追溯到 message。追溯不到的 beat 要砍掉。

这一步是最便宜的迭代点：在这里改一个 beat 花 30 秒，建完之后改要花几分钟。

用户只改文件、或明确说"直接做"时，跳过提案。

## 第四步：写 .jsx

布局先于动画。对每个 scene，先写最完整那一帧的静态布局，检查构图、层级、字号、视觉路径。布局成立后，再写动画。

按需读取：

- `references/jsx-format.md` — 文件结构、`meta` / `fonts` / `audio` 导出、`use*` 钩子、解析硬规则
- `references/animations.md` — 动画接口（`useGSAP`）
- `references/transitions.md` — 多场景转场
- `references/canvaskit.md` — `useCanvas`、`ly` 效果内核、render target
- `references/three.md` — `useThree` 三维场景
- `references/templates.md` — 起手骨架

GSAP、CanvasKit、three.js 是通用知识。参考文件只写 OpenCat 特有的接口和差异，不重复这三个库的用法。

`references/design-principles.md` 是「有质感」的判断核心，按需查它的分节：叙事顺序、设计身份、节奏规划、视频构图、字体、数据、动效、字幕、声音、转场、交付检查。

`scripts/font-discover.py` 是字体发现工具，用法见 `design-principles.md` 的「字体」一节。

## 修改已有 .jsx

1. 先读实际文件，不从记忆重建
2. 只改用户要求的区域
3. 需要格式能力时读对应 reference

## 硬规则

- **元素是静态的。** 元素只描述静止状态，不带动效。动效只来自 `useGSAP` 或每帧脚本。先在 class 里把最终构图摆好，再用 `useGSAP` 把它从起点动到这个状态。
- 只输出一个完整可渲染的 `.jsx` 文件。不默认启动预览、渲染或截图。
- 只有一个默认导出组件作为可视根。所有时间行为走 `use*` 钩子。
- 动画必须可 seek：写成对时间的纯函数。不用 wall-clock、异步回调、非种子随机。
- 用 `class`，不用 `style` / `className` / `parentId`。class 里不写动画类（`transition-*` / `animate-*` / `translate-*` 等，实测会被忽略）。
- **时间分两条时钟。** `ctx.global` 是整条合成，`ctx.current` 是钩子所属的 scene。在 `<tl>` 里写 `useGSAP`，`ctx.current.time` 自动是该 scene 的局部时间，不用手抄起点；单 scene 时两者相等。
- 多 scene 中，scene 内只做入场和呼吸；scene 间退出交给 `<transition>`。
- 每个 scene 至少有背景层、中景内容、前景细节。
- 所有 `id` 在整份合成内全局唯一。

## 质量门

交付前自检：

- 结构符合 `jsx-format.md`，动画符合 `animations.md`
- 用 canvas 时，ref 来自 `useRef()`，入口是 `useCanvas(ref, draw)`
- 每个 scene 有明确 concept、至少两个焦点、三层结构、持续微运动
- 字号、边框、装饰透明度按视频尺度处理，不照搬网页尺寸
- 动效符合 `design-principles.md` 的「动效」：ease 和时长有变化，入场 `.out`、退场 `.in`，没有全片统一的 delay
- 转场服务叙事，不是全片机械 crossfade
- `meta.duration` 等于所有 scene `duration` 加所有 `<transition>` `duration`

最后按 `design-principles.md` 的「交付检查」逐项核对：有设计文件时对照品牌核对颜色、字体、圆角、间距、禁用项；没有时核对配色一致性和默认味清单。
