# Transitions

`<transition>` 是 OpenCat 多场景 `<tl>` 的场景交接机制。普通转场、`light_leak`、以及 `gltransition.json` 中的 GLSL 转场都通过同一个 XML 节点声明。

读取本文件只在需要设计或修改 `<transition>` 时进行。基础 XML 规则仍以 `opencat.md` 为准。

---

## 结构规则

```xml
<transition
  from="scene1"
  to="scene2"
  effect="fade"
  duration="0.6"
  timing="ease-in-out"
/>
```

硬规则：

- 必须是 `<tl>` 的直接子节点。
- `from` / `to` 必须引用该 `<tl>` 的直接子场景。
- `from` 和 `to` 必须相邻，且 `from != to`。
- `duration` 必须是正数，单位秒。
- 每对相邻场景必须恰好有一个 `<transition>`。

可选公共参数：

| 属性 | 说明 |
| --- | --- |
| `timing` | 缓动名，默认 `linear`。支持 `animations.md` 中的 easing 名称 |
| `damping` / `stiffness` / `mass` | 任一出现时使用 spring 缓动配置 |

`timing`、`effect`、`direction` 会做基础规范化：大小写不敏感，`-` 和空格会转成 `_`。

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
| `clock_wipe` | - | 当前实现为淡入式时钟语义占位 |
| `iris` | - | 新场景从中心缩放展开 |
| `light_leak` | `seed` / `hueShift` / `maskScale` | RuntimeEffect 漏光转场 |

### `slide`

```xml
<transition from="scene1" to="scene2" effect="slide" direction="from_right" duration="0.45" timing="ease-out" />
```

方向：

```text
from_left    默认；新场景从左进入
from_right   新场景从右进入
from_top     新场景从上进入
from_bottom  新场景从下进入
```

### `wipe`

```xml
<transition from="scene1" to="scene2" effect="wipe" direction="from_top_left" duration="0.5" timing="ease-in-out" />
```

方向：

```text
from_left
from_right
from_top
from_bottom
from_top_left
from_top_right
from_bottom_left
from_bottom_right
```

### `light_leak`

```xml
<transition
  from="scene1"
  to="scene2"
  effect="light_leak"
  duration="0.7"
  seed="5"
  hueShift="45"
  maskScale="0.6"
/>
```

参数：

| 属性 | 默认 | 说明 |
| --- | --- | --- |
| `seed` | `0` | 漏光噪声种子；固定值保证确定性 |
| `hueShift` | `0` | 色相偏移 |
| `maskScale` | `0.25` | 遮罩尺度，内部 clamp 到 `0.03125..1.0` |

实现上，`light_leak` 不是普通 alpha fade；它用 RuntimeEffect 生成 mask，再用 from/to/mask 三个 picture child 做复合。

---

## GLTransition

除了内置普通转场，任意未识别的 `effect` 名都会走 GLTransition：

```xml
<transition from="scene1" to="scene2" effect="AdvancedMosaic" duration="0.8" />
```

运行时会在 `crates/opencat-core/gltransition.json` 中按名称查找 GLSL 转场，转换为 SKSL，并以 RuntimeEffect 采样 `fromScene` / `toScene`。

名称匹配规则：

- 大小写不敏感。
- 查找时忽略空格、`-`、`_`。
- 例如 `AdvancedMosaic`、`advanced_mosaic`、`advanced-mosaic` 都会归一到同一个 key。

如果 GLTransition 名称找不到，渲染层会回退成 fade。为了避免静默降级，写 XML 时优先使用下面列出的真实名称。

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

### 全量名称

当前 `gltransition.json` 包含 121 个转场：

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

```xml
<tl id="main-tl" class="absolute inset-0">
  <div id="scene1" duration="2.6">...</div>
  <transition from="scene1" to="scene2" effect="fade" duration="0.5" timing="ease-in-out" />

  <div id="scene2" duration="2.8">...</div>
  <transition from="scene2" to="scene3" effect="slide" direction="from_right" duration="0.45" timing="spring-default" />

  <div id="scene3" duration="2.8">...</div>
  <transition from="scene3" to="scene4" effect="light_leak" duration="0.7" seed="5" hueShift="45" maskScale="0.6" />

  <div id="scene4" duration="3.0">...</div>
  <transition from="scene4" to="scene5" effect="AdvancedMosaic" duration="0.8" timing="ease-in-out" />

  <div id="scene5" duration="2.6">...</div>
</tl>
```

---

## 注意事项

- 多场景中，scene 内部通常只做入场和呼吸；scene 间交接交给 `<transition>`。
- `clock_wipe` 当前渲染路径更接近 fade，占位语义强于视觉差异；需要强烈“时钟扫过”时优先试 GLTransition。
- GLTransition 复杂度不一；如果渲染性能敏感，优先用普通转场或少量 GL 高光转场。
- 不要把每个转场都做成视觉高潮。强转场太多会抹平节奏。
