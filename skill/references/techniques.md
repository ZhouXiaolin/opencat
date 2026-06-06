# 动效技法目录

13 种基础动效技法，来自生产级视频。这些是构建拍点的积木，不是现成配方。每个组合应至少使用 2-3 种。

**这些是起点，不是粘贴模板。** 理解每种技法背后的**原理**，根据需求调整颜色、尺寸、时间、缓动、元素数量。

---

| # | 技法 | 作用 | 最适合 |
|---|------|------|--------|
| 1 | **SVG 路径绘制** | Logo/图标逐笔画出 | Logo 揭示、图表连线、连接线 |
| 2 | **Canvas 程序化生成** | 动画噪点、粒子、数据可视化 | 生成式背景、环境纹理 |
| 3 | **3D 变换** | 卡片翻转、透视网格 | 产品展示、对比场景 |
| 4 | **逐词动效排版** | 文字逐词动画 | 核心论点、引言、关键信息 |
| 5 | **Lottie 动画** | 矢量动画播放 | 品牌动画、微交互（待确认支持状态） |
| 6 | **视频合成** | 内嵌视频播放、遮罩、叠加 | 演示画面、屏幕录制 |
| 7 | **逐字打字机** | 终端风格代码揭示 | 开发者工具、终端演示 |
| 8 | **可变字体轴动画** | 字重/字宽/倾角随时间变化 | 高级排版、品牌字标 |
| 9 | **路径动画** | 元素沿 SVG 曲线运动 | 动态入场、连接线动画 |
| 10 | **速度匹配转场** | 离场/入场方向一致的速度感 | 拍点过渡、场景切换 |
| 11 | **音频响应动效** | 元素随音频节奏脉动 | 品牌短片、产品发布、音乐驱动视频 |
| 12 | **Clip-Path 遮罩揭示** | 固定窗口内容从中穿过 | 标题开场、图片揭示 |
| 13 | **WebGL 着色器背景** | GPU 生成式背景 — FBM、余弦配色函数 | Hero 背景、氛围场景 |

---

## 1. SVG 路径绘制

路径在实时中一笔笔画出，像用笔描摹。适合揭示图表、箭头、连接线或品牌标志。

```xml
<div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
  <path id="draw-path" class="stroke-orange-600 stroke-[4] fill-none stroke-linecap-round"
    d="M 50 100 L 200 50 L 350 100" />
</div>
<script>
  ctx.fromTo('draw-path', { strokeDashoffset: 280 }, { strokeDashoffset: 0, duration: 0.7, ease: 'ease-out' }, 0.5);
</script>
```

用 `path.getTotalLength()` 计算 dasharray 值。

---

## 2. Canvas 程序化生成

按输出采样点重绘的动画噪点、粒子场、数据可视化。用 `<canvas>` + CanvasKit 绘制，用 `ctx.time` / `ctx.currentTime` 驱动时间。

```xml
<canvas id="proc-canvas" class="w-full h-full" />
<script>
  var CK = ctx.CanvasKit;
  var canvas = ctx.getCanvas();
  var paint = new CK.Paint();
  paint.setStyle(CK.PaintStyle.Fill);

  function hash(x, y) {
    var n = x * 374761393 + y * 668265263;
    n = (n ^ (n >> 13)) * 1274126177;
    return ((n ^ (n >> 16)) & 0x7fffffff) / 0x7fffffff;
  }

  canvas.clear(CK.Color(10, 10, 10, 1));
  for (var i = 0; i < 200; i++) {
    var x = hash(i, 0) * 1920;
    var y = hash(i, 1) * 1080;
    var brightness = hash(i, Math.floor(ctx.time * 3)) * 255;
    paint.setColor(CK.Color4f(1, 1, 1, brightness / 255));
    canvas.drawCircle(x, y, 2, paint);
  }
</script>
```

`hash()` 函数是确定性的 — 同一时间点每次渲染相同。

---

## 3. 3D 变换

透视旋转创造深度。适合产品展示、卡片翻转。

```xml
<div id="stage" class="w-[400px] h-[300px]">
  <div id="card-3d" class="w-full h-full bg-gradient-to-br from-blue-500 to-purple-600 rounded-[12px]">
    <text id="face" class="text-[36px] text-white font-bold">Product</text>
  </div>
</div>
<script>
  ctx.fromTo('card-3d', { rotateY: 0, rotateX: 0 }, { rotateY: 360, rotateX: 15, duration: 1.2, ease: 'ease-in-out' }, 0);
</script>
```

---

## 4. 逐词动效排版

逐词出现，与字幕时间戳同步。叙事驱动视频的核心技法。

```xml
<div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
  <text id="headline" class="text-[48px] text-white">Anything a browser can render</text>
</div>
<script>
  var words = ctx.splitText('headline', { type: 'words' });
  var timings = [0, 0.23, 0.3, 0.63, 0.8];
  var slides = [80, 60, 50, 25, 12];

  words.forEach(function (w, i) {
    ctx.from(w, { x: slides[i], y: 14, opacity: 0, duration: 0.37, ease: 'ease-out', delay: timings[i] });
  });
</script>
```

滑动距离逐词**递减**（80→12px）— 模拟相机稳定下来的过程。

---

## 5. Lottie 动画

矢量动画在合成中播放。适合 Logo、角色动画、图标。

> **注意：** OpenCat 当前渲染器不支持 Lottie 播放。本条目预留作为将来能力。支持后，通过 `<image>` 节点引用 `.json` 或 `.lottie` 文件，用 `ctx.*()` 控制可见性和缩放。

```xml
<!-- 预留语法，lottie 支持后启用 -->
<!-- <image id="logo-anim" query="lottie animation" /> -->
```

---

## 6. 视频合成

在合成中嵌入真实视频。视频必须使用 `<video>` 节点。

```xml
<div id="root" class="relative w-full h-full bg-slate-950">
  <video id="footage" class="w-full h-full object-cover"
    src="https://example.com/clip.mp4" data-start="0" data-duration="60" loop />
  <div id="overlay" class="absolute bottom-[40px] left-[40px] px-[20px] py-[12px] rounded-[12px] bg-black/60">
    <text id="label" class="text-[24px] text-white">Demo</text>
  </div>
</div>
<script>
  ctx.from('overlay', { opacity: 0, y: 20, duration: 0.4, ease: 'ease-out' }, 0.2);
</script>
```

---

## 7. 逐字打字机

通过 `text` 属性动画实现字符逐个显示。

```xml
<div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
  <div class="flex items-center gap-[8px]">
    <text id="prompt" class="text-[24px] text-green-400">❯</text>
    <text id="typed" class="text-[24px] text-white"></text>
    <div id="cursor" class="w-[11px] h-[22px] bg-slate-300"></div>
  </div>
</div>
<script>
  // 打字效果
  ctx.to('typed', { text: 'npx opencat render', duration: 1, ease: 'linear' });
  // 光标闪烁（循环使用 ctx.time）
  // 光标用 CSS 或 canvas 绘制；此处示意
</script>
```

---

## 8. 可变字体轴动画

动画 `font-variation-settings` 属性实时改变字形。需要支持可变字体的字体文件。

```xml
<div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
  <text id="wordmark" class="text-[200px] text-white"
    style="font-family:'Fraunces',serif;font-variation-settings:'opsz' 144,'wght' 440;">
    OpenCat
  </text>
</div>
<script>
  ctx.fromTo('wordmark', { fontVariation: { opsz: 144, wght: 440 } }, { fontVariation: { opsz: 72, wght: 300 }, duration: 0.47, ease: 'ease-out' }, 0);
</script>
```

> **注意：** OpenCat 渲染器需要支持 `font-variation-settings` 属性。以上使用 `fontVariation` 作为 CSS 变量映射的约定属性名，具体实现请查阅运行时支持情况。

---

## 9. 路径动画

沿 SVG 路径运动。适合曲线轨迹、轨道运动。

```xml
<svg class="absolute inset-0 w-full h-full">
  <path id="motion-path" class="fill-none" d="M 12 300 C 280 280 520 80 820 50 S 1200 48 1308 38" />
</svg>
<div id="dot" class="w-[20px] h-[20px] bg-teal-500 rounded-full absolute" />
<script>
  ctx.to('dot', { path: 'M 12 300 C 280 280 520 80 820 50 S 1200 48 1308 38', orient: 0, duration: 1.5, ease: 'ease-out' }, 0);
</script>
```

---

## 10. 速度匹配转场

离场和入场使用方向一致的速度感。离场用 `ease-in`（加速），入场用 `ease-out`（减速），两条曲线在切点速度匹配。

```js
// 场景 A 离场（在结束前 0.33 秒开始）
var exitStart = Math.max(ctx.sceneDuration - 0.33, 0);
ctx.to('.content', { opacity: 0, y: -150, duration: 0.33, ease: 'ease-in' }, exitStart);

// 场景 B 入场（从相同方向的相反侧进入）
  // 初始值来自节点 class 或上一场景的残留值
  // 如果担心残值，用 fromTo 指定两端
  // ctx.fromTo('.content', { opacity: 0, y: 150 }, { opacity: 1, y: 0, duration: 0.6 });
ctx.to('.content', { opacity: 1, y: 0, duration: 0.6, ease: 'ease-out' }, 0);
```

两条曲线的最快点在转场处相遇 — 观众感知到连续相机运动。

---

## 11. 音频响应动效

驱动任意 tween 属性从播放音频中。低音驱动 Logo 缩放，高音驱动 CTA 辉光。

> 完整 API 和反模式见 [audio-reactive.md](audio-reactive.md)。

```js
// 音频数据预提取后，用 ctx.time 换算到音频分析数组
var index = Math.floor(ctx.time * audioData.fps);
var sample = audioData.samples[index];
var bass = sample ? sample.bands[0] : 0; // 0-1
ctx.getNode('logo').scale(1 + bass * 0.04);
```

---

## 12. Clip-Path 遮罩揭示

固定窗口，内容从一侧穿入。不同于 SVG 路径绘制：遮罩静止，内容运动。

```xml
<div id="root" class="relative w-full h-full bg-slate-950 flex items-center justify-center overflow-hidden">
  <div id="slide-content" class="absolute">
    <text id="headline" class="text-[108px] text-white whitespace-nowrap">Your Headline</text>
  </div>
</div>
<script>
  // 初始 x 和 opacity 来自节点 class 或 fromTo
  // 使用 fromTo 更确定：
  // ctx.fromTo('slide-content', { x: 400, opacity: 0 }, { x: 0, opacity: 1, duration: 1 });
  ctx.to('slide-content', { x: 0, opacity: 1, duration: 1, ease: 'ease-out' }, 0);
</script>
```

变形：从中心圆形展开（配合 canvas 绘制遮罩）。

---

## 13. WebGL 着色器背景

GPU 生成式背景 — 域扭曲 FBM 噪点、余弦配色函数。OpenCat 通过 `gl_transition` 机制和 `<transition effect="着色器名">` 支持。

```xml
<transition from="scene1" to="scene2" effect="Dreamy" duration="0.5" />
```

预装着色器列表见 [transitions.md](transitions.md) 的 GL 转场章节。自定义着色器放在 `gltransition.json` 中。

---

## 缓动词汇表

每种组合应使用至少 3 种不同的缓动 — 全部用 `ease-out` 产生单调呆板的运动。

| 缓动 | 性格 | 典型用途 |
|------|------|---------|
| `ease-out` | 自信、响应 | 入场（默认） |
| `ease-in` | 加速离开 | 退场 |
| `ease-in-out` | 平稳、梦幻 | 位置间移动 |
| `back-out(N)` | 轻微回弹 | Logo 揭示、卡片弹出 |
| `elastic-out(amp,period)` | 弹性振荡 | 面板散落、能量展示 |
| `bounce-out` | 球体弹跳 | 物理交互、计分器 |
| `spring.gentle` | 弹簧柔和 | 常规入场 |
| `spring.wobbly` | 弹簧摇摆 | 俏皮入场 |

缓动映射到情感：`ease-out` = 自信果断，`ease-in-out` = 梦幻流畅，`elastic-out` = 俏皮活泼。
