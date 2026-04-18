# 复用架构演进路线图

日期:2026-04-19
状态:规划中
适用范围:opencat 渲染引擎的跨帧 / 帧内复用体系
作者:Solaren + Claude

---

## 1. 背景与问题陈述

当前系统已经完成了一次关键纠偏:抛弃 `LayeredScene`、回到 `OrderedSceneProgram`,重新确立"原始树顺序是唯一真相、复用必须发生在原位置"的执行模型。这一步的收益是正确性(不再出现 `A + C + B` 的 paint order 错乱)和架构干净度,不是原始性能。

但最近一次 profile(`rtk cargo run --bin parse_json -- json/opencat-project-showcase-landscape.jsonl`)显示:

```
frames: 480
backend avg ms/frame:        98.45
draw_script:                 96.40    <-- 总时间的 ~98%
item_picture_record:         79.27
item_picture_draw:            1.37
subtree_snapshot_record:      0.02
subtree_snapshot_draw:        0.56
display_tree_direct_draw:    82.66 incl / 82.09 excl
```

**结论:几乎所有 backend 时间都花在 `DrawScript` 的 live record/draw 上,而 `DrawScript` 当前被一刀切为 `TimeVariant`,完全不进跨帧缓存。**

也就是说,现在整套多层缓存架构处理得最干净的部分(subtree snapshot / item picture / text snapshot)合计只占不到 1ms,真正吃 CPU 的那条路径还完全没有被缓存体系触及。

---

## 2. 第一性原理

本路线图所有决策都基于以下五条原则,之后出现的任何 task 如果和这些原则冲突,需要回头重新审视原则本身。

### 2.1 节点属性必须严格拆成三个维度

| 维度 | 包含 | 对复用的影响 |
|------|------|---------------|
| **paint** | content / asset / color / radius / border / stroke / text content / text style | 决定 picture 能否跨帧复用 |
| **composite** | transform(translation + scale + rotate)/ opacity / backdrop-blur | **仅影响本帧怎么摆**,不影响 picture 复用 |
| **structural** | children 集合 / clip | 影响 subtree snapshot 是否失效 |

**不变量(强制,违反即 bug):**

- cache key 只能来自 `paint` + `structural`,严禁掺入 `composite`
- "静态内容 + 动画 transform / opacity" 必须能无损命中缓存

现有 `fingerprint/mod.rs` 已经遵守这条原则(`snapshot_fingerprint_ignores_current_node_transform` 测试佐证),本路线图所有新 cache 必须继续遵守。

### 2.2 素材的 paint variance 由源头决定

| 素材类型 | 默认 variance | 说明 |
|----------|---------------|------|
| Image(静态图片) | Stable | 资源生命周期内不变 |
| Lucide SVG | Stable(当前);如未来支持 stroke/color 动画则需下沉判定 |
| Text | 取决于 `(content, style)` | 仅内容 + 样式稳定即 Stable |
| Video | 默认 TimeVariant;**暂停段应被识别为 Stable**(见 P1-2) |
| DrawScript(canvas 脚本) | 默认 TimeVariant;**命令 + 入参稳定时应升级为 Stable**(见 P1-1) |
| NodeStyle 属性脚本 | 脚本本身不独立 fingerprint,**只看最终落到节点上的 paint 属性** |

### 2.3 复用存在两条独立时间轴

| 时间轴 | 缓存 key | 生命周期 | 典型用途 |
|--------|----------|-----------|-----------|
| 跨帧 | paint fingerprint | 多帧 | stable subtree / stable item |
| 帧内 | `(frame_id, node_handle)` | 单帧 | drop shadow 二次 replay / 同素材多实例共享 |

视频与 DrawScript 是典型的"跨帧不复用,但帧内仍可复用"场景。当前 `FrameLocalPicture`(`reuse.rs:37-48`)是这条时间轴的雏形,需要继续扩张。

### 2.4 缓存本体有三档物化形式

1. **DirectDraw(无缓存)**:节点足够便宜,`record + replay` 反而比直接重画慢
2. **Picture(命令回放)**:小、可缩放重 raster、record 便宜、replay 要重走命令
3. **Image / GPU texture(光栅化结果)**:大、固定分辨率、blit 最快、不能缩放

物化形式必须按成本模型选择,而不是一刀切。详见 §3.4 决策框架。

### 2.5 复用粒度与 bounds 面积有关

纯粹的"父优先"(只要父稳定就存整棵 snapshot)是工程近似,不是第一性原则。当父 bounds 覆盖全屏、子 bounds 只占 1% 时,存整棵父 snapshot 是浪费——小子节点改变时整张大 picture 都失效。粒度选择必须进入决策模型(见 P1-4)。

---

## 3. 目标复用模型

### 3.1 节点稳定性判定

```
subtree_stable(node) ≡
    paint_stable(node)
  ∧ structural_stable(node)
  ∧ ∀ c ∈ children(node). subtree_stable(c)
```

当前 `subtree_contains_time_variant` 的反面定义已经实现了 paint 部分,本路线图需要补上 `structural_stable`(children 集合 / clip 稳定)作为显式一等字段。

### 3.2 反例兜底

若 `cost_record(node) + cost_replay(node) > cost_direct_draw(node)`,则即便节点 stable 也走 `DirectDraw`。

现状(`reuse.rs:28-29`)对 solid Rect 已经走 DirectLeaf,这是一个硬编码近似。本路线图将其扩展成显式成本函数:

```
if approx_cost_direct(item) < PICTURE_RECORD_BASELINE:
    DirectLeaf
else:
    ItemPictureLeaf  // 进入 ItemPictureCache
```

`PICTURE_RECORD_BASELINE` 初期用经验常数(建议 ~50µs),后续可改为滑动窗口实测。

### 3.3 粒度选择(bounds-aware)

```
ratio = parent.bounds.area / max(child.bounds.area)
if ratio > GRANULARITY_THRESHOLD:    // 建议初值 16
    // 父子面积悬殊,整棵存是浪费
    prefer child-level caching
    parent -> LiveSubtree
else:
    prefer subtree snapshot
    parent -> CachedSubtree
```

### 3.4 物化形式决策框架

|  条件 | 建议物化 |
|-------|----------|
| item 为 solid Rect / 空 item | **DirectDraw** |
| 首次 miss,不确定复用价值 | **Picture** |
| 同一 key 在滑动窗口内命中 ≥ N 次且 bounds 不变 | 升级为 **Image** |
| 节点处于 scale 动画链路中 | **永远 Picture**(Image 会走样) |
| 节点 bounds 非常大且子树复杂 | 倾向 **Image**(blit 摊销收益最高) |
| 整帧内存预算紧 | 倾向 **Picture** |

### 3.5 间歇性静态感知

`Video` 和 `DrawScript` 不再一刀切 TimeVariant:

- `Video`:paint fingerprint = `hash(asset_id, quantize_pts(time_secs))`
  - 暂停段 `pts` 稳定 → fingerprint 稳定 → ItemPictureCache 命中
- `DrawScript`:paint fingerprint = `hash(commands, 非时变入参)`
  - 命令稳定且不依赖 `frame_ctx.time` → Stable → ItemPictureCache 命中

这把现在被完全浪费的暂停 / 稳态时间窗全部回收。

---

## 4. Task 清单

按 P0(正确性)→ P1(性能 ROI)→ P2(工程整洁度)分三档。每个 task 给出:目标、实现要点、验收、参考文件。

### 4.1 P0 — 正确性

#### Task P0-1:snapshot fingerprint 命中路径加碰撞二次验证

状态:`[x] 已完成`

目标:
- 当前 `SubtreeSnapshotCache` 命中即用,64-bit hash 一旦碰撞就画错画面
- 在 cache value 旁冗余一个二次 fingerprint(不同 hasher),命中后做轻量比对
- miss 降级为重录,不影响正确性

实现要点:
- cache 值类型:`(Picture, secondary_fp: u64, recorded_bounds: DisplayRect)`
- secondary hasher 用 AHash 或 FxHash,和主 hasher(SipHash / 未来的 FxHash)独立

验收:
- 新测试:人工构造两棵 paint 不同但 primary hash 相同的子树,验证命中被拒绝并触发 re-record
- 运行时新增指标 `SubtreeSnapshotCollisionRejected`

参考文件:
- `src/backend/skia/canvas.rs:194-208`
- `src/runtime/fingerprint/mod.rs:180-184`

#### Task P0-2:`ItemLeaf` 命名与缓存路径对齐

状态:`[x] 已完成`

目标:
- 当前 `reuse.rs:30` 将 Text/Bitmap/Lucide 一律归为 `ItemLeaf`,但 `should_cache_item_picture`(`canvas.rs:895-900`)**不含 Text**
- Text 实际走 `TextSnapshotCache` 另一条路径,语义糊在一起会误导后续 task

落地选项(选其一):
1. 拆分枚举:`ItemLeaf` → `ItemPictureLeaf` + `TextSnapshotLeaf`
2. 合并缓存:`TextSnapshotCache` 并入 `ItemPictureCache`,统一按 paint fingerprint 索引

验收:
- 代码中 `ItemLeaf` 的每个变体都能直接对应到一个明确的 cache 桶
- 文档与代码同步更新

参考文件:
- `src/runtime/compositor/reuse.rs:19-35`
- `src/backend/skia/canvas.rs:895-900`

---

### 4.2 P1 — 性能 ROI

#### Task P1-1:`DrawScript` 命令 fingerprint 与 Stable 升级

状态:`[ ]` 未开始

**这是整份路线图预期收益最大的 task**(当前 `DrawScript` 占 backend 95% 时间)。

**关键澄清:不是给脚本代码做 fingerprint,是给脚本执行产出的 commands 序列做 fingerprint。** 脚本代码等价性是 halting problem 的变体,不可判定;但 `Vec<Command>` 是纯数据,hash 后比较完全可行。

### 落地分层(从保守到激进)

**层次 A(本轮实现):输出 fingerprint**

- 脚本照常每帧执行,产出 `Vec<Command>`
- 对 `(commands, drop_shadow, bounds)` 做 hash,作为 DrawScript 的 paint fingerprint
- fingerprint 稳定 → ItemPictureCache 命中 → 跳过 `PictureRecorder` 重录
- **省的是什么**:Skia record 的 ~79ms(当前 profile 最大头)
- **不省的是什么**:脚本执行本身(剩余 ~17ms)

**层次 B(未来,不在本轮):输入 fingerprint**

- 在脚本宿主 runtime 中追踪"本帧脚本读取了哪些 getter"(时间 / 属性 / asset)
- 若输入全部稳定 → 脚本根本不用跑,直接复用上帧 picture
- 工程成本大(要改 runtime);收益是再抢 ~17ms

**层次 C(未来,需内容协作):脚本作者声明**

- 脚本通过 pragma / 注解自声明 `@static`
- runtime 直接跳过执行,走缓存
- 最快,但要求内容侧配合

### 本轮实现要点(层次 A)

- `fingerprint/display_item.rs` 里 `item_is_time_variant` 对 `DrawScript` 不再无条件返回 true,改为基于命令序列判断
- 若命令中引用 `frame_ctx.time_secs` / `frame_ctx.frame_index` 等时变源,或调用了时变 API,则保持 TimeVariant
- 否则 `item_paint_fingerprint` 对 `DrawScript` 产出稳定 fingerprint,进入 `ItemPictureCache`

验收:
- 单元测试:纯静态命令的 DrawScript 被标 Stable,带 `frame_ctx.time` 引用的保持 TimeVariant
- profile:`parse_json` benchmark 的 `backend avg ms/frame` 显著下降;`ItemPictureCacheHit` 随帧数增长
- 目标区间:`backend avg ms/frame` 从 ~98ms 降至 25~40ms

参考文件:
- `src/runtime/fingerprint/mod.rs:80-101`
- `src/runtime/fingerprint/display_item.rs`
- `src/runtime/compositor/reuse.rs:37-48`

#### Task P1-2:Video 暂停段自动 Stable

状态:`[ ]` 未开始

**关键概念:这不是"视频变成 Stable",而是"视频在同一个量化时间点内是 Stable"。** fingerprint 仍然会随 pts 变化,只是变化频率由量化精度决定,不再是"每帧必变"。

目标:
- `Bitmap(video)` 按 `(asset_id, quantize_pts(time_secs))` 算 paint fingerprint
- 相邻帧 pts 相同 → fingerprint 相同 → ItemPictureCache 命中
- 让 ItemPictureCache(上层)与 VideoFrameCache(下层)的"一帧"定义**严格对齐**,消除"解码命中但 picture 不命中"的错位状态

实现要点:
- 复用 `cache/video_frames.rs:43-45` 的 `quantize_pts`(精度 1/10000 秒)
- pts 的量化精度**必须**与 `VideoFrameCache` 一致,避免上下两层错位
- `item_is_time_variant` 对 `Bitmap(video)` 仍返回 true(表示"跨帧整体 TimeVariant"),但 `item_paint_fingerprint` 改为返回 `Some(hash(asset_id, quantize_pts, paint_style))`,允许"同一量化时间点内复用"

验收:
- 暂停场景 `ItemPictureCacheHit` 明显增长
- 正常播放 fingerprint 每帧变化,不会误命中
- `VideoFrameCacheHit` 与 `ItemPictureCacheHit` 同步增长,无错位

参考文件:
- `src/runtime/fingerprint/display_item.rs`
- `src/runtime/cache/video_frames.rs`

#### Task P1-3:物化两段式升级(Picture → Image)

状态:`[ ]` 未开始

**依赖关系:必须放在 P1-1 / P1-2 之后。** 当前 profile 里 `subtree_snapshot_draw` 只占 0.56ms,看起来"升级 Image 没必要"——这是因为 DrawScript / Video 污染导致几乎没有大子树真正稳定命中。一旦 P1-1 把 DrawScript 升级 Stable,命中率飙升,Picture replay 的成本才会涨到值得优化的量级。**两段式升级是为 P1-1 之后的"高命中率世界"准备的,不是为现状准备的。**

### 为什么叫"两段式"

单档策略的问题:

- **全用 Picture**(现状):每次命中都要 replay 命令序列,产生多条 GPU draw call
- **全用 Image**:每个 miss 都要 raster + 上 GPU,短命 key 白付成本;缩放动画会走样

两段式的逻辑:

```
Frame N:    miss → record Picture → 命中计数 = 0
Frame N+1:  hit  → draw Picture,命中计数 = 1
Frame N+2:  hit  → draw Picture,命中计数 = 2
Frame N+3:  hit  → 命中计数 ≥ 阈值(如 3)且 bounds 稳定
                → 顺手 raster 为 SkImage,存入 SubtreeImageCache
Frame N+4+: hit  → 优先查 Image,一条 draw call blit 到屏幕
```

先用便宜的 Picture "试水"确定 key 是热的,再花一次性成本升级为 Image 榨取 GPU blit。

### 实现

目标:
- `SubtreeSnapshotCache` value 加命中计数
- 连续命中 ≥ N 次且 bounds 稳定 → 光栅化为 `SkImage` 存入新的 `SubtreeImageCache`
- 后续优先走 Image,GPU blit 代替 Picture replay

约束(不可违反):
- **绝不**对"祖先含 scale 动画"的节点升级为 Image(会走样)
- Image 缓存独立容量封顶,防止 GPU 内存爆炸
- 降级路径:bounds 变化 / 命中衰减后自动回退到 Picture

验收:
- 测试:同 key 命中 N 次后 `SubtreeImageCache` 被填充;引入 scale 动画后升级被拒绝
- profile(在 P1-1 / P1-2 落地后):高频重复场景的 `subtree_snapshot_draw` 进一步下降

参考文件:
- `src/runtime/cache/mod.rs`
- `src/backend/skia/canvas.rs:194-556`

#### Task P1-4:bounds-aware 粒度选择

状态:`[ ]` 未开始

目标:
- `OrderedSceneProgram::build` 遇到 `SubtreeSnapshot` 时检查 `parent.bounds.area / max(child.bounds.area)`
- 比例超过阈值 → 退化为 `LiveSubtree`,让 children 自行决定

实现要点:
- `AnnotatedDisplayTree.layer_bounds` 已经提供了每节点 bounds
- 阈值初期硬编码(建议 16),后续可随 profile 调优

验收:
- 构造"全屏 container + 单个小图标稳定子"场景,container 不再被标 CachedSubtree
- 整体内存占用下降且命中率不显著恶化

参考文件:
- `src/runtime/compositor/ordered_scene.rs:34-59`

---

### 4.3 P2 — 工程整洁度与观测

#### Task P2-1:`CompositeSig` 闭环接入 ordered scene

状态:`[ ]` 未开始

目标:
- 当前 `composite_dirty` 被计算但 executor 几乎不用
- `LiveSubtree` 执行路径:若 `!composite_dirty` 且 cache hit,跳过 `save/translate/restore`,复用上一帧 layer 状态

参考文件:
- `src/runtime/invalidation/propagation.rs`
- `src/backend/skia/canvas.rs:250-297`

#### Task P2-2:删除退化的 `SceneRenderStrategy` enum

状态:`[ ]` 未开始

目标:
- `compositor/plan.rs:3-6` 的 enum 只剩单一值,属于过度设计残留
- 替换为 `SceneRenderPlan { allows_scene_snapshot_cache: bool }`

参考文件:
- `src/runtime/compositor/plan.rs`

#### Task P2-3:统一子树录制路径走 ordered scene

状态:`[ ]` 未开始

目标:
- `record_cached_subtree_snapshot`(`canvas.rs:527-528`)当前递归走旧 `draw_display_children`
- 改为 `OrderedSceneProgram::build_subtree(handle)` 递归,与主路径单一来源

参考文件:
- `src/backend/skia/canvas.rs:505-535`

#### Task P2-4:cost-aware LRU

状态:`[ ]` 未开始

目标:
- `BoundedLruCache` 增加 `weight` 字段(建议用 `bounds.area × approximate_op_count`)
- 淘汰评分 = `recency / weight`,避免小 item 把大 subtree 挤出

参考文件:
- `src/runtime/cache/lru.rs`

#### Task P2-5:cache 压力指标

状态:`[ ]` 未开始

目标:
- `BackendCountMetric` 新增:
  - `*_CacheEvict`
  - `*_CacheRecordRepeat`(同 key 多次 record,说明 thrashing)
  - `*_CacheCapacityUtilization`
- `parse_json` 结束打印 pressure report,长片场景可看出缓存策略是否失衡

参考文件:
- `src/runtime/profile/*`

#### Task P2-6:fingerprint hasher 切换到 FxHash

状态:`[ ]` 未开始

**前置依赖(强制):P0-1 必须先落地。** 没有二次验证直接换 FxHash 等于自己埋碰撞 bug——FxHash 设计目标是"对非恶意输入快",碰撞特性比 SipHash 差。

目标:
- `DefaultHasher`(SipHash-1-3)对渲染管线过于保守
- 切换到 FxHash(短输入快 3~5×)作为主 hasher
- secondary hasher 保持 AHash(由 P0-1 引入),形成"两个独立快 hasher 兜底"组合
- 碰撞概率 ≈ 单 hasher 碰撞概率的平方,在 256 项缓存规模下接近 0

参考文件:
- `src/runtime/fingerprint/mod.rs:180-184`
- `src/runtime/fingerprint/mod.rs:106-152`(两个 subtree fingerprint 函数)

---

## 5. 非目标(本轮不做)

刻意推迟,避免范围失控:

1. 多 surface / 多 DPR / 多输出目标
2. GPU 自定义 composite pipeline / 自实现 backdrop filter
3. 视频预取 / 硬件解码 / GPU video path
4. 文本样式层的增量渲染(font fallback cache / glyph atlas 复用)
5. 非 OrderedScene 的其他场景执行模式(retained mode 直绘等)
6. 跨进程 / 跨会话缓存持久化

---

## 6. 推荐执行顺序

强烈建议**按 Round 划分,每 Round 结束跑一次完整 profile 并回写文档**。

| Round | 范围 | 预期收益 |
|-------|------|----------|
| 1(正确性兜底) | P0-1、P0-2 | 杜绝碰撞 bug;消除命名 / 路径不一致 |
| 2(最大性能 ROI) | P1-1、P1-2 | `backend avg ms/frame` 从 ~98ms → 25~40ms |
| 3(架构升级) | P1-3、P1-4 | 进一步降到 10~20ms;内存占用下降 |
| 4(整洁度 + 观测) | P2-* 全部(可并行) | 为后续更细粒度优化铺平观测路径 |

Round 2 的预期区间是根据 `DrawScript` 占 backend 95%、升级 Stable 后命中率 70~90% 线性推演得出的保守估计。

---

## 7. 成功标准

整个路线图完成后,必须同时满足:

1. **概念层面**:paint / composite / structural 三维在代码和文档中都是一等概念,任何新增缓存都能清晰归属某一维度
2. **时间轴**:帧内 / 跨帧两条时间轴独立运行,缓存桶职责清晰,不重叠不遗漏
3. **素材感知**:DrawScript / Video 能感知间歇性静态,暂停 / 稳态时间窗自动进入缓存
4. **物化策略**:支持 DirectDraw / Picture / Image 三档,按成本函数 + 命中率自动升级
5. **粒度策略**:不再机械地父优先,bounds-aware 粒度可量化、可 profile
6. **正确性兜底**:cache 命中有二次验证,杜绝 64-bit hash 碰撞导致的画面错乱
7. **可观测性**:cache pressure 指标成为 profile 一等公民,长期 regression 可追踪
8. **性能基线**:`parse_json` showcase benchmark 的 `backend avg ms/frame` 稳定在 20ms 以下

---

## 8. 进度日志

### 2026-04-19

- 旧 spec 目录(`docs/superpowers/specs/` 下两份文档)已废弃并删除
- 本路线图落地五条核心原则:三维分解 / 两条时间轴 / 物化三档 / bounds-aware 粒度 / 间歇性静态
- Task 清单按 P0 / P1 / P2 排期完成,合计 12 个 task
- 沉淀四处澄清:P1-1 拆分层次 A/B/C(本轮只做 A);P1-2 "同一量化时间点内 Stable"的本质;P1-3 为 P1-1 之后高命中率世界准备的说明;P2-6 FxHash 与 AHash 双 hasher 搭配
- 下一步:进入 Round 1,plan 见 `2026-04-19-round-1-correctness.md`

### 2026-04-19 (Round 1 landing)

- P0-1 完成:`SubtreeSnapshotCache` value 升级为 `CachedSubtreeSnapshot { picture, secondary_fingerprint }`;命中路径通过 `resolve_subtree_snapshot_lookup` 纯函数做 secondary 比对;新增 `SubtreeSnapshotCollisionRejected` 指标
- P0-2 完成:`StableNodeReuse::ItemLeaf` 已拆分为 `ItemPictureLeaf` 与 `TextSnapshotLeaf`,与 `ItemPictureCache` / `TextSnapshotCache` 路径一一对应
- 下一步:Round 2 做 P1-1(DrawScript 层次 A 输出 fingerprint)+ P1-2(Video 暂停段 Stable)
