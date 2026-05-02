# 转场

转场告诉观众两个场景之间的关系。选择匹配情感作用，而非技术。

---

## 多场景 Composition 动画规则

1. **始终用转场。** 无跳切。无例外。
2. **每个场景有入场动画。** 每个元素通过 `ctx.from()` 入场。
3. **永不使用退场动画**（末场景除外）。转场就是退场。
4. **末场景例外：** 可淡出元素（如淡出到黑场）。

---

## 叙事位置

| 位置 | Duration（帧） |
|------|---------------|
| 开场 | 12-18 |
| 相关点之间 | 9 |
| 主题变化 | 9-12 |
| 高潮/揭示 | 5-9 |
| 放松 | 15-21 |
| 结尾 | 18-30 |

---

## 能量 → Timing

| 能量 | Duration（帧） | Timing |
|------|---------------|--------|
| **平静** | 15-24 | `'ease-in-out'` |
| **中等** | 9-15 | `'ease-out'` |
| **高能** | 5-9 | `'linear'` |

---

## 普通转场

| effect | 说明 | direction（可选） |
|--------|------|-------------------|
| `fade` | Cross fade | — |
| `slide` | Sliding transition | `from_left` (default) / `from_right` / `from_top` / `from_bottom` |
| `wipe` | Wipe transition | `from_left` (default) / `from_right` / `from_top` / `from_bottom` / `from_top_left` / `from_top_right` / `from_bottom_left` / `from_bottom_right` |
| `clock_wipe` | Clock wipe | — |
| `iris` | Iris open/close | — |
| `light_leak` | 漏光 | — |

### `fade` — Cross fade

**意图：** 最安全、最通用的转场。两个场景交叉淡入淡出，传达"这还在继续"的连续感。

**适配场景：**
- 高级/奢华内容（18-24 帧慢速 fade）
- 叙事/故事类视频
- 需要平滑过渡的任何场景
- 结尾淡出到黑场

**参数：**
```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"fade","duration":18,"timing":"ease-in-out"}
```

---

### `slide` — Sliding transition

**意图：** 有方向性的滑动，传达动感和能量。旧场景滑出，新场景滑入。

**适配场景：**
- 高能量内容（产品发布、社交广告）
- 需要方向感的叙事
- 展示多个并列内容
- 节奏感强的视频

**参数：**
```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"slide","direction":"from_right","duration":12,"timing":"ease-out"}
```

**方向选择：**
- `from_left` — 从左滑入（阅读方向）
- `from_right` — 从右滑入（反向，强调）
- `from_top` — 从上滑入（下降感）
- `from_bottom` — 从下滑入（上升感）

---

### `wipe` — Wipe transition

**意图：** 像擦除一样揭示新场景，比 slide 更有结构性。传达"清除旧的，展示新的"。

**适配场景：**
- 科技/数据类内容
- 需要清晰分隔的场景
- 展示对比（before/after）
- 几何感强的设计

**参数：**
```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"wipe","direction":"from_top_left","duration":15,"timing":"ease-in-out"}
```

**方向选择：**
- 基础方向：`from_left`/`from_right`/`from_top`/`from_bottom`
- 对角线：`from_top_left`/`from_top_right`/`from_bottom_left`/`from_bottom_right`

---

### `clock_wipe` — Clock wipe

**意图：** 像时钟指针一样扫过，传达"时间流逝"或"揭示"的感觉。

**适配场景：**
- 时间相关的内容（倒计时、时间线）
- 戏剧性揭示
- 产品发布预告
- 需要仪式感的场景

**参数：**
```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"clock_wipe","duration":15,"timing":"ease-in-out"}
```

---

### `iris` — Iris open/close

**意图：** 像相机光圈一样开合，聚焦注意力。传达"聚焦于此"或"电影感"。

**适配场景：**
- 电影/戏剧内容
- 需要聚焦的揭示
- 奢侈品/高端品牌
- 复古风格视频

**参数：**
```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"iris","duration":18,"timing":"ease-in-out"}
```

---

### `light_leak` — 漏光

**意图：** 模拟胶片漏光效果，传达温暖、怀旧、梦幻的感觉。

**适配场景：**
- 温暖/人文内容
- 回忆/怀旧场景
- 婚礼/生活方式视频
- 艺术/创意项目

**参数：**
```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"light_leak","duration":18,"seed":0.5,"hueShift":0.1,"maskScale":0.8}
```

---

## GL 转场

任何不在内置效果表中的名称作为 `effect`，运行时会在 `gltransition.json` 中查找同名 GLSL 着色器。所有 GL 转场都有默认参数，无需额外传入。

```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"crosswarp","duration":15}
```

### 回忆/浪漫/梦幻

| 效果名 | 情感表达 |
|--------|----------|
| `Dreamy` | 梦幻、柔和 |
| `DreamyZoom` | 梦幻旋转 |
| `SoftBlur` | 柔和模糊 |

### 儿童/活泼/有趣

| 效果名 | 情感表达 |
|--------|----------|
| `Bounce` | 弹跳、活泼 |
| `WaterDrop` | 宁静、自然 |
| `ripple` | 平静、流动 |

### 科技/未来/数字

| 效果名 | 情感表达 |
|--------|----------|
| `GlitchDisplace` | 故障、数字 |
| `GlitchMemories` | 复古故障 |
| `parametric_glitch` | 参数化故障 |
| `crosswarp` | 交叉扭曲 |
| `hexagonalize` | 蜂窝变形 |

### 电影/戏剧/复古

| 效果名 | 情感表达 |
|--------|----------|
| `FilmBurn` | 胶片灼烧 |
| `burn` | 灼烧扩散 |

### 展示/揭示/几何

| 效果名 | 情感表达 |
|--------|----------|
| `BowTieHorizontal` | 对称展开 |
| `BowTieVertical` | 垂直展开 |
| `Box` | 矩形缩放 |
| `CircleCrop` | 圆形裁剪 |
| `Radial` | 径向擦除 |

### 自然/动态/能量

| 效果名 | 情感表达 |
|--------|----------|
| `wind` | 风吹 |
| `Swirl` | 漩涡 |
| `kaleidoscope` | 万花筒 |

---

### 淡入淡出/渐变

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `fade` | 简单淡入淡出 | 平静、延续 |
| `fadecolor` | 带中间色的淡入淡出 | 过渡、中间状态 |
| `fadegrayscale` | 先转灰度再淡出 | 怀旧、消逝 |
| `HSVfade` | HSV 色彩空间渐变 | 色彩流动、梦幻 |
| `StaticFade` | 静态噪点淡入淡出 | 复古、电视感 |
| `luma` | 亮度键控 | 光影变化、自然 |
| `luminance_melt` | 高亮区域先融化 | 融化、消散 |
| `multiply_blend` | 正片叠底混合 | 叠加、融合 |
| `colorphase` | 逐颜色通道分阶段 | 色彩分解、艺术 |
| `ColourDistance` | 颜色差值逐像素过渡 | 色彩流动、渐变 |

### 滑动/平移

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `Directional` | 定向滑动覆盖 | 方向感、推进 |
| `DirectionalScaled` | 带缩放的定向滑动 | 动感、强调 |
| `directional-easing` | 带缓动的定向滑动 | 流畅、优雅 |
| `LeftRight` | 左右错位拉伸 | 错位、动感 |
| `TopBottom` | 上下错位拉伸 | 垂直流动 |
| `x_axis_translation` | X 轴平移滑动 | 水平移动、简洁 |
| `Slides` | 幻灯片滑入滑出 | 展示、切换 |
| `splitSlideInHorizontal` | 水平分裂滑入 | 分裂、展开 |
| `splitSlideInOutHorizontal` | 水平分裂滑出+滑入 | 完整切换 |
| `splitSlideInOutVertical` | 垂直分裂滑出+滑入 | 垂直切换 |
| `splitSlideInVertical` | 垂直分裂滑入 | 垂直展开 |
| `splitSlideOutHorizontal` | 水平分裂滑出 | 水平收拢 |
| `splitSlideOutVertical` | 垂直分裂滑出 | 垂直收拢 |

### 擦除/扫除

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `directionalwipe` | 定向擦除 | 清除、揭示 |
| `wipeDown` | 向下擦除 | 下降、覆盖 |
| `wipeLeft` | 向左擦除 | 左向流动 |
| `wipeRight` | 向右擦除 | 右向流动 |
| `wipeUp` | 向上擦除 | 上升、提升 |
| `angular` | 角度擦除扇形展开 | 扇形、旋转 |
| `Radial` | 径向擦除扇形展开 | 中心向外、绽放 |
| `StarWipe` | 星形擦除 | 闪耀、星光 |
| `pinwheel` | 风车旋转擦除 | 旋转、童趣 |
| `squareswire` | 方块网格线擦除 | 网格、科技 |
| `static_wipe` | 静态噪点擦除 | 静态、电视感 |
| `windowblinds` | 百叶窗 | 光线、遮蔽 |
| `windowslice` | 窗户切片垂直条显露 | 条纹、揭示 |

### 缩放/放大

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `SimpleZoom` | 简单放大 | 聚焦、接近 |
| `SimpleZoomOut` | 简单缩小 | 远离、全景 |
| `scale-in` | 缩放入场 | 进入、出现 |
| `CrossZoom` | 交叉缩放模糊 | 穿梭、动感 |
| `zoomInOut` | 先放大旧画面再缩小新画面 | 穿梭、转换 |
| `ZoomInCircles` | 多圆形放大 | 多重聚焦 |
| `ZoomLeftWipe` | 左侧缩放擦除 | 左向聚焦 |
| `ZoomRigthWipe` | 右侧缩放擦除 | 右向聚焦 |

### 裁剪/遮罩

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `CircleCrop` | 圆形裁剪收缩/展开 | 聚焦、圆形 |
| `RectangleCrop` | 矩形裁剪 | 框架、裁剪 |
| `Rectangle` | 矩形缩放展开 | 矩形、展开 |
| `circle` | 圆形遮罩展开/收缩 | 圆形、聚焦 |
| `circleopen` | 圆形打开/关闭 | 开合、光圈 |
| `heart` | 心形遮罩 | 爱心、浪漫 |
| `cannabisleaf` | 大麻叶形状遮罩 | 特殊形状 |
| `polar_function` | 极坐标函数花瓣遮罩 | 花瓣、绽放 |

### 翻转/旋转

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `SimpleFlip` | 简单 3D 翻转 | 翻转、切换 |
| `BookFlip` | 书本翻页 | 翻页、阅读 |
| `InvertedPageCurl` | 反向翻页卷角 | 卷曲、复古 |
| `Fold` | 折纸翻折 | 折纸、手工 |
| `GridFlip` | 网格逐个翻牌 | 网格、揭示 |
| `cube` | 3D 立方体旋转 | 立体、空间 |
| `rotateTransition` | 旋转拼贴 | 旋转、拼贴 |
| `rotate_scale_fade` | 旋转+缩放+淡出 | 复合动效 |
| `RotateScaleVanish` | 旋转缩放消失 | 消失、旋转 |
| `swap` | 3D 翻转交换 | 交换、翻转 |
| `Rolls` | 卷轴滚动 | 卷轴、展开 |
| `squeeze` | 垂直挤压压扁再展开 | 挤压、弹性 |

### 扭曲/变形

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `crosswarp` | 交叉扭曲变形 | 扭曲、变形 |
| `directionalwarp` | 定向扭曲变形 | 定向扭曲 |
| `displacement` | 位移映射扭曲 | 位移、扭曲 |
| `morph` | 图像变形 | 变形、过渡 |
| `Swirl` | 漩涡扭曲 | 漩涡、旋转 |
| `ButterflyWaveScrawler` | 蝴蝶翅膀波浪扭曲 | 蝴蝶、波浪 |
| `DefocusBlur` | 散焦模糊 | 模糊、失焦 |
| `LinearBlur` | 线性运动模糊 | 运动、速度 |
| `tangentMotionBlur` | 正切运动模糊 | 高速运动 |

### 马赛克/像素

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `AdvancedMosaic` | 像素 mosaic 从中心向外扩散再收缩 | 像素化、扩散 |
| `Mosaic` | 马赛克方块变形 | 马赛克、变形 |
| `mosaic_transition` | 马赛克方块渐变 | 马赛克、渐变 |
| `pixelize` | 像素化先像素化再恢复 | 像素化、复古 |
| `BlockDissolve` | 随机方块逐个溶解 | 方块、溶解 |
| `randomsquares` | 随机方块逐个翻转 | 随机、翻转 |
| `chessboard` | 棋盘格逐个翻转 | 棋盘、翻转 |
| `PuzzleRight` | 拼图方块滑入 | 拼图、滑入 |
| `TilesWave` | 瓦片波浪对角线 | 瓦片、波浪 |
| `PolkaDotsCurtain` | 波尔卡圆点幕布 | 圆点、幕布 |

### 几何/图案

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `BowTieHorizontal` | 水平蝴蝶结/菱形展开 | 蝴蝶结、对称 |
| `BowTieVertical` | 垂直蝴蝶结/菱形展开 | 垂直对称 |
| `BowTieWithParameter` | 可调参数的蝴蝶结过渡 | 可调蝴蝶结 |
| `Box` | 矩形框缩放展开 | 矩形、缩放 |
| `hexagonalize` | 六边形蜂窝变形 | 蜂窝、六边形 |
| `kaleidoscope` | 万花筒旋转 | 万花筒、旋转 |
| `powerKaleido` | 强力万花筒多重对称 | 多重对称 |

### 故障/特效

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `GlitchDisplace` | 故障位移色彩偏移 | 故障、数字 |
| `GlitchMemories` | 复古故障像素块 | 复古故障 |
| `parametric_glitch` | 参数化故障螺旋+色彩 | 参数化故障 |
| `DoomScreenTransition` | Doom 风格条形故障 | Doom、游戏 |
| `EdgeTransition` | 边缘高亮扫描 | 边缘、高亮 |
| `CrazyParametricFun` | 参数方程螺旋扫描 | 螺旋、疯狂 |
| `fragment` | 碎片飞散 | 碎片、飞散 |

### 灼烧/火焰

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `burn` | 灼烧边缘燃烧扩散 | 灼烧、燃烧 |
| `burn0` | 带颜色的灼烧 | 彩色灼烧 |
| `FilmBurn` | 胶片灼烧噪点 | 胶片、复古 |
| `undulatingBurnOut` | 波动灼烧波纹状燃烧 | 波动燃烧 |
| `dissolve` | 热熔溶解炽热边缘 | 热熔、溶解 |
| `Overexposure` | 过曝闪烁 | 过曝、闪光 |

### 水/自然

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `WaterDrop` | 水滴涟漪 | 水滴、涟漪 |
| `ripple` | 水波涟漪 | 水波、流动 |
| `wind` | 风吹像素被刮走 | 风、吹散 |

### 梦幻/柔和

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `Dreamy` | 梦幻漂浮波浪偏移 | 梦幻、柔和 |
| `DreamyZoom` | 梦幻旋转缩放 | 梦幻旋转 |
| `perlin` | 柏林噪声扰动 | 噪声、柔和 |
| `randomNoisex` | 随机噪点 | 随机、噪点 |

### 门/洞

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `doorway` | 门洞效果 slit 打开 | 门洞、进入 |
| `HorizontalClose` | 水平关门 | 关闭、合拢 |
| `HorizontalOpen` | 水平开门 | 打开、展开 |
| `VerticalClose` | 垂直关门 | 垂直关闭 |
| `VerticalOpen` | 垂直开门 | 垂直打开 |

### 其他

| 效果名 | 说明 | 情感表达 |
|--------|------|----------|
| `coord-from-in` | 坐标错位入场 | 错位、入场 |
| `crosshatch` | 交叉阴影线溶解 | 阴影、溶解 |
| `Bounce` | 弹跳球效果 | 弹跳、活泼 |
| `StereoViewer` | 立体查看器分割缩放 | 立体、分割 |
| `TVStatic` | 电视雪花静态 | 电视、静态 |

---

## 注意事项

- 避免暗背景全屏线性渐变（H.264 条带）
- `light_leak` 的 `seed` 控制随机性 — 固定 seed 确保确定性
- `gl_transition` 性能取决于着色器复杂度
