# OpenCat

OpenCat 是一个用 Rust 编写的声明式视频渲染库。它的核心目标不是做时间线编辑器，而是回答一个更基础的问题：

给定某一帧的上下文，当前画面应该是什么样，以及如何把这个结果稳定、高效地变成像素和视频。

当前架构已经明确收敛为一条单向主链路：

`ui tree -> element tree -> layout tree -> display tree/display list -> backend(skia)`

这条链路是项目当前最重要的约束。各层职责需要保持单一，import 只能单向流动，缓存、dirty、diff 这类策略只能建立在某一层已经产出的“事实”之上，不能反向污染更低层。

## 当前架构

### 1. UI Tree

用户入口层负责表达“我要什么场景”，不负责解释如何布局或如何绘制。

- `view.rs`
  - 通用 `Node` / `NodeKind`
- `nodes/`
  - `div` / `text` / `image` / `video` / `canvas` / `lucide`
- `composition.rs`
  - `Composition`、尺寸、帧率、总帧数、根组件
- `timeline.rs`
  - 普通场景和转场时间语义
- `transitions.rs`
  - 场景切换与转场配置
- `parser.rs`
  - JSONL -> UI tree

这一层的产物是用户声明的场景树。它可以是 Rust API，也可以来自 JSONL，但两者都只是同一层的不同输入形式。

### 2. Element Tree

`element` 层负责把用户表达解析成“可布局、可渲染语义”的稳定结构。

- `element/resolve.rs`
  - 组件展开
  - 脚本驱动的逐帧样式修改
  - 文本继承样式合并
  - 图片/视频资源绑定
- `element/tree.rs`
  - `ElementNode`
- `element/style.rs`
  - `ComputedStyle`

这一层的重点不是几何，而是渲染语义归一化。到这里为止，系统已经知道“节点是什么、样式是什么、资源是什么”，但还不知道几何落位。

### 3. Layout Tree

`layout` 层现在是纯几何层，不再携带 paint 语义。

- `layout/mod.rs`
  - `LayoutSession`
  - Taffy 布局计算
  - 结构/布局/栅格/合成 dirty 统计
- `layout/tree.rs`
  - `LayoutTree`
  - `LayoutNode { id, rect, children }`

这一层只回答：

- 每个节点的矩形几何是什么
- 本帧相对上一帧发生了哪种级别的变化

它不负责决定如何绘制，也不负责决定具体的缓存粒度。

### 4. Display Tree / Display List

`display` 层负责把“element 语义 + layout 几何”组装成后端可消费的绘制语义。

- `display/tree.rs`
  - retained `DisplayTree`
- `display/list.rs`
  - flat `DisplayList`
- `display/build.rs`
  - `ElementNode + LayoutTree -> DisplayTree`
  - `DisplayTree -> DisplayList`
- `display/analysis.rs`
  - display-level 分析，比如是否包含视频
- `display/cache_key.rs`
  - text picture key
  - subtree picture key

这里是当前渲染语义真正收敛的地方。

`DisplayTree` 保留层级关系，供 subtree cache / snapshot planning 使用。  
`DisplayList` 是线性执行形式，供 backend 快速执行。

### 5. Render / Snapshot Planning

`render` 层只负责 orchestration，不直接实现低层绘制细节。

- `render.rs`
  - frame orchestration
  - scene / transition 分发
  - profile 汇总
- `render/invalidation.rs`
  - 把 `LayoutPassStats + contains_video` 解释为统一的 invalidation 语义
- `scene_snapshot.rs`
  - scene snapshot planning
  - snapshot 复用 / 录制 / direct draw 回退

当前 invalidation 语义已经统一为：

- `Clean`
- `Composite`
- `Raster`
- `Layout`
- `Structure`
- `TimeVariant`

这里的原则是：

- `layout` 产出 dirty facts
- `display` 产出渲染语义与 cache key
- `render` 解释这些事实，并决定采用哪种 snapshot / reuse 策略

### 6. Backend

`backend` 只执行 display 语义，不再直接依赖 `layout`。

- `backend/skia.rs`
  - 执行 `DisplayList`
  - 执行 `DisplayTree` 上的 subtree cache 绘制
- `backend/skia_transition.rs`
  - transition 实现
- `backend/cache.rs`
  - image/text/subtree picture caches
- `backend/resource_cache.rs`
  - backend cache 聚合访问

这一层不再负责解释“节点是什么”，也不再直接理解 layout tree。

### 7. Media / Codec / Assets

- `assets.rs`
  - 资源注册、Openverse 查询、远程图片预拉取
- `media.rs`
  - 图片/视频位图访问
- `codec/decode.rs`
  - 视频解码
- `codec/encode.rs`
  - RGBA -> MP4

## 当前每帧主流程

一帧的渲染主链路现在是：

1. 构造 `FrameCtx`
2. 从 `Composition` 得到 UI tree
3. 解析为 `ElementNode`
4. 用 `LayoutSession` 计算 `LayoutTree + LayoutPassStats`
5. 从 `ElementNode + LayoutTree` 构建 `DisplayTree`
6. 从 `DisplayTree` 展平为 `DisplayList`
7. 用 `display` 分析结果和 `layout` dirty facts 生成 invalidation
8. `render/scene_snapshot` 决定：
   - 直接执行 `DisplayList`
   - 记录并复用整场景 snapshot
   - 基于 `DisplayTree` 走 subtree cache
9. `backend/skia` 执行 display 语义
10. 输出 PNG 或编码 MP4

## 当前边界约束

这是当前代码最需要守住的部分：

### `ui -> element`

- UI 层只表达用户意图
- 组件展开、样式继承、脚本修改都应在 element 层完成

### `element -> layout`

- layout 只消费 element 的结构与 layout style
- layout 产出几何与 dirty facts
- layout 不负责 paint 语义

### `layout + element -> display`

- display 是第一层完整的渲染语义
- 所有 backend 可见的结构都应来自 display，而不是 layout

### `display -> backend`

- backend 只能执行 display 语义
- backend 不应直接 import `layout`

### `facts -> policy`

- dirty / diff / contains_video / cache key 都是 facts
- snapshot policy / reuse policy / fallback policy 都应集中在 render 层解释

## 代码结构速览

```text
src/
  composition.rs
  frame_ctx.rs
  view.rs
  nodes/
  parser.rs
  timeline.rs
  transitions.rs

  element/
    resolve.rs
    style.rs
    tree.rs

  layout/
    mod.rs
    tree.rs

  display/
    analysis.rs
    build.rs
    cache_key.rs
    list.rs
    tree.rs

  render.rs
  render/
    invalidation.rs
  scene_snapshot.rs
  render_cache.rs
  profile.rs

  backend/
    cache.rs
    resource_cache.rs
    skia.rs
    skia_transition.rs

  assets.rs
  media.rs
  codec/
```

## 开发与验证

基础命令：

```bash
rtk cargo fmt
 rtk cargo test
 rtk cargo run --example hello_world
```

性能/架构相关改动后，必须额外跑：

```bash
 rtk cargo run --example video_playback
```

`examples/video_playback.rs` 是当前最重要的 profile 场景。它同时覆盖：

- 文本
- 图片
- 视频
- canvas
- transforms
- transitions
- snapshot / subtree cache 路径

任何对 `element/layout/display/render/backend` 边界、cache reuse、dirty 语义、snapshot planning 的修改，都应至少跑一次这个 example，并查看最终打印的 `Render profile`。

## 当前建议

如果继续沿当前方向推进，优先级应该是：

1. 继续保持 `layout` 纯几何，不把 paint 语义放回去
2. 让所有 backend-facing 结构都从 `display` 产出
3. 让 invalidation / snapshot policy 继续集中在 `render`
4. 用 `video_playback` 持续盯住 profile，避免边界纯化造成隐藏性能退化

现在最重要的不是继续加功能，而是保持这条主链路稳定、单向、可解释。
