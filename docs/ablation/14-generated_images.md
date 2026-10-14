# 消融 14 · `generated_images`

## 变体

- **关闭的缓存**：宿主 generated-image LRU（engine 侧 `generated_cache`，`GeneratedImageId -> skia::Image`）。
  消融后 `prepare_frame` 对每个 generated image 都重新 `raster_from_data`
  （复制整幅 RGBA 并上传）。
- **关闭点**：`crates/opencat-engine/src/consumer.rs:324`（`prepare_frame` 的
  `generated_cache.get_cloned` 查表）。
- **开关**：`OC_ABLATE=generated_images`。日志头自描述行：`ablate: generated_images`。

> 注意：本 hook **只关查表，不关插入**。命中分支返回缓存句柄，未命中分支照常
> `insert_with_weight`。所以消融后内存占用与基线一致 —— 这点在下面的 RSS 门禁里得到验证。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r14-generated_images/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `generated_images` | 判定 |
|------|----------------:|-------------------:|------|
| 墙钟 < 10min | 119s（120 / 118） | 118s | pass（Δ -1s，噪声内；本轮基线自身极稳，仅差 2s） |
| 峰值 RSS 不劣化 | 3601MiB（3601 / 3601） | 3599MiB | pass（-2MiB，噪声） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

本轮基线反复性最好：墙钟两次基线仅差 2s、`script` 仅差 0.65ms/frame。

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| script | 27.18 | 26.34 | 26.53 | -0.52（≤ 噪声地板 0.65） |
| layout | 0.16 | 0.15 | 0.15 | 0.00 |
| display | 1.34 | 1.30 | 1.28 | -0.01（噪声） |
| backend | 0.27 | 0.25 | 0.26 | +0.005 |

变体在每一列都落在"略快一点"的一侧，且幅度小于两基线的摆幅 →
符合"缓存从不命中、消融只去掉一次查表"的解释，而不是"消融付出代价"。

## 机制 / 为什么读不出收益

两条结构性原因让这条 LRU 在本片**不可能命中**：

1. **id 逐帧变化**。本片唯一的生成图来自 canvas-close 的
   `surf.bake('cc-frame-' + ctx.frame)`（`examples/hyperframes-launch/hyperframes-launch.jsx:2155`）。
   `ctx.frame` 就是合成帧号，所以 id 每帧都不同，跨帧永不重复。
2. **帧内重复已被上游去重**。`prepare_frame` 在进入这段循环前先判
   `image_index.contains_key(&image_ref)` 并 `continue`
   （`consumer.rs:317`）——同一 id 在一帧里出现多次也只会构建一次。

因此 LRU 的实际作用**不是命中收益，而是内存上限**：它把"每帧一张 ~8MiB 的
1920×1080 RGBA SkImage"压在一个有界 LRU 里（`consumer.rs:321-325` 的注释说明了这一点），
否则整片会按帧数线性增长。

计数器口径：这是一条 engine 路径，profile 的 `img_hit / img_miss` 与
`cache pressure` 的 `image_*` 三次运行全部为 0.00 → 属相位/计数盲区，
但本例的相位 Δ 已经足够小，不需要额外探针。

## 结论

- **判定：无信号（透明）—— 本片结构上永不命中，缓存的价值只在"内存有界"。**
- 像素逐位相同；RSS 与基线一致（hook 不关插入，符合预期）。
- 与 `scene_snapshot`（08）同类"本片永不命中"，但成因不同：
  08 是每帧都有变更导致整帧复用失效，14 是 **id 逐帧唯一**。
- 对项目三门禁的意义：这条缓存**不产生速度收益**，也不占额外内存（基线本来就有它）。
  若关心内存，本片 ~1~2 张 8MiB SkImage 的常驻量级远小于 `video_frames`（11 的 0.5GiB）；
  若关心速度，本片无优化空间 —— 真正的收益方是别的、连续多帧复用同一 id 的合成。

## 复现

```sh
R=out/ablation/r14-generated_images   # bash /tmp/round.sh generated_images generated_images 14
grep -o "avg ms/frame: script.*" "$R"/hyperframes-launch-*.log
for f in base-before generated_images base-after; do
  printf "%-18s " $f; sort -t, -k2 -n "$R/hyperframes-launch-$f.rss.csv" | tail -1
done
python3 tools/k3diff.py "$R/base-before.mp4" "$R/generated_images.mp4" --out "$R/diff-vs-base"
python3 tools/k3diff.py /home/solaren/Projects/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4 \
  "$R/generated_images.mp4" --out "$R/ref-variant"
```
