# 音频响应动效

将任何可动画属性从播放音频中驱动。低音在鼓点上脉冲 Logo。高音在镲片上辉光 CTA。振幅在安静段落中让背景呼吸。结果：感觉与音轨锁定的运动，是预先编写的补间永远做不到的。

---

## 何时使用

**任何有音乐或戏剧性旁白的视频** — 品牌短片、产品发布、高能剪辑。对平静/教程节奏跳过。

## 核心原理

音频数据是预提取的逐帧频段能量。在每帧通过 `ctx.frame` 采样：

```js
// audio-data.json: { fps: 30, totalFrames: 900, frames: [{ bands: [0.82, 0.45, 0.31, ...] }, ...] }

// 在 canvas 脚本或 script 中：
var frame = audioData.frames[ctx.frame];
if (frame) {
  var bass = frame.bands[0]; // 0-1, 低频
  var treble = frame.bands[13]; // 0-1, 高频

  // 低音驱动缩放
  ctx.getNode('logo').scale(1 + bass * 0.04);

  // 高音驱动辉光
  ctx.getNode('cta').textColor(lerpColor('#ffffff', '#00C3FF', treble));
}
```

## 常用模式

| 元素 | 驱动 | 幅度 | 效果 |
|------|------|------|------|
| Logo / 品牌 | 低音 | 3-5% 缩放 | 在鼓点上呼吸 |
| CTA / 标题 | 高音 | 0-30% 辉光 | 在镲片上闪烁 |
| 背景 | 综合振幅 | 10-30% 亮度 | 随音乐律动 |
| Canvas 粒子 | 中频 | 大小/速度 | 随节奏跳动 |

## 提取音频数据

```bash
python3 scripts/extract-audio-data.py audio.mp3 --fps 30 --bands 16 -o audio-data.json
```

加载到合成中：

```xml
<opencat width="1280" height="720" fps="30" frames="360">
  <script>
    var audioData = JSON.parse(audioDataJson);
    // 在每帧使用 audioData.frames[ctx.frame]
  </script>
</opencat>
```

## 反模式

- **不要做：** 均衡器条、频谱分析仪、波形显示、频闪、彩虹颜色循环
- **不要过度：** 文字/Logo 强度保持 ≤5% 缩放、≤30% 辉光 — 小元素上的音频响应读作抖动
- **不要依赖实时 FFT：** 使用预提取数据，不在运行时计算

音频提供**时机和强度**；视觉词汇仍然来自品牌和设计系统。
