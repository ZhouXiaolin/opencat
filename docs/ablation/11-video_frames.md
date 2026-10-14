# 消融 11 · `video_frames`（唯一显著动内存的缓存）

## 变体

- **关闭的缓存**：`VideoFrameCache`（按 `(path, target_time, scale_target)` 复用已解码/已缩放的视频帧）。
- **关闭点**：`crates/opencat-engine/src/media/video.rs:157`（命中查询恒不命中）与 `:170`（不再插入）。
- **开关**：`OC_ABLATE=video_frames`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r11-video_frames/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `video_frames` | 判定 |
|------|----------------:|---------------:|------|
| 墙钟 < 10min | 121.5s（123 / 120） | 134s | pass（**+12.5s，+10.3%**） |
| **峰值 RSS 不劣化** | 3600MiB（3601 / 3599） | **3093MiB** | **-507MiB（-14.1%）** |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

RSS 峰值来自每轮 `render-guard` 的逐秒采样（`*.rss.csv` 最大值）：
3601 / **3093** / 3599 MiB。

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| script | 27.71 | 26.86 | 26.62 | -0.31（噪声地板） |
| layout | 0.16 | 0.16 | 0.15 | +0.005 |
| display | 1.35 | 1.36 | 1.33 | +0.02（噪声） |
| backend | 0.26 | 0.27 | 0.27 | +0.005 |

**四个阶段全部读不出 Δ**，但墙钟 +12.5s、RSS -507MiB 都远超噪声。原因见下。

## 机制 / 为什么相位看不见它

- 该缓存在 `MediaContext`（engine 层），命中的收益是**免掉一次视频解码 + 缩放**。
- profile 的 `backend` 是 **core 的 DrawOp 录制阶段**，engine 的 executor / 解码 / 采样
  不在其中；帧间大量时间也不在四个相位里（相位合计 ~28ms/frame，墙钟 ~81ms/frame）。
- 该路径在 profile 里也没有计数器：`video_hit/video_miss/video_decode`、
  `img_hit/img_miss`、`subtree_image_*` 在基线与变体**全部为 0.00**。
  → 这条缓存的收益只能靠墙钟 + RSS 观测，属于"相位归因盲区"，后续优化需注意。

## 结论

- **判定：正向缓存，但用内存换速度 —— 本批唯一显著动内存的缓存。**
- 交换比：**+10.3% 墙钟速度 ↔ +507MiB 峰值 RSS**（缓存开启时占约 14% 峰值内存）。
- 像素逐位相同 → 纯复用，无行为语义。
- 对项目三门禁的意义：这是"内存要小 vs 渲染要快"冲突最明显的一项。
  若要压内存，第一顺位就是它的 LRU 容量（代价是墙钟按每 100MiB ≈ 2.5s 的速度回退）；
  若要保持速度，则应明确它 ~0.5GiB 的常驻预算。当前是无上限说明的隐式 0.5GiB。

## 复现

```sh
R=out/ablation/r11-video_frames   # bash /tmp/round.sh video_frames video_frames 11
for f in base-before video_frames base-after; do
  printf "%-13s " $f; sort -t, -k2 -n "$R/hyperframes-launch-$f.rss.csv" | tail -1
done
python3 tools/k3diff.py "$R/base-before.mp4" "$R/video_frames.mp4" --out "$R/diff-vs-base"
```
