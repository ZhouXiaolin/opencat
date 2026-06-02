# 字幕与配音

字幕（captions/subtitles）是叙事的视觉层。与旁白同步，增强理解，在高噪音环境中留住观众。

---

## 基础字幕（SRT）

OpenCat 通过 `<caption>` 节点支持 SRT 字幕：

```xml
<opencat width="1280" height="720" fps="30" frames="360">
  <div id="root" class="relative w-full h-full">
    <caption id="subs" path="subtitles.srt" />
  </div>
</opencat>
```

SRT 格式：
```
1
00:00:01,000 --> 00:00:04,000
第一行字幕

2
00:00:05,000 --> 00:00:09,000
第二行字幕
```

字幕节点自动渲染在画面底部，跟随时间轴。

---

## 动效字幕（Karaoke 风格）

对于需要逐词同步的旁白驱动视频，使用 `ctx.splitText()` + 时间戳驱动高亮。

### 数据来源

从 `transcript.json` 或 `audio-data.json` 获取逐词时间戳。格式：

```json
{
  "groups": [
    { "start": 0.0, "end": 1.2, "text": "Anything a browser" },
    { "start": 1.2, "end": 2.5, "text": "can render" }
  ]
}
```

### 实现模式

```xml
<opencat width="1280" height="720" fps="30" frames="180">
  <div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
    <!-- 预先放置所有词组 -->
    <text id="cap-0" class="absolute text-[36px] text-white/40">Anything a browser</text>
    <text id="cap-1" class="absolute text-[36px] text-white/40">can render</text>
  </div>
  <script>
    var GROUPS = [
      { start: 0, end: 1.2, el: 'cap-0' },
      { start: 1.2, end: 2.5, el: 'cap-1' },
    ];
    var tl = ctx.timeline();
    GROUPS.forEach(function (g) {
      var startF = Math.round(g.start * 30);
      var endF = Math.round(g.end * 30);
      // 淡入
      tl.fromTo(g.el, { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 9, ease: 'ease-out' }, startF);
      // 退出
      tl.to(g.el, { opacity: 0, duration: 6, ease: 'ease-in' }, endF - 6);
    });
  </script>
</opencat>
```

### Karaoke 逐词高亮

```xml
<div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
  <text id="lyrics" class="text-[36px] text-white/40">Anything a browser can render</text>
</div>
<script>
  var words = ctx.splitText('lyrics', { type: 'words' });
  var timings = [0.0, 0.23, 0.28, 0.63, 0.78];
  var tl = ctx.timeline();
  words.forEach(function (w, i) {
    var startF = Math.round(timings[i] * 30);
    tl.to(w, { color: '#00C3FF', scale: 1.1, duration: 3, ease: 'ease-out' }, startF);
    tl.to(w, { color: '#ffffff', scale: 1, duration: 3, ease: 'ease-in-out' }, startF + 6);
  });
</script>
```

---

## 与配音音频同步

配音音频通过 `<soundtrack>` + `<audio>` + `attach` 关联到 `<tl>`：

```xml
<opencat width="1280" height="720" fps="30" frames="360">
  <soundtrack>
    <audio id="narration" path="narration.wav" attach="main-tl" />
  </soundtrack>
  <div id="root" class="relative w-full h-full">
    <tl id="main-tl" class="absolute inset-0">
      <div id="scene1" class="..." duration="180">
        <!-- 场景内容 -->
      </div>
      <transition from="scene1" to="scene2" effect="fade" duration="18" />
      <div id="scene2" class="..." duration="162">
        <!-- 场景内容 -->
      </div>
    </tl>
  </div>
</opencat>
```

### 旁白时间线对齐

旁白录制的 WAV 文件，其时长直接影响画面时间线。每个字幕组的 `start`/`end` 时间来自 `transcript.json`：

```json
{
  "words": [
    { "word": "Anything", "start": 0.0, "end": 0.23 },
    { "word": "a", "start": 0.23, "end": 0.28 },
    { "word": "browser", "start": 0.28, "end": 0.63 },
    { "word": "can", "start": 0.63, "end": 0.78 },
    { "word": "render", "start": 0.78, "end": 1.2 }
  ]
}
```

将 word 时间戳对齐到帧（@30fps）：`startFrame = Math.round(word.start * 30)`。

---

## 字幕能量与风格

见 [dynamic-techniques.md](dynamic-techniques.md) 的完整能量级别映射。

| 能量 | 高亮 | 退场 | 轮换 |
|------|------|------|------|
| 高 | 强调色 + 辉光 + 15% 缩放 | 散落或掉落 | 每 2 组 |
| 中 | 白色偏移 + 微缩放 | 淡出 + 滑动 | 每 3 组 |
| 低 | 暖色调慢速过渡 | 折叠 | 单一风格 |

---

## 规则

- **每张字幕卡片只显示 2-3 行** — 太多文字让观众无法同时阅读和看画面
- **单行不超过 6-8 词** — 短词组节奏更好
- **每个词组至少保持 1 秒** — 低于 1 秒观众无法读完
- **高亮词用 `#00C3FF` 风格色，不默认用黄色**
- **字幕不遮挡画面焦点** — 放在画面底部 10-15% 区域
- **场景边界硬杀死** — 退场 `to` 过 duration 后自动 clamp 在终点值，无需额外 `set`
