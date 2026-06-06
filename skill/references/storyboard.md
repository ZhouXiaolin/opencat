# Storyboard + Script

OpenCat XML 写得好不好，取决于分镜是否清楚。分镜是 creative north star，不是布局草图。

## 核心顺序

```
message -> narrative arc -> beats that serve the arc -> assets + techniques -> XML
```

不要从素材清单倒推：“有几张截图，所以每张截图飞进来一次”。如果素材服务 message，就用；否则不用。产品 UI 截图不能作为默认主视觉，除非这个 beat 的概念就是展示该截图本身。

## STORYBOARD.md 结构

```markdown
# Storyboard

## Concept

**Message:** ...
**Arc:** ...
**Audience:** ...
**Brand Voice:** ...
**Why this matters now:** ...

## Global Direction

**Format:** 1920x1080
**Duration:** 18s
**Audio:** narration / music / sfx / none
**Style Basis:** design.md
**Pacing:** Fast / Moderate / Slow / Arc
**Rhythm:** hook-PUNCH-hold-CTA

## Asset Plan

[列出用户给的或项目中已有的素材；逐项 USE/SKIP，说明原因]

## Beat Timing

| Beat | Time | Duration | Transition Out |
|------|------|----------|----------------|
| B1 Hook | 0.0-3.2 | 3.2s | slide 0.3s |

## Beats

### Beat 1 — Hook

**Shot:** Close-up / Medium / Wide / Extreme close-up / Over-the-shoulder / Dutch angle
**Concept:** ...
**Mood:** ...
**Message role:** ...
**Depth layers:** BG / MG / FG
**Camera move:** dolly in / pull-back / parallax pan / orbit / rack focus
**Techniques:** 2-4 concrete techniques
**Text effects:** named effects from text-animations.md if text animates
**Assets:** USE paths or NONE
**Animation choreography:** every element gets a verb
**SFX / Captions:** exact cue if any
**Transition out:** effect + duration + timing + reason
```

如果有旁白，同时写 `SCRIPT.md`：

```markdown
# Script

## Voice Direction

[语气、速度、停顿]

## Lines

| Beat | Time | Narration |
|------|------|-----------|
| B1 | 0.0-3.2 | ... |
```

## Pacing

| User signal | Pacing | Beat count | Beat duration | XML architecture |
|-------------|--------|------------|---------------|------------------|
| fast, punchy, social | Fast | 6-12 | 0.7-2s | one `<tl>` with short scenes |
| demo, walkthrough | Moderate | 4-6 | 3-6s | one `<tl>` with clear transitions |
| cinematic, premium, slow | Slow | 3-4 | 5-8s | longer scenes + slower transitions |
| launch, story, arc | Arc | 5-7 | varied | slow opener -> build -> peak -> CTA |

## Technique checklist

每个 beat 必须命名 2-4 个技法。一个技法通常不够，会像静态 slide。

可选来源：

- `techniques.md` — SVG path, Canvas, kinetic type, typing, counters, path motion, shaders 等
- `text-animations.md` — 命名文字动效和高亮
- `canvaskit.md` — 自定义绘制、subtree texture、SkSL
- `data-in-motion.md` — 数据场景
- `audio-reactive.md` — 音频响应
- `captions.md` / `dynamic-techniques.md` — 字幕

## Asset plan

如果项目中有素材目录、截图、Logo、插画、产品图或 capture 结果，先盘点再写 beats：

```markdown
Asset: assets/logo.svg
  Description: ...
  USE/SKIP: USE in B1/B5 because ...
```

至少一个关键 beat 应该使用品牌签名资产（Logo 之外的 hero illustration、产品视觉、图形系统、真实截图等），除非用户明确要求纯图形/纯文字。

## Shot discipline

每个 beat 是镜头，不是网页布局：

- 多用 close-up / medium；wide 只用于建立全貌
- 每个 beat 至少有一个 camera-style move
- settled hold 超过 1.5s 必须有连续相机运动、视差或新元素进入
- 不要默认浏览器 chrome、导航栏、侧边栏、tooltip、居中卡片

## Transition discipline

OpenCat 多场景必须用 `<transition>`。分镜里要说明每个转场为什么存在：

- `fade`：相关想法延续
- `slide` / `wipe`：方向性推进或结构切换
- `iris` / `clock_wipe`：揭示、聚焦、仪式感
- `light_leak`：温暖、记忆、梦感
- `gl_transition`：自定义 shader，需要在 XML 实现时确认效果名/参数

除最终场景外，不规划场景元素退场动画；转场承担退出。
