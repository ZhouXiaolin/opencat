---
name: opencat-creator
description: 用 OpenCat XML 格式设计并生成视频：品牌视觉、分镜脚本、动画编排、字幕、音频、转场和可交给 OpenCat/Web runtime 使用的 XML。适用于创建或修改 OpenCat 视频合成、标题卡、产品演示、品牌短片、社交广告、数据动效和程序化动画。
---

# OpenCat Creator

XML 是 OpenCat 视频的事实来源。目标不是产出 HTML/Studio 项目，也不是启动本地渲染或视频文件生成，而是产出一个结构清晰、符合 OpenCat 语法、可交给 Rust 或 Web runtime 使用的 XML 视频设计。

## 总流程

```
意图
  -> Step 0: 策略简报 VIDEO_BRIEF.md
  -> Step 1: 设计系统 design.md
  -> Step 2: 扩展方案 expanded-prompt.md
  -> Step 3: 分镜/脚本 STORYBOARD.md / SCRIPT.md
  -> Step 4: OpenCat XML
  -> Step 5: 交付 XML
```

**最终交付物是 XML。** 默认输出 `index.xml`；如果用户指定文件名或在编辑现有合成，则使用指定 XML。

**简单任务可裁剪流程。** 标题卡、小修、改色、调 timing、修已有 XML 时，直接读取现有 XML 和相关引用文件，不强行生成所有中间文档。多场景、品牌视频、产品演示、社交广告走完整流程。

**自主模式直接推进。** 用户说“你决定”“直接做”“surprise me”时，可以自行决定风格、TTS、节奏、字幕等偏好，并直接产出 XML。不要因为偏好问题反复停下来。

## 渐进式加载

| 阶段 | 读取文件 | 何时读取 |
|------|---------|---------|
| 策略 | `references/strategy-brief.md` | 多场景或开放式需求的 Step 0 |
| 设计 | 项目根目录 `design.md` 或 `DESIGN.md` | Step 1 |
| 扩展 | `references/prompt-expansion.md`, `references/beat-direction.md`, `references/video-composition.md`, `references/motion-principles.md` 的设计原则 | Step 2 |
| 分镜 | `references/storyboard.md`, `references/techniques.md`, `references/text-animations.md` | Step 3 |
| XML 实现 | `references/xml-build.md`, `references/opencat.md`, `references/transitions.md`, `references/animations.md`, `references/canvaskit.md` | Step 4 |
| 专项能力 | `references/captions.md`, `references/dynamic-techniques.md`, `references/data-in-motion.md`, `references/audio-reactive.md`, `references/patterns.md` | 只有对应能力被使用时 |

只读当前阶段需要的文件。不要为了“保险”一次性加载所有 references。

## Step 0: 策略简报

开放式视频需求先读 [references/strategy-brief.md](references/strategy-brief.md)，锁定：

- **Message** — 视频只允许有一个必须传达的核心句子
- **Narrative arc** — Problem->Solution / Reveal / Demonstration / Vibe / Comparison / 自定义
- **Audience + platform** — 谁看、在哪看、横竖屏和时长
- **Video type** — 社交广告 / 产品演示 / 品牌短片 / 发布预告 / 功能公告
- **Narration** — 旁白 / 无旁白 / 极简旁白

产出 `VIDEO_BRIEF.md`。如果用户 prompt 已经给足信息，直接写入简报并继续，不重复提问。

## Step 1: 设计系统

项目默认有 `design.md` 或 `DESIGN.md`。先读取它（Linux 区分大小写）。使用其精确颜色、字体、圆角、间距、禁用项；不要发明色值或替换字体。

如果项目确实没有设计文件，暂停并让用户提供或确认一份最小 `design.md`。不要从本地预设中推导颜色。写 XML 前必须有明确设计文件。

## Step 2: Prompt 扩展

多场景或非平凡视频必须读 [references/prompt-expansion.md](references/prompt-expansion.md)，产出 `expanded-prompt.md`。扩展不是复述用户需求，而是把需求变成逐场景 production spec：

- 精确引用 `design.md` token
- 声明节奏模式
- 为每个场景补足背景层、中景内容、前景细节
- 为每个元素指定动效动词
- 指定转场意图、能量峰值和负面清单

单场景和小修可以跳过。

## Step 3: 分镜与脚本

多场景视频必须读 [references/storyboard.md](references/storyboard.md)，产出：

- `STORYBOARD.md` — 概念、节奏、场景时长、镜头类型、技法、资产、转场和 SFX/字幕规划
- `SCRIPT.md` — 仅当有旁白或字幕脚本时生成

分镜顺序固定为：**message -> narrative arc -> beats -> assets/techniques**。不要从“有哪些截图/素材”倒推成 slideshow。

## Step 4: 生成 OpenCat XML

读 [references/xml-build.md](references/xml-build.md) 和 [references/opencat.md](references/opencat.md)。实现时遵守：

- **单文件 XML**：OpenCat 没有子 composition；多场景放在一个 `<tl id="main-tl">` 里。
- **单可视根**：`<opencat>` 只能有一个可视根。多场景推荐 root `<div>` 包住 `<tl>`、字幕和叠加层。
- **布局先于动画**：先把每场景最可见时刻的静态布局写对，再用 `<script>` 里的 `ctx.fromTo()` / timeline 加动效。
- **视频构图优先**：场景是 shot，不是网页 section；每场景至少 2 个焦点、3 层结构、足够密度和持续相机式运动。
- **转场即退出**：多场景中，除最终场景外不要给场景元素写退场动画；`<transition>` 承担场景交接。
- **确定性**：不使用 wall-clock、非种子随机、异步 timeline 构建；所有动效由 `ctx.timeline()` / `ctx.fromTo()` 等脚本控制。

### XML 解析器硬规则

完整速查见 [references/opencat.md](references/opencat.md)。最少不能违反：

- `<opencat>` 只有一个可视根
- `<script>` 最多一个、必须是 `<opencat>` 直接子节点、无属性、非自闭合
- 禁止 `className` / `parentId` / `style`
- 未知属性会直接报错
- `<audio>` 必须在 `<soundtrack>` 内，`attach` 引用 `<tl>` id 或该时间线内的场景 id
- `<transition>` 必须在 `<tl>` 内，`from`/`to` 是直接相邻子场景，`duration > 0`
- 数字属性必须是 ASCII 合法数字，不能有空白、`+`、全角数字

### 布局约束

- OpenCat 使用 Taffy，`div` 默认 `display: block`；需要 flex/grid 必须显式写 `flex` / `grid`。
- 优先用 flex/grid 和 gap/padding 组织内容，避免把整个画面写成散落 absolute 坐标。
- `absolute` 必须带明确定位：`inset-0`、`top-[Npx] left-[Npx]`、`right-[Npx] bottom-[Npx]` 等。裸 `absolute` 是 bug。
- 不使用 CSS 动画/transform 类；动画、transform、颜色变化都在 `<script>` 里完成。

## Step 5: 交付 XML

交付阶段只提供 XML 和必要说明，不运行本地渲染、桌面预览或视频文件生成工具。OpenCat 可能在 Web runtime 中运行，所以 skill 不能假设本地原生运行环境存在。

交付前做静态自检即可：

- XML 结构符合 `opencat.md` 的硬规则
- 场景、时长、转场与 `STORYBOARD.md` 对齐
- 颜色、字体、圆角、间距与 `design.md` 对齐
- 没有明显的网页化构图、tiny text、裸 `absolute` 或多视觉根

**不要自动运行本地渲染、桌面预览、视频文件生成或截图流程。**

## 编辑现有 XML

- 先读取实际 XML，不要从记忆重建颜色、字体、缓动和时间。
- 只改用户要求的部分，保留无关场景 timing。
- 如果现有 XML 违反硬规则但不影响本次请求，先说明风险；只有为完成任务必要时才修。

## XML 静态自检清单

### 解析器必检

- [ ] `<opencat>` 只有一个可视根
- [ ] `<script>` 只有一个、直接子、无属性、非自闭合
- [ ] 无 `className` / `parentId` / `style`
- [ ] 无未知属性
- [ ] 数字属性合法
- [ ] `<soundtrack>` / `<audio>` 结构合法，`attach` 真实存在
- [ ] `<tl>` 至少 2 个直接子场景；每个场景有 `duration`
- [ ] 每对相邻场景有且仅有一个 `<transition>`
- [ ] `<opencat duration>` 与场景+转场总长对齐

### 视觉质量必检

- [ ] 有明确 message、arc、audience
- [ ] 遵守 `design.md`
- [ ] 每场景有入场动画
- [ ] 末场景外无退场动画
- [ ] 每场景 8-10 个视觉元素，至少 3 层结构
- [ ] 每场景 2+ 焦点，背景不为空
- [ ] 字号符合视频尺度：标题 64-120px、正文 28-42px、标签 18-24px；更小必须有理由
- [ ] 动效编排有变化：缓动、速度、方向、stagger 不单一
- [ ] 转场类型服务叙事，不是全片无脑 fade

### 事实准确性

涉及科学、数学、金融、地图、时间轴等事实内容时，几何参数、坐标、比例、轨迹、数值必须复核；用边界条件检查公式方向，不靠目测。
