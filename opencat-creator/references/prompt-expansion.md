# Prompt 扩展

每个 composition 运行。扩展不是为了拉长简短的 prompt — 而是将用户意图锚定到 `design.md` 和 `house-style.md`，产生一致的中间产物，让所有下游读取方式相同。

在 Step 1（设计系统）建立之后运行。扩展消费 design.md（如果存在），并输出引用其精确值的产物。

## 前置条件

生成前读取：

- `design.md`（如果存在）— 提取品牌色、情绪、约束。扩展引用这些精确值（Tailwind token）；不发明新值。
- [beat-direction.md](beat-direction.md) — 按场次规划格式。扩展使用此格式输出每个场景。
- [video-composition.md](video-composition.md) — 视频量级规则（密度、色彩呈现、缩放）。扩展自动应用这些规则。
- [house-style.md](../house-style.md) — 其背景层规则、色彩、动效、排版规则适用于每个场景。扩展写入符合规则的输出。

如果 `design.md` 还不存在，先运行 Step 1（设计系统）。没有设计上下文的扩展会产生通用场景分解。

## 为什么总是运行

**扩展从不是直通。** 每个用户 prompt — 无论多详细 — 都是一个**种子**。扩展的工作是将其丰富为一个完整的逐场景生产规范。

即使是一个详细的 7 场景 brief 也缺少扩展才能添加的东西：

- **每场景的氛围层**（house-style 要求的：径向发光、幽灵文字、强调线、杂色、主题装饰）— 用户 prompt 几乎从不列出这些；扩展添加它们。
- **每个装饰的次级动效** — breath、drift、pulse、orbit。没有环境动效的装饰感觉死板。
- **让场景感觉真实的微细节** — 刻度标记、标签、排版强调、背景网格图案。用户没想到要请求的东西。
- **对象级别的转场编排** — "crossfade" → "X 向外展开变成 Y"。具体的 duration、ease、morph 源/目标。
- **每个场景内的节奏拍点** — 紧张感在哪里建立、哪里停留让观众呼吸、强调词落在哪。
- **来自 design.md 的精确 easing 选择** — 留给场景子 agent 猜测的空间为零。

扩展在详细 prompt 上的工作不是总结或直通 — 而是**把用户写的内容变得更丰富**。用户的内容保留；氛围、环境动效和微细节叠加在上面。这就是使场景从"符合 brief"到"感觉活着"的区别。

单场景 composition 和简单修改是唯一的例外。

## 生成什么

扩展为一个完整的 production spec，包含以下部分：

1. **标题 + 风格块** — 引用 design.md 的精确 Tailwind token 和 mood。不要发明色板 — 引用 design.md 提供的。

2. **节奏声明** — 在任何场景细节之前命名场景节奏。例如：`hook-PUNCH-breathe-CTA` 或 `slow-build-BUILD-PEAK-breathe-CTA`。参见 [beat-direction.md](beat-direction.md) 的节奏模板。

3. **全局规则** — 视差层、微动效要求、转场风格。将 energy 匹配到 mood（calm → 慢 ease、high → 快 ease）。

4. **逐场景拍点** — 对每个场景，使用 beat-direction 格式：
   - **Concept** — 2-3 句的大想法。什么视觉世界？什么隐喻？观众应该**感觉**什么？
   - **Mood direction** — 文化/设计参考，不是色值。
   - **Depth layers** — BG（2-5 个装饰元素带环境动效）、MG（内容）、FG（强调、结构元素、微细节）。每场景 8-10 个元素。
   - **动效编排** — 每个元素的具体动词。High：SLAMS、CRASHES。Medium：CASCADE、SLIDES。Low：floats、types on、counts up。
   - **退场转场** — 具体 effect 和 duration。不是 "crossfade" 而是 "fade, 15 frames, ease-out"。

5. **复用视觉主题** — 跨场景的品牌色板视觉线索。

6. **负面清单** — 避免什么，由 design.md 的约束决定。

## 输出

将扩展后的 prompt 写入 `.opencat/expanded-prompt.md`。不要 dump 到聊天中 — 它可能有几百行。

告知用户：

> "我已将你的需求扩展为完整的制作方案。查看：`.opencat/expanded-prompt.md`
>
> 包含 [N] 个场景，共 [X] 秒（[Y] 帧@30fps）及具体视觉元素、转场和节奏。如有需要可修改，然后告诉我继续。"

在用户批准或说继续之前，不要进入构建阶段。
