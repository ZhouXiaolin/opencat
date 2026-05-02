# 场景转场

转场告诉观众两个场景之间的关系。选择匹配情感作用，而非技术。

## 多场景 Composition 动画规则

1. **始终用转场。** 无跳切。无例外。
2. **每个场景有入场动画。** 每个元素通过 `ctx.from()` 入场。无元素完整弹出。
3. **永不使用退场动画**（末场景除外）。`ctx.to()` 将 opacity 动画到 0 等在转场触发前**禁止**。转场就是退场。退出场景内容在转场开始时必须完全可见。
4. **末场景例外：** 可淡出元素（如淡出到黑场）。此为唯一允许 `ctx.to(..., { opacity: 0 })` 的场景。

## 能量 → 首选转场

| 能量 | 首选 | 备选 | Duration（帧） | Timing |
|------|------|------|---------------|--------|
| **平静** | `fade` | `light_leak` | 15-24 | `'ease-in-out'` |
| **中等** | `fade` / `slide` | `wipe` | 9-15 | `'ease-out'` |
| **高能** | `slide` | `clock_wipe` / `gl_transition` | 5-9 | `'linear'` |

选一个主要转场（60-70% 场景变化）+ 1-2 个强调。永不每个场景不同转场。

## 情绪 → 效果

| 情绪 | 推荐 |
|------|------|
| 温暖/邀请 | `fade`、`light_leak` |
| 冷/临床 | `wipe`、`slide` |
| 科技/未来 | `gl_transition`、`clock_wipe` |
| 有趣/好玩 | `slide`、`wipe`、`iris` |
| 戏剧/电影 | `iris`、`light_leak`、`fade` |
| 高级/奢华 | `fade`（18-24 帧） |

## 叙事位置

| 位置 | Duration（帧） |
|------|---------------|
| 开场 | 12-18 |
| 相关点之间 | 9 |
| 主题变化 | 9-12 |
| 高潮/揭示 | 5-9 |
| 放松 | 15-21 |
| 结尾 | 18-30 |

## 效果类型

| effect | 描述 | 可选 direction |
|--------|------|----------------|
| `fade` | 交叉淡入淡出 | — |
| `slide` | 滑动 | `from_left`(默认)/`from_right`/`from_top`/`from_bottom` |
| `wipe` | 擦除 | `from_left`(默认)/`from_right`/`from_top`/`from_bottom`/`from_top_left`/`from_top_right`/`from_bottom_left`/`from_bottom_right` |
| `clock_wipe` | 时钟擦除 | — |
| `iris` | 虹膜开合 | — |
| `light_leak` | 漏光（额外：`seed`、`hueShift`、`maskScale`） | — |
| `gl_transition` | GLSL 运行时着色器 | — |

未识别的 `effect` 名回退到 `gl_transition`，在 `gltransition.json` 中查找着色器。参数作为额外 JSON 字段。完整 GL 转场列表见 [gl-transitions.md](gl-transitions.md)。

## 模糊强度（按能量）

速度匹配过渡中的模糊强度应匹配能量：

| 能量 | 退场模糊 | 入场模糊 |
|------|----------|----------|
| 高 | 30px | 30px |
| 中 | 15px | 15px |
| 低 | 0px（无模糊） | 0px |

OpenCat 不直接支持 `blur-*` 的 tween。替代方案：用 `opacity` + `scale` 模拟速度感，或在 `type: "canvas"` 节点中通过 CanvasKit 实现模糊效果。

## 预设

### 速度匹配过渡

出口用加速缓动（`'ease-in'`），入口用减速缓动（`'ease-out'`）。两段曲线的最高速度在切点处相遇。

```jsonl
{"type":"script","parentId":"scene-out","src":"ctx.fromTo('content',{y:0,opacity:1},{y:-150,opacity:0,duration:10,delay:ctx.sceneFrames-10,ease:'ease-in'});"}
{"type":"script","parentId":"scene-in","src":"ctx.fromTo('content',{y:150,opacity:0},{y:0,opacity:1,duration:30,ease:'ease-out'});"}
```

### 滑动过渡

```jsonl
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"slide","direction":"from_right","duration":12,"timing":"ease-out"}
```

### 擦除过渡

```jsonl
{"type":"transition","parentId":"main-tl","from":"scene1","to":"scene2","effect":"wipe","direction":"from_top_left","duration":15,"timing":"ease-in-out"}
```

## 转场在 CSS 中不起作用的

OpenCat 的转场由运行时处理，不由 CSS。以下 CSS 模式**禁止**：

- `transition-*` className 类
- `animate-*` className 类
- `@keyframes` 声明
- `blur-*` 类动画（不支持 tween）

所有动效通过 `ctx.to()` / `ctx.from()` / `ctx.fromTo()` 或 JSONL `transition` 节点实现。

## 视觉模式警告

- 避免在暗背景上使用全屏线性渐变过渡（H.264 条带——用径向或实色+局部发光）
- `light_leak` 效果的 `seed` 参数控制随机性——固定 seed 确保确定性
- `gl_transition` 的性能取决于着色器复杂度——简单着色器（crosswarp、fade）比复杂着色器（BowTie、Flyeye）更快
