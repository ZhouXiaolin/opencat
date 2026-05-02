# 字幕

分析语音内容以确定字幕风格。如果用户指定了风格，使用指定风格。否则，从转录文本中检测语气。

## 字幕节点

OpenCat 内置 `type: "caption"` 节点，直接读取 SRT 文件：

```jsonl
{"type":"composition","width":1920,"height":1080,"fps":30,"frames":150}
{"id":"scene1","parentId":"root","type":"div","className":"w-full h-full","duration":150}
{"id":"subs","parentId":"scene1","type":"caption","className":"absolute inset-x-[48px] bottom-[32px] text-center text-white","path":"subtitles.utf8.srt"}
```

- `path` 相对于 JSONL 文件所在目录
- SRT 时间戳根据 composition 的 `fps` 转换为帧
- 在 timeline 场景中，caption 使用该场景的本地帧上下文
- 字幕内容可按帧覆盖：`ctx.getNode('subs').text(...)`
- SRT 必须为 UTF-8 编码。UTF-16/GBK 文件无法解析

## 风格检测（未指定风格时）

读取完整转录后选择。四个维度：

**1. 视觉感受** — 企业→清爽；能量感→粗犷；叙事→优雅；技术→精确；社交→活泼

**2. 色彩调色板** — 暗色+亮色用于能量；柔和用于专业；高对比用于清晰；一个强调色

**3. 字体气质** — 重型/紧缩用于冲击；干净无衬线用于现代；圆润用于友好；衬线用于优雅

**4. 动画性格** — scale-pop 用于冲击感；柔和淡入用于平静；逐词强调；打字机用于技术

## 逐词样式

扫描需要特殊处理的词语：

- **品牌/产品名** — 更大字号、独特颜色
- **全大写** — 放大、闪光、强调色
- **数字/统计** — 粗体、强调色
- **情感关键词** — 夸张动画（overshoot、bounce）
- **行动号召** — 高亮、下划线、颜色弹出
- **标记高亮** — 需要超越颜色的强调时，参见 [text-highlight.md](text-highlight.md)

## 语气到风格映射

| 语气 | 字体气质 | 动画 | 颜色 | 字号 |
|------|----------|------|------|------|
| 发布/宣传 | 重型紧缩，`font-bold` 或 `font-extrabold` | scale-pop，`back-out`，3-6 帧 | 亮色在暗色上 | `text-[72px]-[96px]` |
| 企业 | 干净无衬线，`font-semibold` | 淡入+滑动，`ease-out`，9 帧 | 白色/中性色，柔和强调 | `text-[56px]-[72px]` |
| 教程 | 等宽/无衬线，`font-medium` | 打字机/淡入，12-15 帧 | 高对比，极简 | `text-[48px]-[64px]` |
| 叙事 | 衬线/优雅，`font-normal` 或 `font-light` | 慢速淡入，`ease-out`，15-18 帧 | 温暖柔和调 | `text-[44px]-[56px]` |
| 社交 | 圆润无衬线，`font-bold` | 弹跳，`elastic-out`，逐词 | 活泼，彩色标签 | `text-[56px]-[80px]` |

## 词语分组

- **高能量：** 2-3 词。快速切换。
- **对话式：** 3-5 词。自然短语。
- **沉稳/平静：** 4-6 词。较长分组。

在句子边界、150ms+ 停顿或达到最大词数处分组。

## 定位

- **横屏（1920×1080）：** 底部 80-120px，居中
- **竖屏（1080×1920）：** 中下部，距底部约 600-700px，居中
- 切勿遮挡人物面部
- 使用 `className` 中的 Tailwind 布局类 — `absolute`、`inset-x-[Npx]`、`bottom-[Npx]`、`text-center`
- 同一时间只显示一个字幕组

## 文本溢出控制

OpenCat 不提供 `fitTextFontSize()`。改用 Tailwind 和 CSS：

```html
<!-- 方法 A：截断 + 限制宽度 -->
<div class="absolute inset-x-[48px] bottom-[32px] text-center">
  <span class="inline-block max-w-[90%] truncate text-[72px] font-bold text-white">
    GROUP TEXT
  </span>
</div>

<!-- 方法 B：手动字号控制 -->
<div class="absolute inset-x-[48px] bottom-[32px] text-center">
  <span class="text-[56px] font-bold text-white max-w-[1600px] inline-block">
    GROUP TEXT
  </span>
</div>
```

**参考宽度：** 横屏最大 1600px，竖屏最大 900px。长文本使用更小字号（`text-[42px]-[56px]`）。

安全网：容器上 `max-w-[Npx]`，`overflow-visible`（**不是** `overflow-hidden` — hidden 会裁剪缩放的重点词和发光效果），使用 `absolute` 定位，显式设置高度。当逐词样式使用 `scale > 1.0` 时，应当计算 `maxWidth = safeWidth / maxScale` 以预留空间。

**容器模式：** 全宽绝对定位容器，居中。不要使用 `left-1/2 -translate-x-1/2` — 会在画面边缘产生裁切。

## 字幕退出保证

每个分组在退出动画后**必须**有一个硬性 kill：

```js
ctx.timeline()
  .to(groupEl, { opacity: 0, duration: 3 }, groupEnd - 3)
ctx.set(groupEl, { opacity: 0 }, groupEnd) // or node.opacity(0)
```

`ctx.set()` 确保确定性 kill — 即使动画被 seek 跳过，组也在正确帧消失。

## 进一步参考

- [techniques.md](techniques.md) — karaoke、clip-path 揭示、slam 词、散射退出、弹性、3D 旋转
- [text-highlight.md](text-highlight.md) — CSS + ctx.* 标记高亮（确定性、完全可 seek）

## 约束

- 确定性。不使用 `Math.random()`、`Date.now()`。
- 与转录时间戳同步。
- 同一时间只显示一个字幕组。
- 每个分组必须有硬性的 `ctx.set()` kill 在 `groupEnd`。
- 字体通过 JSONL className 中的 Tailwind 类声明（如 `font-bold`、`font-[Outfit]`），编译时自动嵌入。
