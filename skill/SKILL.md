---
name: opencat-creator
description: 用 OpenCat XML 格式设计并生成视频合成。适用于创建或修改 OpenCat XML、规划多场景视频、设计标题卡/产品演示/品牌短片/社交广告、编排动画、CanvasKit 绘制、Subtree/RuntimeEffect 视觉、字幕和转场。输出以可渲染的 OpenCat XML 为准。
---

# OpenCat Creator

OpenCat XML 是视频的事实来源。你的目标不是复刻网页，也不是堆 API 示例，而是把用户意图转成一个结构清晰、审美成立、可由 OpenCat Rust/Web runtime 渲染的 XML 合成。

## 工作流

### 新建视频

#### 1. 意图

先明确或推断：
- **Message**：视频最终要让观众记住的一句话
- **Audience**：谁在看
- **Platform**：社交短视频、官网 hero、产品演示、发布预告
- **Duration**：时长决定 beat 数量，15 秒不要塞 6 个完整观点
- **Tone**：克制、精密、热烈、奢华、温暖、实验、戏剧化

如果用户只要小改，跳过这一步，直接改 XML。

#### 2. 设计方向

先读 `references/design-principles.md`，用它引导你形成设计方向。优先读取项目中的 `frame.md` → `design.md` → `DESIGN.md`，继承品牌颜色、字体、禁用项。

没有设计文件时，自己声明最小设计身份：背景色 / 前景色 / 强调色 / 字体倾向 / 圆角与边框语言 / 动效性格 / 禁用项。

开放式任务，形成设计方向后让用户确认再继续。

#### 3. 规划

每个 scene 是一个 beat。先规划节奏（哪些快、哪些慢、峰值在哪），再规划每个 beat 的 concept、mood、depth layers、motion verbs。详细指导见 `references/design-principles.md` 的 beat 设计和节奏规划部分。

#### 4. 构建 XML

设计方向确认后，按需读取格式参考：

- `references/opencat.md` — XML 结构、节点、属性、布局硬规则
- `references/animations.md` — 动画 API 和插件（需要写 `<script>` 时）
- `references/transitions.md` — 转场效果（多场景时）
- `references/canvaskit.md` — CanvasKit 子集（需要 canvas / Subtree / RuntimeEffect 时）
- `references/templates.md` — 经典模板和常用模式

布局先于动画：先写每个 scene 最可见时刻的静态 hero frame，再用脚本描述入场、呼吸和转场。

### 修改已有 XML

1. 先读取实际 XML，不从记忆重建
2. 只改用户要求的区域
3. 涉及格式能力时按需读取对应 reference

## 布局先于动画

对每个 scene，先找到最可见的一帧——所有关键元素已经进入、还没退出、构图最完整。

1. 用 XML 写这个静态 hero frame
2. 检查布局、层级、字号、密度、视觉路径
3. 再写 `<script>`，用 `ctx.fromTo()` / `ctx.timeline()` 从起点动画到 hero frame

不要把元素静态写在动画起点。静态 XML 应表达最终可读布局。

## 硬规则

- 只输出 OpenCat XML 或对现有 XML 的修改；不默认启动预览、渲染或截图
- `<opencat>` 只有一个可视根；`<script>` 最多一个且是 `<opencat>` 直接子节点
- 动画必须可 seek：不用 wall-clock、异步回调、非种子随机；优先 `ctx.fromTo()` 和 `ctx.timeline()`
- XML 中不使用 `style`、`className`、`parentId`；使用 OpenCat 支持的 XML 属性和 class
- 多 scene 中，scene 内只做入场和呼吸；scene 间退出交给 `<transition>` 承担
- 每个 scene 至少有背景层、中景内容、前景细节；画面要有密度和视觉路径

## 质量门

交付前自检：

- 结构符合 `opencat.md`
- 动画符合 `animations.md`
- 如果用了 canvas，入口是 `ctx.getCanvasById(id)`
- 每个 scene 有明确概念、至少两处焦点、三层结构和持续微运动
- 字号、边框、装饰透明度按视频尺度处理，不照搬网页尺寸
- 转场服务叙事；不是全片机械 crossfade
