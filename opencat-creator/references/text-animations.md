# 文字动效

OpenCat 提供两层独立的文字动画：
1. **内容层** — `ctx.to('title', { text: '...' })` 逐字显示（打字机）
2. **单元层** — `ctx.splitText()` + `ctx.from()`/`ctx.fromTo()` 对 chars/words 做属性动画

---

## 打字机效果

```js
ctx.to('title', {
  text: 'Hello OpenCat',
  duration: 30,
  delay: 6,
  ease: 'linear',
});
```

- 逐 grapheme cluster 显示
- ZWJ emoji 和组合标记不会被拆分
- `duration` 控制总显示时长

---

## splitText 拆分

`ctx.splitText(id, { type })` 读取文字源并返回可动画的视觉单元：

| type | 含义 |
|------|------|
| `'chars'` | grapheme clusters |
| `'words'` | Unicode word-boundary 单元；CJK 回退到 chars |
| `'lines'` | 布局行范围（保留） |

每个 part 暴露 `index`、`text`、`start`、`end`。

---

## 逐字符入场

```js
ctx.from(ctx.splitText('title', { type: 'chars' }), {
  opacity: 0,
  y: 38,
  scale: 0.86,
  duration: 22,
  stagger: 2,
  ease: 'spring.wobbly',
});
```

---

## 逐词入场

```js
ctx.from(ctx.splitText('headline', { type: 'words' }), {
  opacity: 0,
  y: 24,
  duration: 18,
  stagger: 4,
  ease: 'spring.gentle',
});
```

---

## 四面八方聚拢/分散

```js
var dirs = [[-220, -120], [0, -180], [220, -120], [-220, 120], [220, 120]];

ctx.fromTo(ctx.splitText('headline', { type: 'chars' }), {
  opacity: 0,
  x: function(i) { return dirs[i % dirs.length][0]; },
  y: function(i) { return dirs[i % dirs.length][1]; },
  scale: 0.8,
  rotate: function(i) { return i % 2 === 0 ? -14 : 14; },
}, {
  opacity: 1, x: 0, y: 0, scale: 1, rotate: 0,
  duration: 48, stagger: 2, ease: 'spring.gentle',
});
```

---

## 五颜六色的文字

每个字符不同颜色：

```js
var colors = ['#ef4444', '#f97316', '#eab308', '#22c55e', '#3b82f6', '#8b5cf6'];

ctx.fromTo(ctx.splitText('rainbow', { type: 'chars' }), {
  opacity: 0, y: 30,
}, {
  opacity: 1, y: 0,
  duration: 20, stagger: 2, ease: 'spring.gentle',
  onStart: function(i) {
    ctx.getNode('rainbow').textColor(colors[i % colors.length]);
  },
});
```

---

## 打字 + 逐词高亮组合

```js
// 先打字显示
ctx.to('quote', { text: 'Less is more', duration: 36, ease: 'linear' });
// 然后逐词高亮
ctx.fromTo(ctx.splitText('quote', { type: 'words' }), {
  color: '#ffffff',
}, {
  color: '#fbbf24',
  duration: 8, stagger: 12, delay: 36, ease: 'ease-out',
});
```

---

## 文字色彩边框

```js
// 设置文字描边
ctx.getNode('title')
  .strokeWidth(2)
  .strokeColor('#00C3FF');

// 动态边框颜色变化
ctx.fromTo('title', {
  strokeColor: '#00C3FF',
}, {
  strokeColor: '#8b5cf6',
  duration: 60, repeat: -1, yoyo: true, ease: 'sine.inOut',
});
```

---

## 字幕进度中高亮着色

为特定字/词高亮着色：

```js
var highlightWords = ['重要', '关键', '核心'];
var words = ctx.splitText('subtitle', { type: 'words' });

words.forEach(function(word, i) {
  if (highlightWords.indexOf(word.text) !== -1) {
    ctx.fromTo(word, {
      color: '#ffffff',
    }, {
      color: '#fbbf24',
      scale: 1.1,
      duration: 6,
      ease: 'ease-out',
    });
  }
});
```

---

## 半透明流光效果

```jsonl
{"id":"glow-overlay","parentId":"title-wrap","type":"div","className":"absolute inset-0 bg-gradient-to-r from-transparent via-white/20 to-transparent -translate-x-full"}
{"type":"script","parentId":"scene1","src":"var tl = ctx.timeline();\ntl.to('glow-overlay', { x: '200%', duration: 30, ease: 'ease-in-out', repeat: -1, repeatDelay: 20 }, 0);"}
```

或者使用 CanvasKit：

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();

// 绘制文字
var font = new CK.Font(null, 48);
var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Fill);
paint.setColor(CK.WHITE);
canvas.drawText('Hello OpenCat', 100, 200, paint, font);

// 绘制流光效果（循环动画使用 ctx.frame）
var glowPaint = new CK.Paint();
glowPaint.setStyle(CK.PaintStyle.Fill);
glowPaint.setAlphaf(0.3);
glowPaint.setColor(CK.WHITE);

var progress = (ctx.frame % 60) / 60;
var x = progress * 400 - 100;
canvas.drawRect(CK.XYWHRect(x, 150, 100, 100), glowPaint);
```

---

## 五种高亮模式

### 1. 高亮模式 (highlight)

黄色半透明条从左到右扫过文字：

```jsonl
{"id":"hl-bar","parentId":"hl-wrap","type":"div","className":"absolute inset-0 -left-[6px] -right-[6px] bg-yellow-400 opacity-35 scale-x-0 origin-left rounded-[3px] z-0"}
{"type":"script","parentId":"root","src":"var tl = ctx.timeline();\ntl.to('hl-bar', { scaleX: 1, duration: 15, ease: 'ease-out' }, 18);"}
```

### 2. 圆圈模式 (circle)

红色圆环从中心放大包住文字：

```jsonl
{"id":"circle-ring","parentId":"circle-wrap","type":"div","className":"absolute top-1/2 left-1/2 w-[130%] h-[160%] border-2 border-red-500 rounded-full pointer-events-none z-0"}
{"type":"script","parentId":"root","src":"ctx.set('circle-ring', { x: '-50%', y: '-50%', rotate: -3, scale: 0 });\nvar tl = ctx.timeline();\ntl.to('circle-ring', { scale: 1, rotation: -3, duration: 18, ease: 'back-out' }, 21);"}
```

### 3. 爆发模式 (burst)

从文字中心向外辐射颜色线：

```jsonl
{"id":"line-0","parentId":"burst-container","type":"div","className":"absolute block w-[3px] h-[70px] bg-blue-500 -left-[1.5px]"}
{"type":"script","parentId":"root","src":"var lines = ['line-0','line-1','line-2','line-3','line-4','line-5','line-6','line-7','line-8','line-9','line-10','line-11'];\nvar tl = ctx.timeline();\ntl.fromTo(lines, { scaleY: 0, opacity: 0 }, { scaleY: 1, opacity: 1, duration: 12, ease: 'ease-out', stagger: 1 }, 21);"}
```

### 4. 涂鸦模式 (scribble)

SVG 波浪路径自绘效果：

```jsonl
{"id":"scribble-path","parentId":"scribble-svg","type":"path","className":"fill-none stroke-[#FDD835] stroke-[3px]","d":"M0,12 Q31,0 62,12 Q93,24 125,12 Q156,0 187,12 Q218,24 250,12 Q281,0 312,12 Q343,24 375,12 Q406,0 437,12 Q468,24 500,12"}
{"type":"script","parentId":"root","src":"ctx.set('scribble-path', { strokeDasharray: 500, strokeDashoffset: 500 });\nvar tl = ctx.timeline();\ntl.to('scribble-path', { strokeDashoffset: 0, duration: 24, ease: 'ease-in-out' }, 21);"}
```

### 5. 划掉模式 (sketchout)

两条交叉红线：

```jsonl
{"id":"line-fwd","parentId":"sketchout-lines","type":"div","className":"absolute block top-1/2 left-0 w-full h-[2px] bg-red-500 origin-left"}
{"id":"line-bwd","parentId":"sketchout-lines","type":"div","className":"absolute block top-1/2 left-0 w-full h-[2px] bg-red-500 origin-left"}
{"type":"script","parentId":"root","src":"ctx.set('line-fwd', { scaleX: 0, rotate: -12 });\nctx.set('line-bwd', { scaleX: 0, rotate: 12 });\nvar tl = ctx.timeline();\ntl.to('line-fwd', { scaleX: 1, duration: 9, ease: 'ease-out' }, 30);\ntl.to('line-bwd', { scaleX: 1, duration: 9, ease: 'ease-out' }, 35);"}
```

---

## Karaoke 逐词高亮

```js
var words = ctx.splitText('lyrics', { type: 'words' });
var tl = ctx.timeline();
words.forEach(function(w, i) {
  tl.to(w, { color: '#00C3FF', scale: 1.15, duration: 3, ease: 'ease-out' }, i * 6);
  tl.to(w, { color: '#ffffff', scale: 1, duration: 3, ease: 'ease-in-out' }, i * 6 + 3);
});
```

---

## 模式组合与轮换

```js
var MODES = ['highlight', 'circle', 'burst', 'scribble'];
GROUPS.forEach(function(group, gi) {
  var mode = MODES[gi % MODES.length];
  // 应用模式
});
```

轮换节奏：
- 高能量：每 2-3 组轮换
- 中等能量：每 3-4 组轮换
- 低能量：每 4-5 组轮换
