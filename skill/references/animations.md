# 普通动效

配合节点、变换、颜色、路径动画、变形 SVG。

---

## 节点变换

```js
// 基础变换
ctx.fromTo('card', {
  opacity: 0, x: -100, y: 50, scale: 0.8, rotation: -10,
}, {
  opacity: 1, x: 0, y: 0, scale: 1, rotation: 0,
  duration: 30, ease: 'spring.gentle',
});

// 组合变换
ctx.to('logo', {
  scale: 1.2, rotation: 360,
  duration: 60, ease: 'ease-in-out', repeat: -1, yoyo: true,
});
```

---

## 颜色动画

颜色在 HSLA 空间插值，色相取最短路径：

```js
// 背景色渐变
ctx.fromTo('card', {
  backgroundColor: '#ef4444',
}, {
  backgroundColor: 'hsl(220, 90%, 55%)',
  duration: 60, repeat: -1, yoyo: true,
});

// 文字颜色变化
ctx.fromTo('title', {
  color: '#ffffff',
}, {
  color: '#00C3FF',
  duration: 30, ease: 'ease-in-out',
});
```

---

## 路径动画

沿 SVG 路径运动：

```js
// 基础路径动画
ctx.to('rocket', {
  path: 'M100 360 C400 80 880 640 1180 360',
  orient: -90,
  duration: 120, ease: 'ease-in-out', repeat: -1, yoyo: true,
});

// 循环轨道
ctx.to('planet', {
  path: 'M540 360 A200 200 0 1 1 539.99 360',
  orient: 0,
  duration: 180, ease: 'linear', repeat: -1,
});

// 沿曲线入场
ctx.from('icon', {
  path: 'M-100 800 C200 600 600 200 960 540',
  orient: -90, opacity: 0,
  duration: 45, ease: 'ease-out',
});
```

语义：
- `path` 接受 SVG path data 字符串
- 进度 `0 → 1` 映射到弧长
- 目标接收 `x`、`y`、`rotation` 采样
- `rotation` 跟随切线角度；`orient` 添加常量偏移

---

## 变形路径 (morphSVG)

改变 `type: "path"` 节点的几何形状：

```js
// 三角形翻转
ctx.fromTo('shape', {
  d: 'M100 0 L200 200 L0 200 Z',
}, {
  d: 'M100 200 L200 0 L0 0 Z',
  duration: 30, ease: 'ease-in-out',
});

// blob 呼吸
ctx.timeline().to('blob', {
  keyframes: {
    d: [
      { at: 0, value: 'M100 20 C155 20 180 60 180 100 C180 155 140 180 100 180 C45 180 20 140 20 100 C20 55 50 20 100 20 Z' },
      { at: 0.5, value: 'M110 25 C160 40 175 70 170 105 C160 150 130 175 95 170 C50 165 25 135 30 95 C35 50 60 15 110 25 Z', easing: 'ease-in-out' },
      { at: 1, value: 'M100 20 C155 20 180 60 180 100 C180 155 140 180 100 180 C45 180 20 140 20 100 C20 55 50 20 100 20 Z' },
    ],
  },
  duration: 90, repeat: -1,
}, 0);
```

规则：
- 目标必须是 `type: "path"` 节点
- `from` 和 `to` 必须拓扑匹配（相同命令数、同开/闭状态）
- 中间帧通过弧长重采样和点对应生成

---

## Keyframes

```js
// 均匀分布
ctx.to('card', { keyframes: { scale: [1, 1.4, 0.8, 1] }, duration: 60 });

// 逐帧缓动
ctx.to('logo', {
  keyframes: {
    rotate: [
      { at: 0, value: 0 },
      { at: 0.5, value: 360, easing: 'back-out' },
      { at: 1, value: 0 },
    ],
  },
  duration: 60,
});
```

仅支持数值 keyframes。颜色 keyframes 需拆分为独立 tween。

---

## 交错入场

```js
ctx.fromTo(
  ['card-1', 'card-2', 'card-3'],
  { opacity: 0, y: 30, scale: 0.9 },
  {
    opacity: 1, y: 0, scale: 1,
    stagger: 4,
    ease: { spring: { stiffness: 80, damping: 14, mass: 1 } },
  }
);
```

---

## 联动运动

```js
var hero = ctx.fromTo('title',
  { opacity: 0, y: 40 },
  { opacity: 1, y: 0, duration: 20, ease: 'spring.gentle' }
);
ctx.getNode('subtitle')
  .opacity(Math.min(0.85, hero.opacity * 0.85))
  .translateY(hero.y * 0.6);
```

---

## 循环脉冲

```js
ctx.timeline().to('glow', {
  scale: 1.08, yoyo: true, repeat: 5, duration: 36, ease: 'sine.inOut',
}, 0);
```

---

## 逐节点手动控制

```js
var items = ['card-1', 'card-2', 'card-3'];
var anims = ctx.fromTo(items,
  { opacity: 0, y: 30, scale: 0.9 },
  { opacity: 1, y: 0, scale: 1, stagger: 4, ease: 'spring.gentle' }
);
items.forEach(function(id, i) {
  ctx.getNode(id).opacity(anims[i].opacity).translateY(anims[i].y).scale(anims[i].scale);
});
```

---

## Node API

```js
ctx.getNode('id')
  .opacity(0.5).translateX(100).translateY(50)
  .scale(1.5).rotate(45).skewX(10)
  .position('absolute').left(100).top(50)
  .width(200).height(100)
  .bg('blue-500').borderRadius(16)
  .textColor('white').textSize(24).fontWeight('bold')
  .strokeWidth(2).strokeColor('gray-300').fillColor('blue-500');
```

---

## 最佳实践

- 优先用变换属性（`x`/`y`/`scale`/`rotation`/`opacity`）
- 用 timeline 编排而非 delay 链
- 保存 tween/timeline 返回值以控制播放
- 每场景 3+ 种 easing
- 入场比退场长（12 帧出现、7-8 帧消失）
- 首个 tween 偏移 3-9 帧
