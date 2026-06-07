# 动画 API

OpenCat 的动效脚本模仿 GSAP 的 tween / timeline 模型，但运行方式是视频渲染友好的：每一帧都会重新执行脚本，引擎根据 `ctx.currentTime` 采样 tween，输出当前帧的属性值。

因此脚本应该写成确定性的时间函数，不要依赖 JS 变量在帧与帧之间累积状态。所有时间单位都是秒。

---

## 核心模型

核心 API 只有四个：

| API | 用途 | 起点 | 终点 |
| --- | --- | --- | --- |
| `ctx.from(targets, vars)` | 入场动画 | `vars` | XML / class / 当前静态样式 |
| `ctx.to(targets, vars)` | 到达目标状态 | XML / class / 上一个 tween 的采样结果 | `vars` |
| `ctx.fromTo(targets, fromVars, toVars)` | 显式两端动画 | `fromVars` | `toVars` |
| `ctx.timeline(opts)` | 多段动画编排 | 子 tween 决定 | 子 tween 决定 |

`ctx.set(targets, vars)` 是瞬时写入，不是独立动画模型。它等价于零时长状态设置，常用于 timeline 中的开关状态。

`targets` 支持：

```js
'title'                              // 单个 id
['card-1', 'card-2', 'card-3']       // id 数组
ctx.splitText('title', { type: 'chars' }) // SplitText 返回的 part 数组
```

---

## 选择规则

优先按下面的规则选择 API：

1. 视频和非线性 seek 场景，优先用 `fromTo`。起点和终点都写清楚，任意时间点采样都稳定。
2. 入场动画可以用 `from`。终点由 XML 静态布局、class 或节点当前样式决定。
3. `to` 适合从静态样式运动到目标值，或做循环 / 呼吸 / 路径运动。不要用它隐式承载复杂前置状态。
4. 多段动作使用 `timeline` 编排。不要散落大量独立 tween 和 `delay`。
5. `set` 只做瞬时状态，不要把它当成动画。

推荐：

```js
ctx.fromTo('title',
  { opacity: 0, y: 36 },
  { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' }
);
```

可以：

```js
ctx.from('title', {
  opacity: 0,
  y: 36,
  duration: 0.7,
  ease: 'power3.out',
});
```

谨慎：

```js
ctx.to('title', {
  opacity: 1,
  y: 0,
  duration: 0.7,
});
```

如果 XML 中 `title` 的初始样式不是你以为的状态，`to` 的结果也会变。

---

## 返回值

顶层 `ctx.from()` / `ctx.to()` / `ctx.fromTo()` 会立即返回当前帧的采样结果。单目标返回对象，多目标返回对象数组，可用于驱动联动。

```js
var title = ctx.fromTo('title',
  { opacity: 0, y: 40 },
  { opacity: 1, y: 0, duration: 0.7, ease: 'spring.gentle' }
);

ctx.getNode('subtitle')
  .opacity(title.opacity * 0.85)
  .translateY(title.y * 0.5);
```

Timeline 链式方法返回 timeline 本身，用于继续 `.to()` / `.from()` / `.fromTo()` 编排。不要依赖 timeline 子项直接返回采样值。

---

## `from`

`from` 表示“从给定状态动画到当前静态状态”。适合入场、显露、弹入。

```js
ctx.from('headline', {
  opacity: 0,
  y: 48,
  scale: 0.96,
  duration: 0.8,
  ease: 'spring.gentle',
});
```

语义：

- `vars` 中的动画属性是起点。
- 终点来自 XML / class / 当前节点样式。
- `duration`、`delay`、`ease`、`stagger` 等时间字段也写在 `vars` 中。

多目标：

```js
ctx.from(['card-1', 'card-2', 'card-3'], {
  opacity: 0,
  y: 28,
  duration: 0.6,
  stagger: 0.08,
  ease: 'power2.out',
});
```

---

## `to`

`to` 表示“从当前状态动画到给定状态”。适合目标明确的状态变化、循环动画、插件动画。

```js
ctx.to('glow', {
  opacity: 0.8,
  scale: 1.08,
  duration: 1.2,
  repeat: -1,
  yoyo: true,
  ease: 'ease-in-out',
});
```

语义：

- 起点来自 XML / class / 当前节点样式，或同一时间线上前一个 tween 的输出。
- `vars` 中的动画属性是终点。
- 循环动画通常用 `to`，因为它只描述目标偏移状态。

文字插件也使用 `to`：

```js
ctx.to('title', {
  text: 'OpenCat',
  duration: 1,
  ease: 'linear',
});
```

---

## `fromTo`

`fromTo` 表示“从显式起点到显式终点”。这是 OpenCat 中最稳的动画写法，尤其适合视频渲染、CLI 抽帧、倒放、跳帧和最终帧校验。

```js
ctx.fromTo('badge',
  { opacity: 0, y: 20, scale: 0.9 },
  { opacity: 1, y: 0, scale: 1, duration: 0.55, ease: 'back.out' }
);
```

语义：

- `fromVars` 只放起点属性。
- `toVars` 放终点属性和时间字段。
- 同一个属性的两端都应显式出现，避免从静态样式推断。

推荐用于复杂编排：

```js
ctx.fromTo(['metric-1', 'metric-2', 'metric-3'],
  { opacity: 0, y: 18 },
  {
    opacity: 1,
    y: 0,
    duration: 0.5,
    stagger: { each: 0.08, from: 'start' },
    ease: 'power2.out',
  }
);
```

---

## `set`

`set` 是瞬时写入，适合在 timeline 的某个时间点切换状态。

```js
ctx.timeline()
  .set('cursor', { opacity: 0 }, 0)
  .set('cursor', { opacity: 1 }, 0.4)
  .to('cursor', { opacity: 0, duration: 0.2 }, 1.4);
```

不要用多个 `set` 模拟连续动画。连续变化应该使用 `from` / `to` / `fromTo`。

---

## Timeline

`ctx.timeline(opts)` 用来组织多段 tween。它是编排工具，不改变 `from` / `to` / `fromTo` 的核心语义。

```js
var tl = ctx.timeline({
  defaults: { duration: 0.6, ease: 'power2.out' },
});

tl.fromTo('title',
    { opacity: 0, y: 34 },
    { opacity: 1, y: 0 },
    0
  )
  .from('subtitle', { opacity: 0, y: 18 }, '-=0.25')
  .to('accent', { scaleX: 1, duration: 0.5 }, '<+=0.1');
```

支持的方法：

```js
tl.from(targets, vars, position)
tl.to(targets, vars, position)
tl.fromTo(targets, fromVars, toVars, position)
tl.set(targets, vars, position)
tl.addLabel(name, position)
```

### Position 参数

| 写法 | 含义 |
| --- | --- |
| 省略 | 接在当前游标后 |
| `0.8` | 绝对时间 0.8 秒 |
| `'+=0.2'` | 当前游标后 0.2 秒 |
| `'-=0.2'` | 当前游标前 0.2 秒 |
| `'<'` | 与前一个子项同起点 |
| `'>'` | 接在前一个子项终点 |
| `'<+=0.15'` | 前一个子项起点后 0.15 秒 |
| `'>-=0.15'` | 前一个子项终点前 0.15 秒 |
| `'intro'` | 标签 `intro` 的位置 |
| `'intro+=0.3'` | 标签 `intro` 后 0.3 秒 |

标签：

```js
var tl = ctx.timeline();

tl.addLabel('enter', 0)
  .fromTo('logo', { opacity: 0, scale: 0.9 }, { opacity: 1, scale: 1 }, 'enter')
  .addLabel('exit', 4)
  .to('logo', { opacity: 0, y: -20, duration: 0.35 }, 'exit');
```

如果 timeline 总时长超过 `ctx.sceneDuration`，引擎会等比缩放到场景时长内。

---

## 时间字段

时间字段写在 `vars` 中；`fromTo` 写在 `toVars` 中。

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `duration` | 非弹簧建议显式写 | 时长，秒 |
| `delay` | `0` | 当前 tween 的起始延迟 |
| `ease` / `easing` | `'linear'` | 缓动 |
| `repeat` | `0` | 重复次数；`-1` 表示无限 |
| `yoyo` | `false` | 重复时反向播放 |
| `repeatDelay` | `0` | 重复之间的间隔 |
| `stagger` | `0` | 多目标交错 |
| `clamp` | `true` | 是否把进度钳位到 `[0, 1]` |
| `keyframes` | 无 | 单个 tween 内的数值关键帧 |
| `at` | 无 | keyframe 的归一化时间点 |

---

## Easing

常用字符串：

```js
ease: 'linear'
ease: 'none'
ease: 'ease-in-out'
ease: 'power2.out'
ease: 'power3.inOut'
ease: 'sine.inOut'
ease: 'circ.out'
ease: 'expo.in'
ease: 'back.out'
ease: 'bounce.out'
ease: 'elastic.out'
ease: 'spring.gentle'
```

支持的类别：

- 基础：`linear` / `none` / `ease` / `ease-in` / `ease-out` / `ease-in-out`
- Back：`back-in` / `back-out` / `back-in-out`，也支持 `back.in` / `back.out` / `back.inOut`
- Elastic：`elastic-in` / `elastic-out` / `elastic-in-out`
- Bounce：`bounce-in` / `bounce-out` / `bounce-in-out`
- Power：`power1.in` 到 `power4.inOut`
- Sine / Circ / Expo：`sine.inOut` / `circ.out` / `expo.in` 等
- Steps：`steps(8)`
- Spring：`spring.default` / `spring.gentle` / `spring.stiff` / `spring.slow` / `spring.wobbly`

也可以使用 cubic-bezier 数组或 spring 对象：

```js
ease: [0.25, 0.1, 0.25, 1]
ease: { spring: { stiffness: 120, damping: 12, mass: 0.9 } }
```

注意：`back.out(1.7)` 这种 GSAP 风格参数可解析，但当前实现不使用 overshoot 参数。

---

## Stagger

当 `targets` 是数组时，`stagger` 控制每个目标的起始偏移。

```js
ctx.fromTo(['a', 'b', 'c', 'd'],
  { opacity: 0, y: 20 },
  {
    opacity: 1,
    y: 0,
    duration: 0.5,
    stagger: 0.08,
    ease: 'power2.out',
  }
);
```

支持写法：

```js
stagger: 0.08
stagger: { each: 0.08, from: 'start' }
stagger: { each: 0.08, from: 'center' }
stagger: { each: 0.08, from: 'edges' }
stagger: { amount: 0.4, from: 'end' }
stagger: { each: 0.08, from: 'random' }
stagger: { each: 0.08, from: 'center', grid: 'auto' }
stagger: { each: 0.08, from: 'center', grid: [3, 4], axis: 'y' }
```

---

## Keyframes

`keyframes` 用于单个 tween 内的多段数值变化。

```js
ctx.to('mark', {
  keyframes: {
    scale: [1, 1.2, 0.96, 1],
    rotation: [
      { at: 0, value: -4 },
      { at: 0.5, value: 6, easing: 'ease-in-out' },
      { at: 1, value: 0 },
    ],
  },
  duration: 1.4,
});
```

当前 keyframes 只建议用于数值属性。颜色、文字、路径变形更稳的写法是拆成多个 tween 或使用对应插件字段。

---

## 可动画属性

这些属性可直接写在 `from` / `to` / `fromTo` / `set` 的 vars 中。

### Transform

| 属性 | 别名 | 说明 |
| --- | --- | --- |
| `opacity` | - | 透明度 |
| `x` | `translateX` | 水平位移 |
| `y` | `translateY` | 垂直位移 |
| `scale` | - | 等比缩放 |
| `scaleX` | - | 水平缩放 |
| `scaleY` | - | 垂直缩放 |
| `rotation` | `rotate` | 旋转角度 |
| `skewX` | - | X 轴倾斜 |
| `skewY` | - | Y 轴倾斜 |

### Layout / Style

| 属性 | 说明 |
| --- | --- |
| `left` / `top` / `right` / `bottom` | 定位偏移 |
| `width` / `height` | 尺寸 |
| `borderRadius` | 圆角 |
| `borderWidth` | 边框宽度 |
| `strokeWidth` | SVG 描边宽度 |
| `strokeDasharray` | 描边虚线长度 |
| `strokeDashoffset` | 描边虚线偏移 |
| `textSize` | 文字大小 |
| `letterSpacing` | 字间距 |
| `lineHeight` | 行高 |
| `backdropBlur` | 背景模糊 |

### Color

| 属性 | 别名 |
| --- | --- |
| `backgroundColor` | `bg` |
| `textColor` | `color` |
| `borderColor` | - |
| `fillColor` | - |
| `strokeColor` | - |

### Filter

| 属性 | 默认语义 |
| --- | --- |
| `blur` / `blurSigma` | 高斯模糊 |
| `brightness` | 亮度 |
| `contrast` | 对比度 |
| `grayscale` | 灰度 |
| `hueRotate` | 色相旋转 |
| `invert` | 反转 |
| `saturate` | 饱和度 |
| `sepia` | 褐色 |
| `filter` | CSS filter 字符串，会解析并插值 |

---

## 插件

插件是对 tween vars 的扩展。OpenCat 内置插件已经接入运行时，常规脚本中直接使用字段，不需要手动 `registerPlugin()`。

### Text

Text 插件用于打字机式文字替换。

```js
ctx.to('headline', {
  text: 'Hello OpenCat',
  duration: 1.2,
  ease: 'linear',
});
```

可选字段：

| 字段 | 说明 |
| --- | --- |
| `mode` / `textMode` | 当前仅建议使用默认 `typewriter` |
| `cursor` / `typewriterCursor` | 光标字符 |
| `cursorBlink` | `false` 常显；数字表示闪烁周期，单位秒 |

示例：

```js
ctx.to('headline', {
  text: 'RENDER READY',
  cursor: '|',
  cursorBlink: 0.35,
  duration: 1.1,
  ease: 'linear',
});
```

### ScrambleText

ScrambleText 插件用于 GSAP 风格的乱码揭示。它会在动画过程中用随机字符扰动文本，并逐步收敛到目标文本。

简写：

```js
ctx.to('code', {
  scrambleText: 'SHORTHAND DIRECTION',
  duration: 1.4,
  ease: 'linear',
});
```

完整写法：

```js
ctx.to('code', {
  scrambleText: {
    text: 'SHORTHAND DIRECTION',
    chars: 'upperCase',
    speed: 20,
    revealDelay: 0.1,
    tweenLength: true,
    delimiter: '',
    rightToLeft: false,
  },
  duration: 1.4,
  ease: 'linear',
});
```

字段：

| 字段 | 说明 |
| --- | --- |
| `text` | 最终文本 |
| `chars` | 干扰字符集；可传字符串或预设名 |
| `speed` | 随机字符刷新频率，约等于每秒 tick 数 |
| `revealDelay` | 开始揭示前的延迟比例 / 时间 |
| `tweenLength` | 是否同时插值文本长度 |
| `delimiter` | 揭示单位分隔符，默认逐字符 |
| `rightToLeft` | 是否从右向左揭示 |

`chars` 预设：

```js
chars: 'upperCase'
chars: 'lowerCase'
chars: 'upperAndLowerCase'
chars: 'letters'
chars: 'numbers'
chars: 'digits'
chars: 'symbols'
chars: 'all'
```

注意：GSAP 的 `newClass` / `oldClass` 是 DOM span 语义。OpenCat 不是 DOM 文本节点拆 span 渲染，这两个字段不要作为有效样式能力使用。

### SplitText

SplitText 把一个文本节点拆成多个 part，再用普通 tween 动画这些 part。

```js
var chars = ctx.splitText('title', { type: 'chars' });

ctx.from(chars, {
  opacity: 0,
  y: 32,
  scale: 0.9,
  duration: 0.55,
  stagger: 0.035,
  ease: 'power2.out',
});
```

支持：

| 字段 | 说明 |
| --- | --- |
| `type: 'chars'` | 按字符拆分 |
| `type: 'words'` | 按词拆分 |

`lines` 当前未实现，不要依赖。

part 支持的常用属性：

```js
opacity
x / translateX
y / translateY
scale
rotate / rotation
color / textColor
```

### MotionPath

MotionPath 用 `path` 驱动节点沿 SVG path data 运动。

```js
ctx.to('rocket', {
  path: 'M100 360 C400 80 880 640 1180 360',
  orient: -90,
  duration: 4,
  ease: 'ease-in-out',
});
```

语义：

- `path` 是 SVG path data 字符串。
- 进度 `0 -> 1` 映射到路径弧长。
- 插件自动输出 `x`、`y`、`rotation`。
- `orient` 是切线角度上的额外偏移。

### MorphSVG

MorphSVG 用于 `<path>` 形状变形。字段名可以写 `d` 或 `morphSVG`。

```js
ctx.fromTo('shape',
  { d: 'M100 0 L200 200 L0 200 Z' },
  {
    d: 'M100 200 L200 0 L0 0 Z',
    duration: 1,
    ease: 'ease-in-out',
  }
);
```

也可以用 `morphSVG` 表达目标路径：

```js
ctx.to('shape', {
  morphSVG: 'M100 200 L200 0 L0 0 Z',
  duration: 1,
  ease: 'power2.inOut',
});
```

当前实现会对路径采样并对齐点，不要求源路径和目标路径拥有完全相同的 SVG 命令数。仍需注意：

- 目标节点应是 `<path>`。
- 源路径和目标路径应保持兼容的开闭状态。
- 当前主要支持单 contour 形状。
- 可选参数：`gridResolution`、`simplifyTolerance`。

---

## Node API

`ctx.getNode(id)` 返回节点代理，可瞬时写入属性。它不是 tween，不经过缓动，适合把 tween 采样结果用于联动。

```js
var hero = ctx.fromTo('title',
  { opacity: 0, y: 40 },
  { opacity: 1, y: 0, duration: 0.7, ease: 'spring.gentle' }
);

ctx.getNode('subtitle')
  .opacity(hero.opacity * 0.85)
  .translateY(hero.y * 0.5);
```

只在需要跨节点联动或特殊计算时使用 Node API。普通动画优先写成 tween。

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

有随机值时，优先传 seed，保证 CLI 渲染和预览一致。

---

## 模板

### 稳定入场

```js
ctx.fromTo('title',
  { opacity: 0, y: 32 },
  { opacity: 1, y: 0, duration: 0.65, ease: 'power3.out' }
);
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
  opacity: 0,
  y: 28,
  duration: 0.5,
  stagger: 0.035,
  ease: 'power2.out',
});
```

### 乱码揭示

```js
ctx.to('title', {
  scrambleText: {
    text: 'SYSTEM ONLINE',
    chars: 'upperCase',
    speed: 24,
    revealDelay: 0.1,
  },
  duration: 1.2,
  ease: 'linear',
});
```

---

## 反模式

- 不要在脚本顶层用可变变量记录上一帧状态。
- 不要依赖 `setTimeout`、`setInterval`、异步回调或事件监听驱动渲染关键动画。
- 不要把多段编排拆成一堆独立 tween 加手写 `delay`；用 `timeline`。
- 不要用 `to` 隐式假设复杂起点；需要稳定采样时用 `fromTo`。
- 不要在 OpenCat 文本插件中套用 GSAP DOM-only 字段，比如 ScrambleText 的 `newClass` / `oldClass`。
