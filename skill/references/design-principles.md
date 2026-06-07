# 设计原则

这份文件定义视频设计的判断原则，不涉及 XML 格式或 API。

目标不是复刻网页，也不是堆示例。先设计观看体验，再把它翻译成具体的格式落地。

---

## 1. 从意图开始

先问清或自行推断：

- **Message**：视频最终要让观众记住的一句话
- **Audience**：开发者、消费者、投资人、内部团队，谁在看
- **Platform**：社交短视频、官网 hero、产品演示、发布预告、数据故事
- **Duration**：时长决定 beat 数量。15 秒不要塞 6 个完整观点
- **Tone**：克制、精密、热烈、奢华、温暖、实验、戏剧化

如果用户只要小改，跳过这一步。

---

## 2. 设计身份

设计身份先于一切。优先读取项目内的 `frame.md` → `design.md` → `DESIGN.md`。

设计文件定义品牌，不定义视频构图。严格继承：颜色、字体、圆角、禁用项、品牌情绪和视觉约束。

为视频重新判断：字号、边框、装饰透明度、背景密度、动效强度。Web UI 里的 1px 边框、14px 正文、5% 透明装饰在视频里通常不可见。

没有设计文件时，自己声明最小设计身份：

```text
背景色 / 前景色 / 主强调色 / 辅助色
标题字体倾向 / 正文字体倾向
圆角与边框语言
动效性格：snappy / cinematic / editorial / playful / mechanical
禁用项：例如不要霓虹、不要卡片海、不要单色渐变
```

避免默认设计味：

- 紫蓝渐变、cyan-on-dark、渐变文字
- 每个 scene 都是居中标题 + 副标题
- 同尺寸卡片网格
- 纯黑纯白、死灰中性色
- 背景空到像没加载

---

## 3. 风格原型

风格不是模板，而是判断画面应该如何说话。从这些原型里选一个，再按项目品牌改写。

| 原型 | 气质 | 构图 | 运动 | 适合转场 |
| --- | --- | --- | --- | --- |
| Precise System | 精密、理性、数据驱动 | 网格、坐标、注册线、大数字 | snap、lock、count、calibrate | wipe、slide |
| Premium Editorial | 高级、克制、发布会感 | 大留白、低密度、强层级 | glide、fade、slow reveal | fade、light_leak |
| Industrial Raw | 粗粝、攻击性、反建制 | 破格排版、斜切、边缘溢出 | slam、shatter、scramble、step | GlitchDisplace、hard cut |
| Maximal Type | 高能、发布、口号驱动 | 字就是画面，50-80% 画面被 type 占据 | punch、scale、stack、wipe | hard cut、burn、slide |
| Data Immersive | 未来感、AI、流体数据 | 粒子、光迹、宏观/微观尺度切换 | drift、coalesce、morph、orbit | iris、perlin |
| Warm Human | 亲近、叙事、生活方式 | 近景、柔和色块、手工质感 | breathe、float、soft reveal | fade、Dreamy、light_leak |
| Cultural Vivid | 消费、社区、节庆 | 图案、重复、饱和色块 | bounce、pop、spin、burst | Swirl、ripple、wipe |
| Dark Cinematic | 悬疑、安全、戏剧化 | 黑场、强对比、窄光、遮挡 | emerge、creep、cut、snap | crosswarp、iris、hard cut |

选择风格时先匹配情绪，再匹配行业。技术产品不一定要冷蓝，奢侈感不一定要 serif，温暖也不等于米色铺满。

---

## 4. 把 Beat 当成一个世界

每个 scene 是一个 beat。Beat 不是布局清单，而是观众体验。

弱描述：
```text
左边标题，右边三张卡片，背景深色。
```

强描述：
```text
画面像一块被点亮的控制台。左侧标题像系统状态锁定一样进入，右侧三张卡片依次校准，背景细线和坐标标记缓慢漂移，让这个功能看起来正在运行。
```

每个 beat 至少写清：

- **Concept**：这个画面的核心隐喻是什么
- **Mood**：文化/视觉参考，不只是颜色
- **Depth**：BG / MG / FG 三层分别有什么
- **Motion verbs**：每个重要元素如何运动
- **Transition**：它如何把观众交给下一幕

动词比"动画效果"更重要：

- Impact：SLAMS、DROPS、PUNCHES、SHATTERS
- Directional：SLIDES、PUSHES、WIPES、PULLS
- Reveals：DRAWS、FILLS、GROWS、ASSEMBLES、COUNTS UP
- Organic：FLOATS、DRIFTS、BREATHES、PULSES、ORBITS、MORPHS
- Mechanical：TYPES ON、CLICKS、LOCKS IN、SNAPS、STEPS

如果一个元素没有动词，它很可能还没被设计好。

---

## 5. 节奏规划

先声明整片节奏，再进入实现：

```text
hook-hit -> hold -> feature build -> shader peak -> CTA settle
rapid-fire -> rapid-fire -> slow hero -> proof -> hard CTA
slow reveal -> data lock -> product pass -> editorial hold
```

节奏由品牌和内容决定，不只由时长决定。同样 15 秒，建筑事务所可能需要 slow-reveal-hold，游戏发布可能需要 rapid-fire-slam。

判断：

- 哪一幕是情绪峰值，强转场只应服务它
- 哪一句文案最重，画面能量应在那里最高
- 快节奏不是每幕都短，而是快慢对比明确
- 15 秒通常承载 3-5 个 beat；30 秒通常承载 5-8 个 beat
- 如果每个 scene 都同样强、同样快，观众只会记住噪音

---

## 6. 视频构图

视频帧不是网页。这些规则不受品牌、风格或设计文件影响。

### 密度

一个 beat 只有 3 个元素看起来空。8-10 个元素才有生命力。

每个 scene 需要：

- **背景纹理**：径向光晕、ghost type、色块、噪点、网格。绝不要纯平色
- **中景内容**：卡片、数据、代码块、图片——实际要传达的信息
- **前景细节**：分割线、标签、数据条、注册线、等宽元数据。让画面感觉是"制作的"

### 尺度

| 元素 | 建议 |
| --- | --- |
| 标题 | 64-120px |
| 正文 | 28-42px |
| 标签 | 18-24px |
| 装饰透明度 | 12-25% |
| 边框 | 2-4px |
| 大画面 padding | 60-140px |

如果字号低于 24px，需要有理由。如果装饰透明度低于 10%，它不可见。

### 色彩

- 品牌强调色必须可见——不是 5% 透明度的光晕。大气效果 12-25%，焦点元素全饱和
- 中性色要向品牌色相偏移。死灰色 = 没设计
- 暗色背景上浅色文字视觉上更粗，正文不要过重

### 构图

- 至少两个焦点。眼睛要有移动路线
- Hero 文字占画面 60-80% 宽度
- 锚定到边缘。居中浮动是网页模式
- 分区构图：左右分区、顶部 metadata + 主内容区、斜向视觉路径
- 结构元素：分割线、标签、坐标、边框面板——它们让画面可读且便于动画

---

## 7. 字体即时间

字体选择先回答"谁在说话"，再回答"好不好看"。

- 不要配两个相似的 sans；要么同一字体做权重/宽度对比，要么跨类别对比
- 一幕最多一个表达型字体，另一个字体应退让
- 视频需要极端 weight contrast：300 vs 900 往往比 400 vs 700 更清楚
- 时间就是层级：先出现的元素比位置更能决定重要性
- Motion 也是 typography：同一个词 0.1s slam 和 1.2s fade 是两种语气
- 3 秒展示必须 2 秒内读完；字少、字大、断句明确
- 数据和金额按 tabular 思维处理，数位要稳定对齐

排版不要只靠字号区分。可混合：权重（light body + black headline）、宽度（condensed headline + mono data）、语气（人文标题 + 精密标签）、时间（先给关键词，再让说明补充进入）。

---

## 8. 数据视频

数据视频的核心不是"画图"，而是让数字有重量。

- 每个重要数字配一个视觉承载物：条、环、形状、色块、空间位置或粒子量
- 同一概念的连续 stats 保持同一视觉空间，只变数值和局部强调
- aesthetic change 表示新概念，不要每换一个数字就换一种版式
- 2-3 个相关指标可以并列；6-panel dashboard 是网页，不是视频
- 数字 count up 要有明确结束帧，最终值必须稳定可读

避免：pie chart、多轴图、复杂 legend、密集 gridline、直接复刻 chart library 输出、只把大数字放在空背景上。

---

## 9. 动效编排

每个 scene 有三个阶段：

- **Build 0-30%**：元素进入，按重要性 stagger，不按 DOM 顺序机械进入
- **Breathe 30-70%**：内容可读，背景和装饰有轻微持续运动
- **Resolve 70-100%**：收束或交给转场

原则：

- 入场通常 `.out`，退场通常 `.in`，位置变换通常 `.inOut`
- 同一 scene 不要所有 tween 都是同一个 ease、同一个方向、同一个 duration
- 最慢动作约为最快动作的 2-3 倍，层级自然出现
- 开场首个动作延迟 0.1-0.3s，避免像突然跳出

常用时长：

```text
0.15-0.30s  快速、敲击、机械反馈
0.30-0.50s  专业、利落、常规入场
0.50-0.80s  有重量、值得注视
0.80s+      氛围、漂移、镜头感
```

单调动效反模式：

- 所有元素都从下方 fade up
- 所有 scene 都 crossfade
- 所有元素同一个 delay 间隔
- 所有背景都只是静态渐变
- 每个镜头都追求"酷"，没有 hold 和可读时间

---

## 10. 转场判断

转场说明两个 beat 的关系：

- **Crossfade / fade**：关系连续，语气克制
- **Push / wipe / slide**：内容在同一空间里推进
- **Zoom / blur**：镜头穿越或注意力切换
- **Hard cut**：强调、反差、节奏点
- **Shader / RuntimeEffect**：hero reveal、品牌 moment、视觉高潮

不要每一幕都用最强转场。强转场通常只放在峰值或 CTA 前。

---

## 11. 交付检查

- Message 是否清晰，是否只有一个主张
- 是否先有设计身份，再有格式落地
- 每个 scene 是否有 concept，而不只是布局
- 每个重要元素是否有 motion verb
- 是否有三层结构、两个焦点、足够密度
- 背景是否有生命，装饰是否有微运动
- 字号、边框、透明度是否是视频尺度
- 文字是否能在展示时间内读完
- 数据是否有视觉重量，而不只是大数字
- 动效是否可 seek、确定、两端明确
- 转场是否服务叙事，而不是机械套用
