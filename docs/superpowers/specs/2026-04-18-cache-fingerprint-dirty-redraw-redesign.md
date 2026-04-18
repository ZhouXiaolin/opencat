# Cache / Fingerprint / Dirty / Redraw 子系统重设计

日期：2026-04-18
分支：`cache-refactor-2026-04-18`
基准 Profile：`cargo run --example video_playback`

---

## 1. 背景与动机

### 1.1 基准 Profile
```
frames: 366
avg ms/frame: backend 155.22, transition 10.45
transition avg ms/active-frame: slide 8.21 (24 f), light_leak 50.40 (72 f)
avg nodes/frame: reused 13.8, layout_dirty 0.4, raster_dirty 0.0, composite_dirty 4.5
backend avg ms/frame: video_decode 150.37, scene_snapshot_record 148.05,
                      light_leak_mask 3.21, light_leak_composite 6.70
backend avg counts/frame: scene_snapshot_hit 0.00, scene_snapshot_miss 0.00,
                          subtree_snapshot_hit 6.24, subtree_snapshot_miss 1.11,
                          video_decode 1.26, text_hit 0.00, text_miss 0.09, img_hit 0.00, img_miss 0.01
```

### 1.2 现状诊断

当前缓存体系散落在 6 个位置，耦合且有覆盖盲区：

| 位置 | 职责 | 问题 |
|---|---|---|
| `backend/skia/cache.rs` | 3 个 `HashMap<u64, Picture>` | 无 LRU、无容量上限 |
| `backend/skia/resources.rs` | 打包 3 个 cache 的容器 | 作为 10 参数通过函数签名散布 |
| `display/cache_key.rs` | paint 指纹 | **transform/opacity 被焊进 hash（见 §1.3-A）** |
| `runtime/policy/snapshot.rs` | 场景快照策略 | **contains_video → 完全放弃场景缓存（§1.3-E）** |
| `runtime/policy/invalidation.rs` | `SceneInvalidation` 状态机 | 场景粒度，无法表达"视频节点外其余可复用" |
| `runtime/profile.rs` + `BackendProfile` | 指标 | **作为 `Option<&mut BackendProfile>` 在 30+ 位置穿针引线；嵌套 span 双计（§1.3-F）** |

### 1.3 根因清单（按严重度排序）

**A. Transform/opacity 污染 paint 指纹（错写）**
`display/cache_key.rs:41-45` 在子树 key 的 hash 中混入了 `translation_x/y`、`opacity`、`transforms`、`backdrop_blur`。脚本驱动的 transform 动画（`badge_scale`、`title_y`、`rotate_deg(orbit*8)` 等）每帧让 key 变化，触发 record miss。对应 profile 中的 `composite_dirty 4.5/frame` 基本都是本该命中但实际 miss 的节点。

**B. Bitmap paint 无 Picture 缓存（漏写）**
`draw_bitmap`（canvas.rs:762-890）每帧重新执行：`draw_rect(background)` → `clip_rrect` → `draw_image_rect` → `draw_inset_shadow` → `draw_rrect(border)`。只有图片解码有 `image_cache`，最终绘制序列无缓存。

**C. DrawScript 无 Picture 缓存（漏写）**
`draw_script_item` 每帧 replay `CanvasCommand` 序列。`CanvasCommand` 已实现 `Hash`，但没有任何地方用来做缓存键。

**D. Lucide 无 Picture 缓存（漏写）**
每帧重绘 lucide 路径。

**E. 场景级对视频零容忍（漏写）**
`runtime/policy/snapshot.rs:143-152` 发现 `contains_video=true` 就 `store_scene_snapshot(slot, None)` 并强制 `record_display_tree_snapshot`。违反了"视频节点以外仍可复用"的事实。这也是 profile 中 `scene_snapshot_hit/miss == 0.00` 的原因：`video_playback` 每个场景都含视频，cache 探针路径从未被执行。

**F. Profile 耦合与双计**
- `Option<&mut BackendProfile>` 在 `draw_*`、`record_*`、`draw_transition` 等函数签名中反复出现
- `record_cached_subtree_snapshot`（canvas.rs:346）和 `record_display_tree_composite_source_with_subtree_cache`（canvas.rs:~520）都往同一个 `scene_snapshot_record_ms` 里累加（命名类别混淆）
- 嵌套 span 直接累加：`scene_snapshot_record_ms` 包含内部 `video_decode_ms`，同一段时间被计入两个桶

**G. Fingerprint 探针时重算（浪费）**
`subtree_snapshot_cache_key_inner` 每次 `draw_display_subtree` 都递归 hash 整棵子树，命中与否都付代价。指纹应该在 DisplayTree 构建阶段一次性落盘到节点。

**H. 缓存无界**
所有 `HashMap<u64, Picture>` 无 LRU，长渲染会持续增长。

---

## 2. 设计原则

1. **关注点正交**：cache / fingerprint / invalidation / compositor / profile 五个维度各自独立模块，彼此通过稳定接口通信。
2. **Paint ⊥ Composite**：缓存键只包含 paint 内容（画什么），不含 transform/opacity（怎么摆）。composite 维度是 draw-time 参数。
3. **细粒度**：判定和缓存粒度落到单个 `DisplayItem` 和 `DisplayNode`，不是整个场景。
4. **指纹构建期一次写入**：`DisplayNode.paint_fingerprint` 在 `build_display_tree` 完成时计算并存储，draw-time 只比对，不重算。
5. **Profile 是观察者**：渲染热路径函数签名中不再有 `&mut BackendProfile`；通过 scoped span + 事件订阅采集指标。
6. **所有 cache 走统一 Registry**：共享 LRU 容量、统一事件流、统一命中率观测。
7. **固有成本（light_leak shader、必要的 video decode）不在本次优化目标内**。优化目标是消除非固有成本。

---

## 3. 架构

### 3.1 新模块布局

```
src/runtime/
├── cache/                    # 缓存存储与淘汰
│   ├── mod.rs                # CacheRegistry facade + 类型定义
│   ├── lru.rs                # 通用 BoundedLruCache<K,V>
│   └── video_frames.rs       # 视频帧缓存（path + pts-quantized key）
├── fingerprint/              # 纯 hashing，无状态
│   ├── mod.rs                # paint_fingerprint + composite_signature
│   └── display_item.rs       # 每种 DisplayItem 的 Hash 实现
├── invalidation/             # 帧间变化分类
│   ├── mod.rs                # PaintVariance + DirtyClassifier
│   └── propagation.rs        # variance 与 dirty 的向上/向下传播
├── compositor/               # 记录 + 合成策略
│   ├── mod.rs                # LayeredScene + compose()
│   ├── layer.rs              # StaticLayer / DynamicLayer
│   └── record.rs             # 记录 Picture（带 item 级 cache 探针）
├── profile/                  # 观察者
│   ├── mod.rs                # span!() 宏、事件枚举、聚合器
│   └── bus.rs                # 事件总线（thread-local + 聚合）
└── policy/
    └── snapshot.rs           # 编排：组装 SceneSnapshotRuntime，不再内嵌策略
```

旧 `runtime/policy/invalidation.rs`、`runtime/policy/cache.rs`、`display/cache_key.rs`、`backend/skia/cache.rs` 的功能被分拆并入上面的新模块；旧文件在迁移完成后删除。

### 3.2 DisplayNode 扩展

```rust
// src/display/tree.rs
pub struct DisplayNode {
    // ... existing fields
    pub paint_fingerprint: Option<u64>,   // None = paint_variance::TimeVariant
    pub paint_variance: PaintVariance,    // Stable | TimeVariant
    pub subtree_contains_time_variant: bool, // 子树是否含 TimeVariant（向上传播标记）
}
```

`build_display_tree`（`display/build.rs`）在构建末尾自底向上填充这三个字段：
- 叶子：按 `DisplayItem` 种类判定 variance（`Bitmap+video` / `DrawScript+asset_is_video` → TimeVariant，其余 Stable）
- 内部节点：`paint_variance = Stable`；`subtree_contains_time_variant = 任一子树为 true`；`paint_fingerprint` 仅当 `subtree_contains_time_variant == false` 时计算，否则 `None`

### 3.3 Fingerprint 分裂

```rust
// src/runtime/fingerprint/mod.rs

/// Paint 维度：缓存键。纯内容，不含 transform/opacity。
/// 对 TimeVariant 子树返回 None。
pub fn paint_fingerprint(node: &DisplayNode) -> Option<u64>;

/// Composite 维度：帧间比对用，不进缓存键。
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompositeSig {
    pub translation: [u32; 2],      // f32 bits
    pub transforms: u64,             // 已有的 hash
    pub opacity: u32,                // f32 bits
    pub backdrop_blur: Option<u32>,  // f32 bits
    pub save_layer_flags: u8,
}
pub fn composite_signature(node: &DisplayNode) -> CompositeSig;
```

原 `display/cache_key.rs` 中的 `F32Hash`、`DisplayItemFingerprint`、`TextSnapshotFingerprint` 等结构迁移到 `runtime/fingerprint/display_item.rs`，仅保留 paint 相关字段。

### 3.4 PaintVariance 与 TimeVariant 判定

```rust
// src/runtime/invalidation/mod.rs
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum PaintVariance {
    Stable,       // paint 内容跨帧不变（除非 paint_fingerprint 变）
    TimeVariant,  // 内容每帧都可能变（video、含 video 的 draw_script）
}

pub fn classify_paint(item: &DisplayItem, assets: &AssetsMap) -> PaintVariance {
    match item {
        DisplayItem::Bitmap(b) if asset_is_video(b, assets) => PaintVariance::TimeVariant,
        DisplayItem::DrawScript(s) if script_references_video(s, assets) => PaintVariance::TimeVariant,
        _ => PaintVariance::Stable,
    }
}
```

旧的 `SceneInvalidation` 枚举被删除。场景级 dirty 不再是枚举态，而是下面 LayeredScene 中每个 layer 独立决定。

### 3.5 CacheRegistry

```rust
// src/runtime/cache/mod.rs

pub struct CacheRegistry {
    paint_pictures: BoundedLruCache<u64, Picture>,    // 子树 paint
    text_pictures: BoundedLruCache<u64, Picture>,     // 文字 paint
    item_pictures: BoundedLruCache<u64, Picture>,     // 单个 DisplayItem 的 paint（Bitmap/DrawScript/Lucide）
    scene_static_pictures: BoundedLruCache<u64, Picture>, // 场景静态层
    decoded_images: BoundedLruCache<String, Image>,   // 解码图像
    video_frames: VideoFrameCache,                    // (path, quantized_pts) → Arc<Vec<u8>>
    events: ProfileEventSink,                         // 命中/miss 上报
}

impl CacheRegistry {
    pub fn paint_picture(&self, fp: u64) -> CacheProbe<Picture>;  // Hit | Miss
    pub fn store_paint_picture(&self, fp: u64, p: Picture);
    // ... 其余类似
}
```

`BoundedLruCache` 使用 `hashlink::LinkedHashMap` 风格实现（手写即可，避免加依赖）。默认容量：
- paint/text/item/scene_static：各 256 条
- decoded_images：128 条
- video_frames：16 条（帧数据大）

容量可由 `RenderSession::with_cache_caps(...)` 覆盖。

### 3.6 LayeredScene：视频场景缓存的核心

```rust
// src/runtime/compositor/layer.rs

pub struct DynamicLayer {
    /// 挂载节点在屏幕坐标系中的变换链
    pub composite: CompositeSig,
    /// 子树根（视频/脚本节点）的 DisplayNode 引用（frame 生命周期内）
    pub node: NodeRef,
    /// 该子树自身的 opacity 叠加
    pub opacity: f32,
    pub clip: Option<DisplayClip>,
}

pub struct LayeredScene {
    /// 场景中所有非 TimeVariant 内容的已录制画面（BackendObject 包裹后端原生 Picture）
    pub static_layer: Option<BackendObject>,
    /// TimeVariant 子树按 DFS 顺序的列表
    pub dynamic: Vec<DynamicLayer>,
    pub bounds: Rect,
}

// 说明：LayeredScene 放在 `runtime/compositor/`，用 `BackendObject` 包裹后端原生
// Picture/Canvas 句柄，保持跨后端中立。实际 draw_picture / PictureRecorder 调用
// 在 skia 后端的 `backend/skia/compositor_impl.rs` 中实现，由 `RenderEngine` trait
// 调用。

impl LayeredScene {
    pub fn compose(
        &self,
        canvas: &Canvas,
        ctx: &mut SceneRenderContext<'_>,
    ) -> Result<()> {
        if let Some(static_pic) = &self.static_layer {
            let _span = profile::span("scene_static_draw");
            canvas.draw_picture(static_pic, None, None);
        }
        for layer in &self.dynamic {
            let _span = profile::span("scene_dynamic_draw");
            layer.draw(canvas, ctx)?;
        }
        Ok(())
    }
}
```

#### 构建流程（`runtime/compositor/record.rs`）

```
record_scene_layered(tree, registry, ctx) -> LayeredScene {
    1. 计算场景 "static skeleton" paint_fingerprint：
       遍历树产出一个 canonical hash：对每个节点，如果
       subtree_contains_time_variant 则把该子树压缩为哨兵形式
       hash(TIMEVARIANT_SENTINEL, bounds.size, clip, CompositeSig)
       不参与 paint 内容 hash；其他节点正常聚合 paint_fingerprint。
       —— 这样 skeleton_fp 仅在"静态骨架"变化时变，视频子树的帧变化
       不会污染 skeleton_fp。
    2. registry.scene_static_picture(skeleton_fp)：
       hit → 直接拿 BackendObject；
       miss → 用后端 PictureRecorder 绘制：遍历树，遇到 TimeVariant
               节点直接 skip（不画，保持原有 save_layer/clip 结构以
               免 bbox 漂移），其余子树/item 走 draw_display_item_cached
               / draw_display_subtree_cached 逻辑（内部再探针
               item_pictures / paint_pictures / text_pictures）。
               完成后存入 scene_static_pictures。
    3. 再次遍历树，按 DFS 收集 paint_variance=TimeVariant 的节点
       → Vec<DynamicLayer>，每个携带当前帧的 CompositeSig 与屏幕
       坐标系下的累计 transform（由调用方前序累积）。
    4. 返回 LayeredScene { static_layer, dynamic, bounds }。
}
```

记录 static 时，**递归 draw 的内部仍然走 CacheRegistry**：遇到带 paint_fingerprint 的 DisplayItem/子树先探针 `item_pictures`/`paint_pictures`/`text_pictures`，miss 才真正画。这样静态骨架内部的每块画面也独立缓存，首次录制后频繁复用。

#### 合成流程

`LayeredScene::compose` 先画 static Picture，再按原 DFS 顺序对每个 DynamicLayer：
- 取当前帧 CompositeSig（来自节点的 transform/opacity）
- `canvas.save(); canvas.transform(...); canvas.set_alpha(...);`
- 对 dynamic 子树执行 draw（会触发 video decode、script 重演）
- `canvas.restore();`

DynamicLayer 不在帧间缓存（因为 paint_variance=TimeVariant）。但若同一帧内同一 video 在两处引用（例：transition 的 from/to 都含同一 path），`video_frames` cache 负责避免重复解码。

### 3.7 Per-DisplayItem Picture 缓存（覆盖审计 §1.3-B/C/D）

所有 DisplayItem 走统一路径：

```rust
// src/runtime/compositor/record.rs
fn draw_display_item_cached(
    canvas: &Canvas,
    item: &DisplayItem,
    bounds: Rect,
    registry: &CacheRegistry,
    ctx: &mut SceneRenderContext<'_>,
) -> Result<()> {
    let fp = item_paint_fingerprint(item, bounds);
    if let Some(fp) = fp {
        match registry.item_picture(fp) {
            CacheProbe::Hit(pic) => {
                let _s = profile::span("item_picture_draw");
                canvas.draw_picture(&pic, None, None);
                return Ok(());
            }
            CacheProbe::Miss => {
                let _s = profile::span("item_picture_record");
                let pic = record_item_picture(item, bounds, ctx)?;
                registry.store_item_picture(fp, pic.clone());
                canvas.draw_picture(&pic, None, None);
                return Ok(());
            }
        }
    }
    // TimeVariant 路径：直接画
    draw_display_item_direct(canvas, item, bounds, ctx)
}
```

`item_paint_fingerprint`：
- `Text` → 走已有 `text_snapshot_cache_key` 迁移版本
- `Bitmap(non-video)` → `hash(asset_id, paint, bounds.size, object_fit)`
- `Bitmap(video)` → None
- `DrawScript(no-video)` → `hash(commands, bounds.size, asset_refs)`，其中 `asset_refs` 是脚本内部 `CanvasCommand::DrawImage { asset_id, .. }` 出现的全部非视频 asset_id 的稳定排序串联
- `DrawScript(video)` → None
- `Lucide` → `hash(icon, paint, bounds.size)`
- `Rect` → None（draw_rect 单次开销 <50μs，缓存开销更大；保留直绘）

### 3.8 Profile 重构为观察者

```rust
// src/runtime/profile/mod.rs

/// Drop-guard 风格的 span，作用域结束自动上报耗时。
pub fn span(name: &'static str) -> SpanGuard;

/// 计数事件（命中、miss、解码次数等）。
pub fn count(event: &'static str, delta: usize);

pub struct SpanGuard {
    name: &'static str,
    started: Instant,
}

impl Drop for SpanGuard {
    fn drop(&mut self) {
        ProfileBus::current().emit(ProfileEvent::Span {
            name: self.name,
            ms: self.started.elapsed().as_secs_f64() * 1000.0,
        });
    }
}
```

`ProfileBus` 是 thread-local 的事件汇聚器；`RenderSession` 在每帧开始 `enter_frame()`，结束 `exit_frame()` 把该帧的聚合结果 push 给 `RenderProfiler`。

**嵌套语义明确化**：每个 span 记录 `inclusive_ms`；同时按 parent-child 关系计算 `exclusive_ms`（从父 inclusive 扣除子 inclusive 之和）。打印时分两栏输出，消除之前 scene_snapshot_record 和 video_decode 双计的歧义。

**计数类别修正**：
- `scene_static_record` / `scene_static_draw`（新）
- `scene_dynamic_record` / `scene_dynamic_draw`（新）
- `subtree_picture_record` / `subtree_picture_draw`（取代旧混淆的 scene_snapshot_record）
- `item_picture_record` / `item_picture_draw`（新）
- `video_decode`（保留）
- `text_record` / `text_draw`（保留）
- `light_leak_mask` / `light_leak_composite`（保留）

### 3.9 视频帧缓存（cache/video_frames.rs）

key：`(PathBuf, quantized_pts_micros: u64)` —— 量化单位取 100μs（等价于 1/10000 秒），足以消除浮点误差且对常见 fps 无损（30fps 帧间距 33333μs，60fps 16666μs，远大于量化粒度）。具体：`(resolve_time_secs * 10_000.0).round() as u64`。
value：`Arc<Vec<u8>>`（RGBA，与 VideoDecoder 保持一致）。
容量：16 条 × 约 3.5MB/条（1280×720×4）≈ 56MB 上限，可调。

`MediaContext::get_bitmap`（`resource/media.rs`）先查缓存，miss 才走 `VideoDecodeCache.get_frame`。这是唯一跟视频解码路径的 surgical 变动，本次不做更激进的预取/流水线。

---

## 4. 数据流

```
composition.root_node(ctx)
    │
    ▼
resolve_ui_tree (elements) ───────┐
    │                             │
    ▼                             │
compute_layout ───────────────────┤  这一段保持不变
    │                             │
    ▼                             │
build_display_tree ───────────────┘
    │
    │ 【新】build 末尾：自底向上填
    │      paint_variance / subtree_contains_time_variant / paint_fingerprint
    ▼
DisplayTree (annotated)
    │
    ▼
record_scene_layered(tree, registry, ctx)
    ├─ 计算 static skeleton paint_fingerprint
    ├─ probe registry.scene_static_picture(fp)
    │    ├─ Hit  → 用缓存
    │    └─ Miss → 用 PictureRecorder 录制
    │              └─ 内部 draw 时对每个子树/item probe paint_pictures/item_pictures
    └─ 收集 DynamicLayer 列表（TimeVariant 子树）
    │
    ▼
LayeredScene { static_layer, dynamic }
    │
    ├─ 非 transition：LayeredScene::compose 直接画到 frame_view
    └─ transition：from_scene.compose() 录到 PictureRecorder → from_picture
                   to_scene.compose() 录到 PictureRecorder → to_picture
                   draw_transition(from_picture, to_picture, progress, kind)
```

transition 路径的关键点：`LayeredScene::compose` 在 PictureRecorder 的 canvas 上执行，**static_layer 直接 draw_picture（零开销引用）**，dynamic_layers 正常录到 recorder。最终得到的 from/to Picture 即可喂给 light_leak/slide shader。

---

## 5. 错误处理

- `paint_fingerprint` 返回 `Option<u64>`：None 表示不应缓存（TimeVariant），永不冒充 hit。
- `CacheProbe<T>`：枚举态 `Hit | Miss`，禁用 "silent miss"（不把 None 当 miss 处理）。
- LRU 淘汰只影响性能，不影响正确性。
- 迁移期旧 `SceneSnapshot = BackendObject`（`runtime/render_engine.rs:20`）被 `LayeredScene` 替换，所有消费点同步更新；不保留 backward-compat 桥。

---

## 6. 测试策略

### 6.1 单元测试
- `fingerprint/display_item.rs` tests：
  - transform/opacity/translation 变化时 `paint_fingerprint` 不变
  - DisplayItem 内容变化时 `paint_fingerprint` 变
  - TimeVariant 子树返回 None
- `invalidation/mod.rs` tests：
  - `classify_paint` 各 DisplayItem 种类正确标注
  - 含 video 的 DrawScript 被识别
  - `subtree_contains_time_variant` 向上传播
- `cache/lru.rs` tests：LRU 淘汰顺序、容量封顶

### 6.2 集成测试
- `video_playback` 渲染结果做 "同一 seed 两次渲染应逐像素一致" 的回归基线（本次重构不改变画面）
- 记录基准 profile 作为 golden，CI 比对关键指标回归

### 6.3 性能基准
在 `examples/` 下保留 `video_playback` 作为 bench。重构完成后目标：
- `backend` avg：155ms → ≤ 80ms（对半）
- `composite_dirty` 节点不再导致 cache miss（通过新增的 `composite_reused_nodes` 计数验证）
- 每个渲染热路径函数签名不再包含 `Option<&mut BackendProfile>`
- 各 cache 的 hits/misses 非零且命中率 > 70%（在 video_playback 场景下）

---

## 7. 迁移顺序

每步独立可测，可分别 review/merge：

1. **提取 `fingerprint/` 模块**：从 `display/cache_key.rs` 迁移，拆出 `paint_fingerprint`（剥离 transform）与 `composite_signature`。先不改调用方，保留旧函数别名作为阴影对照。写 §6.1 fingerprint tests。
2. **DisplayNode 字段扩展 + build 阶段填充**：在 `display/build.rs` 末尾自底向上写 `paint_variance` / `paint_fingerprint` / `subtree_contains_time_variant`。`draw_display_subtree` 改为读字段而非重算。
3. **CacheRegistry + BoundedLruCache**：新建 `runtime/cache/`，把 `backend/skia/cache.rs` 的 3 个 HashMap 以 LRU 包装统一放入 registry；`SkiaBackendResources` 改为持有 `CacheRegistry`。
4. **Per-item Picture 缓存**：为 Bitmap/DrawScript/Lucide 实现 `item_paint_fingerprint` + `item_pictures` 走统一 `draw_display_item_cached`。
5. **Profile 事件化**：引入 `runtime/profile/`，把 `BackendProfile` 所有累加点替换为 `profile::span!()` / `profile::count()`；从所有渲染函数签名中移除 `Option<&mut BackendProfile>`。`RenderProfiler::print_summary` 改为消费 `ProfileBus` 聚合结果。
6. **LayeredScene + record_scene_layered**：新 `runtime/compositor/`，替换 `record_display_tree_composite_source_with_subtree_cache` 与 `record_display_list_composite_source`。`runtime/policy/snapshot.rs` 重写为薄编排层。
7. **删除 `SceneInvalidation` 与 `runtime/policy/invalidation.rs`**：过渡字段最终移除。
8. **视频帧缓存**：`cache/video_frames.rs` 接入 `MediaContext::get_bitmap`。

---

## 8. 非目标（YAGNI）

- 保留模式跨帧 DisplayTree 身份（Approach 3）：未来话题，本次不做
- GPU picture cache：交给 skia 自身
- 视频解码预取/并发流水线：独立优化，可与本次重构正交推进
- Audio cache 重构
- Window/实时播放器路径的改动（本次只针对离线渲染路径）
- 跨 `RenderSession` 持久化缓存

---

## 9. 成功判据

1. `cargo run --example video_playback` 输出像素级不变（golden diff = 0）
2. `backend` avg ms/frame 减少 ≥ 40%（155 → ≤ 93）
3. Profile 区分 inclusive/exclusive；`scene_snapshot_record` 条目消失（被拆成 static/dynamic/subtree/item 四类）
4. `composite_dirty` 节点的 cache 命中率 ≥ 80%
5. 渲染热路径（`draw_*`、`record_*`、`compose_*`）函数签名不含 `Option<&mut BackendProfile>`
6. 所有缓存内存使用有上限（LRU 生效）
7. 单元测试覆盖：transform 变化不破坏 paint_fingerprint、TimeVariant 传播、LRU 淘汰

---

## 10. 关键文件清单

### 新增
- `src/runtime/cache/mod.rs`
- `src/runtime/cache/lru.rs`
- `src/runtime/cache/video_frames.rs`
- `src/runtime/fingerprint/mod.rs`
- `src/runtime/fingerprint/display_item.rs`
- `src/runtime/invalidation/mod.rs`
- `src/runtime/invalidation/propagation.rs`
- `src/runtime/compositor/mod.rs`
- `src/runtime/compositor/layer.rs`
- `src/runtime/compositor/record.rs`
- `src/runtime/profile/mod.rs`
- `src/runtime/profile/bus.rs`

### 重写
- `src/runtime/policy/snapshot.rs`（改为薄编排层）
- `src/backend/skia/canvas.rs`（draw_* 走 CacheRegistry；移除 BackendProfile 参数；subtree 递归读 DisplayNode.paint_fingerprint）
- `src/backend/skia/renderer.rs`（RenderEngine trait 方法签名去 BackendProfile；SceneSnapshot → LayeredScene）
- `src/runtime/pipeline.rs`（使用 LayeredScene；profile 改订阅 bus）
- `src/runtime/profile.rs`（变成 RenderProfiler 聚合器，不再持有 BackendProfile）
- `src/display/build.rs`（末尾填充新字段）
- `src/display/tree.rs`（DisplayNode 新字段）

### 删除
- `src/display/cache_key.rs`（内容迁移到 runtime/fingerprint/）
- `src/runtime/policy/invalidation.rs`（SceneInvalidation 枚举退役）
- `src/runtime/policy/cache.rs`（SceneSnapshotCache 并入 CacheRegistry）
- `src/backend/skia/cache.rs`（三个 HashMap 并入 CacheRegistry）
- `src/backend/skia/resources.rs`（替换为 CacheRegistry）
- `src/backend/resource_cache.rs`（同上）
