# 动画接口（`useGSAP`）

动效用 `useGSAP` 写，模型和 GSAP 的 tween / timeline 一致：`ctx.from` / `to` / `fromTo` / `set` / `timeline`，position 参数、`stagger`、`keyframes`、ease 名的写法都沿用 GSAP 惯例。

这份文件只写 OpenCat 特有的部分：钩子入口、`ctx` 上的扩展、引擎真正支持的属性集合、内置插件、节点 API 和工具函数。GSAP 本身的用法不再重复。

所有时间单位都是秒。

---

## 入口

```jsx
import { useGSAP } from 'opencat/runtime';

useGSAP((ctx) => {
  ctx.fromTo('title',
    { opacity: 0, y: 36 },
    { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' });
});
```

钩子在每帧被调用一次。因此动画要写成对时间的纯函数：不要在钩子顶层用变量记录上一帧状态，不要用 `setTimeout` / `setInterval` / 事件监听驱动关键动画。

**边界**：`useGSAP` 只写 tween。CanvasKit 绘制放 `useCanvas`，three.js 场景放 `useThree`。不要在 `useGSAP` 里调 `getCanvasById` 加绘制——一个组件可以挂多个钩子，按职责分开写。

`targets` 支持单个 id、id 数组、`ctx.splitText(...)` 返回的 part 数组。

### 选择规则

1. 优先 `fromTo`。两端写死，任意时间点采样都稳定，适合跳帧和抽帧校验。
2. 入场可以用 `from`，终点由 class 决定。
3. `to` 适合循环 / 呼吸 / 路径运动。
4. 多段动作用 `timeline` 编排，不要散落大量独立 tween 加手写 `delay`。
5. `set` 只做瞬时状态。

### 返回值

顶层 `ctx.from()` / `to()` / `fromTo()` 立即返回当前帧的采样结果。单目标返回对象，多目标返回数组，可用于跨节点联动。

```js
var title = ctx.fromTo('title',
  { opacity: 0, y: 40 },
  { opacity: 1, y: 0, duration: 0.7, ease: 'spring.gentle' });

ctx.getNode('subtitle').opacity(title.opacity * 0.85).translateY(title.y * 0.5);
```

Timeline 链式方法返回 timeline 本身，不要依赖子项直接返回采样值。

---

## OpenCat 的差异

- **ease 名沿用 GSAP**，额外支持 spring：`spring.default` / `spring.gentle` / `spring.stiff` / `spring.slow` / `spring.wobbly`，或 `{ spring: { stiffness, damping, mass } }`。
- 带参 ease 的两种行为不同：`back.out(2.5)` 的过冲参数**真的生效**；`elastic.out(a, p)` 的幅度/周期参数**被忽略**，总是用默认弹性曲线。
- `clamp` 默认 `true`，进度钳位到 `[0, 1]`。
- `transformOrigin` / `svgOrigin` 是**快照属性**（不插值）：整个 tween 内恒为该 tween 自己声明的值（`.to` / `.fromTo` 里写的就是终点值）。所以中途换锚点要拆成两个 tween，不能指望它平滑过渡。取值同 CSS，百分比要带 `%`。
- `keyframes` 两种形式：状态数组（每段一组属性快照，至少一个带 `duration`）与数值曲线对象（`{ scale: [1, 1.2, 1] }`）。只建议用于数值属性；颜色、文字、路径变形拆成多个 tween 更稳。

---

## 可动画属性

引擎实际支持的属性集合。用集合外的属性不会生效。

### Transform

| 属性 | 别名 | 说明 |
| --- | --- | --- |
| `opacity` | - | 透明度 |
| `x` | `translateX` | 水平位移 |
| `y` | `translateY` | 垂直位移 |
| `scale` | - | 等比缩放 |
| `scaleX` / `scaleY` | - | 单向缩放 |
| `rotation` | `rotate` | 旋转角度 |
| `skewX` / `skewY` | - | 倾斜 |
| `transformOrigin` | `svgOrigin` | 变换锚点（快照属性） |
| `rotationX` / `rotationY` | `rotateX` / `rotateY` | 3D 旋转 |
| `transformPerspective` | - | 透视，全量写入不插值 |

### Layout / Style

| 属性 | 说明 |
| --- | --- |
| `left` / `top` / `right` / `bottom` | 定位偏移 |
| `width` / `height` | 尺寸 |
| `borderRadius` | 圆角 |
| `borderWidth` | 边框宽度 |
| `strokeWidth` | 描边宽度 |
| `strokeDasharray` | 描边虚线长度 |
| `strokeDashoffset` | 描边虚线偏移 |
| `textSize` | 文字大小 |
| `letterSpacing` | 字间距 |
| `lineHeight` | 行高 |
| `backdropBlur` | 背景模糊 |
| `clipPath` | CSS clip-path，可插值 |

### Color

`backgroundColor`（别名 `bg`）、`textColor`（别名 `color`）、`borderColor`、`fillColor`、`strokeColor`。

### Filter

`blur` / `blurSigma` / `brightness` / `contrast` / `grayscale` / `hueRotate` / `invert` / `saturate` / `sepia` / `filter`（CSS filter 字符串，会解析并插值）。

---

## 官方插件

`import { gsap } from 'gsap'` 加上插件自己的子路径（`import { CustomEase } from 'gsap/CustomEase'`）。
插件由 core 的 bundle 在启动时注册，不需要 `gsap.registerPlugin()`。

### 物理插件：vars 里不要给同名终点

`physics2D` / `physicsProps` 自己驱动属性。再写一个同名终点会生成第二条 PropTween，
把物理结果覆盖成线性插值：

```js
// 错：x 的普通 tween 排在物理插件之后，覆盖它的输出
gsap.to('ball', { x: 1400, physics2D: { velocity: 700, angle: -50 }, duration: 3 });

// 对：终点由插件算，vars 只放物理参数
gsap.to('ball', { physics2D: { velocity: 700, angle: -50 }, duration: 3, ease: 'none' });
```

这是官方语义，不是引擎差异——真实浏览器里用普通对象作 target 同样如此。

### RoughEase：用对象形式固定随机表

`rough(...)` 的**字符串形式**把参数按位置传给 `config()`，只有第一个（`points`）生效，
`randomize` 仍是默认 `true`，缓动表由 `Math.random` 生成——每次运行都不一样。
要可复现就用对象形式：

```js
gsap.to('box', {
  x: 1560, duration: 3,
  ease: 'rough({strength:3, points:2, taper:"none", randomize:false})',
});
```

---

## 内置插件

直接写字段，不需要 `registerPlugin()`。

### Text — 打字机

```js
ctx.to('headline', { text: 'RENDER READY', cursor: '|', cursorBlink: 0.35, duration: 1.1, ease: 'linear' });
```

`mode` / `textMode` 只用默认 `typewriter`。`cursorBlink`：`false` 常显，数字是闪烁周期（秒）。

### ScrambleText — 乱码揭示

```js
ctx.to('code', {
  scrambleText: {
    text: 'SHORTHAND DIRECTION',
    chars: 'upperCase',   // upperCase / lowerCase / upperAndLowerCase / letters / numbers / digits / symbols / all
    speed: 20,            // 每秒 tick 数
    revealDelay: 0.1,
    tweenLength: true,
    rightToLeft: false,
  },
  duration: 1.4, ease: 'linear',
});
```

DOM-only 字段（`newClass` / `oldClass`）无效。

### SplitText — 拆成 part

```js
var chars = ctx.splitText('title', { type: 'chars' });

ctx.from(chars, { opacity: 0, y: 32, scale: 0.9, duration: 0.55, stagger: 0.035, ease: 'power2.out' });
```

`type: 'chars'` / `'words'`。`lines` 未实现。

part 支持的属性：`opacity`、`x` / `y`、`scale`、`rotate`、`color` / `textColor`。

### MotionPath — 沿 SVG path 运动

```js
ctx.to('rocket', {
  path: 'M100 360 C400 80 880 640 1180 360',
  orient: -90,
  duration: 4, ease: 'ease-in-out',
});
```

进度 `0→1` 映射到弧长，插件自动输出 `x` / `y` / `rotation`。`orient` 是切线角度额外偏移。

### MorphSVG — `<path>` 形状变形

```js
ctx.fromTo('shape',
  { d: 'M100 0 L200 200 L0 200 Z' },
  { d: 'M100 200 L200 0 L0 0 Z', duration: 1, ease: 'ease-in-out' });
```

字段写 `d` 或 `morphSVG`。对路径采样并对齐点，不要求命令数相同。目标节点应是 `<path>`，源和目标保持兼容开闭状态，主要支持单 contour。可选 `gridResolution` / `simplifyTolerance`。

---

## Node API

`ctx.getNode(id)` 返回节点代理，瞬时写入属性，不经过缓动。只在跨节点联动或特殊计算时用。

```js
ctx.getNode('subtitle').opacity(hero.opacity * 0.85).translateY(hero.y * 0.5);
```

---

## 工具函数

```js
ctx.utils.clamp(value, min, max)
ctx.utils.snap(value, step)
ctx.utils.wrap(value, min, max)
ctx.utils.mapRange(value, inMin, inMax, outMin, outMax)
ctx.utils.random(min, max)
ctx.utils.random(min, max, seed)
ctx.utils.randomInt(min, max, seed)
```

有随机值时传 seed，保证渲染确定。

---

## 常用写法

**稳定入场**

```js
ctx.fromTo('title', { opacity: 0, y: 32 }, { opacity: 1, y: 0, duration: 0.65, ease: 'power3.out' });
```

**分阶段编排**

```js
ctx.timeline({ defaults: { ease: 'power2.out' } })
  .fromTo('title', { opacity: 0, y: 32 }, { opacity: 1, y: 0, duration: 0.6 }, 0)
  .fromTo('subtitle', { opacity: 0, y: 18 }, { opacity: 1, y: 0, duration: 0.5 }, '<+=0.15')
  .to('accent', { scaleX: 1, duration: 0.45 }, '<+=0.1');
```

**呼吸循环**

```js
ctx.to('glow', { opacity: 0.8, scale: 1.08, duration: 1.2, repeat: -1, yoyo: true, ease: 'ease-in-out' });
```

**逐字入场**

```js
ctx.from(ctx.splitText('title', { type: 'chars' }), {
  opacity: 0, y: 28, duration: 0.5, stagger: 0.035, ease: 'power2.out',
});
```

---

## 同元素上的 tween 冲突

同一个元素上，**不要有两条并发的 transform tween**。入场和持续运动（呼吸、Ken Burns 推近）如果各写一条，后写的会覆盖前一条，元素可能永远不可见，而且没有任何报错。

两种正确做法：

```js
// A. 合并成一条：入场和持续运动写在同一个 fromTo 里
ctx.fromTo('hero',
  { opacity: 0, y: 50, scale: 1.0 },
  { opacity: 1, y: 0, scale: 1.04, duration: 2.4, ease: 'none' });

// B. 拆到父子两层：入场在外层，持续运动在内层
ctx.fromTo('hero-wrap', { opacity: 0, y: 50 }, { opacity: 1, y: 0, duration: 0.6, ease: 'power2.out' });
ctx.to('hero', { scale: 1.04, duration: 2.4, ease: 'none' });
```

顺序执行、不重叠的 transform 阶段是安全的。危险的是并发：每帧会独立采样每条 tween，同一个属性被两条 tween 同时写，结果不确定。

---

## 反模式

- 不要在钩子顶层用可变变量记录上一帧状态。
- 不要用 `setTimeout`、`setInterval`、异步回调或事件监听驱动关键动画。
- 不要用非种子随机（`Math.random()`）——同一次渲染每次结果不同。
- 不要用 `to` 隐式假设复杂起点；需要稳定采样用 `fromTo`。
- 不要在同一个 scene 里堆大量独立 tween；用 `timeline`。
- 不要在同一元素上并发两条 transform tween。
