# OpenCat XML 格式参考

OpenCat 使用 XML 格式描述动态图形合成。运行时解析 XML，构建场景树，使用 Skia + Taffy + QuickJS 渲染帧。

---

## 基本结构

```xml
<opencat width="1280" height="720" fps="30" frames="90">
  <script>
    // 动画脚本（可选，最多一个）
    ctx.fromTo('title', {opacity: 0}, {opacity: 1, duration: 30});
  </script>
  <div id="root" class="flex items-center justify-center w-full h-full bg-white">
    <text id="title" class="text-[48px] font-bold">Hello</text>
  </div>
</opencat>
```

---

## 根元素 `<opencat>`

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `width` | 正整数 | 1920 | 画布宽度（像素） |
| `height` | 正整数 | 1080 | 画布高度（像素） |
| `fps` | 正整数 | 30 | 每秒帧数 |
| `frames` | 正整数 | 90 | 总帧数 |

---

## `<script>` 规则

**严格限制：**
- **只能有一个** `<script>` 标签
- **必须是 `<opencat>` 的直接子节点**（不能嵌套在其他元素内）
- **不能有属性**（如 `type`、`src` 等都不允许）
- **不能自闭合**（必须有 `</script>` 结束标签）

script 内容会在解析时被提取，并自动附加到 visual root 节点（即第一个视觉元素）。

```xml
<!-- ✅ 正确 -->
<opencat>
  <script>ctx.fromTo('title', {opacity: 0}, {opacity: 1, duration: 30});</script>
  <div id="root">...</div>
</opencat>

<!-- ❌ 错误：script 嵌套在 div 内 -->
<opencat>
  <div id="root">
    <script>...</script>
  </div>
</opencat>

<!-- ❌ 错误：script 有属性 -->
<opencat>
  <script type="text/javascript">...</script>
  <div id="root">...</div>
</opencat>

<!-- ❌ 错误：多个 script -->
<opencat>
  <script>...</script>
  <script>...</script>
  <div id="root">...</div>
</opencat>
```

---

## 元素类型

| 标签 | 说明 | 必填属性 |
|------|------|----------|
| `<div>` | 容器，默认 `display: block` | `id` |
| `<text>` | 文本节点 | `id` |
| `<image>` | 图像 | `id` + 一个图像源 |
| `<video>` | 视频 | `id` + 一个视频源 |
| `<icon>` | Lucide 图标 | `id` + `icon` |
| `<path>` | SVG 路径 | `id` + `d` |
| `<canvas>` | Canvas 绘制表面 | `id` |
| `<audio>` | 音频（必须在 `<soundtrack>` 内） | `id` + `attach` + 一个音频源 |
| `<caption>` | SRT 字幕 | `id` + `path` |
| `<tl>` | Timeline 容器 | `id` |
| `<transition>` | 场景转场 | `from` + `to` + `effect` + `duration` |
| `<soundtrack>` | 音频容器 | — |

---

## 资源指定

### 图像源（三选一）

| 属性 | 说明 |
|------|------|
| `path` | 本地文件路径 |
| `url` | 远程 URL |
| `query` | Openverse 搜索查询（1-4 个名词） |

可选：`queryCount`（默认 1）、`aspectRatio`（需配合 `query`）

```xml
<image id="local" path="/tmp/photo.png" />
<image id="remote" url="https://example.com/photo.png" />
<image id="search" query="mountain landscape" queryCount="3" aspectRatio="16:9" />
```

### 视频源（三选一）

| 属性 | 说明 |
|------|------|
| `path` | 本地文件路径 |
| `url` | 远程 URL |
| `src` | 兼容属性（自动判断本地/远程） |

```xml
<video id="clip" url="https://example.com/video.mp4" />
```

视频时间控制：

| 属性 | 说明 |
|------|------|
| `data-start` | 时间线起点（秒） |
| `data-duration` | 时间线时长（秒） |
| `data-media-start` | 媒体内起始点（秒） |
| `loop` | 循环播放（`true`/`false`） |

### 音频源（二选一，必须在 `<soundtrack>` 内）

```xml
<soundtrack>
  <audio id="bgm" url="https://example.com/music.mp3" attach="scene1" />
</soundtrack>
```

| 属性 | 说明 |
|------|------|
| `id` | 节点标识 |
| `path`/`url` | 音频源 |
| `attach` | 附加到的场景 ID |
| `duration` | 可选，持续帧数 |

---

## 布局系统

样式使用 `class` 属性，采用 Tailwind 风格类：

```xml
<div id="root" class="flex flex-col items-center justify-center gap-4 p-6 bg-white rounded-[12px]">
```

**布局硬性规则：**

- **优先使用 flex**。容器应以 `flex flex-col` / `flex items-center justify-center` 等起手
- **`absolute` 必须显式坐标**。至少包含 `top` / `left` / `right` / `bottom` / `inset-X` 之一

```xml
<!-- ✅ 正确 -->
<div id="overlay" class="absolute inset-0 bg-black/50" />
<div id="badge" class="absolute left-[10px] top-[10px] px-[8px] py-[4px] bg-white rounded-full" />

<!-- ❌ 错误：absolute 无坐标 -->
<div id="overlay" class="absolute bg-black/50" />
```

**样式限制：**
- 不要使用 CSS 动画类（`transition-*`、`animate-*`、`duration-*`、`ease-*`、`delay-*`）
- 不要使用 transform 类（`transform`、`translate-*`、`rotate-*`、`scale-*`、`skew-*`）

---

## Timeline（多场景 + 转场）

```xml
<opencat width="1280" height="720" fps="30" frames="360">
  <div id="root" class="relative w-[1280px] h-[720px]">
    <tl id="main-tl" class="absolute inset-0">
      <div id="scene1" class="w-full h-full bg-white" duration="120">
        <text id="title" class="text-[48px] font-bold">Scene 1</text>
      </div>

      <transition from="scene1" to="scene2" effect="fade" duration="18" timing="ease-in-out" />

      <div id="scene2" class="w-full h-full bg-slate-900" duration="222">
        <text id="title2" class="text-[48px] font-bold text-white">Scene 2</text>
      </div>
    </tl>
  </div>
</opencat>
```

**Timeline 规则：**
- `<tl>` 必须至少有两个直接子场景
- 每对相邻场景必须有匹配的 `<transition>`
- `<tl>` 没有 `duration` 属性，总长推导：`sum(scene.duration) + sum(transition.duration)`
- `<transition>` 必须是 `<tl>` 的直接子节点
- 保持 `frames` 与推导总长对齐

---

## 转场

```xml
<transition from="scene1" to="scene2" effect="fade" duration="18" />
```

| 属性 | 必填 | 说明 |
|------|------|------|
| `from` | 是 | 源场景 id |
| `to` | 是 | 目标场景 id |
| `effect` | 是 | 效果名称 |
| `duration` | 是 | 转场帧数 |
| `direction` | 否 | `slide`/`wipe` 的方向 |
| `timing` | 否 | 缓动名称（默认 `linear`） |
| `damping` | 否 | 弹簧阻尼 |
| `stiffness` | 否 | 弹簧刚度 |
| `mass` | 否 | 弹簧质量 |
| `seed` | 否 | 随机种子 |
| `hueShift` | 否 | 色相偏移 |
| `maskScale` | 否 | 遮罩缩放 |

### 效果类型

| effect | 说明 | direction（可选） |
|--------|------|-------------------|
| `fade` | 交叉淡入淡出 | — |
| `slide` | 滑动 | `from_left`/`from_right`/`from_top`/`from_bottom` |
| `wipe` | 擦除 | 8 个方向 |
| `clock_wipe` | 时钟擦除 | — |
| `iris` | 虹膜开合 | — |
| `light_leak` | 漏光 | — |
| `gl_transition` | GLSL 运行时着色器 | — |

未识别的 `effect` 名回退到 `gl_transition` 模式。

---

## 动画系统

动画脚本通过 `<script>` 标签编写，使用 QuickJS 在每帧运行。

### 执行上下文

| 字段 | 说明 |
|------|------|
| `ctx.frame` | 全局帧索引 |
| `ctx.totalFrames` | 总帧数 |
| `ctx.currentFrame` | 当前场景内的帧索引 |
| `ctx.sceneFrames` | 当前场景的帧数 |

**使用指南：**
- **循环动画**（呼吸、闪烁、持续旋转）：使用 `ctx.frame`
- **场景内进度**（路径绘制、淡入淡出）：使用 `ctx.currentFrame / ctx.sceneFrames`

### Tween API

```js
ctx.set(targets, vars);                // 立即设置
ctx.to(targets, vars);                 // 当前 → 目标
ctx.from(targets, vars);               // 初始 → 当前
ctx.fromTo(targets, fromVars, toVars); // 完整控制
```

**属性别名：**

| 属性 | 说明 |
|------|------|
| `opacity` | 视觉透明度 |
| `x`, `y` | translateX/Y |
| `scale`, `scaleX`, `scaleY` | 缩放 |
| `rotate`, `rotation` | 旋转 |
| `skewX`, `skewY` | 倾斜 |
| `path` | 路径动画 |
| `morphSVG`, `d` | SVG 路径变形 |
| `left`, `top`, `width`, `height` | 布局 |
| `backgroundColor`, `bg` | 背景色 |
| `color`, `textColor` | 文字色 |
| `borderColor`, `borderRadius`, `borderWidth` | 边框 |
| `fillColor`, `strokeColor`, `strokeWidth` | SVG 绘制 |
| `text` | 文字内容层（打字机） |

**时间字段：**

| 字段 | 默认 | 说明 |
|------|------|------|
| `duration` | 非弹簧必填 | 帧数 |
| `delay` | `0` | 起始偏移 |
| `ease`/`easing` | `'linear'` | 缓动 |
| `repeat` | `0` | 循环。`-1` = 无限 |
| `yoyo` | `false` | 交替反向 |
| `stagger` | `0` | 数组目标延迟 |

### Timeline

```js
ctx.timeline({ defaults: { duration: 18, ease: 'spring.gentle' } })
  .from('title', { opacity: 0, y: 30 })
  .from('subtitle', { opacity: 0, y: 18 }, '-=8')
  .fromTo('cta', { scale: 0.8 }, { scale: 1, duration: 24 }, '+=6');
```

**Position 参数：**

| Position | 含义 |
|----------|------|
| 省略 | 从当前游标开始 |
| 数字 | 绝对帧 |
| `'+=N'` | 游标后 N 帧 |
| `'-=N'` | 游标前 N 帧 |
| `'<'`/`'>'` | 前一个子项的开始/结束 |

### 路径动画

```js
ctx.to('rocket', {
  path: 'M100 360 C400 80 880 640 1180 360',
  orient: -90,
  duration: 120,
  ease: 'ease-in-out',
});
```

### 变形路径

```js
ctx.fromTo('blob',
  { d: 'M55 0 L110 95 L0 95 Z' },
  { d: 'M55 95 L110 0 L0 0 Z', duration: 45, ease: 'ease-in-out' }
);
```

### 文字动画

**打字机：**
```js
ctx.to('title', { text: 'Hello OpenCat', duration: 30, ease: 'linear' });
```

**splitText：**
```js
ctx.from(ctx.splitText('title', { type: 'chars' }), {
  opacity: 0, y: 38, scale: 0.86,
  duration: 22, stagger: 2, ease: 'spring.wobbly',
});
```

### Node API

```js
ctx.getNode('id')
  .opacity(0.5).translateX(100).translateY(50)
  .scale(1.5).rotate(45)
  .bg('blue-500').borderRadius(16)
  .textColor('white').textSize(24).fontWeight('bold');
```

---

## 缓动参考

| 预设 | 效果 |
|------|------|
| `'linear'` / `'none'` | 匀速 |
| `'ease'`/`'ease-in'`/`'ease-out'`/`'ease-in-out'` | 标准 CSS 三次曲线 |
| `'back-in'`/`'back-out'`/`'back-in-out'` | 轻微回弹 |
| `'elastic-in'`/`'elastic-out'`/`'elastic-in-out'` | 阻尼振荡 |
| `'bounce-in'`/`'bounce-out'`/`'bounce-in-out'` | 地面弹跳 |
| `'steps(N)'` | N 步离散 |
| `'spring.default'`/`'spring.gentle'`/`'spring.stiff'`/`'spring.slow'`/`'spring.wobbly'` | 弹簧 |

**GSAP 风格缓动：**

| 预设 | 效果 |
|------|------|
| `'power1.in'`/`'power1.out'`/`'power1.inOut'` | 二次曲线（轻度） |
| `'power2.in'`/`'power2.out'`/`'power2.inOut'` | 三次曲线（中度） |
| `'power3.in'`/`'power3.out'`/`'power3.inOut'` | 四次曲线（重度） |
| `'power4.in'`/`'power4.out'`/`'power4.inOut'` | 五次曲线（极重） |
| `'circ.in'`/`'circ.out'`/`'circ.inOut'` | 圆形曲线 |
| `'expo.in'`/`'expo.out'`/`'expo.inOut'` | 指数曲线 |
| `'sine.in'`/`'sine.out'`/`'sine.inOut'` | 正弦曲线 |
| `'back.in(overshoot)'`/`'back.out(overshoot)'` | 带参数的回弹 |
| `'elastic.in(amp,period)'`/`'elastic.out(amp,period)'` | 带参数的弹性 |

自定义弹簧：
```js
ease: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

三次贝塞尔：
```js
ease: [0.25, 0.1, 0.25, 1.0]
```

---

## Canvas API

`canvas` 节点提供 CanvasKit 风格的绘制表面。

### 入口点

| 对象 | 用途 |
|------|------|
| `ctx.CanvasKit` | 辅助函数、构造函数、枚举 |
| `ctx.getCanvas()` | 绘制接口 |
| `ctx.getImage(assetId)` | 图像句柄 |

### 常用方法

```js
var CK = ctx.CanvasKit;
var canvas = ctx.getCanvas();

// 状态
canvas.clear(color?);
canvas.save(); canvas.restore();
canvas.translate(dx, dy);
canvas.scale(sx, sy?);
canvas.rotate(degrees, rx?, ry?);

// 形状
canvas.drawRect(rect, paint);
canvas.drawCircle(cx, cy, radius, paint);
canvas.drawPath(path, paint);
canvas.drawText(text, x, y, paint, font);
canvas.drawImage(image, x, y, paint?);
```

### Paint

```js
var paint = new CK.Paint();
paint.setStyle(CK.PaintStyle.Fill);
paint.setColor(CK.parseColorString('#ff0000'));
paint.setAlphaf(0.8);
paint.setStrokeWidth(2);
paint.setStrokeCap(CK.StrokeCap.Round);
```

### Path

```js
var path = new CK.Path();
path.moveTo(x, y);
path.lineTo(x, y);
path.cubicTo(x1, y1, x2, y2, x3, y3);
path.close();
```

### 限制

- `clipRect()`/`clipPath()`/`clipRRect()` — 仅 `CK.ClipOp.Intersect`
- `PathEffect` — 仅 `MakeDash()`
- 文字 — 仅系统默认字体
- `ctx.getImage()` — 仅资源 id 句柄

---

## 完整示例

### 简单场景（无 Timeline）

```xml
<opencat width="390" height="844" fps="30" frames="60">
  <script>
    ctx.fromTo('title', {opacity: 0, y: 30}, {opacity: 1, y: 0, duration: 20, ease: 'spring.gentle'});
  </script>
  <div id="root" class="flex flex-col items-center justify-center w-full h-full bg-white">
    <text id="title" class="text-[48px] font-bold text-slate-900">Hello OpenCat</text>
  </div>
</opencat>
```

### 多场景 Timeline

```xml
<opencat width="1280" height="720" fps="30" frames="360">
  <soundtrack>
    <audio id="bgm" url="https://example.com/music.mp3" attach="scene1" />
  </soundtrack>

  <script>
    ctx.fromTo(['title', 'subtitle'], {opacity: 0, y: 24}, {opacity: 1, y: 0, stagger: 6, duration: 24, ease: 'spring.gentle'});
  </script>

  <div id="root" class="relative w-[1280px] h-[720px] bg-slate-950">
    <tl id="main-tl" class="absolute inset-0">
      <div id="scene1" class="flex flex-col items-center justify-center w-full h-full" duration="180">
        <text id="title" class="text-[72px] font-bold text-white">Scene 1</text>
        <text id="subtitle" class="text-[24px] text-slate-400">With animation</text>
      </div>

      <transition from="scene1" to="scene2" effect="fade" duration="18" timing="ease-in-out" />

      <div id="scene2" class="flex flex-col items-center justify-center w-full h-full bg-slate-900" duration="180">
        <text id="title2" class="text-[72px] font-bold text-white">Scene 2</text>
      </div>
    </tl>
  </div>
</opencat>
```

### Canvas 绘制

```xml
<opencat width="640" height="480" fps="30" frames="120">
  <script>
    var CK = ctx.CanvasKit;
    var canvas = ctx.getCanvas();
    canvas.clear(CK.WHITE);
    var paint = new CK.Paint();
    paint.setColor(CK.parseColorString('#ff0000'));
    canvas.drawCircle(320, 240, 100, paint);
  </script>
  <div id="root" class="w-[640px] h-[480px] bg-white">
    <canvas id="my-canvas" class="w-full h-full" />
  </div>
</opencat>
```

### 视频叠加

```xml
<opencat width="1280" height="720" fps="30" frames="180">
  <div id="root" class="relative w-full h-full bg-black">
    <video id="bg-video" class="absolute inset-0 w-full h-full object-cover" src="https://example.com/video.mp4" loop="true" />
    <div id="overlay" class="absolute bottom-[40px] left-[40px] px-[20px] py-[12px] rounded-[12px] bg-black/60">
      <text id="caption" class="text-[24px] text-white font-semibold">Video Overlay</text>
    </div>
  </div>
</opencat>
```

---

## 附录：常见错误

| 错误 | 正确 |
|------|------|
| `<div>` 标签内直接写文本 | 用 `<text>` 包裹 |
| 用 `bg-{color}` 给图标/路径着色 | 用 `fill-{color}` / `stroke-{color}` |
| `class` 中放 transform 类 | 用脚本控制动画 |
| `<tl>` 缺转场或场景少于 2 | 添加缺失的 `<transition>` |
| 帧数不匹配 | `frames = sum(scene.duration) + sum(transition.duration)` |
| `<script>` 嵌套在其他元素内 | 必须是 `<opencat>` 的直接子节点 |
| `<audio>` 直接放在 `<opencat>` 下 | 必须在 `<soundtrack>` 内 |
