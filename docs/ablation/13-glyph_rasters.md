# 消融 13 · `glyph_rasters`

## 变体

- **关闭的缓存**：`GLYPH_RASTERS`（thread-local，`(FaceKey, size_px, gid) -> Option<Arc<GlyphRaster>>`）。
  消融后每次 `glyph_raster()` 都新建 `ScaleContext` 并重栅格化轮廓。
- **关闭点**：`crates/opencat-core/src/text/surface.rs:272`（`glyph_raster` 查表/插入两处）。
- **开关**：`OC_ABLATE=glyph_rasters`。日志头自描述行：`ablate: glyph_rasters`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r13-glyph_rasters/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `glyph_rasters` | 判定 |
|------|----------------:|----------------:|------|
| 墙钟 < 10min | 120s（116 / 124） | 122s | pass（Δ +2s，+1.7%，在噪声内） |
| 峰值 RSS 不劣化 | 3596.5MiB（3600 / 3593） | 3602MiB | pass（+5.5MiB，噪声） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

墙钟两基线差 8s（6.7%），本轮又以**相位 Δ** 为准（见下）：
`script` +2.31ms/frame × 1494 ≈ **+3.5s**，与墙钟 +2s 同量级、同符号。

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| **script** | 26.17 | **28.80** | 26.80 | **+2.31（基线自身差仅 0.63 → 真信号）** |
| layout | 0.15 | 0.15 | 0.15 | 0.00 |
| display | 1.30 | 1.31 | 1.33 | 0.00（噪声） |
| backend | 0.25 | 0.26 | 0.26 | +0.005 |

反向佐证：backend 的 `draw_item_text: incl 0.14 / 0.14 / 0.15` 三次完全相同 →
这笔开销**不在** core 的文本绘制车道，而在 `script` 相位里的 **canvas 文本 API**。

## 机制 / 为什么落在 `script` 相位

- `glyph_raster` 只有两个调用点，都在 `text/surface.rs`：
  `surface_measure_text`（`:346`，`canvas.measureText`）与 `surface_fill_text`（`:388`）——
  即宿主 canvas 的 `fillText/measureText`。这些 API 由合成脚本在 `useFrame` 内直接调用，
  所以开销天然计入 `script` 相位，而不进 core 的 DrawOp 录制。
- 本片唯一的调用者就是 **canvas-close 场景**的 `ccRender`
  （`examples/hyperframes-launch/hyperframes-launch.jsx:2039`）：
  它按 `192 × 67 = 12864` 个字符格（`:1733-1734`）逐格 `surf.fillText`
  （`:2143`），正是 `GlyphRaster` 注释里点名的"逐字符画满 12864 格"用法
  （`text/surface.rs:253`）。
- 该场景只占 **113 帧**：`CC_START=46.02s`、`CC_DUR=3.75s`（`:1731`）→ 帧 ≈ 1381–1493。
  把平均 +2.31ms/frame 折算回该场景区间：

  **≈ 2.31 × 1494 / 113 ≈ 30.6 ms/frame**（每帧最多 12864 次 fillText → ~2.4µs/字形）。

  这解释了"平均很小、瞬时很大"的分布：收益集中在片尾 3.75s。

## 结论

- **判定：弱正向缓存（真实但小，约 +2s / +1.7% 墙钟；相位口径 +3.5s / +2.9%），透明。**
- 归因唯一且可对账：四相位里只有 `script` 动，且 `draw_item_text` 不变。
- 量级对照：与 `swash`（10，+1.01ms/frame）、`display_merkle`（02，+1.55ms/frame）同档，
  比 `font_system`（09，+24ms/frame backend）小一个数量级。
- 与 `swash` 的分工要分清：`swash` 管 core 文本绘制的字形位图（`backend`），
  本缓存管脚本 canvas `fillText` 的 coverage 位图（`script`）。两者互不重叠。
- 收益集中、容量极小（`GLYPH_RASTER_CACHE_CAP = 4096` 条、每条约 <1KiB）：
  这是"以极小内存换分区收益"的良性缓存，无内存门禁压力。
- 判定边界：只对含逐字符 canvas 文本（ASCII art / 单元格渲染）的合成成立。

## 复现

```sh
R=out/ablation/r13-glyph_rasters   # bash /tmp/round.sh glyph_rasters glyph_rasters 13
grep -o "avg ms/frame: script.*" "$R"/hyperframes-launch-*.log
grep -o "draw_item_text: incl [0-9.]*, excl [0-9.]*" "$R"/hyperframes-launch-*.log
for f in base-before glyph_rasters base-after; do
  printf "%-14s " $f; sort -t, -k2 -n "$R/hyperframes-launch-$f.rss.csv" | tail -1
done
python3 tools/k3diff.py "$R/base-before.mp4" "$R/glyph_rasters.mp4" --out "$R/diff-vs-base"
python3 tools/k3diff.py /home/solaren/Projects/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4 \
  "$R/glyph_rasters.mp4" --out "$R/ref-variant"
```
