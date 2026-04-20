# Div / Timeline 统一节点架构设计

状态：已评审对齐，待按实现计划落地

日期：2026-04-21

## 结论摘要

本次重构采用减法架构，核心结论如下：

1. `Div` 与 `Timeline` 都是 `NodeKind` 的普通成员，不再保留 `Layer`、`root special case`、`transition::Timeline` 等特权结构。
2. `Div` 是空间维度容器，`Timeline` 是时间维度容器；两者都遵循统一的 `NodeStyle` 语义，仅在“子节点沿哪个维度排布”上不同。
3. `Composition.root` 改为直接持有 `Node`，不再持有 `RootComponent` 函数别名。
4. `TimelineNode` 以 `Vec<Segment>` 表示，`Transition` 作为相邻段之间的关系，挂在后一段的 `transition_in` 上。
5. JSONL 新增 `tl`，移除 `layer`；`transition` 保持为关系对象，但新增 `parentId`，明确归属到对应 `tl`。
6. 动态行为不属于 `NodeStyle` 或 Tailwind 类名；动画通过节点挂载的 `script_driver` 脚本驱动实现，`Timeline` 继续独占时间编排权。

一句话心智模型：

> `Div = 空间容器，Timeline = 时间容器。两者都是 NodeKind，一样能挂 style、一样能接 children，仅在排布维度上不同。`

## 背景与问题

当前架构存在四类特权：

1. `layer` 是特殊容器。
2. `transition` 依附于特殊的 `transition::Timeline` 结构，而不是统一节点体系中的时间关系。
3. `Composition.root` 通过函数闭包获得隐式特权，而不是普通节点。
4. 时间编排逻辑分散在 `layer / timeline / scene / transition` 的层级关系里，学习成本高。

这些特权带来的主要问题不是代码量，而是心智负担：

1. 新作者必须先记住概念等级，再去理解能力边界。
2. 同样的样式和定位需求，在 `Div`、`Layer`、`Timeline` 上需要不同解释。
3. JSONL 与 Rust API 虽然表面可互相映射，但底层本体不一致，导致 schema 演化容易出现历史化石。

本次重构的目标是把这些特权全部压平，形成单一心智模型。

## 设计目标

1. 消除 `Layer`，让容器语义统一到 `Div` 与 `Timeline`。
2. 让 `Composition.root` 成为普通 `Node`，不再存在根节点专属容器或专属闭包类型。
3. 保持 Rust API 与 JSONL schema 的同源语义。
4. 允许空间容器与时间容器任意嵌套。
5. 把时间编排权严格收束到 `Timeline + Segment + Transition`，避免出现第二套时间系统。

## 非目标

1. 本次不节点化 `audio_sources`。
2. 本次不重新设计渲染后端的全部 compositor 管线，只在必要处接入新数据模型。
3. 本次不引入 shader/GL transition 扩展能力。
4. 本次不允许通过 Tailwind / `NodeStyle` 声明式描述动画或转场。

## 架构公理

### A1. 层级均质

`NodeKind` 的所有变体互为兄弟，不存在“顶层专属”或“容器专属”的节点型。`Composition.root: Node` 可以是任意 `NodeKind` 实例。

### A2. 样式公理化

每种 `NodeKind` 都遵循统一的 `NodeStyle`。`position / size / transform / opacity / filter / text / layout` 等语义在所有 kind 上保持一致，不设置节点型特例。

注意：

1. `NodeStyle` 只表达静态样式与脚本挂点，不负责声明式时间动画。
2. 动态变化由节点级 `script_driver` 执行后生成逐帧 mutation，再叠加回节点样式。
3. Tailwind `className` 继续只承载静态样式，不承载动画、过渡、时长、缓动等动态属性。

### A3. 容器 = 子节点 + 排布维度

只有两种容器：

1. `Div`：子节点沿空间维度排布。
2. `Timeline`：子节点沿时间维度排布。

其他 `NodeKind` 默认视为叶子节点。

## 必然推论

### P1. 定位与变换统一走 `NodeStyle`

`timeline().style(...)` 与 `div().style(...)` 语义一致，因此“让某个 timeline 占左上 1/4”不需要额外包一层 `Div`。

### P2. 序列化语义二分

1. 声明式动态节点可序列化进 JSONL，例如 `Timeline`、`Caption`。
2. 命令式动态节点不可序列化，例如 `Component` 的 `Fn(FrameCtx) -> Node` 闭包。

这与 React 中“JSX 可序列化、组件函数不可序列化”的心智一致。

### P3. 嵌套自由

`Div` 与 `Timeline` 可以自由嵌套。`div().child(timeline().child(div().child(caption())))` 是合法且有意义的组合，不再有“只有某类顶层节点才能承载时间结构”的特权约束。

## 时间代数

时间轴总时长公式如下：

```text
timeline_duration = Σ segment_i.duration - Σ transition_i.duration
```

约束解释：

1. `Transition` 吃掉的是相邻两个 segment 的 overlap。
2. 转场不是额外追加帧，而是复用相邻场景的重叠帧区间。

## Rust 数据模型

### `NodeKind` 变更

移除 `Layer` 分支。

```rust
pub enum NodeKind {
    Component(ComponentNode),
    Div(Div),
    Canvas(Canvas),
    Text(Text),
    Image(Image),
    Lucide(Lucide),
    Video(Video),
    Timeline(TimelineNode),
    Caption(CaptionNode),
}
```

同步删除：

1. `LayerNode`
2. `From<LayerNode> for NodeKind`
3. `From<LayerNode> for Node`
4. 所有 `match NodeKind::Layer` 分支

### `TimelineNode`

```rust
pub struct TimelineNode {
    pub(crate) segments: Vec<Segment>,
    pub(crate) style: NodeStyle,
}

pub struct Segment {
    pub child: Node,
    pub duration: u32,
    pub transition_in: Option<Transition>,
}
```

`TimelineNode` 不保留 `duration_in_frames` 字段，而提供计算方法：

```rust
impl TimelineNode {
    pub fn duration_in_frames(&self) -> u32 {
        let sum_seg: u32 = self.segments.iter().map(|s| s.duration).sum();
        let sum_trans: u32 = self
            .segments
            .iter()
            .filter_map(|s| s.transition_in.as_ref())
            .map(|t| t.duration_in_frames())
            .sum();
        sum_seg.saturating_sub(sum_trans)
    }
}
```

设计纪律：

1. `duration_in_frames` 是方法，不是缓存字段，避免和 `segments` 漂移。
2. `transition_in` 挂在后一段，而不是前一段的 `transition_out`。
3. 第一个 `Segment` 的 `transition_in` 必须为 `None`。

### `CaptionNode`

```rust
pub struct CaptionNode {
    pub(crate) style: NodeStyle,
    pub(crate) entries: Vec<SrtEntry>,
    pub(crate) text_template: Text,
}
```

约束：

1. `Caption` 最终展开为 `Text` display item，而不是新增独立文本渲染后端。
2. `Caption` 沿用统一 `NodeStyle` 语义，不新增 caption 专属布局字段。

### `Composition`

旧：

```rust
type RootComponent = dyn Fn(&FrameCtx) -> Node + Send + Sync;

pub struct Composition {
    pub root: Arc<RootComponent>,
    ...
}
```

新：

```rust
pub struct Composition {
    pub root: Node,
    ...
}
```

结论：

1. 删除 `RootComponent` 类型别名。
2. 根节点不再拥有函数式特权。
3. 若需要命令式动态根，直接把 `ComponentNode` 作为 `root`。

### `ComponentNode`

旧结构中的 `duration_in_frames` 字段删除，仅保留：

```rust
pub struct ComponentNode {
    render: Arc<dyn Fn(&FrameCtx) -> Node + Send + Sync>,
    style: NodeStyle,
}
```

理由：

1. 根节点总时长由 `Composition.frames` 决定。
2. 若组件被时间轴消费，其段落时长由 `Segment.duration` 决定。
3. 保留 `ComponentNode.duration_in_frames` 会引入第三个时长源，造成语义漂移。

## 构建器 API

推荐 Rust API 形状：

```rust
div().child(simple_scene).child(caption(srt))

div()
    .child(
        timeline()
            .segment(scene_a, 180)
            .transition(fade().duration(30))
            .segment(scene_b, 180),
    )
    .child(caption(srt))

div()
    .child(
        timeline()
            .style("absolute top-0 left-0 w-1/2 h-1/2")
            .segment(scene_a, 180)
            .transition(slide().duration(30))
            .segment(scene_b, 180),
    )
```

`TimelineBuilder` 状态机纪律：

1. 内部维护 `segments: Vec<Segment>` 与 `pending_transition: Option<Transition>`。
2. `.segment(child, duration)` 会消耗 `pending_transition` 并写入新 `Segment.transition_in`。
3. `.transition(t)` 要求当前至少已有一个 segment，且不能重复挂 pending transition。
4. `.build()` 或 `Into<Node>` 负责最终封装成 `TimelineNode`。

建议保留 panic 级构建期校验，用于尽早暴露非法链式调用。

## 文件与模块重划

### 删除

1. `scene/layer.rs`
2. `scene::layer::*` 的全部 re-export
3. `layer()` 构建器 API

### 收缩

`scene/transition.rs` 仅保留转场算子定义与 builder：

1. `Transition enum`
2. `fade / slide / wipe / clock_wipe / iris / light_leak`

移除：

1. 旧 `Timeline`
2. `TimelineItem`
3. `timeline()` 函数
4. `impl Timeline::into_timeline()`

### 扩展

`scene/time.rs` 承担时间容器全部职责：

1. `TimelineNode`
2. `Segment`
3. `TimelineBuilder`
4. `timeline() -> TimelineBuilder`

### 修订

1. `scene/node.rs`
2. `scene/composition.rs`
3. `scene/primitives/caption.rs`
4. `jsonl/builder.rs`
5. `lib.rs`

## JSONL Schema

### 总览

本次 schema 变化很小，主要是组织方式变化：

1. 新增 `type: "tl"`
2. 删除 `type: "layer"`
3. `div / text / image / video / canvas / lucide / caption` 保持原样
4. `transition` 保持为关系对象，但新增 `parentId`

### `tl` 节点

示例：

```json
{"id":"main-tl","parentId":"root","type":"tl","className":"absolute inset-0"}
```

字段：

1. `id`：建议必填，便于关系校验。
2. `parentId`：必填。
3. `type: "tl"`：必填。
4. `className`：可选。

说明：

1. `tl` 自身不带 `duration`。
2. timeline 时长由其子 segment 与 transition 自动推导。

### `transition` 关系对象

示例：

```json
{
  "type":"transition",
  "parentId":"tl-1",
  "from":"scene-a",
  "to":"scene-b",
  "effect":"fade",
  "duration":30
}
```

字段：

1. `type: "transition"`
2. `parentId`：必填，必须指向一个 `tl`
3. `from`
4. `to`
5. `effect`
6. `duration`
7. effect-specific 附加字段

关键约束：

1. `transition` 不再通过全局扫描推断属于哪条 timeline。
2. `parentId` 直接声明其归属 timeline。
3. `from / to` 只允许引用该 timeline 的直接 children。

### 样例

```json
{"id":"root","parentId":null,"type":"div","className":"relative w-full h-full"}
{"id":"tl-1","parentId":"root","type":"tl","className":"absolute inset-0"}
{"id":"scene-a","parentId":"tl-1","type":"div","className":"...","duration":180}
{"id":"scene-b","parentId":"tl-1","type":"div","className":"...","duration":180}
{"id":"subs","parentId":"root","type":"caption","className":"absolute bottom-0 inset-x-0 text-white","path":"sub.srt"}
{"type":"transition","parentId":"tl-1","from":"scene-a","to":"scene-b","effect":"fade","duration":30}
```

这里 `subs` 与 `tl-1` 是兄弟节点，因此字幕贯穿整片，而不是成为某个时间段。

## JSONL 构建规则

### 建树

仍按 `parentId` 建树。`parentId: null` 为根节点。

### `tl` 的 build 规则

伪代码：

```text
build_timeline(tl):
  1. children = JSONL 行序中 parentId == tl.id 且 type != "transition" 的节点
  2. transitions = JSONL 行序中 parentId == tl.id 且 type == "transition" 的关系对象
  3. segments = 按 children 顺序构造 Segment { child: build_node(child), duration: child.duration }
  4. 对每个 transition:
       - 校验 from / to 都属于 children
       - 校验 to 紧邻 from 的下一个 child
       - 把 transition 写入 segments[idx_to].transition_in
  5. 返回 TimelineNode { segments, style: parse_className(tl.className) }
```

### 校验规则

1. `tl` 的每个 child 必须带 `duration`。
2. `transition.parentId` 必须指向一个 `tl` 节点。
3. `transition.from / to` 必须存在，且必须是该 `tl` 的直接 children。
4. `transition.to` 必须是 `transition.from` 的相邻后继节点。
5. 任意相邻 segment 对之间最多一个 transition。
6. `caption` 不能作为 `tl` 的直接 child；若需要全片字幕，应放在外层 `div` 下与 timeline 并列。
7. 第一个 segment 不能有 `transition_in`。

## 迁移策略

### 阶段 0：勘查

1. 盘点 `json/` 下所有 JSONL 样本。
2. 盘点 `src/` 中内嵌 JSONL 测试。
3. 盘点全部 `layer()` 调用点。
4. 盘点全部旧 `transition::Timeline` 调用点。

### 阶段 1：Rust 侧非破坏性并存

1. 在 `scene/time.rs` 引入新 `Segment / TimelineNode / TimelineBuilder`。
2. 为 `CaptionNode` 增加 `text_template`。
3. 为 `Composition` 新增直接接收 `Node` 的构造入口。
4. 旧 API 暂时并存，但新代码只使用新模型。

阶段目标：始终保持可编译、测试全绿。

### 阶段 2：Rust 侧切换

1. 全项目 `layer()` 迁移到 `div()`。
2. 全项目 `timeline()` 改为使用 `scene::time::timeline`。
3. 根闭包调用点改为 `Node` 根或 `ComponentNode` 根。
4. 删除旧 `Layer`、旧 `transition::Timeline`、`TimelineItem`、`RootComponent`、`ComponentNode.duration_in_frames`。

阶段目标：Rust API 全部完成切换，不再保留旧本体。

### 阶段 3：JSONL schema 升级

1. 解析层新增 `tl`，删除 `layer`。
2. builder 层按新 timeline 规则组装 `Segment`。
3. 全量更新测试 JSONL 与样本 JSONL。
4. 对关键样本做渲染回归确认视觉不变。

阶段目标：JSONL 与 Rust API 的新本体完全对齐。

### 阶段 4：文档与清理

1. 更新 README / 设计文档 / 示例注释。
2. 删除遗留 re-export 和无用 import。
3. 全局确认 `LayerNode`、`RootComponent` 等历史名词已清零。

阶段目标：只剩新心智模型，不保留教学层面的双轨。

## 脚本驱动与时间系统边界

这是本次设计里最需要写死的一条边界：

1. 动态行为通过节点挂载 `script_driver` 实现，而不是通过 `NodeStyle.animation` 或 Tailwind 动效类实现。
2. `NodeStyle` 是静态样式容器加脚本挂点，不是 declarative animation schema。
3. 脚本运行后产生的是逐帧 mutation，例如 `opacity / transform / text_content` 等。
4. `Timeline` 继续独占时间编排权，`script_driver` 不得修改 `segments`、`duration`、`transition` 等时间结构。

这条边界的理由是：

1. 若样式层再引入一套声明式时间系统，会与 `Timeline` 的时间语义冲突。
2. 若脚本可以修改 timeline 结构，就会把“时间编排”和“视觉 mutation”混在同一层，长期不可维护。

## 开放项

以下问题不阻塞 spec 定稿，但需要在实现计划阶段收口。

### O1. `Caption` 展开为 `Text` 的时机

两个候选方案：

1. build 阶段展开：display list 中不再保留 `Caption`。
2. render 阶段展开：display list 保留 `Caption`，渲染时再委托给 `Text`。

当前倾向：

1. 优先 build 阶段展开。
2. 最终取决于现有 `display/build.rs` 与缓存路径的实际结构。

### O2. `Timeline` 在显示列表与 compositor 中的接合点

候选：

1. build 阶段完全展开
2. render 阶段保留整体
3. 混合策略，仅对 transition overlap 走专门路径

当前倾向：

1. 采用混合策略，以最小改动接入现有 compositor。
2. 最终决策依赖 `runtime/compositor/*` 现状勘查。

### O3. `TimelineNode` 是否允许挂 `script_driver`

推荐结论：

1. 允许挂载。
2. 其作用域只限于 timeline 自身作为一个节点的样式表现，以及其子树样式 mutation。
3. 明确禁止通过脚本改写时间结构。

若实现期发现这会污染边界，可退化为“仅叶子和普通空间容器支持脚本”。

## Future Work

1. `audio_sources` 节点化。
2. nested timelines 的系统化测试与文档化。
3. `Transition::Gl { shader, uniforms }` 类 GPU 转场能力。
4. 动态 `segment.duration` 设计。

## 风险与长期维护成本

1. 若 `transition` 继续允许脱离 `parentId` 全局扫描归属，后续 nested timelines 会立即变得歧义化，因此本次必须引入 `parentId`。
2. 若 `Composition.root` 继续保留闭包特权，根节点与普通节点会长期分裂成两套模型，本次必须合并。
3. 若 `ComponentNode.duration_in_frames` 不删除，未来排查时长 bug 会出现三源对账，维护成本会持续升高。
4. 若文档不明确写出“脚本负责动画，timeline 负责编排”，后续作者会自然尝试把动态 utility 塞进 `className`，最终把静态样式系统污染成半套动画 DSL。

## 最终总结

本次重构的价值不在于新增能力，而在于删除特权。

删除 `Layer`、删除旧 `transition::Timeline`、删除 `RootComponent` 特权、删除重复时长来源之后，代码库、JSONL schema、脚本 API 将共享同一条公理：

> `Div` 管空间，`Timeline` 管时间，脚本只做逐帧 mutation，时间编排只由 `Timeline + Segment + Transition` 负责。
