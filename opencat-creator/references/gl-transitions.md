# GL Transitions Reference

可以使用任何不在内置效果表中的名称作为 `effect`，运行时会在 `gltransition.json` 中查找同名 GLSL 着色器并编译为 Skia 着色器执行。

```json
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"crosswarp","duration":15}
```

## 效果列表

| Name | 描述 |
|------|------|
| `AdvancedMosaic` | 像素 mosaic 从中心向外扩散再收缩 |
| `BlockDissolve` | 随机方块逐个溶解过渡 |
| `BookFlip` | 书本翻页效果 |
| `Bounce` | 弹跳球效果，新画面从顶部弹入 |
| `BowTieHorizontal` | 水平蝴蝶结/菱形从中心展开 |
| `BowTieVertical` | 垂直蝴蝶结/菱形从中心展开 |
| `BowTieWithParameter` | 可调参数的蝴蝶结过渡，支持反向 |
| `Box` | 矩形框从指定角/中心缩放展开 |
| `ButterflyWaveScrawler` | 蝴蝶翅膀波浪扭曲过渡 |
| `CircleCrop` | 圆形裁剪收缩/展开 |
| `ColourDistance` | 根据像素颜色差值逐像素过渡 |
| `CrazyParametricFun` | 参数方程驱动的螺旋扫描过渡 |
| `CrossZoom` | 交叉缩放模糊过渡 |
| `DefocusBlur` | 散焦模糊 — 先模糊再清晰 |
| `Directional` | 沿指定方向的滑动覆盖 |
| `DirectionalScaled` | 带缩放的定向滑动 |
| `DoomScreenTransition` | Doom 风格 — 条形故障扫描+波纹 |
| `Dreamy` | 梦幻漂浮 — 柔和波浪形偏移 |
| `DreamyZoom` | 梦幻旋转缩放 |
| `EdgeTransition` | 边缘高亮扫描过渡 |
| `FilmBurn` | 胶片灼烧 — 随机噪点烧灼效果 |
| `Fold` | 折叠 — 像折纸一样翻折 |
| `GlitchDisplace` | 故障位移 — 色彩偏移+扭曲 |
| `GlitchMemories` | 复古故障 — 像素块噪点错位 |
| `GridFlip` | 网格逐个翻牌 |
| `HSVfade` | HSV 色彩空间渐变过渡 |
| `HorizontalClose` | 水平关门 — 两侧向中间合拢 |
| `HorizontalOpen` | 水平开门 — 从中间向两侧打开 |
| `InvertedPageCurl` | 反向翻页卷角 |
| `LeftRight` | 左右错位拉伸过渡 |
| `LinearBlur` | 线性运动模糊过渡 |
| `Mosaic` | 马赛克方块变形过渡 |
| `Overexposure` | 过曝闪烁 — 亮度爆发过渡 |
| `PolkaDotsCurtain` | 波尔卡圆点幕布效果 |
| `PuzzleRight` | 拼图方块从右滑入 |
| `Radial` | 径向擦除 — 从中心向外扇形展开 |
| `Rectangle` | 矩形缩放展开（带背景色） |
| `RectangleCrop` | 矩形裁剪 — 缩小到中心再放大 |
| `Rolls` | 卷轴滚动 — 从角落卷起 |
| `RotateScaleVanish` | 旋转缩放消失/出现 |
| `SimpleFlip` | 简单 3D 翻转 |
| `SimpleZoom` | 简单放大过渡 |
| `SimpleZoomOut` | 简单缩小过渡 |
| `Slides` | 幻灯片 — 从边/角滑入滑出 |
| `StarWipe` | 星形擦除 — 星星形状展开 |
| `StaticFade` | 静态噪点淡入淡出 |
| `StereoViewer` | 立体查看器 — 从中间分割缩放 |
| `Swirl` | 漩涡扭曲 — 先扭曲再恢复 |
| `TVStatic` | 电视雪花静态杂讯过渡 |
| `TilesWave` | 瓦片波浪 — 从左下到右上的对角线波 |
| `TopBottom` | 上下错位拉伸过渡 |
| `VerticalClose` | 垂直关门 — 上下向中间合拢 |
| `VerticalOpen` | 垂直开门 — 从中间向上下打开 |
| `WaterDrop` | 水滴滴落涟漪效果 |
| `ZoomInCircles` | 多圆形放大过渡 |
| `ZoomLeftWipe` | 左侧缩放擦除 |
| `ZoomRigthWipe` | 右侧缩放擦除 |
| `angular` | 角度擦除 — 从指定角度扇形展开 |
| `burn` | 灼烧效果 — 边缘燃烧扩散 |
| `burn0` | 带颜色的灼烧效果 |
| `cannabisleaf` | 大麻叶形状遮罩过渡 |
| `chessboard` | 棋盘格逐个翻转 |
| `circle` | 圆形遮罩展开/收缩 |
| `circleopen` | 圆形打开/关闭 |
| `colorphase` | 逐颜色通道分阶段过渡 |
| `coord-from-in` | 坐标错位入场 |
| `crosshatch` | 交叉阴影线溶解 |
| `crosswarp` | 交叉扭曲变形 |
| `cube` | 3D 立方体旋转 |
| `directional-easing` | 带缓动的定向滑动 |
| `directionalwarp` | 定向扭曲变形 |
| `directionalwipe` | 定向擦除 |
| `displacement` | 位移映射扭曲 |
| `dissolve` | 热熔溶解 — 炽热边缘消散 |
| `doorway` | 门洞效果 — 中间 slit 打开 |
| `fade` | 简单淡入淡出 |
| `fadecolor` | 带中间色的淡入淡出 |
| `fadegrayscale` | 先转灰度再淡出 |
| `flyeye` | 苍蝇复眼 — 六边形透镜扭曲 |
| `fragment` | 碎片飞散效果 |
| `heart` | 心形遮罩过渡 |
| `hexagonalize` | 六边形蜂窝状变形 |
| `kaleidoscope` | 万花筒旋转过渡 |
| `luma` | 亮度键控过渡 |
| `luminance_melt` | 高亮区域先融化流失 |
| `morph` | 图像变形 morph |
| `mosaic_transition` | 马赛克方块渐变 |
| `multiply_blend` | 正片叠底混合过渡 |
| `parametric_glitch` | 参数化故障 — 螺旋+色彩偏移 |
| `perlin` | 柏林噪声扰动过渡 |
| `pinwheel` | 风车旋转擦除 |
| `pixelize` | 像素化 — 先像素化再恢复 |
| `polar_function` | 极坐标函数花瓣遮罩 |
| `powerKaleido` | 强力万花筒 — 多重对称旋转 |
| `randomNoisex` | 随机噪点过渡 |
| `randomsquares` | 随机方块逐个翻转 |
| `ripple` | 水波涟漪效果 |
| `rotateTransition` | 旋转拼贴过渡 |
| `rotate_scale_fade` | 旋转+缩放+淡出组合 |
| `scale-in` | 缩放入场 |
| `splitSlideInHorizontal` | 水平分裂滑入（从中间分开） |
| `splitSlideInOutHorizontal` | 水平分裂 — 旧画面滑出+新画面滑入 |
| `splitSlideInOutVertical` | 垂直分裂 — 旧画面滑出+新画面滑入 |
| `splitSlideInVertical` | 垂直分裂滑入（从中间分开） |
| `splitSlideOutHorizontal` | 水平分裂滑出 |
| `splitSlideOutVertical` | 垂直分裂滑出 |
| `squareswire` | 方块网格线擦除 |
| `squeeze` | 垂直挤压 — 压扁再展开 |
| `static_wipe` | 静态噪点擦除 |
| `swap` | 3D 翻转交换 |
| `tangentMotionBlur` | 正切运动模糊过渡 |
| `undulatingBurnOut` | 波动灼烧 — 从中心波纹状燃烧 |
| `wind` | 风吹 — 像素被风刮走 |
| `windowblinds` | 百叶窗效果 |
| `windowslice` | 窗户切片 — 垂直条逐个显露 |
| `wipeDown` | 向下擦除 |
| `wipeLeft` | 向左擦除 |
| `wipeRight` | 向右擦除 |
| `wipeUp` | 向上擦除 |
| `x_axis_translation` | X 轴平移滑动 |
| `zoomInOut` | 先放大旧画面再缩小显示新画面 |
