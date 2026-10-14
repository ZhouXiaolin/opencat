# 消融 07 · `item_pictures`（非透明缓存）

> **已修复**：本轮结论（非透明）已闭环，真因是段回放的字节表偏移未重基，
> 不是缓存 key。见 [消融 07b · 修复验证](07b-item_pictures-fix.md)。

## 变体

- **关闭的缓存**：`RenderCache::item_ranges` + 其 item 段（Bitmap / Lottie / DrawScript /
  SvgPath 这四类"图片型"元素的整段 IR 复用）。
- **关闭点**：`crates/opencat-core/src/render/dispatch.rs:63`（命中查询恒为空）
  与 `:119`（不再写入）。
- **开关**：`OC_ABLATE=item_pictures`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r07-item_pictures/`。

## 门禁

| 门禁 | 基线（夹逼） | `item_pictures` | 判定 |
|------|-------------:|----------------:|------|
| 墙钟 < 10min | 133s / 104s | 104s | pass（与本轮次基线后 104s 持平） |
| 峰值 RSS 不劣化 | 3601 / 3602MiB | 3599MiB | pass（-2MiB） |
| **与基线逐位一致** | — | **mae 0.0109 / p8 0.000201 / maxd 141** | **FAIL —— 不是透明缓存** |
| 与参考 swgl | p8 0.06426145 / mae 3.146308 | **p8 0.06411281 / mae 3.140438** | 消融后反而**更接近**原版 |

**本轮机器负载在下降**：`script` 从 26.90（基线前）掉到 24.05 / 24.26ms/frame，
两次基线墙钟差 29s（28%）。因此墙钟必须以 `script` 归一，或只与紧邻的基线后配对比较。
按基线后归一：变体 104s × 24.26/24.05 ≈ 104.9s，对基线后 104s 的 Δ ≈ **+1s**。

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 基线后） | 说明 |
|------|-------:|-----:|-------:|---------------:|------|
| script | 26.90 | 24.05 | 24.26 | -0.21 | 噪声地板；变体那轮机器略快 * |
| layout | 0.15 | 0.14 | 0.14 | 0.00 | 噪声 |
| display | 1.40 | 1.15 | 1.16 | -0.01 | 噪声 |
| backend | 0.29 | 0.21 | 0.23 | -0.02 | 噪声（缓存服务的目标阶段） |

\* 上表 Δ 记的是"变体减基线后"。把 `script` 当负载代理归一后，各阶段 Δ 都 ≈ 0。

**缓存计数器**

| 计数器 | 基线 | 变体 | 说明 |
|--------|-----:|-----:|------|
| `item_hit` | 2.65 | **0.00** | 命中被消融 |
| `item_miss` | 1.26 | **3.92** | = 每帧图片型元素总数（3.92/帧） |

## 像素差定位

- 非零帧：**43 帧，frame 528–570**（连续一段），峰值在 frame 532（mae 0.417，maxd 141）。
- 该窗口即 flex-css 场景的 `filter: blur(...)` 渐变段（CSS 段 519–570，见
  `examples/hyperframes-launch/hyperframes-launch.jsx` 的 blur tween）。
- 差异形状（frame 550 热图）：整片 flex-css 线框的边缘 + 一个实心圆盘整体发亮 ——
  典型的"模糊半径不同"特征，而非位移。

## 机制（代码事实 + 假设）

代码事实：

1. 只有 Bitmap / Lottie / DrawScript / SvgPath 会被这条缓存接管（`should_cache_item_picture`）。
2. 缓存 key = `item_paint_fingerprint` = hash(`layout_output_fingerprint.record_size`,
   `DisplayItemFp(item)`)（`crates/opencat-core/src/analyze/fingerprint/mod.rs:161`）。
3. `DisplayItemFp` 对 Bitmap/Lottie 只哈希 asset id、宽高、视频时间、`paint_epoch`、
   `object_fit` 与 `BitmapPaintFp`（background / border / 各类 shadow）——
   **不含 `image_filter`（即 `filter: blur(...)` 落地的那个字段）**
   （`crates/opencat-core/src/analyze/fingerprint/display_item.rs:77,192`）。

假设（据上述事实，需一次定点实验确认）：blur 半径在 528–570 之间逐帧变化，而 item key
不变，于是缓存把**较早一帧录下的段（旧 sigma）**重放到这一段，模糊半径被部分冻结。
关掉缓存后每帧按当前 sigma 重录，所以更接近原版。

确认实验（下一步，不算本轮结论）：在 519–575 区间单帧渲染，打印该元素的
`paint.image_filter` sigma 与 `item_ranges` 命中/未命中，比对录段时的 sigma。

## 结论

- **判定：非透明缓存，且收益≈0 —— 目前 7 个变体里唯一"只赔不赚"的一个。**
- 收益：`item_hit` 2.65 次/帧换来的时间在噪声内（墙钟 Δ ≈ 0、display/backend Δ ≤ 0.02ms/frame）。
- 代价：43 帧（528–570）偏离基线，且**偏离方向是变差**：基线 p8 0.06426145 →
  消融后 0.06411281，mae 3.146308 → 3.140438，两条指标都显示关掉缓存更接近原版。
- 因此这条缓存既没有可测量的性能收益，又让"结果与原版对比"出现可定位的劣化。
  按项目门禁（"结果和原版对比不能劣化"），它应当被收紧，而不是保留现状。
- 收紧方案（二选一，均不损失可测量性能，因为收益为 0）：
  ① 把 `image_filter` 纳入 `DisplayItemFp`/`item_paint_fingerprint`；
  ② 直接不对带 `image_filter` 的 item 建缓存（`should_cache_item_picture` 里排除）。

## 复现

```sh
R=out/ablation/r07-item_pictures   # bash /tmp/round.sh item_pictures item_pictures 07
rg "item_hit|avg ms/frame" "$R"/hyperframes-launch-*.log
python3 tools/k3diff.py "$R/base-before.mp4" "$R/item_pictures.mp4" --out "$R/diff-vs-base"
python3 - <<'PY'
import json, itertools
rows=[r for r in json.load(open("out/ablation/r07-item_pictures/diff-vs-base/summary.json"))["rows"] if r["mae"]>0]
print(len(rows), [(r["frame"], round(r["mae"],3)) for r in rows[:3]], rows[-1]["frame"])
PY
```
