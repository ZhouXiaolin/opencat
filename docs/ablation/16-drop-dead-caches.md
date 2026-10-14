# 消融 16 · 删除死缓存（`feat/drop-dead-caches`）

## 决策

前 15 轮消融给出判定后，删除**零收益且已证透明**的 6 条缓存。判据是各自记录里
"关掉后相位 Δ 在噪声内 + 像素逐位相同"的实测结论：

| 缓存 | 关闭点（原） | 判据 |
|------|--------------|------|
| `03 analyze_history` | 指纹历史复用 | [03](03-analyze_history.md)：相位 Δ ≤ 0.1ms/frame，噪声内 |
| `04 composite_history` | 变更检测历史 | [04](04-composite_history.md)：`apply_segment_*` 与相位全不变 |
| `05 node_own` | 节点自身段复用 | [05](05-node_own.md)：命中 12.45 次/帧只值 0.02ms/frame |
| `06 apply_segments` | apply 前缀段重放 | [06](06-apply_segments.md)：命中 82% 只值 0.05ms/frame |
| `07 item_pictures` | item 绘制图缓存 | [07](07-item_pictures.md)：收益 0，且 43 帧劣化（非透明） |
| `08 scene_snapshot` | 整帧 DrawOp 复用 | [08](08-scene_snapshot.md)：本片结构上永不命中 |

保留 `01 layout_merkle`、`02 display_merkle`、`09 font_system`、`10 swash`、
`11 video_frames`、`13 glyph_rasters`、`14 generated_images`。`12 runtime_effects`、
`15 liangyi_compile` 本片测不到信号，按"测不到不许删"保留。

## 代码改动

- 删除文件：`analyze/invalidation.rs`（含 `DisplayInvalidationTable`）、`ir/cache.rs`。
- `analyze/annotation.rs`：去掉 `AnalyzeFingerprintHistory` 与 `AnnotatedDisplayTree.invalidation`，
  `compute_display_tree_fingerprints` 每帧全量计算。
- `analyze/compositor.rs`、`analyze/fingerprint/mod.rs`：删 `SceneRenderPlan`/`CompositeSig`/`item_paint_fingerprint`。
- `render/builder.rs`：删 `import_segment`/`snapshot_range`/`compact_byte_table`/`SegmentReferences`
  及 11 个相关测试（1400 → 308 行）。07b 的字节表重基修复随段复用一并移除。
- `render/dispatch.rs`、`pipeline/frame.rs`、`pipeline/default.rs`：去掉 `cache`/`CompositeHistory`/
  `AnalyzeFingerprintHistory` 参数、node-own 分支、scene snapshot 分支。
- `profile/*`：删掉上述缓存的计数字段、聚合臂与输出行。
- `opencat-engine/src/executor/mod.rs`：删 `compile_range`。
- `ablate.rs` 开关表由 15 个变体缩至 9 个；`tools/ablate-*.sh` 同步删掉 6 个 token。

## 门禁（删前 vs 删后，同机串行）

渲染走 `tools/render-guard.sh`（20GiB 硬闸 + RSS 采样）。原始件：`out/dropcaches/{before,after}/`。

| 素材 | 帧数 | before 墙钟 / RSS | after 墙钟 / RSS | 判定 |
|------|-----:|------------------:|-----------------:|------|
| hyperframes-launch | 1494 | 117s / 3608MiB | 116s / 3592MiB | pass（<10min，RSS -16MiB） |
| k3-promo | 495 | 27s / 1322MiB | 22s / 1282MiB | pass（<10min，RSS -40MiB） |

**逐位一致**：hf `mae 0 / p8 0 / maxd 0`；k3 `mae 0 / p8 0 / maxd 0`（均 0 帧非零）。

**对参考视频不劣化**：

| 素材 | ref vs after | 基线记录 |
|------|--------------|----------|
| hyperframes-launch | p8 0.064113 / mae 3.140438 | p8 0.06426145 / mae 3.146308（略优） |
| k3-promo | p8 0.013574 / mae 0.859908 | — |

删前后逐位相同，故 ref-vs-before 与 ref-vs-after 相同。

## 归因（profile，ms/frame）

| 素材 | 阶段 | before | after | Δ |
|------|------|-------:|------:|---:|
| hyperframes-launch | display | 1.30 | 1.09 | -0.21 |
| | backend | 0.26 | 0.20 | -0.06 |
| | script | 26.54 | 25.95 | -0.59 |
| k3-promo | display | 0.67 | 0.52 | -0.15 |
| | backend | 0.31 | 0.19 | -0.12 |
| | script | 13.31 | 10.71 | -2.60 |

删掉的计数器（`node_own_*`、`apply_segment_*`、`item_hit/item_miss`、`scene_snapshot_*`、
`analyze_*`）已从 profile 头部消失。

## 结论

- 6 条缓存确认是**死重量**：删掉后像素逐位相同、RSS 不升、墙钟不劣（k3 反而快 5s）。
- 三条门禁同时成立：内存更小、渲染不慢、结果与原版对比不劣化。
- 收益从"删掉的工作"转为"删掉的代码"：`render/builder.rs` 减 1092 行，
  两条缓存模块整体消失，profile 计数器减少。
- 07 `item_pictures` 是唯一"只赔不赚"的一条：删除同时移除了它的非透明风险与 07b 特例修复。

## 复现

```sh
cargo build --release --features profile --bin opencat
cargo test -p opencat-core          # 609 passed / 3 ignored

for n in hyperframes-launch k3-promo; do
  tools/render-guard.sh examples/$n/$n.jsx out/dropcaches/after/$n.mp4 20
done
python3 tools/k3diff.py out/dropcaches/before/hyperframes-launch.mp4 \
        out/dropcaches/after/hyperframes-launch.mp4 --out out/dropcaches/diff-hf-before-after
python3 tools/k3diff.py out/dropcaches/before/k3-promo.mp4 \
        out/dropcaches/after/k3-promo.mp4 --out out/dropcaches/diff-k3-before-after
```
