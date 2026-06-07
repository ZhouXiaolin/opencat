# 经典模板

常用 OpenCat XML 合成模式。每个模板展示一种结构思路，按需改编。格式细节参照 `opencat.md` 和 `animations.md`。

---

## 1. 简单标题卡

单 scene，居中标题 + 入场动画。适合短消息、CTA、开场定帧。

```xml
<opencat width="1280" height="720" fps="30" duration="3">
  <div id="root" class="flex flex-col items-center justify-center w-full h-full bg-slate-950">
    <text id="title" class="text-[96px] font-bold text-white">Hello OpenCat</text>
    <text id="subtitle" class="text-[28px] text-slate-400 mt-[16px]">Design-driven video synthesis</text>
  </div>
  <script>
    ctx.fromTo('title',
      { opacity: 0, y: 32 },
      { opacity: 1, y: 0, duration: 0.65, ease: 'power3.out' });
    ctx.fromTo('subtitle',
      { opacity: 0, y: 18 },
      { opacity: 1, y: 0, duration: 0.5, delay: 0.12, ease: 'power2.out' });
  </script>
</opencat>
```

---

## 2. 分区信息卡（Feature Panel）

左右或上下分区。一侧标题/标签，一侧内容面板。背景网格线 + 坐标标记增强精密感。

```xml
<opencat width="1280" height="720" fps="30" duration="4">
  <div id="root" class="relative w-[1280px] h-[720px] bg-[#0a0d10]">
    <!-- 结构线 -->
    <div id="grid-h" class="absolute left-[80px] top-[360px] w-[1120px] h-[1px] bg-[#1d2934]" />

    <!-- 左侧：标签 + 标题 -->
    <div id="left-panel" class="absolute left-[80px] top-[100px] flex flex-col gap-[16px]">
      <text id="label" class="text-[14px] font-semibold text-[#46f5b5]">FEATURE</text>
      <text id="headline" class="text-[64px] font-bold text-white leading-[1]">Real-time\nrendering</text>
    </div>

    <!-- 右侧：信息卡 -->
    <div id="card" class="absolute right-[80px] top-[100px] w-[480px] h-[520px] border-[1px] border-[#2b3a46] bg-[#111820]">
      <text id="card-content" class="absolute left-[32px] top-[32px] text-[20px] text-slate-400">
        Content goes here — stats, code, images...
      </text>
    </div>
  </div>
  <script>
    ctx.fromTo('grid-h', { scaleX: 0 }, { scaleX: 1, duration: 0.4, ease: 'power2.out' });
    ctx.fromTo('label',
      { opacity: 0, x: -20 },
      { opacity: 1, x: 0, duration: 0.5, ease: 'power3.out' });
    ctx.fromTo('headline',
      { opacity: 0, y: 28, scale: 0.96 },
      { opacity: 1, y: 0, scale: 1, duration: 0.7, ease: 'spring.gentle' });
    ctx.fromTo('card',
      { opacity: 0, y: 18, scale: 0.95 },
      { opacity: 1, y: 0, scale: 1, duration: 0.6, delay: 0.15, ease: 'power2.out' });
  </script>
</opencat>
```

---

## 3. 多场景 Timeline + 转场

多 scene 通过 `<tl>` 串联，每对相邻 scene 有 `<transition>`。

```xml
<opencat width="1280" height="720" fps="30" duration="8.6">
  <div id="root" class="relative w-[1280px] h-[720px] bg-slate-950">
    <tl id="main-tl" class="absolute inset-0">
      <div id="scene1" class="flex flex-col items-center justify-center w-full h-full" duration="4">
        <text id="s1-title" class="text-[72px] font-bold text-white">Scene 1</text>
      </div>

      <transition from="scene1" to="scene2" effect="fade" duration="0.6" timing="ease-in-out" />

      <div id="scene2" class="flex flex-col items-center justify-center w-full h-full bg-slate-900" duration="4">
        <text id="s2-heading" class="text-[72px] font-bold text-white">Scene 2</text>
      </div>
    </tl>
  </div>
   <script>
    // Scene 1 动画
    ctx.fromTo('s1-title',
      { opacity: 0, y: 30 },
      { opacity: 1, y: 0, duration: 0.7, ease: 'spring.gentle' });
    // Scene 2 动画
    ctx.fromTo('s2-heading',
      { opacity: 0, x: -40 },
      { opacity: 1, x: 0, duration: 0.6, delay: 4.7, ease: 'power3.out' });
  </script>
</opencat>
```

---

## 4. 数据展示

数字 count up + 进度条 + 标签。数据视觉化的核心是让数字有重量，不只是大号字。

```xml
<opencat width="1280" height="720" fps="30" duration="5">
  <div id="root" class="relative w-[1280px] h-[720px] bg-slate-950">
    <!-- 背景装饰 -->
    <div class="absolute left-[100px] top-[140px] w-[1px] h-[440px] bg-slate-800" />
    <div class="absolute left-[100px] top-[580px] w-[1080px] h-[1px] bg-slate-800" />

    <div class="absolute left-[140px] top-[180px] flex flex-col gap-[20px]">
      <text id="metric-label" class="text-[18px] font-semibold tracking-[2px] text-sky-400">MONTHLY ACTIVE USERS</text>
      <text id="metric-value" class="text-[120px] font-bold text-white leading-[1]">2.4M</text>
      <div id="bar-track" class="w-[600px] h-[8px] bg-slate-800 rounded-full overflow-hidden">
        <div id="bar-fill" class="w-full h-full bg-sky-400 rounded-full origin-left" />
      </div>
      <text id="metric-desc" class="text-[22px] text-slate-400">↑ 23% from last quarter</text>
    </div>
  </div>
  <script>
    // 标签入场
    ctx.fromTo('metric-label',
      { opacity: 0, y: 14 },
      { opacity: 1, y: 0, duration: 0.4, ease: 'power2.out' });

    // 数字放大入场
    ctx.fromTo('metric-value',
      { opacity: 0, scale: 0.8, y: 20 },
      { opacity: 1, scale: 1, y: 0, duration: 0.7, ease: 'spring.gentle' });

    // 进度条从 0 拉伸
    ctx.fromTo('bar-fill',
      { scaleX: 0 },
      { scaleX: 1, duration: 1.2, delay: 0.3, ease: 'power2.out' });

    // 描述文字淡入
    ctx.fromTo('metric-desc',
      { opacity: 0 },
      { opacity: 1, duration: 0.5, delay: 0.6, ease: 'power2.out' });
  </script>
</opencat>
```

---

## 5. Canvas 程序化背景

用 CanvasKit 生成粒子、噪点、程序化纹理。Canvas 作为背景层，XML 布局叠加在上面。

```xml
<opencat width="1280" height="720" fps="30" duration="4">
  <div id="root" class="relative w-[1280px] h-[720px]">
    <canvas id="bg-canvas" class="absolute inset-0 w-[1280px] h-[720px]" />
    <text id="title" class="absolute left-[100px] top-[280px] text-[80px] font-bold text-white">
      Canvas Background
    </text>
  </div>
  <script>
    var CK = ctx.CanvasKit;
    var canvas = ctx.getCanvasById('bg-canvas');

    function hash(x, y) {
      var n = x * 374761393 + y * 668265263;
      n = (n ^ (n >> 13)) * 1274126177;
      return ((n ^ (n >> 16)) & 0x7fffffff) / 0x7fffffff;
    }

    var paint = new CK.Paint();
    paint.setStyle(CK.PaintStyle.Fill);

    canvas.clear(CK.Color(10, 10, 16, 1));

    for (var i = 0; i < 150; i++) {
      var x = hash(i, 0) * 1280;
      var y = hash(i, 1) * 720;
      var size = hash(i, 2) * 3 + 0.5;
      var pulse = 0.3 + 0.7 * Math.sin(ctx.time * 2.5 + i * 0.7);
      paint.setColorComponents(0.27, 0.96, 0.71, 0.15 + 0.45 * pulse);
      canvas.drawCircle(x, y, size, paint);
    }

    // 标题动画
    ctx.fromTo('title',
      { opacity: 0, y: 30 },
      { opacity: 1, y: 0, duration: 0.65, ease: 'power3.out' });
  </script>
</opencat>
```

---

## 6. 视频 + 叠加层

视频作为背景或主画面，叠加标签、badge、caption。

```xml
<opencat width="1280" height="720" fps="30" duration="6">
  <div id="root" class="relative w-full h-full bg-black">
    <video id="bg-video"
      class="absolute inset-0 w-full h-full object-cover"
      url="https://example.com/video.mp4"
      loop="true" />
    <div id="badge"
      class="absolute left-[40px] top-[40px] px-[14px] py-[8px] rounded-full bg-black/60 border border-white/20">
      <text class="text-[14px] font-semibold tracking-[1px] text-white">LIVE</text>
    </div>
    <div id="caption"
      class="absolute bottom-[40px] left-[40px] px-[24px] py-[14px] rounded-[12px] bg-black/60">
      <text class="text-[24px] text-white font-semibold">Video Overlay</text>
    </div>
  </div>
   <script>
    ctx.fromTo('badge',
      { opacity: 0, scale: 0.8 },
      { opacity: 1, scale: 1, duration: 0.5, delay: 0.3, ease: 'back.out' });
    ctx.fromTo('caption',
      { opacity: 0, y: 20 },
      { opacity: 1, y: 0, duration: 0.5, delay: 0.5, ease: 'power2.out' });
  </script>
</opencat>
```

---

## 7. Subtree + RuntimeEffect

把 XML 子树作为画面纹理，经过 SKSL shader 处理后画回 canvas。用于扭曲、玻璃、portal 等 hero 效果。

```xml
<opencat width="1280" height="720" fps="30" duration="4">

  <div id="root" class="flex w-[1280px] h-[720px] bg-black">
    <canvas id="surface" class="w-[640px] h-[720px]">
      <!-- Hidden subtree: 实际 XML 布局 -->
      <div id="subtree" class="flex flex-col items-center justify-center w-[640px] h-[720px] bg-sky-900">
        <text id="subtree-content" class="text-[48px] font-bold text-white">Subtree Content</text>
      </div>
    </canvas>
    <div id="right-panel" class="flex flex-col items-center justify-center w-[640px] h-[720px]">
      <text id="right-panel-content" class="text-[24px] text-slate-400">Right side normal XML</text>
    </div>
  </div>
  <script>
    var CK = ctx.CanvasKit;
    var canvas = ctx.getCanvasById('surface');
    var child = canvas.getSubTree().makeShader(CK.TileMode.Clamp, CK.TileMode.Clamp);

    var sksl = [
      'uniform shader image;',
      'uniform float t;',
      'half4 main(float2 xy) {',
      '  float2 uv = xy + float2(sin(xy.y * 0.04 + t * 4.0) * 6.0, 0);',
      '  return image.eval(uv);',
      '}',
    ].join('\n');

    var effect = CK.RuntimeEffect.Make(sksl);
    if (effect) {
      var shader = effect.makeShaderWithChildren([ctx.currentTime], [child]);
      var paint = new CK.Paint();
      paint.setShader(shader);
      canvas.drawRect(CK.XYWHRect(0, 0, 640, 720), paint);
    }
  </script>
</opencat>
```

---

## 常用动画模式

### 稳定入场

```js
ctx.fromTo('title',
  { opacity: 0, y: 32 },
  { opacity: 1, y: 0, duration: 0.65, ease: 'power3.out' });
```

### 分阶段编排

```js
ctx.timeline({ defaults: { ease: 'power2.out' } })
  .fromTo('title', { opacity: 0, y: 32 }, { opacity: 1, y: 0, duration: 0.6 }, 0)
  .fromTo('subtitle', { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.5 }, '<+=0.15')
  .to('accent', { scaleX: 1, duration: 0.45 }, '<+=0.1');
```

### 逐字入场

```js
ctx.from(ctx.splitText('title', { type: 'chars' }), {
  opacity: 0, y: 28, scale: 0.9,
  duration: 0.5, stagger: 0.035, ease: 'power2.out',
});
```

### 乱码揭示

```js
ctx.to('title', {
  scrambleText: { text: 'SYSTEM ONLINE', chars: 'upperCase', speed: 24 },
  duration: 1.2, ease: 'linear',
});
```

### 呼吸微运动

```js
ctx.to('decoration', {
  scale: 1.08, opacity: 0.6,
  duration: 1.5, repeat: -1, yoyo: true, ease: 'sine.inOut',
});
```

### 进度条拉伸

```js
ctx.fromTo('bar', { scaleX: 0 }, { scaleX: 1, duration: 1.2, ease: 'power2.out' });
```

### 路径绘制

```js
ctx.to('icon-path', {
  strokeDashoffset: 0, duration: 0.7, ease: 'power2.out',
});
```
