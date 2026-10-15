# 转场

`<transition>` 是多场景 `<tl>` 的场景交接机制。普通转场、`light_leak`、以及 GLTransition 库的着色器转场都通过同一个节点声明。

---

## 结构

```jsx
<transition
  from="scene-01"
  to="scene-02"
  effect="fade"
  duration="0.5"
  timing="ease-in-out"
/>
```

硬规则：

- 必须是 `<tl>` 的直接子节点
- `from` / `to` 必须引用该 `<tl>` 的直接子场景
- `from` 和 `to` 必须相邻，且 `from != to`
- `duration` 必须是正数，单位秒
- 每对相邻场景必须恰好有一个 `<transition>`

可选公共参数：

| 属性 | 说明 |
| --- | --- |
| `timing` | 缓动名，默认 `linear` |
| `damping` / `stiffness` / `mass` | 任一出现时用 spring 缓动配置 |

`timing`、`effect`、`direction` 会做规范化：大小写不敏感，`-` 和空格转成 `_`。

---

## 选择原则

转场表达两个 scene 的关系，不是装饰清单。

| 关系 | 推荐 |
| --- | --- |
| 连续叙事、语气克制 | `fade` |
| 空间推进、列表/功能切换 | `slide` |
| 结构化揭示、before/after | `wipe` |
| 聚焦、仪式感、产品/Logo reveal | `iris` / `clock_wipe` |
| 温暖、记忆、胶片感、情绪变化 | `light_leak` |
| 视觉高潮、品牌 moment、强风格切换 | GLTransition |

不要每一幕都用强转场。5-7 个 beat 的视频通常只需要 1-2 个突出转场，其余用 `fade` / `slide` / `wipe` 承接节奏。

常用时长：

```text
0.20-0.35s  快速推进、强节奏
0.40-0.60s  常规转场
0.60-0.90s  情绪/电影感/光效转场
```

---

## 内置普通转场

| effect | 参数 | 说明 |
| --- | --- | --- |
| `fade` | - | 新场景随 progress 淡入 |
| `slide` | `direction` | 新场景从指定方向滑入，旧场景留在底层 |
| `wipe` | `direction` | 新场景按矩形裁剪区域揭示 |
| `clock_wipe` | - | 当前实现更接近 fade，占位语义强于视觉差异 |
| `iris` | - | 新场景从中心缩放展开 |
| `light_leak` | `seed` / `hueShift` / `maskScale` | 漏光转场 |

### `slide`

```jsx
<transition from="a" to="b" effect="slide" direction="from_right" duration="0.45" timing="ease-out" />
```

```text
from_left    默认
from_right
from_top
from_bottom
```

### `wipe`

```jsx
<transition from="a" to="b" effect="wipe" direction="from_top_left" duration="0.5" timing="ease-in-out" />
```

```text
from_left  from_right  from_top  from_bottom
from_top_left  from_top_right  from_bottom_left  from_bottom_right
```

### `light_leak`

```jsx
<transition from="a" to="b" effect="light_leak" duration="0.7" seed={5} hueShift={45} maskScale={0.6} />
```

| 属性 | 默认 | 说明 |
| --- | --- | --- |
| `seed` | `0` | 漏光噪声种子；固定值保证确定性 |
| `hueShift` | `0` | 色相偏移 |
| `maskScale` | `0.25` | 遮罩尺度，内部 clamp 到 `0.03125..1.0` |

---

## GLTransition

任意未识别的 `effect` 名走 GLTransition，按名称查 GLSL 转场表并采样 `fromScene` / `toScene`。

名称匹配：大小写不敏感，忽略空格、`-`、`_`。名称找不到会回退成 fade——**为了避免静默降级，优先用下面列出的真实名称**。

### 常用 GLTransition

| 目的 | 推荐 effect |
| --- | --- |
| 马赛克 / 方块 | `AdvancedMosaic`, `Mosaic`, `mosaic_transition`, `BlockDissolve`, `pixelize` |
| 模糊 / 景深 | `CrossZoom`, `DefocusBlur`, `LinearBlur`, `Dreamy`, `DreamyZoom` |
| 故障 / 数字 | `GlitchDisplace`, `GlitchMemories`, `parametric_glitch`, `TVStatic`, `randomNoisex` |
| 胶片 / 光燃烧 | `FilmBurn`, `burn`, `burn0`, `Overexposure` |
| 几何揭示 | `CircleCrop`, `Radial`, `Rectangle`, `RectangleCrop`, `Box`, `StarWipe` |
| 方向 / 推进 | `Directional`, `DirectionalScaled`, `directionalwipe`, `wipeLeft`, `wipeRight`, `wipeUp`, `wipeDown` |
| 翻页 / 3D | `BookFlip`, `SimpleFlip`, `InvertedPageCurl`, `Fold`, `cube`, `doorway` |
| 有机 / 流体 | `Swirl`, `WaterDrop`, `ripple`, `wind`, `perlin`, `luminance_melt` |
| 分屏 / 条带 | `windowblinds`, `windowslice`, `splitSlideInHorizontal`, `splitSlideOutVertical` |

### 全量名称（121 个）

```text
AdvancedMosaic, BlockDissolve, BookFlip, Bounce, BowTieHorizontal, BowTieVertical, BowTieWithParameter, Box, ButterflyWaveScrawler, CircleCrop, ColourDistance, CrazyParametricFun
CrossZoom, DefocusBlur, Directional, DirectionalScaled, DoomScreenTransition, Dreamy, DreamyZoom, EdgeTransition, FilmBurn, Fold, GlitchDisplace, GlitchMemories
GridFlip, HSVfade, HorizontalClose, HorizontalOpen, InvertedPageCurl, LeftRight, LinearBlur, Mosaic, Overexposure, PolkaDotsCurtain, PuzzleRight, Radial
Rectangle, RectangleCrop, Rolls, RotateScaleVanish, SimpleFlip, SimpleZoom, SimpleZoomOut, Slides, StarWipe, StaticFade, StereoViewer, Swirl
TVStatic, TilesWave, TopBottom, VerticalClose, VerticalOpen, WaterDrop, ZoomInCircles, ZoomLeftWipe, ZoomRigthWipe, angular, burn, burn0
cannabisleaf, chessboard, circle, circleopen, colorphase, coord-from-in, crosshatch, crosswarp, cube, directional-easing, directionalwarp, directionalwipe
displacement, dissolve, doorway, fade, fadecolor, fadegrayscale, flyeye, fragment, heart, hexagonalize, kaleidoscope, luma
luminance_melt, morph, mosaic_transition, multiply_blend, parametric_glitch, perlin, pinwheel, pixelize, polar_function, powerKaleido, randomNoisex, randomsquares
ripple, rotateTransition, rotate_scale_fade, scale-in, splitSlideInHorizontal, splitSlideInOutHorizontal, splitSlideInOutVertical, splitSlideInVertical, splitSlideOutHorizontal, splitSlideOutVertical, squareswire, squeeze
static_wipe, swap, tangentMotionBlur, undulatingBurnOut, wind, windowblinds, windowslice, wipeDown, wipeLeft, wipeRight, wipeUp, x_axis_translation
zoomInOut
```

---

## 示例组合

```jsx
<tl id="main-tl" class="absolute inset-0">
  <Scene01 />
  <transition from="scene-01" to="scene-02" effect="fade" duration="0.5" timing="ease-in-out" />
  <Scene02 />
  <transition from="scene-02" to="scene-03" effect="slide" direction="from_right" duration="0.45" timing="power2.inOut" />
  <Scene03 />
  <transition from="scene-03" to="scene-04" effect="light_leak" duration="0.7" seed={5} hueShift={45} maskScale={0.6} />
  <Scene04 />
  <transition from="scene-04" to="scene-05" effect="AdvancedMosaic" duration="0.8" timing="ease-in-out" />
  <Scene05 />
</tl>
```

---

## 注意事项

- 多场景中，scene 内部只做入场和呼吸；scene 间交接交给 `<transition>`。
- 强转场太多会抹平节奏。峰值和 CTA 前各放一个就够。
- GLTransition 复杂度不一；渲染性能敏感时优先用普通转场。
- 需要强烈"时钟扫过"时优先试 GLTransition，`clock_wipe` 视觉差异弱。
