# OpenCat XML 格式参考

OpenCat 使用 XML 格式描述动态图形合成。运行时解析 XML，构建场景树，使用 Skia + Taffy + QuickJS 生成视频画面。

---

## 基本结构

```xml
<opencat width="1280" height="720" fps="30" duration="3">
  <div id="root" class="flex items-center justify-center w-full h-full bg-white">
    <text id="title" class="text-[48px] font-bold">Hello</text>
  </div>
   <script>
    // 动画脚本（可选，最多一个）
    ctx.fromTo('title', {opacity: 0}, {opacity: 1, duration: 1});
  </script>
</opencat>
```

---

## 根元素 `<opencat>`

| 属性 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `width` | 正整数 | 1920 | 画布宽度（像素） |
| `height` | 正整数 | 1080 | 画布高度（像素） |
| `fps` | 正整数 | 30 | 输出采样率（每秒图像数） |
| `duration` | 正数，单位秒 | 3 | 总时长（与 `<tl>` 推导的总秒数对齐是约定，不是硬约束） |

---

## 元素类型

| 标签 | 说明 | 必填属性 |
|------|------|----------|
| `<div>` | 容器，markup 中默认 `display: block`（要 flex 显式写 `flex`） | `id` |
| `<text>` | 文本节点（文字写在标签之间） | `id` |
| `<image>` | 图像 | `id` + 一个图像源 |
| `<video>` | 视频，可叠加子节点 | `id` + 一个视频源 |
| `<lottie>` | Lottie 动画 | `id` + 一个 Lottie 源 |
| `<icon>` | Lucide 图标 | `id` + `icon` |
| `<path>` | SVG 路径 | `id` + `d` |
| `<canvas>` | Canvas 绘制表面，允许子元素（作为 hidden children） | `id` |
| `<caption>` | SRT 字幕 | `id` + `path` |
| `<tl>` | Timeline 容器 | `id` |
| `<transition>` | 场景转场 | `from` + `to` + `effect` + `duration` |
| `<fonts>` | 字体容器 | — |
| `<font>` | 字体声明（**必须嵌在 `<fonts>` 内**） | `id` + 一个字体源 |
| `<soundtrack>` | 音频容器 | — |
| `<audio>` | 音频（**必须嵌在 `<soundtrack>` 内**） | `id` + `attach` + 一个音频源 |

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

### 视频 / Lottie 源（二选一）

`<video>` 和 `<lottie>` 使用相同的源属性和时间控制：

| 属性 | 说明 |
|------|------|
| `path` | 本地文件路径 |
| `url` | 远程 URL |

```xml
<video id="clip" url="https://example.com/video.mp4" />
<lottie id="loader" path="animation.json" data-start="2" data-media-start="0" loop="true" />
```

时间控制（`<video>` / `<lottie>` 通用）：

| 属性 | 说明 |
|------|------|
| `data-start` | 时间线起点（秒） |
| `data-duration` | 时间线时长（秒） |
| `data-media-start` | 媒体内起始点（秒） |
| `loop` | 循环播放（`true`/`false`） |

### 音频源（二选一，必须在 `<soundtrack>` 内）

```xml
<soundtrack>
  <audio id="bgm" url="https://example.com/music.mp3" attach="main-tl" />
  <audio id="scene-audio" url="https://example.com/sfx.mp3" attach="scene-1" />
</soundtrack>
```

| 属性 | 说明 |
|------|------|
| `id` | 节点标识 |
| `path`/`url` | 音频源（二选一，不能并存） |
| `attach` | 引用的元素 id：**`<tl>` id**（整条时间线附加）**或** `<tl>` 内的**场景 id**（场景附加）；引用不存在的 id 会直接 bail |
| `duration` | 可选，单位秒，音频持续时长 |

### 字体系统（`<fonts>` / `<font>`）

```xml
<fonts default="my-sans">
  <font id="my-sans" family="Noto Sans SC" url="https://example.com/NotoSansSC-Regular.otf" role="sans" />
  <font id="my-emoji" path="/tmp/NotoColorEmoji.ttf" role="emoji" />
</fonts>
```

`<fonts>` 是 `<opencat>` 的直接子节点，用于声明文档使用的自定义字体。整个合成只允许一个 `<fonts>` 块。

**`<fonts>` 属性：**

| 属性 | 必填 | 说明 |
|------|------|------|
| `default` | 否 | 默认字体 id；若省略，默认字体依次回退到第一个 `role="sans"` 的 `<font>` 或第一个 `<font>` |

**`<font>` 属性（`<fonts>` 内部）：**

| 属性 | 必填 | 说明 |
|------|------|------|
| `id` | 是 | 字体标识，全局唯一，不能为空 |
| `path` / `url` | 二选一 | 本地路径 / 远程 URL |
| `family` | 否 | 字体族名，用于文本渲染匹配；省略时以 `id` 作为族名 |
| `role` | 否 | 语义角色：`sans`（无衬线）/ `emoji`（彩色表情）/ `mono`（等宽） |

**`<font>` 使用规则：**
- `<fonts>` 内部只允许 `<font>` 子元素，其他元素会报错
- `id` 不能重复
- `default` 引用的 id 必须存在
- 使用 Tailwind 类 `font-sans` / `font-[id]` 应用字体

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

`<tl>` 用来把多个 scene 串成一条可推导总时长的播放序列；scene 之间的交接由 `<transition>` 明确声明。

```xml
<opencat width="1280" height="720" fps="30" duration="8.6">
  <div id="root" class="relative w-[1280px] h-[720px]">
    <tl id="main-tl" class="absolute inset-0">
      <div id="scene1" class="w-full h-full bg-white" duration="4">
        <text id="title" class="text-[48px] font-bold">Scene 1</text>
      </div>

      <transition from="scene1" to="scene2" effect="fade" duration="0.6" timing="ease-in-out" />

      <div id="scene2" class="w-full h-full bg-slate-900" duration="4">
        <text id="title2" class="text-[48px] font-bold text-white">Scene 2</text>
      </div>
    </tl>
  </div>
</opencat>
```

**Timeline 规则：**
- `<tl>` 可以嵌套在任何可视容器内（`<div>` / `<canvas>` 等），不必是 `<opencat>` 的直接子节点
- `<tl>` 必须至少有两个直接子场景
- 每对相邻场景必须有匹配的 `<transition>`
- `<tl>` 没有 `duration` 属性，总长推导：`sum(scene.duration) + sum(transition.duration)`，单位秒
- `<transition>` 必须是 `<tl>` 的直接子节点
- 保持 `<opencat duration>` 与推导总长对齐

---

## 转场

`<transition>` 表达两个相邻 scene 的叙事关系。转场效果语义、方向参数和 GLTransition 名称见 [transitions.md](transitions.md)。

```xml
<transition from="scene1" to="scene2" effect="fade" duration="0.6" />
```

必填属性：`from` + `to` + `effect` + `duration`（秒，大于 0）。

可选属性：`direction`（slide/wipe 方向）、`timing`（缓动）、`damping`/`stiffness`/`mass`（弹簧）、`seed`/`hueShift`/`maskScale`（light_leak）。未识别的 `effect` 名回退到 `gl_transition` 模式。

---

## 动画系统

动画脚本通过 `<script>` 标签编写，使用 QuickJS 在每个输出采样点运行。完整的 `from` / `to` / `fromTo` / `timeline` 用法、可动画属性、插件和缓动参考见 [animations.md](animations.md)。

### `<script>` 标签规则

- 整个合成只允许一个 `<script>` 块
- 必须是 `<opencat>` 的直接子节点（不能嵌在 `div` / `canvas` / `tl` 等任何子元素里）
- 应放在 `<opencat>` 可视子节点的最后。
- **不允许带任何属性**（`type` / `src` 等都拒）
- **不允许自闭合**（必须有 `</script>`）
- 脚本会被附加到可视根节点（不是顶层 `<opencat>`）

### 执行上下文

| 字段 | 说明 |
|------|------|
| `ctx.time` | 全局时间，单位秒 |
| `ctx.duration` / `ctx.totalDuration` | 合成总时长，单位秒 |
| `ctx.currentTime` | 当前场景内时间，单位秒 |
| `ctx.sceneDuration` | 当前场景时长，单位秒 |

**使用指南：**
- **循环动画**（呼吸、闪烁、持续旋转）：优先使用 `ctx.time`
- **场景内进度**（路径绘制、淡入淡出）：使用 `ctx.currentTime / ctx.sceneDuration`
- **不要用输出图像计数表达 timing**：创作脚本中的 `duration` / `delay` / `stagger` / position 参数都写秒

### Tween API 速查

| API | 行为 |
|-----|------|
| `ctx.from(targets, vars)` | 从 `vars` 动画到当前值 |
| `ctx.to(targets, vars)` | 从当前值动画到 `vars` |
| `ctx.fromTo(targets, fromVars, toVars)` | 两端写死 |
| `ctx.set(targets, vars)` | 瞬时写入 |

---

## Canvas API

`<canvas>` 用于程序化视觉：粒子、噪声、数据纹理、路径绘制，以及需要 Skia/RuntimeEffect 的画面层。完整的 CanvasKit 子集、Subtree 和 RuntimeEffect 用法见 [canvaskit.md](canvaskit.md)。

`<canvas>` 在 markup 模式下允许子元素（作为 hidden children），但子节点里不能有 `<audio>`。

入口：`ctx.getCanvasById(id)` 获取绘制接口，`ctx.CanvasKit` 访问辅助函数。

---

## 完整示例

### 简单场景（无 Timeline）

```xml
<opencat width="390" height="844" fps="30" duration="2">
  <script>
    ctx.fromTo('title', {opacity: 0, y: 30}, {opacity: 1, y: 0, duration: 0.67, ease: 'spring.gentle'});
  </script>
  <div id="root" class="flex flex-col items-center justify-center w-full h-full bg-white">
    <text id="title" class="text-[48px] font-bold text-slate-900">Hello OpenCat</text>
  </div>
</opencat>
```

### 多场景 Timeline + 音频

```xml
<opencat width="1280" height="720" fps="30" duration="12.6">
  <soundtrack>
    <audio id="bgm" url="https://example.com/music.mp3" attach="scene1" />
  </soundtrack>

  <script>
    ctx.fromTo(['title', 'subtitle'], {opacity: 0, y: 24}, {opacity: 1, y: 0, stagger: 0.2, duration: 0.8, ease: 'spring.gentle'});
  </script>

  <div id="root" class="relative w-[1280px] h-[720px] bg-slate-950">
    <tl id="main-tl" class="absolute inset-0">
      <div id="scene1" class="flex flex-col items-center justify-center w-full h-full" duration="6">
        <text id="title" class="text-[72px] font-bold text-white">Scene 1</text>
        <text id="subtitle" class="text-[24px] text-slate-400">With animation</text>
      </div>

      <transition from="scene1" to="scene2" effect="fade" duration="0.6" timing="ease-in-out" />

      <div id="scene2" class="flex flex-col items-center justify-center w-full h-full bg-slate-900" duration="6">
        <text id="title2" class="text-[72px] font-bold text-white">Scene 2</text>
      </div>
    </tl>
  </div>
</opencat>
```

### Canvas 绘制

```xml
<opencat width="640" height="480" fps="30" duration="4">
  <script>
    var CK = ctx.CanvasKit;
    var canvas = ctx.getCanvasById('my-canvas');
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

### Lottie 动画

```xml
<opencat width="400" height="300" fps="25" duration="12.8">
  <div id="stage" class="w-full h-full flex items-center justify-center bg-slate-100">
    <lottie
      id="loader"
      class="w-[280px] h-[200px]"
      path="animation.json"
      data-start="5"
      data-media-start="0"
      loop="true"
    />
  </div>
</opencat>
```

### 视频叠加

```xml
<opencat width="1280" height="720" fps="30" duration="6">
  <div id="root" class="relative w-full h-full bg-black">
    <video id="bg-video" class="absolute inset-0 w-full h-full object-cover" url="https://example.com/video.mp4" loop="true" />
    <div id="overlay" class="absolute bottom-[40px] left-[40px] px-[20px] py-[12px] rounded-[12px] bg-black/60">
      <text id="caption" class="text-[24px] text-white font-semibold">Video Overlay</text>
    </div>
  </div>
</opencat>
```

---

## 解析器硬规则

**生成 XML 时必须遵守的硬约束**。任何违反都会被解析器拒绝，导致整份合成加载失败。

### 全局

- **单一可视根**：`<opencat>` 下有且仅有一个可视根节点（`div` / `text` / `canvas` / `image` / `video` / `lottie` / `icon` / `path` / `caption` / `tl`）
- **未知属性静默忽略**：不在已知属性列表中的属性会被静默跳过，不会报错
- **三个禁用属性**：`className` / `parentId` / `style` 在任何位置都禁止使用
- **数字属性严格**：`width` / `height` / `fps` / `queryCount` 是 ASCII 正整数；`duration` / `data-start` / `data-duration` / `data-media-start` 是秒数，必须是 finite 且按字段要求大于 0 或不小于 0；所有数字都不能有空白、`+` 或全角数字
- **id 全局唯一**：可视节点和 `<audio>` 的 id 都不能重复
- **非空白文本节点**：只能在 `<text>` 内部出现，其他位置的非空白文本会报错

### 元素

| 元素 | 必填属性 | 特有规则 |
|------|---------|---------|
| `div` | `id` | 容器；可嵌套任何可视子节点（含 `<tl>`） |
| `text` | `id` | 文本写在标签之间；**禁止子元素**（`<text><span/></text>` 报错）；实体/`<![CDATA[...]]>`/`<!-- -->` 正常解析 |
| `canvas` | `id` | markup 模式下允许子元素（作为 hidden children），子节点里不能有 `<audio>` |
| `image` | `id` | 资源**三选一**：`path` / `url` / `query`；`query` 可配 `queryCount`、`aspectRatio`（`queryCount > 0`，`queryCount`/`aspectRatio` 必须配合 `query`）；**不允许子元素** |
| `video` | `id` | 资源**二选一**：`path` / `url`；可叠加子节点；`data-start` / `data-duration` / `data-media-start` / `loop` 控制时间 |
| `lottie` | `id` | 资源**二选一**：`path` / `url`；**不允许子元素**；`data-start` / `data-duration` / `data-media-start` / `loop` 控制时间 |
| `icon` | `id` + `icon` | Lucide 图标名；**不允许子元素** |
| `path` | `id` + `d` | SVG path d 字符串；**不允许子元素** |
| `caption` | `id` + `path` | SRT 字幕文件路径；**不允许子元素** |
| `tl` | `id` | **至少 2 个直接子场景**；无 `duration` 属性；所有子场景都必须有 `duration` |

### `<soundtrack>` 与 `<audio>`

- `<soundtrack>` 是 `<opencat>` 的直接子节点
- `<soundtrack>` 内部**只允许** `<audio>`，其他元素直接报错
- `<audio>` **必须嵌在 `<soundtrack>` 内**（直接放在 `<opencat>` 下会报错）
- `<audio>` 必填：`id` + `attach` + 一个音频源（`path` / `url` 二选一）
- **`attach` 规则**：
  - 引用 `<tl>` 的 id → 整条时间线附加
  - 引用 `<tl>` 内某场景的 id → 场景附加
  - 引用不存在的 id → 报错 `audio ... attach references non-existent element`

### `<fonts>` 与 `<font>`

- `<fonts>` 是 `<opencat>` 的直接子节点，**整个合成只允许一个** `<fonts>` 块
- `<fonts>` 内部**只允许** `<font>`，其他元素直接报错
- `<font>` **必须嵌在 `<fonts>` 内**
- `<font>` 必填：`id`（非空且全局唯一）+ 一个字体源（`path` / `url` 二选一，不能并存）
- `path` 为相对路径时，基于文档目录解析
- `role` 仅接受 `sans` / `emoji` / `mono`，其他值报错
- `<fonts default="...">` 引用的 id 必须在块内存在，否则报错
- `family` 省略时以 `id` 作为字体族名

### `<transition>`

- 必须是 `<tl>` 的直接子节点（放在 `div` / `canvas` 等里会报错）
- 必填：`from` + `to` + `effect` + `duration`
- **`from` / `to` 必须引用该 `<tl>` 的直接子场景**，且**必须相邻**（`to_idx == from_idx + 1`），不满足会报错
- **`from != to`**
- **`duration` 是秒数且必须大于 0**
- 每对相邻场景必须恰好有一个 `<transition>`，缺失或重复都会报错
- 可选属性：

  | 属性 | 适用 | 备注 |
  |------|------|------|
  | `direction` | `slide` / `wipe` | 详见 [transitions.md](transitions.md) |
  | `timing` | 全部 | 缓动名，默认 `linear` |
  | `damping` / `stiffness` / `mass` | 全部 | 三者中任一出现 → spring 配置 |
  | `seed` / `hueShift` / `maskScale` | `light_leak` | 随机种子 / 色相偏移 / 遮罩缩放 |

- 未识别的 `effect` 名会回退到 `gl_transition` 模式（任意 GLSL 着色器名）

### 错误信息速查

遇到错误时按关键词定位规则：

| 触发 | 错误信息关键词 |
|------|---------------|
| 多个可视根 | `multiple visual root elements found` |
| 未知属性 | 静默忽略，不报错 |
| 禁用属性 | `attribute <name> is not allowed in markup` |
| 数字非法 | `must be a positive integer` / `must not have leading zeros` |
| `<tl>` 缺子场景 | `timeline ... must have at least two direct child scenes` |
| `<tl>` 缺转场 | `missing transition between adjacent scenes` |
| `<tl>` 缺 duration | `timeline sequence ... is missing a duration` |
| `<transition>` 非相邻 | `is not between adjacent children` |
| `<transition>` `from`/`to` 不存在 | `references <id>, which is not a direct child of this timeline` |
| `<transition>` `from == to` | `from and to must be distinct` |
| `<audio>` 不在 `<soundtrack>` | `<audio> must be inside <soundtrack>` |
| `<audio>` `attach` 找不到 | `attach references non-existent element` |
| `<script>` 嵌套 | `<script> must be a direct child of <opencat>` |
| `<script>` 带属性 | `<script> tag with attributes is not allowed` |
| `<script>` 自闭合 | `self-closing <script/> is not allowed` |
| 多个 `<fonts>` 块 | `multiple <fonts> blocks are not allowed` |
| `<fonts>` 内非 `<font>` 子元素 | `unknown element <...> inside <fonts>` |
| `<font>` id 重复 | `duplicate font id` |
| `<font>` 缺源或源并存 | `<font> requires one of: path, url` / `<font> accepts only one of: path, url` |
| `<font>` 空 path/url | `<font> path must be non-empty` / `<font> url must be non-empty` |
| `<font>` 无效 role | `unknown font role; expected sans, emoji, or mono` |
| `<fonts default>` 引用不存在 | `<fonts default="..."> references unknown font id` |
| `<lottie>` 缺源或源并存 | `<lottie> requires one of: path, url` / `<lottie> requires only one of: path, url` |

---

## 模板

`<template>` 用来在单个 XML 文件内复用一段结构。模板在解析前展开；运行时、布局、动画和渲染阶段只会看到展开后的普通 OpenCat 节点。

```xml
<opencat width="1280" height="720" fps="30" duration="3">
  <template name="deck-thumb">
    <div id="$id" class="flex flex-row gap-[8px]">
      <text id="$id-num" class="w-[16px] text-[11px] $numTone">$num</text>
      <div id="$id-frame" class="relative w-[144px] h-[81px] rounded-[4px] $frameTone">
        <slot name="preview" />
      </div>
    </div>
  </template>

  <div id="root">
    <deck-thumb id="thumb-1" num="1" numTone="text-white" frameTone="bg-white" />
    <deck-thumb id="thumb-2" num="2" numTone="text-white/55" frameTone="bg-slate-100">
      <slot name="preview">
        <div id="thumb-2-dot" class="absolute left-[10px] top-[10px] w-[12px] h-[12px] rounded-full bg-[#D97757]" />
      </slot>
    </deck-thumb>
  </div>
</opencat>
```

**模板规则：**

- `<template>` 必须是 `<opencat>` 的直接子节点，不算可视根
- `<template>` 必须有非空 `name`
- `name` 不能和内置标签同名（如 `div`、`text`、`image`、`tl`、`script`）
- 定义后可直接用同名标签调用，例如 `<template name="deck-thumb">` 对应 `<deck-thumb ... />`
- 调用节点的属性会替换模板中的 `$变量`，如 `$id`、`$num`、`$class`
- 未提供的 `$变量` 会替换为空字符串
- `<slot name="x" />` 会被调用节点里的 `<slot name="x">...</slot>` 内容替换
- 未提供内容的 slot 会被移除
- 模板调用会递归展开，但不能递归调用自身

**建议：**

- 模板名使用 kebab-case，例如 `deck-thumb`、`right-reveal-panel`
- 让 template 内部维护大段 Tailwind class，调用处只传少量语义 token
- 需要动画的内部节点用 `$id-xxx` 派生 id，避免多个实例撞 id

---

## 附录：常见错误

| 错误 | 正确 |
|------|------|
| `<div>` 标签内直接写文本 | 用 `<text>` 包裹 |
| 用 `bg-{color}` 给图标/路径着色 | 用 `fill-{color}` / `stroke-{color}` |
| `class` 中放 transform 类 | 用脚本控制动画 |
| `<tl>` 缺转场或场景少于 2 | 添加缺失的 `<transition>` |
| 时长不匹配 | `<opencat duration> = sum(scene.duration) + sum(transition.duration)` |
| `<script>` 嵌套在其他元素内 | 必须是 `<opencat>` 的直接子节点 |
| `<audio>` 直接放在 `<opencat>` 下 | 必须在 `<soundtrack>` 内 |
| `ctx.getCanvas()` | 用 `ctx.getCanvasById(id)`（`getCanvas` 调用会抛错） |
| `<audio>` `attach="root"` 当 root 是 div 而非 `<tl>` | attach 引用 `<tl>` id 或其内部场景 id；不能指向普通 `div` |
| 给 transition 加 `damping` / `stiffness` / `mass` 三者之一 | 这是 spring 配置，三者中至少一个出现就走 spring 缓动；不需要时直接用 `timing="ease-in-out"` 等命名缓动 |
