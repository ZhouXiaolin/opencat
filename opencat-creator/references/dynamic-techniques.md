# 动态字幕技术

在写动画代码前阅读此文件。根据转录文本检测到的能量级别选择技术组合，然后用 OpenCat 的 `ctx.*` API 实现。

## 按能量级别选择技术

| 能量级别 | 高亮方式 | 退场方式 | 轮换模式 |
|----------|----------|----------|----------|
| 高 | Karaoke + 强调色发光 + scale pop | 散射或下落 | 每 2 组交替高亮样式 |
| 中高 | Karaoke + 颜色弹出 | 散射或折叠 | 每 3 组交替 |
| 中 | Karaoke（微妙，仅白色） | 淡出 + 滑动 | 每 3 组交替 |
| 中低 | Karaoke（最小 scale 变化） | 淡出 | 单一样式，每组变化 ease |
| 低 | Karaoke（暖色调，慢过渡） | 折叠 | 每 4 组交替 |

**所有能量级别以 Karaoke 高亮为基线。** 区别在于强度——高能量在活跃词上加强调色 + 发光 + 15% scale pop，低能量用柔和的白色偏移 + 3% scale。

**强调词总是打破模式。** 当一个词被标记为强调（情感关键词、全大写、品牌名），给它比周围词更强的动画（更大的 scale、强调色、overshoot ease）。这创造对比。

**标记高亮模式在 Karaoke 之上添加视觉层。** 对需要超越颜色/scale 的强调词，添加标记效果——高亮扫过、圆圈、爆发或涂鸦——使用 [text-highlight.md](text-highlight.md) 中的技术。匹配能量：爆发用于炒作、圆圈用于关键术语、高亮用于标准、涂鸦用于微妙。

## Karaoke 逐词高亮

使用 `ctx.splitText()` 拆分文本，逐词高亮当前词：

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"scene1","parentId":null,"type":"div","className":"w-full h-full bg-[#0a0a0a] flex items-center justify-center","duration":150}
{"id":"lyrics","parentId":"scene1","type":"text","className":"text-[48px] font-bold text-white","text":"Every word lights up in sequence"}
{"type":"script","parentId":"scene1","src":"var words=ctx.splitText('lyrics',{type:'words'});var tl=ctx.timeline();words.forEach(function(w,i){tl.to(w,{color:'#00C3FF',scale:1.15,duration:3,ease:'ease-out'},i*6);tl.to(w,{color:'#ffffff',scale:1,duration:3,ease:'ease-in-out'},i*6+3);});"}
```

每组词的高亮时间应与 SRT 时间戳对齐。如果使用 `type: "caption"` 节点，可通过 `ctx.getNode('subs').text(...)` 按帧覆盖字幕内容。

## 音频响应字幕（音乐必选）

**如果源音频是音乐（人声+伴奏、节拍、任何音乐内容），必须提取音频数据并添加音频响应动画。** 这不是可选的——没有音频响应的音乐看起来脱节。

```jsonl
{"type":"script","parentId":"scene1","src":"var f=ctx.currentFrame;var bass=bassData[f]||0;var treble=trebleData[f]||0;ctx.getNode('lyrics').scale(1+bass*0.06).opacity(0.7+treble*0.3);"}
```

保持音频响应微妙——3-6% scale 变化和柔和发光。大幅脉冲让文字不可读。

详见 [audio-reactive.md](audio-reactive.md) 获取完整的数据格式和映射参考。

## 组合技术

不要在每组上使用相同的高亮动画——使用组索引轮换样式。不要在相同时间戳的同一个词上组合多个竞争动画。跨组变化技术以匹配内容的节奏变化。

**标记高亮效果**（来自 [text-highlight.md](text-highlight.md)）与 Karaoke 叠加良好——用 Karaoke 做逐词揭示，然后只在强调词上添加标记效果。例如：Karaoke 高亮每个词为白色，但品牌名加黄色高亮扫过，数据加红色圆圈。跨组轮换标记模式以增加视觉多样性。

## 注意事项

- `ctx.splitText()` 的 `words` 类型对 CJK 文本回退到 `chars`
- 颜色 tween 必须用显式字面量（`#00C3FF`），不用 Tailwind token
- 退场动画仅在末场景使用（见 SKILL.md 转场规则）
- 所有脚本必须同步构建——不在 `async`/`await` 或 Promise 中
