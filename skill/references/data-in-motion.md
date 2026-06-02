# Data in Motion

数据与统计在视频合成中的轻量指导。[house-style.md](house-style.md) 处理美学 — 这里只处理数据特有的陷阱。

---

## 视觉连续性

当连续统计属于同一概念时（Q1→Q2→Q3→Q4，或同一产品的三个指标），保持在同一视觉空间，使用相同的美学。只有**数值**发生变化。美学变化应标志一个新概念，而不仅仅是一个新数字。

## 数值需要视觉权重

单独的数字在空白空间中漂浮。每个指标都要配一个赋予它存在感的视觉元素 — 比例填充条、背景色偏移、代表值的形状、进度环。这个视觉不需要是图表 — 它只需要填充画面，让数据感觉有实体感，而不仅仅是背景上的文字。

**好的做法：**

- 数值从 0 计数到目标（`COUNTS UP`）
- 伴随进度条或圆环填充
- 数值旁边有比例背景色块
- 数据点的几何排列（散点、气泡）

## 数据计数动画

```xml
<div id="root" class="flex items-center justify-center w-full h-full bg-slate-950">
  <text id="stat" class="text-[120px] text-blue-400 font-bold">0</text>
</div>
<script>
  // 从 0 计数到 1.9 万
  var targetNumber = 19000;
  ctx.to('stat', { number: targetNumber, duration: 45, ease: 'ease-out', format: { style: 'currency', currency: 'USD', maximumFractionDigits: 0 } }, 0);
</script>
```

## 数值 + 进度条组合

```xml
<div id="root" class="flex flex-col items-center justify-center gap-[24px] w-full h-full bg-slate-950 p-[60px]">
  <div class="flex items-baseline gap-[12px]">
    <text id="pct" class="text-[80px] text-white font-bold">87%</text>
    <text id="label" class="text-[24px] text-slate-400">Conversion Rate</text>
  </div>
  <div id="bar-bg" class="w-full h-[12px] rounded-full bg-slate-800 overflow-hidden">
    <div id="bar-fill" class="h-full rounded-full bg-gradient-to-r from-blue-500 to-cyan-400 w-0" />
  </div>
</div>
<script>
  // 初始值 0% 由 class 或 fromTo 提供
  ctx.fromTo('bar-fill', { width: '0%' }, { width: '87%', duration: 36, ease: 'ease-out' }, 6);
  ctx.to('bar-fill', { width: '87%', duration: 36, ease: 'ease-out' }, 6);
</script>
```

## 避免网页模式

- **无饼图** — 难以比较，看起来像 PowerPoint
- **无多轴图表** — 观众无法在 3 秒的展示窗口内研究交叉点
- **无 6 面板仪表盘** — 2-3 个相关指标并排放置就可以了，6+ 是网页模式
- **无网格线、刻度标记、图例** — 运动中无用的视觉噪音
- **无图表库输出** — 用动画系统 + div/canvas 构建，不要用第三方图表库
