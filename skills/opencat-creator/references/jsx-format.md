# OpenCat JSX 格式

一个 OpenCat 合成就是一个 `.jsx` 文件：标准 ES module + 标准 JSX，加两个专有导出（`meta` / `fonts`）和一组 `use*` 钩子。

最小形态：

```jsx
import { useGSAP } from 'opencat/runtime';

export const meta = { width: 1280, height: 720, fps: 30, duration: 3 };

export default function Composition() {
  useGSAP((ctx) => {
    ctx.fromTo('title', { opacity: 0, y: 32 }, { opacity: 1, y: 0, duration: 0.65, ease: 'power3.out' });
  });

  return (
    <div id="root" class="relative w-[1280px] h-[720px] bg-slate-950 overflow-hidden">
      <text id="title" class="absolute left-[120px] top-[300px] text-[96px] font-bold text-white">Hello OpenCat</text>
    </div>
  );
}
```

---

## 元素是静态的，动效走 `useGSAP`

这是整份格式的核心心智模型。

**元素只描述静止状态。** 每个元素写下的 `class` 和属性，表达的是它在画面里"是什么样"，不表达它"怎么动"。元素上不带动效：没有 CSS 动画，没有 transform 类，没有时间属性。

**动效只有两个来源：**

1. `useGSAP` 钩子——绝大多数情况用这个
2. `useCanvas` / `useFrame` / `useThree` 里的每帧脚本

所以正确的写法是：先按最终构图把元素**静态**摆好，再用 `useGSAP` 把元素从起点动到这个静止状态。

```jsx
// 元素：静态 hero frame，class 表达最终可读布局
<text id="title" class="absolute left-[120px] top-[300px] text-[96px] font-bold text-white">Hello</text>

// 动效：在钩子里描述它怎么到达这个状态
useGSAP((ctx) => {
  ctx.fromTo('title', { opacity: 0, y: 36 }, { opacity: 1, y: 0, duration: 0.7, ease: 'power3.out' });
});
```

反过来说，**不要**把元素写在动画的起点上（比如把 `opacity-0` 写进 class），也不要指望元素自己会动。

`class` 里的动画类分两类，实测行为不同：

- **被拒绝的**：`transition-*` / `animate-*` / `duration-*` / `ease-*` / `delay-*` / `translate-*` / `skew-*` / `transform`。引擎打印 `Unsupported Tailwind class ... ignoring it` 并忽略。写了没有效果，只会误导读者。
- **真的生效的**：`rotate-*` 和 `scale-*`。引擎把它们解析成静态几何。

第二类能用，但要想清楚：`rotate-[45deg]` 在元素上是一个**静态**角度，符合"元素是静态的"这条原则。它和 `useGSAP` 里的 `rotation` 是两件事——class 定的是构图里的静止姿态，钩子定的是怎么到达那个姿态。别在 class 里放动画的起点（`rotate-0` 等着钩子转到 45°），那会把起点和终点拆到两处。

这条规则的原因：渲染按帧采样，元素是每帧重算出来的静态快照。只有把动效写成时间的纯函数（在钩子里），任意帧才能稳定重放。

---

## `meta` 导出

```jsx
export const meta = { width: 1280, height: 720, fps: 30, duration: 9.2 };
```

| 字段 | 默认 | 说明 |
| --- | --- | --- |
| `width` | 1920 | 画布宽度（像素） |
| `height` | 1080 | 画布高度（像素） |
| `fps` | 30 | 输出采样率 |
| `duration` | 3 | 总时长（秒）；等于所有 scene `duration` 加所有 `<transition>` `duration` |

`duration` 没有上限，由内容决定。`useComposition()` 可在组件内读这四个值。

---

## `fonts` 导出

```jsx
export const fonts = {
  default: 'inter',
  faces: [
    { id: 'inter', family: 'Inter', path: 'assets/fonts/Inter-500.ttf', role: 'sans' },
    { id: 'inter800', family: 'Inter', path: 'assets/fonts/Inter-800.ttf' },
    { id: 'mono', family: 'JetBrains Mono', path: 'assets/fonts/JetBrainsMono-500.ttf', role: 'mono' },
    { id: 'emoji', path: 'assets/fonts/NotoColorEmoji.ttf', role: 'emoji' },
  ],
};
```

- `default` 省略时回退第一个 `role: 'sans'`，再回退第一个 face
- `role` 接受 `sans` / `emoji` / `mono`
- 相对 `path` 基于 `.jsx` 所在目录解析
- 应用：class 写 `font-[inter]` / `font-[inter800]` / `font-[mono]`
- `id` 全局唯一
- **斜体**：同一 `family` 挂正体与斜体两个 face，class 写 `italic` 选斜体
  （`not-italic` 显式回正体）。家族没有斜体 face 时，引擎合成一个 14° 的
  oblique——与浏览器 `font-style: italic` 的行为一致。

---

## 元素

`<div>` `<text>` `<image>` `<video>` `<lottie>` `<icon>` `<path>` `<canvas>` `<caption>` `<tl>` `<transition>`

Fragment `<>` 可用，把多个元素平铺进父级。

### 组件

大写开头的标签就是组件函数，返回值原地展开——这是复用结构的方式：

```jsx
function Corner({ id, tint }) {
  return (
    <>
      <div id={`${id}-tl`} class={`absolute left-[-1px] top-[-1px] w-[22px] h-[22px] border-l-[3px] border-t-[3px] border-[${tint}]`} />
      <div id={`${id}-br`} class={`absolute right-[-1px] bottom-[-1px] w-[22px] h-[22px] border-r-[3px] border-b-[3px] border-[${tint}]`} />
    </>
  );
}

// 用
<Card id="card-1" tint="#00ff88" />
```

`id` 全局唯一（组件展开后也算），所以多实例必须传不同 `id` prop，内部再派生子 id。

### 资源

```jsx
<image id="remote" url="https://example.com/photo.png" />
<image id="search" query="mountain landscape" queryCount={3} aspectRatio="16:9" />
<video id="clip" url="https://example.com/video.mp4" loop dataStart={2} dataDuration={4} />
<lottie id="loader" path="animation.json" />
<caption id="cc" path="subs.srt" />
```

- 源三选一：`path` / `url` / `query`（`query` 配 `queryCount`、`aspectRatio`）；`<lottie>` 只接受 `path` / `url`
- `<video>` 时间控制：`dataStart` / `dataDuration` / `dataMediaStart` / `loop` / `dataColorMatrix` / `dataHideAfterEnd`

### 音频

分两步：**声明** + **挂载**。

```jsx
// 1) 全局声明音频池（和 fonts 一样是导出常量，只写源，不写 attach）
export const audio = [
  { id: 'bgm', path: 'assets/bgm.mp3' },
  { id: 'hit', url: 'https://example.com/hit.mp3', duration: 0.5 },
];

// 2) 在要发声的节点上写 audioId
<div id="scene-01" class="absolute inset-0" duration="3" audioId="hit">…</div>
<tl id="main-tl" class="absolute inset-0" audioId="bgm">…</tl>
```

`audioId` 指向池里的 id。挂载点决定播放范围：

- 写在 `<tl>` 上 → 整条时间线
- 写在某个 scene 上 → 只在该 scene 内

同一个音频可以挂在多处，各自独立播放。

音频设计的判断（音乐与旁白的关系、音效时机）见 `design-principles.md` 的字幕与动效部分——画面动作和声音要落在同一个节拍上。

| 字段 | 必填 | 说明 |
| --- | --- | --- |
| `id` | 是 | 池内唯一标识 |
| `path` / `url` | 二选一 | 音频源；`path` 相对路径基于 `.jsx` 所在目录 |
| `duration` | 否 | 音频持续时长（秒），必须 > 0；省略则播完整段 |

---

## 布局

`class` 用 Tailwind 风格类，就是普通 CSS 思维：

```jsx
<div id="card" class="absolute left-[80px] top-[120px] w-[480px] p-[32px] border-[1px] border-[#2b3a46] bg-[#111820] rounded-[8px]" />
```

- 优先 flex：`flex flex-col` / `flex items-center justify-center`
- `absolute` 必须带坐标：至少含 `top` / `left` / `right` / `bottom` / `inset-X` 之一
- 不用 `style`、`className`、`parentId`，一律 `class`
- `<div>` 内不直接写文本，用 `<text>`
- `<text>` 内不嵌元素

---

## Timeline

```jsx
<div id="root" class="relative w-[1280px] h-[720px] bg-slate-950 overflow-hidden">
  <tl id="main-tl" class="absolute inset-0">
    <Scene01 />
    <transition from="scene-01" to="scene-02" effect="fade" duration="0.5" timing="ease-in-out" />
    <Scene02 />
  </tl>
</div>
```

- `<tl>` 至少两个直接子场景
- 每对相邻场景必须有且仅有一个匹配的 `<transition>`
- 每个 scene 必须有 `duration`（秒）
- 总长 = `sum(scene duration) + sum(transition duration)`
- `<transition>` 是 `<tl>` 直接子节点，`from` / `to` 相邻且不同

---

## `use*` 钩子

从 `opencat/runtime` 导入。**必须在组件函数体顶层调用**，不能在条件 / 循环 / 回调里。

| 钩子 | 签名 | 用途 |
| --- | --- | --- |
| `useGSAP` | `useGSAP((ctx) => {...})` | tween / timeline 编排 |
| `useCanvas` | `useCanvas(ref, (canvas, t, ctx) => {...})` | CanvasKit 2D 绘制 |
| `useFrame` | `useFrame((ctx, t) => {...})` | 任意每帧 `(ctx, t)` 工作 |
| `useThree` | `useThree(ref, (three, node, t, ctx) => {...})` | three.js 风格 3D 场景 |
| `useCanvasKit` | `useCanvasKit()` | CanvasKit 命名空间 |
| `useComposition` | `useComposition()` | `{ width, height, fps, duration }` |
| `useRef` | `useRef(initial?)` | `{ current }`，给 `useCanvas` / `useThree` |

### 时钟语义

时间有两个命名空间。名字决定读哪条时钟：

| 命名空间 | 含义 |
| --- | --- |
| `ctx.global` | 整条合成 |
| `ctx.current` | 钩子**所属的 scene** |

两者各有三个字段：`time`（秒）、`duration`（秒）、`frame`（帧号）。

**在 `<tl>` 里，钩子绑到它所在的 scene。** 写在 `Scene05` 组件里的 `useGSAP`，它的 `ctx.current` 永远是 Scene05 的时钟——即使当前渲染的是别的 scene。所以每个 scene 自动拿到自己的局部时间，不用手抄起点：

```jsx
function Scene05() {
  useGSAP((ctx) => {
    var t = ctx.current.time;        // Scene05 局部秒，从 0 开始
    var d = ctx.current.duration;    // Scene05 的 duration
    ctx.fromTo('s5-title', { opacity: 0, y: 30 }, { opacity: 1, y: 0, duration: 0.6, ease: 'power3.out' });
  });
  return <div id="scene-05" class="absolute inset-0" duration="6.0">…</div>;
}
```

`ctx.current.time` 从 scene 的窗口起点开始计秒。`ctx.timeline()` 里的 position 用 scene 局部秒。

**不要写可见性判断。** `<tl>` 只渲染当前时间点需要的场景树。场景超出窗口时，它的树被自动剔除，钩子不运行。所以 `if (t < 0 || t >= d) return;` 和 `ctx.getNode(id).opacity(on ? 1 : 0)` 都是多余的。场景根元素也不要写 `opacity-0`。

**没有 `<tl>` 时，两个命名空间相等。** 单 scene 合成里 `ctx.current.time === ctx.global.time`。

`ctx.global` 用来做跨 scene 对齐（例如让两个 scene 的动作落在同一条节拍上）。

### 纯函数要求

元素是静态单帧，动效全部在钩子里，所以钩子必须对时间纯。不要依赖 JS 变量跨帧累积状态。昂贵的表用模块级函数 + `globalThis` 缓存：

```js
globalThis.__cache || (globalThis.__cache = build());
```

### `ctx`

| 字段 | 说明 |
| --- | --- |
| `ctx.global.time` | 合成当前时刻（秒） |
| `ctx.global.duration` | 合成总时长（秒） |
| `ctx.global.frame` | 合成当前帧号 |
| `ctx.current.time` | 所属 scene 内的时刻（秒）；无 `<tl>` 时等于 `ctx.global.time` |
| `ctx.current.duration` | 所属 scene 的时长（秒）；无 `<tl>` 时等于 `ctx.global.duration` |
| `ctx.current.frame` | 所属 scene 内的帧号；无 `<tl>` 时等于 `ctx.global.frame` |

`useGSAP` / `useFrame` / `useCanvas` / `useThree` 传给回调的第二个参数 `t`，就是 `ctx.current.time`。

API：`ctx.from` / `to` / `fromTo` / `set` / `timeline`、`ctx.getNode(id)`（见 `animations.md`）；`ctx.CanvasKit`、`ctx.getCanvasById(id)`、`ctx.getImage(id)`、`ctx.ly`、`ctx.three`。

---

## 解析硬规则

**违反即整份合成加载失败**（实测）：

- 只有一个可视根节点。Fragment 里平铺两个 div 会报 `multiple root elements found`
- `id` 全局唯一。重复 id 报 `duplicate id`
- `<tl>` 至少 2 个直接子场景，否则报 `timeline ... must have at least two direct child scenes`
- 每个 `<tl>` 直接子场景必须有 `duration`，否则报 `timeline sequence ... is missing a duration`
- 每对相邻场景恰好一个 `<transition>`，`from` / `to` 必须相邻且是该 `<tl>` 的直接子节点
- 声明了但加载不到的资产（图片、lottie、字体）会让整份合成失败

**不会让加载失败，但会静默丢掉内容**（实测，比报错更危险）：

- `<div>` 里直接写文本 → 文本**不渲染**，也不报警告。必须用 `<text>` 包裹
- `<text>` 里的子元素 → 子元素**不渲染**，也不报警告
- `<image>` / `<lottie>` / `<icon>` / `<path>` / `<caption>` 里的子元素 → 静默丢弃
- `absolute` 不带坐标 → 不报错，位置按自动布局算，通常不是你要的

**不会失败，但结果可能和你写的数字不一致**：

- **`meta.duration` 是权威值**，渲染帧数由它决定。实测 `meta.duration: 99` 配两个 1s scene，出 2970 帧。scene 时长之和只决定 scene 在时间轴上的分布，不决定总长
- `<tl>` 自身上的 `duration` 被忽略（实测 `meta.duration: 2` + `<tl duration="9">` 仍出 60 帧）
- 所以两者必须对齐：`meta.duration` 要等于所有 scene `duration` 加所有 `<transition>` `duration`。对不齐时，多出来的时间轴尾部会空着，短了则最后的 scene 被截断

### 多文件

`import ... from 'opencat/runtime'` 和相对路径（`./x.jsx`）都支持，相对导入会解析 `.js` / `.jsx` / `/index.js` / `/index.jsx`。

命名 re-export 不支持：写 `export const x = ...`，不要 `export { a, b }`。

---

## 常见错误

| 错误 | 正确 |
| --- | --- |
| 两个节点同 id | 组件传 `id` prop + 内部派生子 id |
| `<div>` 里直接写文字 | `<text>` 包裹（否则静默不渲染） |
| `<text>` 里嵌元素 | 纯文本（否则静默不渲染） |
| `absolute` 无坐标 | 至少一个 inset 类 |
| 用 class 做动画/位移 | 走 `useGSAP` |
| 条件里调 `use*` | 提到组件体顶层 |
| scene 漏 `duration` | 每个 `<tl>` 直接子场景都写 |
| 用 `className` / `style` | 一律 `class` |
| 名字 re-export | `export const` |
| 在 `<tl>` 里手抄 scene 起点做换算 | 直接读 `ctx.current.time`，它已绑到本 scene |
| 单 scene 里读 `ctx.current.time` 却以为它是全局时间 | 无 `<tl>` 时两者相等；跨 scene 对齐用 `ctx.global.time` |
