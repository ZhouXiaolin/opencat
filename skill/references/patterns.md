# 组合模式

可复用的合成布局与动效模式。

---

## 画中画（视频在框架中）

动画包装 div 的位置/大小。视频填满包装器。

```xml
<div id="root" class="relative w-full h-full bg-slate-950">
  <div id="pip-frame" class="absolute top-0 left-0 w-full h-full overflow-hidden">
    <video id="footage" class="w-full h-full object-cover"
      src="talking-head.mp4" data-start="0" data-duration="60" loop />
  </div>
</div>
<script>
  // 缩小到右下角
  ctx.fromTo('pip-frame',
    { top: 0, left: 0, width: '100%', height: '100%' },
    { top: 700, left: 1360, width: 500, height: 280, duration: 30, ease: 'ease-out' },
    300
  );
  // 移动到左边缘
  ctx.to('pip-frame', { left: 40, duration: 18, ease: 'ease-out' }, 900);
</script>
```

---

## 幻灯片（带分段标题）

使用独立元素，每个有其自己的时间范围。

```xml
<div id="root" class="relative w-full h-full">
  <div id="slide1" class="flex items-center justify-center w-full h-full bg-white">
    <text class="text-[48px] text-slate-900">第一部分: 介绍</text>
  </div>
  <div id="slide2" class="flex items-center justify-center w-full h-full bg-slate-100">
    <text class="text-[48px] text-slate-900">第二部分: 核心功能</text>
  </div>
</div>
```

场景通过 `<tl>` + `<transition>` 管理。

---

## 入场 Stagger

```xml
<div id="root" class="flex flex-col items-center justify-center gap-[24px] w-full h-full bg-slate-950 p-[60px]">
  <div id="card-1" class="w-[400px] h-[200px] rounded-[12px] bg-slate-800" />
  <div id="card-2" class="w-[400px] h-[200px] rounded-[12px] bg-slate-800" />
  <div id="card-3" class="w-[400px] h-[200px] rounded-[12px] bg-slate-800" />
</div>
<script>
  ctx.fromTo(['card-1', 'card-2', 'card-3'], {
    opacity: 0, y: 30, scale: 0.95,
  }, {
    opacity: 1, y: 0, scale: 1,
    duration: 15, stagger: 4, ease: 'spring.gentle',
  });
</script>
```

**规则：**
- 最先移动的元素被认为最重要 — 按重要性顺序交错，不按 DOM 顺序
- 交错序列总长不超过 15 帧（0.5s @30fps）
- 入场重叠，不串行等待

---

## 计数器

```xml
<div id="root" class="flex flex-col items-center justify-center w-full h-full bg-slate-950">
  <text id="counter" class="text-[120px] text-white font-bold">0</text>
  <text class="text-[24px] text-slate-400">活跃用户</text>
</div>
<script>
  ctx.to('counter', { number: 135000, duration: 45, ease: 'ease-out', format: { useGrouping: true } }, 6);
</script>
```

---

## 背景装饰层

每场景 2-5 个装饰元素，给场景视觉深度：

```xml
<div id="root" class="relative w-full h-full bg-slate-950 overflow-hidden">
  <!-- 径向发光 -->
  <div id="glow" class="absolute top-1/2 left-1/2 w-[400px] h-[400px] -translate-x-1/2 -translate-y-1/2 rounded-full bg-blue-500/10" />
  <!-- 幽灵文字 -->
  <text id="ghost" class="absolute text-[200px] text-white/5 font-bold">OPEN</text>
  <!-- 强调线 -->
  <div id="rule" class="absolute top-[60px] left-[60px] right-[60px] h-[1px] bg-slate-700" />
  <!-- 前景内容 -->
  <div class="absolute inset-0 flex items-center justify-center">
    <text class="text-[72px] text-white font-bold">标题</text>
  </div>
</div>
<script>
  // 呼吸发光
  var tl = ctx.timeline();
  tl.to('glow', { scale: 1.15, duration: 90, repeat: -1, yoyo: true, ease: 'sine.inOut' }, 0);
  // 幽灵文字漂浮
  tl.to('ghost', { y: -20, duration: 120, repeat: -1, yoyo: true, ease: 'sine.inOut' }, 0);
</script>
```

---

## 标题卡片

```xml
<div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
  <div class="flex flex-col items-center gap-[12px]">
    <text id="title" class="text-[64px] text-white font-bold opacity-0">我的视频标题</text>
    <text id="subtitle" class="text-[28px] text-slate-400 opacity-0">副标题</text>
  </div>
</div>
<script>
  ctx.fromTo('title', { opacity: 0, y: 20 }, { opacity: 1, y: 0, duration: 18, ease: 'ease-out' }, 9);
  ctx.fromTo('subtitle', { opacity: 0, y: 12 }, { opacity: 1, y: 0, duration: 12, ease: 'ease-out' }, 21);
</script>
```
