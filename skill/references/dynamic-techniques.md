# 动态字幕技巧

字幕是受限表面 — 高亮和退场技巧与语音内容的强度密切相关。

---

## 按能量选择技巧

**所有能量级别都以 Karaoke 高亮为基线。** 区别在于强度 — 不是技巧类型。

| 能量 | 高亮幅度 | 退场 | 轮换变化 |
|------|---------|------|---------|
| **高** | 强调色 + 辉光 + 15% 缩放弹出 | 散落或掉落 | 每 2 组 |
| **中高** | 颜色弹出，无辉光 | 散落或折叠 | 每 3 组 |
| **中** | 仅白色偏移 | 淡出 + 滑动 | 每 3 组 |
| **中低** | 最小缩放变化 | 淡出 | 单一风格 |
| **低** | 暖色调，慢速过渡 | 折叠 | 单一风格 |

**强调词总是打破模式。** 当一个词被标记为强调（情感关键词、品牌名），赋予更强的动画（更大缩放、强调色、回弹缓动）。

### 高亮模式

五种标记高亮模式，叠加在 Karaoke 之上：

| 模式 | 适用能量 | 技术方案 |
|------|---------|---------|
| **highlight** | 全部 | Tailwind `bg-yellow-400/35` 条 `scaleX` 动画 |
| **circle** | 中-高 | Tailwind 圆环 `border-2 border-red-500 rounded-full` `scale` 动画 |
| **burst** | 高 | div 辐射线 `scaleY` 动画 |
| **scribble** | 中 | SVG `<path>` `strokeDashoffset` 动画 |
| **sketchout** | 中高 | 两条交叉线 `scaleX` 动画 |

每种模式的精确 XML 代码见 [text-animations.md](text-animations.md) 的 §五种高亮模式。

### 音频响应字幕

当源音频是音乐时，必须提取音频数据并添加音频响应动画：

```js
// 加载音频数据
var AUDIO = JSON.parse(audioDataJson);

function getPeakEnergy(startTime, endTime) {
  var fps = AUDIO.fps || 30;
  var startIndex = Math.floor(startTime * fps);
  var endIndex = Math.min(Math.floor(endTime * fps), AUDIO.samples.length - 1);
  var peak = 0;
  for (var i = startIndex; i <= endIndex; i++) {
    var sample = AUDIO.samples[i];
    if (sample) peak = Math.max(peak, sample.bands[0] || 0);
  }
  return peak;
}
```

## 组合技巧

不同组之间轮换高亮模式 — 匹配内容节奏变化：

```js
var MODES = ['highlight', 'circle', 'burst', 'scribble'];
GROUPS.forEach(function(group, gi) {
  var mode = MODES[gi % MODES.length];
  // 根据组的能量选择模式
});
```

**不要**在同一个词上同时叠加多个竞争动画。Karaoke 做逐词揭示，标记高亮仅用于强调词。

## 轮换节奏

- 高能量：每 2-3 组轮换
- 中等能量：每 3-4 组轮换
- 低能量：每 4-5 组轮换

轮换本身创造能量；一致性创造平静。
