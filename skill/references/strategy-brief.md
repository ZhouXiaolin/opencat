# Strategy Brief

在生成 OpenCat XML 之前，先锁定视频要说什么。这个步骤决定故事，不决定 XML 结构。

## 先解析用户已给信息

不要重复询问用户已经说明的内容。先从 prompt 提取：

- 视频类型：社交广告 / 产品演示 / 品牌短片 / 发布预告 / 功能公告 / 标题卡 / 其他
- 时长：明确秒数，或按类型推断
- 画幅：1920x1080 / 1080x1920 / 1080x1080 / 用户指定
- 风格：节奏、情绪、品牌限制、参考对象
- 必须出现的内容：产品名、功能、数据、Logo、CTA、素材
- 是否旁白、字幕、音乐、SFX

只问缺失且会影响成片方向的问题。

## 必须锁定的 9 项

写入 `VIDEO_BRIEF.md`：

```markdown
# Video Brief

**Message:** [观众只记住一句话时应该记住什么]
**Narrative Arc:** [Problem->Solution / Reveal / Demonstration / Vibe / Comparison / Custom]
**Audience:** [谁看]
**Platform + Format:** [在哪播放，尺寸]
**Video Type:** [类型]
**Duration:** [秒数]
**Style Direction:** [节奏、情绪、参考]
**Specific Requests:** [用户明确要求]
**Narration:** [yes / no / minimal]
```

没有 **Message** 和 **Narrative Arc**，不要写 storyboard；否则场景会退化成素材拼贴。

## 提问策略

如果信息不足，一次只问最少问题。优先问：

1. **这个视频只需要传达的一句话是什么？**
2. **故事怎么展开？** Problem->Solution、Reveal、Demonstration、Vibe、Comparison，或自定义。
3. **谁看、在哪看？**

风格问题用开放式描述，不给固定标签绑定审美：

> 你希望它更慢、更有电影感，还是更快、更有社交媒体冲击力？整体应该偏干净明亮、暗色戏剧化、鲜艳高能，还是别的方向？

## 自主模式

用户说“你决定”“直接做”“surprise me”时：

- 自行锁定缺失偏好，并写进 `VIDEO_BRIEF.md`
- 不再反复询问 TTS、字幕、转场、配色等偏好
- 继续做后续 XML 静态自检，但不要求本地渲染或视频文件生成

## 类型默认值

| Type | Duration | Narration | Rhythm Hint |
|------|----------|-----------|-------------|
| Social ad | 10-20s | optional | hook-PUNCH-hold-CTA |
| Product demo | 30-60s | usually yes | slow-build-BUILD-PEAK-breathe-CTA |
| Feature announcement | 15-30s | yes/minimal | reveal-proof-CTA |
| Brand reel | 20-45s | optional | drift-build-PEAK-resolve |
| Launch teaser | 10-25s | minimal | SLAM-reveal-hold |

这些是起点，不是公式。节奏由 message、arc 和品牌决定。
