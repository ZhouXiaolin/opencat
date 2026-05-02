# 音频响应动画

从音乐、语音或声音驱动视觉。任何 `ctx.*` 可动画属性都可以响应预提取的音频数据。

## 音频数据格式

```js
var AUDIO_DATA = {
  fps: 30,
  totalFrames: 900,
  frames: [{ bands: [0.82, 0.45, 0.31, ...] }, ...]
};
```

- `frames[i].bands[]` — 频段振幅，0-1。Index 0 = 低频，越高 = 高频
- 每个频段在整个音轨范围内独立归一化

## 音频到视觉的映射

| 音频信号 | 视觉属性 | 效果 |
|----------|----------|------|
| 低频 (bands[0]) | `scale` | 节拍脉冲 |
| 高频 (bands[12-14]) | `opacity`、颜色偏移 | 发光强度 |
| 整体振幅 | `y`、`opacity`、`backgroundColor` | 呼吸、抬升、色彩偏移 |
| 中频 (bands[4-8]) | `borderRadius`、`scaleX`/`scaleY` | 形状变形 |

OpenCat 的 `ctx.to()` / `ctx.getNode()` 支持所有视觉属性的动画——`opacity`、`x`、`y`、`scale`、`rotation`、`color`、`backgroundColor`、`borderRadius` 等。

## 内容优先，不是媒介

音频提供**节奏和强度**。视觉词汇来自叙事。

**永远不要添加：** 均衡器条、频谱分析仪、波形显示、音乐符号剪贴画、通用粒子系统、彩虹色循环、节拍频闪白光、抽象脉冲球体。

**而是：** 让内容引导视觉，音频驱动行为。低频让温暖感**膨胀**。高频 sharpen **对比度**。视觉选择来自"这个作品感觉像什么？"

## 采样模式

OpenCat 脚本每帧执行，通过 `ctx.frame` / `ctx.currentFrame` 采样音频数据：

```jsonl
{"type":"script","parentId":"scene1","src":"var f=ctx.currentFrame;var bass=bassData[f];var treble=trebleData[f];ctx.getNode('logo').scale(1+bass*0.04);ctx.getNode('cta').opacity(0.7+treble*0.3);"}
```

如果音频数据量大，可嵌入脚本变量或拆分到外部 `.js` 文件通过 `path` 引用。

纯 procedural 呼吸效果（不需要外部数据）：

```jsonl
{"type":"script","parentId":"scene1","src":"var breathe=0.5+0.5*Math.sin(ctx.frame*0.08);ctx.getNode('logo').scale(1+breathe*0.04);ctx.getNode('cta').opacity(0.7+breathe*0.3);"}
```

## 指导原则

- **文本要微妙** — 3-6% 缩放变化、柔和发光。大幅脉冲让文字不可读
- **非文本可以更大** — 背景和形状可以承受 10-30% 的摆动
- **匹配能量** — 企业 = 微妙；音乐视频 = 戏剧性
- **确定性** — 预提取数据，无 `Math.random()`，无 `Date.now()`

## 约束

- 所有音频数据必须预提取
- 不能用 `Math.random()` 或 `Date.now()`
- 音频响应运行在与所有其他动画相同的脚本执行上下文中
- 不要在脚本中调 `play()`/`pause()` — 运行时拥有播放权
