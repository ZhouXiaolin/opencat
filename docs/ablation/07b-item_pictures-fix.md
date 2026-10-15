# 消融 07b · `item_pictures` 修复验证

本文件是 [消融 07](07-item_pictures.md) 的闭环记录。07 判定 `item_ranges` 非透明，
本文件定位真因、提交修复、重跑四门禁。

## 修复目标（用户提议）

"把 `image_filter` 并进缓存 key"。按 07 的假设，`filter: blur()` 逐帧变 sigma，
而 item key 不变，于是旧 sigma 被重放。

## 定证一：key 是完整的，假设被排除

给 `render_display_item_cached` 的**命中**分支加临时探针（`OC_DBG_ITEM=1`）：
命中时把该 item 用同一 ctx 重录到 scratch builder，再和缓存段逐字段比对。
全片 1494 帧运行，4181 次命中，**0 次 mismatch**（ops / paints / paths /
bytes / f32_pool / effects / resources 全部一致）。

结论：item 段的内容总是等于当帧重录的内容。key 不漏项。

另外，`DisplayItem` 里**没有** `image_filter` 字段（它只存在于 IR 的 `PaintSpec`，
由 node composite 的 `css_filter` 生成）。而 composite 的 `SaveLayer` 在 item 段
**之外**（`save_composite_layer` 先于 `render_display_item`），所以 item 段天然不
依赖 composite。方案①（改 key）在字面和语义上都无效。

## 定证二：真因是字节表偏移没有重基

加临时探针（`OC_DBG_FRAME=532`）转储整帧 IR，对比"缓存开"与
`OC_ABLATE=item_pictures`。两边的 op / paint / child 表完全相同，唯一差异是

| 表项 | 缓存开（有问题） | 缓存关（正确） |
|------|-----------------|---------------|
| `op[1605]` | `RuntimeEffect { effect: EffectId(1), uniforms: BytesRangeId(1) }` | 同，`EffectId(0)` |
| `byte_range[1]` | `{ start: 0, len: 64 }` | `{ start: 64, len: 64 }` |
| `effect[0] / [1]` | 两条同哈希（导入时未去重，无害） | 一条 |

`op[1605]` 是 flex-css 圆锥渐变圆盘的 `RuntimeEffect`。执行器按
`frame.bytes[range.start .. start+len]` 读 uniform（`replay.rs:814`）。缓存开时
`start=0`，圆盘读到**本帧第一个 uniform payload**（另一个渐变盘的配色），而不是
自己的 payload——所以只有那块实心圆盘变色，恰好 43 帧。

（`effect` 哈希两边不同是红鲱鱼：该哈希来自 `ahash::AHasher::default()` 的随机键，
同一配置两次运行也不同，只用于进程内去重。）

**根因**：`DrawOpBuilder::import_segment` 克隆 `segment.byte_ranges` 时直接
`cloned()`，没有把 `TableRange.start` 重基到本帧 `bytes` 表的尾部。
`snapshot_range` 把 payload 压成 0 基偏移，而 `bytes` 表是 append 的——一旦本帧
已经 intern 过别的 payload，回放的段就读错数据。

## 修复

`crates/opencat-core/src/render/builder.rs::import_segment`：重基字节偏移。

```rust
let bytes_offset = self.bytes.len() as u32;
self.bytes.extend_from_slice(&segment.bytes);
self.byte_ranges
    .extend(segment.byte_ranges.iter().map(|range| TableRange {
        start: range.start + bytes_offset,
        len: range.len,
    }));
```

加回归测试 `import_segment_rebases_byte_offsets_onto_existing_table`（目标 builder
先 intern 一个 payload，再导入段，断言回放的 op 读到自己的 payload）。
`cargo test -p opencat-core`：663 passed / 3 ignored。

## 复跑（07b，`out/ablation/r07b-fixed-item_pictures/`）

### 门禁

| 门禁 | 要求 | 实测 | 判定 |
|------|------|------|------|
| 时间 < 10min | — | 118s / 120s / 120s | pass |
| 峰值 RSS 不劣化 | 基线 3600 / 3608 MiB | 3599 MiB | pass（-1 MiB） |
| **与基线逐位一致** | mae 0 / p8 0 / maxd 0 | **mae 0 / p8 0 / maxd 0，0 帧** | **pass（修复生效）** |
| 与参考 swgl 不劣化 | ≤ 0.06426145 | **0.064112811** | pass（更优） |

逐位比对：`base-before` vs `item_pictures` = 全零；`base-before` vs `base-after`
= 全零（渲染可复现）。

### 修复本身的效果

新基线 vs 旧基线：`mae 0.010923 / p8 0.000201 / maxd 141`，**43 帧 528–570**
（首帧 528 mae 0.2236，末帧 570 mae 0.3955）——正是 07 记下的那 43 帧，量级一致。
即修复把基线搬到了原来"缓存关"的结果上。

对齐质量（REF vs 基线）：`avg_p8 0.06426145 → 0.064112811`，
`mae 3.146308 → 3.140437701`。

### 相位（profile，ms/frame）

| 阶段 | base-before | item_pictures(off) | base-after | Δ |
|------|------------:|-------------------:|-----------:|--:|
| script | 26.29 | 26.29 | 26.58 | 0.00 |
| layout | 0.15 | 0.15 | 0.15 | 0.00 |
| display | 1.32 | 1.32 | 1.32 | 0.00 |
| backend | 0.26 | 0.24 | 0.26 | -0.02 |
| `item_hit` | 2.65 | 0.00 | 2.65 | — |
| `item_miss` | 1.26 | 3.92 | 1.26 | — |

## 结论（修订 07）

- 07 的"非透明"已修复，**根因不是缓存 key，而是段回放的字节表偏移未重基**。
- 修复后这条缓存是**透明**的：关掉它像素逐位相同。
- 它的收益仍然 ≈ 0：`backend Δ -0.02ms/frame` 落在噪声内，墙钟 118s→120s、
  相位 Δ 全为 0。也就是"透明但无收益"。
- 因此它现在**可以安全保留**（不再让结果劣化），也可以删掉（无收益）。按项目
  门禁（内存 / 速度 / 与原版对齐），**保留即可**——修复后的重基逻辑对
  node-own / apply 段缓存同样必要。

## 影响面

`import_segment` 同时服务 `item_ranges` / `node_own_segments` / `apply_segments`
三种段缓存。当时 05（node_own）与 06（apply_segments）都量到"逐位一致"，说明本片
它们的段不带 bytes payload，结论不变。凡是段里带 `RuntimeEffect` 的都会受益。

**注意**：修复把基线改好了，其余 14 份记录里引用的"REF vs 基线
`p8 0.06426145 / mae 3.146308`"应更新为 `p8 0.064112811 / mae 3.140437701`。

## 复现

```sh
bash /tmp/round.sh item_pictures item_pictures 07b-fixed
R=out/ablation/r07b-fixed-item_pictures
REF=/home/solaren/Projects/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4
python3 tools/k3diff.py "$R/base-before.mp4" "$R/item_pictures.mp4" --out "$R/diff-vs-base"
python3 tools/k3diff.py "$REF" "$R/base-before.mp4" --out "$R/ref-base-before"
rg "item_hit|item_miss|avg ms/frame: script" "$R"/hyperframes-launch-*.log
```
