---
name: opencat-creator
description: 用 OpenCat JSONL 格式创建视频合成、动画、标题卡片、叠加层、字幕、配音、音频响应式视觉和场景转场。在需要构建任何基于 JSONL 的视频内容、添加与音频同步的字幕/配音、创建音频响应式动画、添加文字高亮动画或添加场景转场时使用。涵盖合成编写、动画编排、转场、时间和媒体。
---

# OpenCat Creator

JSONL 是视频的事实来源。一个 composition 是一个 JSON Lines 文件，包含节点树定义、通过 `ctx.*()` API 脚本驱动的动画、以及 Tailwind 样式的类名。运行时解析 JSONL，构建场景树，并使用 Skia + Taffy + QuickJS 渲染帧。

**始终阅读** [references/opencat.md](references/opencat.md) — 它是格式、节点类型、动画 API、Canvas API、Node API 和样式的完整事实来源。本文档只描述工作流和规则。

---

## 方法

### 探索阶段

对于开放性请求（"做一个产品发布视频"），在用户确定方向之前了解意图：

- **受众** — 谁看？开发者？高管？普通消费者？
- **平台** — 在哪儿播放？社交媒体（15s）、网站主页、产品演示、内部？
- **优先级** — 什么最重要？动效质量？内容准确性？品牌忠实度？速度？
- **变体** — 用户想要多个选项还是一个最佳方案？

对于具体请求（"加一个标题卡片"），跳过探索阶段。

对于探索性请求，可提供 2-3 个有意义的差异方案 — 不同节奏、能量水平或结构方式。一个安全/常规，一个有野心。不强制。

### 步骤 1：设计系统

如果存在 `design.md` 或 `DESIGN.md`，先读取。它是品牌色、字体和约束的事实来源。使用精确值 — 不凭空创造颜色或替换字体。
如果没有 `design.md`：

1. **用户提到了风格或情绪？** → 阅读 [visual-styles.md](visual-styles.md) 查看 8 个命名预设。选最接近的匹配。
2. **想跳过直接开始？** → 问：情绪、亮/暗、品牌色/字体？从 [house-style.md](house-style.md) 选色板。

**design.md 定义品牌，不定义视频合成规则。** 视频规则来自 [references/video-composition.md](references/video-composition.md) 和 [house-style.md](house-style.md)。品牌色以视频适当的比例使用 — 不是 web-UI 的不透明度。

### 步骤 2：Prompt 扩展

始终在每个 composition 上运行（单场景作品和简单编辑除外）。此步骤将用户意图与 `design.md` 和 `house-style.md` 对齐，产生一致的中间产物，供下游所有环节读取。

阅读 [references/prompt-expansion.md](references/prompt-expansion.md) 获取完整流程和输出格式。

将扩展后的 prompt 写入 `.opencat/expanded-prompt.md`。

### 步骤 3：规划

写 JSONL 之前，思考：

1. **什么** — 叙事弧线、关键时刻、情感节奏。
2. **结构** — plain tree（单场景）还是 timeline（多场景+转场）？
3. **节奏** — 声明节奏模式（如 `fast-fast-SLOW-fast-TRANSITION-hold`）。多场景时阅读 [references/beat-direction.md](references/beat-direction.md)。
4. **时间** — 总帧数、fps、转场位置。
5. **布局** — 先构建 end-state。见"静态布局优先"。
6. **动画** — 然后用 `ctx.*()` API 添加动效。

**构建被请求的内容。** 每个场景、每个元素、每个 tween 应赢得它的位置。额外场景若确实能提升作品，**提议**它们 — 不直接添加。

对于小型修改（修复颜色、调整 timing），直接跳到规则。

<硬性关卡>
写 JSONL 前 — 确认有视觉身份（步骤 1）。`text-slate-900`、`bg-white`、`font-bold` 若无 design.md 参考，你跳过了它。
</硬性关卡>

---

## 静态布局优先

将每个元素放在其**最可见的时刻**的位置 — 完全进入、正确定位、尚未退出。先写成静态 JSONL + className。无动画脚本。

**为什么这很重要：** 如果你在动画起始状态（屏幕外、缩放为 0、opacity 0）定位元素并 tween 到你认为应该到达的位置，你是在猜测最终布局。重叠在视频渲染前不可见。先构建最终状态，你可以在添加任何动效之前看到并修复布局问题。

### 流程

1. **确定 hero frame** — 最多元素同时可见的时刻。
2. **写静态 JSONL** — 场景容器用 `className="flex flex-col w-full h-full p-[Npx]"` 填满画布。padding 推内容向内。`absolute` 定位仅用于装饰元素。
3. **入场用 `ctx.from()` / `ctx.fromTo()`** — `ctx.from()` 动画从给定值到属性默认值（opacity:1, x:0, y:0, scale:1）；引擎在 delay 期间自动写入起始值，无需额外 `ctx.set()`。
4. **退场用 `ctx.to()`** — 从 className 位置到屏幕外/不可见。
5. **`ctx.set()` 仅用于动画 prop 之外的初始姿态** — 禁止对将被 `from`/`fromTo`/`to` 动画的同一目标同一 prop 调用 `ctx.set()`。禁止对 `ctx.splitText(id)` 的父 id 调用 `ctx.set({opacity:0})`（CSS opacity 乘性叠加，会让所有子字符不可见）。
6. **首帧可见性规则** — 引擎在 from delay 期间会把元素预写为 fromVars。若场景首帧（含 transition 入场前）必须可见，则首屏元素的 from 入场禁用 `opacity:0`，仅用 `y` / `x` / `scale` / `scaleX` 等位移缩放入场；或将该入场 from 的 timeline 位置置 0 且确认 frame 0 的视觉可接受。

### 示例

```json
{"id":"scene1","parentId":null,"type":"div","className":"flex flex-col justify-center w-full h-full p-[120px] gap-[24px]","duration":90}
{"id":"title","parentId":"scene1","type":"text","className":"text-[80px] font-bold text-slate-900","text":"Hello World"}
```

```js
// 入场动画：from 自动写入起始值，无需 ctx.set
ctx.timeline({ defaults: { duration: 18, ease: 'spring.gentle' } })
  .from('title', { opacity: 0, y: 60 })
  .from('subtitle', { opacity: 0, y: 36 }, '-=8');
```

**错误 — 硬编码尺寸和绝对定位：**

```json
{"id":"scene1","parentId":null,"type":"div","className":"absolute top-[200px] left-[160px] w-[390px] h-[844px] flex flex-col","duration":90}
```

当内容高于剩余空间时，绝对定位的内容容器会溢出。用 flex + padding 代替。

### 元素跨时间共享空间

若 A 在 B 进入同区域前退出，两者都应有各自 hero frame 的正确 className 位置。timeline 排序保证不视觉共存 — 但跳过布局步骤不会发现意外重叠。

### 什么算有意重叠

分层效果（文字背后的辉光、阴影元素、背景图案）和 z 堆叠设计（卡片堆叠、深度层）是有意的。布局步骤是为了捕获**无意的**重叠 — 两个标题落在彼此之上、统计数字覆盖标签、内容溢出画面。

---

## Composition 结构

### plain tree（单场景）

```json
{"type":"composition","width":390,"height":844,"fps":30,"frames":60}
{"id":"scene1","parentId":null,"type":"div","className":"flex flex-col w-full h-full bg-white","duration":60}
{"id":"title","parentId":"scene1","type":"text","className":"text-[24px] font-bold","text":"Hello"}
{"type":"script","parentId":"scene1","src":"ctx.from('title',{opacity:0,y:30,duration:15,ease:'ease-out'});"}
```

### timeline（多场景 + 转场）

```json
{"type":"composition","width":390,"height":844,"fps":30,"frames":162}
{"id":"root","parentId":null,"type":"div","className":"relative w-[390px] h-[844px]"}
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene1","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-white","duration":60}
{"id":"scene2","parentId":"main-tl","type":"div","className":"flex flex-col w-full h-full bg-slate-900","duration":90}
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"fade","duration":12}
```

规则：
- `tl` 至少两个场景，每对相邻场景必须有 `transition`
- `tl` 无 `duration`，总长 = `sum(scene.duration) + sum(transition.duration)`
- `composition.frames` 与推导总长对齐

完整节点类型和字段见 [references/opencat.md §3](references/opencat.md#3-节点类型)。

---

## Video and Audio

视频和音频通过独立 JSONL 节点声明。运行时控制播放 — 不要在脚本中调用播放/暂停/跳转。

```json
{"id":"clip","parentId":"scene1","type":"video","className":"w-full h-full object-cover","path":"clip.mp4"}
{"id":"bgm","parentId":"root","type":"audio","path":"/tmp/bgm.mp3"}
```

规则：
- `video` 必须 `path` 指向本地文件
- `audio` 用 `path` 或 `url`，不能同时

---

## Timeline Contract

- 使用 `ctx.timeline()` 编排动画。timeline 在脚本中同步创建，运行时每帧采样
- 不要在 `async`/`await`、`setTimeout` 或 Promise 内构建 timeline — 运行时同步读取 `ctx.*()` 调用
- 场景 `duration` 来自节点的字段，不从 timeline 长度推导
- 不要创建空 tween 来设 duration

---

## 转场

1. **始终用转场。** 无跳切。

效果选择、能量/情绪匹配、叙事位置、GL 转场参数见 [references/transitions.md](references/transitions.md)。

---

## 规则（不可商量）

**动画冲突：** 不同时用多个 timeline 在同一元素动画同一属性。

**环境动效挂载：** 所有循环/环境动画（呼吸、漂浮、脉冲、辉光缩放）必须挂载到 `ctx.timeline()`，不能用独立的 `ctx.to()`。独立 tween 在非线性 seek（视频渲染）中不会正确拖拽，导致环境动效在渲染视频中消失。

```js
// 错误：游离在时间线之外，渲染中不工作
ctx.to("glow", { scale: 1.08, yoyo: true, repeat: 5, duration: 36 });

// 正确：挂载到时间线，确定性，可渲染
ctx.timeline().to("glow", { scale: 1.08, yoyo: true, repeat: 5, duration: 36 }, 0);
```

**Never do：**

1. 用 `document`、`window`、`requestAnimationFrame` — 只用 `ctx.getNode()`
2. 在 `className` 中用 CSS 动画类（`transition-*`、`animate-*`、`ease-*`）或 transform 类（`transform`、`translate-*`、`rotate-*`、`scale-*`）— 用节点 API 或 `ctx.*()` tween
3. `type: "div"` 上放 `text` 字段 — 仅 `type: "text"` 接受
4. 为 `tl` 设 `duration` — 从子场景推导
5. `tl` 中缺转场或场景少于 2
6. 颜色 tween 中用 Tailwind token — 用显式字面量（`#rrggbb`、`hsl(...)`）
7. 用 `for` 循环写入场 — 用 `stagger` 或函数值
8. `parentId` 指向不存在节点
9. 用视频当音频源 — `video` 始终无声，BGM/配音用独立 `audio` 节点
10. 直接在 `video` 节点上动画尺寸（`width`、`height`）— 在外层 `div` 上控制
11. 在脚本中调 `play()`/`pause()`/`seek()` — 运行时拥有播放权
12. 用 `<br>` 或 `\n` 强制作文本换行 — 用 `max-width` 让文本自然换行。例外：短标题刻意逐行显示

---

## 动画护栏

### 场景三段结构

每个场景有三个阶段。不要把所有动效堆在建场阶段。

- **建场（0-30%）** — 元素交错入场。不要一次全倾倒。用 stagger 或位置参数错开。
- **呼吸（30-70%）** — 内容可见，由一个环境动效赋予生命力（呼吸、漂浮、脉冲）。
- **收场（70-100%）** — 退场或决定性结束。退场比入场快。多场景时转场即收场。

### 护栏规则

- 首动偏移 3-9 帧（非 t=0）
- 按重要性顺序入场，而非 DOM 顺序。最先运动的元素被认为最重要
- 入场比退场长（12 帧出现、7-8 帧消失）
- 重叠入场，不要等上一个完成。总交错序列不超过 15 帧
- 每场景入场用 3+ 种不同 easing
- 不同场景变化入场方向（不都是 `{y:30, opacity:0}`）
- 暗背景避免全屏线性渐变（H.264 条带 — 用径向或实色+局部发光）
- 视频字号：60px+ 标题、20px+ 正文、16px+ 标签
- `font-variant-numeric: tabular-nums` 用于数字列

无 design.md 时遵循 [house-style.md](house-style.md)。

---

## 排版和资源

OpenCat 使用 Tailwind 字体体系，不涉及 `@font-face`。

- **字体：** 通过 Tailwind token 控制 `text-[size]`、`font-bold`、`font-mono` 等
- **文本溢出：** 对动态内容用 `truncate` 或设 `max-w-[Npx]` 让文本自然换行。不用 `<br>` 强制换行
- **图标：** 用 `type: "icon"` + Lucide kebab-case 名称，通过 `stroke-*`/`fill-*` 控制样式
- **图片：** 用 `type: "image"`，支持 `path`/`url`/`query`。query 用 1-4 个名词
- **文件组织：** JSONL 及其引用的 `.js`、`.srt`、媒体文件放在同一项目目录。`path` 相对 JSONL 位置解析

---

## 编辑已有 composition

- **读实际文件，不猜。** JSONL 就是 spec — 提取精确值。
- 匹配现有字体、颜色、动画模式。
- 只改被请求的，保留不相关 clip 的 timing。

---

## 输出检查清单

**快速检查（立即运行，阻塞结果）：**

- [ ] JSONL 语法有效（每行一个 JSON 对象，无注释）
- [ ] 遵守 `design.md` 颜色、字体和约束（若存在）
- [ ] 每对相邻场景有转场
- [ ] 每场景有入场动画
- [ ] 末场景外无退场动画
- [ ] `composition.frames` = `sum(scene.duration) + sum(transition.duration)`（运行时不做此校验，写错会静默截断）
- [ ] className 无 CSS 动画/transform 类
- [ ] tween 颜色用显式字面量
- [ ] 确定性随机（`ctx.utils.random` 带 seed）
- [ ] 所有 `parentId` 引用已存在的节点

**慢速检查（并行运行，同步给用户预览）：**

- [ ] 字号符视频缩放要求（60px+ 标题、20px+ 正文、16px+ 标签）
- [ ] 每场景 8-10 元素（含装饰），装饰有环境动画
- [ ] 对比度问题已处理（见质量检查）
- [ ] 动画编排已验证（见质量检查）
- [ ] 遵循 [references/video-composition.md](references/video-composition.md)（密度、色彩、构图）

---

## 质量检查

### 设计忠实度

有 design.md：检查颜色、排版、转角、间距均在 design.md 范围内，无避免规则违反。

1. **颜色** — composition 中每个 className 中的颜色值（Tailwind token 如 `text-slate-900`、`bg-white`，或任意值如 `text-[#1a2b3c]`）都出现在 design.md 的色板中。标记任何发明的颜色。
2. **排版** — 字体相关 token（`font-bold`、`text-[24px]` 等）匹配 design.md 的排版规范。无替代。
3. **转角** — `rounded-*` 值匹配声明的圆角风格（如指定）。
4. **间距** — `p-*`/`gap-*` 值在声明的密度范围内（如指定）。
5. **深度** — `shadow-*` 使用匹配声明的深度级别（如指定：flat = 无，subtle = 轻，layered = 辉光）。
6. **避免规则** — 如果 design.md 有列出要避免的内容的章节（常见"What NOT to Do"、"Don'ts"、"Anti-patterns"），验证无一存在。

报告违规为清单。修复后再交付。

无 design.md：检查色板一致性、无 house-style "需要警惕的默认项"。

### 动画编排

逐脚本扫描 JSONL 中的 `ctx.*()` 调用和 CanvasKit 绘制调用，验证编排质量。

**Per-tween 摘要：** 对每个 script 节点，提取所有 `ctx.to()`/`ctx.from()`/`ctx.fromTo()`/`ctx.set()` 调用，记录：
- 目标节点 ID
- 动画属性（opacity、x、y、scale、rotation、color 等）
- duration（帧）
- easing
- position 参数（绝对帧或相对偏移）
- 是否 stagger 及数值

**CanvasKit 动效：** 对 `type: "canvas"` 的 script，扫描 `canvas.draw*()` 和 `paint.set*()` 调用，识别：
- 进度驱动的绘制（如 `ctx.frame / totalFrames` 控制 path 绘制进度）
- 颜色/透明度随帧变化
- 几何变换（`canvas.translate()`、`canvas.rotate()`、`canvas.scale()`）

**验证清单：**

- [ ] **easing 多样性** — 每场景 3+ 种 easing，不全用 `ease-out`
- [ ] **入场方向变化** — 不同场景变换方向（不都是 `{y:30, opacity:0}`）
- [ ] **stagger 节奏** — 跨场景变化 stagger 值，不重复相同间隔
- [ ] **无退场动画** — timeline 中无 `ctx.to(..., { opacity: 0 })`（末场景除外）
- [ ] **首动偏移** — 每场景首个 tween 偏移 3-9 帧（非 t=0）
- [ ] **Dead zones** — 场景中无超过 30 帧（1 秒）的无动画间隙。有意停留需标注
- [ ] **动画冲突** — 同一节点同一属性不被多个 timeline 同时驱动
- [ ] **stagger 与元素数匹配** — stagger × 元素数 < 场景 duration（否则末尾元素入场被截断）
- [ ] **CanvasKit 进度连续** — canvas 脚本中的进度变量（如 `ctx.frame / N`）覆盖完整帧范围，无跳跃

**场景节奏验证：** 对多场景 composition，绘制 ASCII 时间线：

```
scene1 [====入场====|====停留====|]  转场 [====入场====|====停留====|]
       0           30           60   60-72         102          162
```

检查：
- 入场 + 停留 + 转场重叠 ≤ 场景 duration
- 转场 duration 匹配能量级别（平静 15-24f、中等 9-15f、高能 5-9f）
- 相邻场景的转场 effect 不重复（选一个主要 + 1-2 个强调）

### 时间检查

总帧数/fps = 预期时长。转场 duration 适合能量水平。场景 duration 容纳入场 + 停留 + 转场重叠。

### 对比度

文字在背景上必须可读。手动检查关键帧：

- 暗背景：文字色需要 4.5:1（正常文字）或 3:1（24px+ 或 19px+ 粗体）
- 亮背景：加深文字色直到通过
- 保持在色板范围内 — 不发明新颜色，调整现有颜色
- 特别注意半透明叠加层上的文字

### 视觉检查

渲染后手动检查：布局溢出、文字可读性、元素重叠、动效节奏。

---

## 参考

**始终阅读：**

- **[references/opencat.md](references/opencat.md)** — 格式、节点类型、动画 API、Canvas API、Node API、样式、常见错误。
- **[references/video-composition.md](references/video-composition.md)** — 密度、色彩呈现、缩放、帧构图。覆盖 Web UI 本能。
- **[references/motion-principles.md](references/motion-principles.md)** — 动效编排原则：easing 情绪、速度与重量、场景三段结构、编排层级、动效不对称性、图片动效处理。
- **[references/typography.md](references/typography.md)** — 字体配对、OpenType 特性、暗背景调整、字体发现脚本。

**每 composition 运行：**

- **[references/prompt-expansion.md](references/prompt-expansion.md)** — 扩展为完整制作 spec。消费 design.md、house-style、beat-direction、video-composition。

**无 design.md 时读取：**

- **[house-style.md](house-style.md)** — 默认方向：色彩、背景层、动效、排版。
- **[visual-styles.md](visual-styles.md)** — 8 个命名风格，含 Tailwind 色板方向、推荐转场。

**多场景 composition 读取：**

- **[references/beat-direction.md](references/beat-direction.md)** — 拍点规划、节奏模板、转场选择。
- **[references/transitions.md](references/transitions.md)** — 转场效果、能量/情绪选择、速度匹配预设、模糊强度。
  - [references/gl-transitions.md](references/gl-transitions.md) — 全部 GL 转场效果名和参数。

**按需读取：**

- **[references/techniques.md](references/techniques.md)** — 9 种视觉技术：SVG 路径绘制、Canvas 2D、3D 变换、逐词排版、视频合成、打字效果、速度匹配过渡、音频响应、路径动画。规划具体技术时读取。
- **[references/captions.md](references/captions.md)** — 字幕风格检测、逐词样式、词组分组和定位、文本溢出预防、字幕退出保证。添加字幕时读取。
- **[references/dynamic-techniques.md](references/dynamic-techniques.md)** — 动态字幕技术：Karaoke 逐词高亮、音频响应字幕、按能量级别选择技术。需要动态字幕效果时读取。
- **[references/text-highlight.md](references/text-highlight.md)** — 文字高亮动效：标记笔划、手绘圆圈、辐射爆发线、波浪下划线、十字删除线。添加文字强调时读取。
- **[references/audio-reactive.md](references/audio-reactive.md)** — 音频响应动画：频段数据格式、音频到视觉映射、采样模式、确定性约束。视觉需要响应音频时读取。
- **[references/narration.md](references/narration.md)** — 旁白脚本指导：节奏、语气、数字发音、结构、开场白模式。编写旁白脚本时读取。
- **[references/tts.md](references/tts.md)** — 文字转语音：外部 TTS 工具、声音选择、语速调节、TTS+字幕工作流。生成配音时读取。
- **[references/transcript-guide.md](references/transcript-guide.md)** — 转录指南：whisper 模型选择、外部 API、质量检查、决策树。需要转录时读取。
- **[patterns.md](patterns.md)** — 常用 composition 模式：画中画、标题卡、幻灯片。需要参考模式模板时读取。
- **[data-in-motion.md](data-in-motion.md)** — 数据可视化规则：视觉连续性、数字权重、避免 Web 模式。包含数据/统计时读取。
