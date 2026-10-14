# 消融 06 · `apply_segments`

## 变体

- **关闭的缓存**：`RenderCache::apply_segments`（apply 前缀指令段，按 `apply_segment_key(plan)` 缓存，
  命中时整段 import 重放）。
- **关闭点**：`crates/opencat-core/src/render/dispatch.rs:464`（`begin_apply_frame_cached` 不再查缓存）
  与 `:498`（不再写入，直接返回刚 emit 的 frame）。
- **开关**：`OC_ABLATE=apply_segments`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r06-apply_segments/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `apply_segments` | 判定 |
|------|----------------:|-----------------:|------|
| 墙钟 < 10min | 150s（158 / 142） | 155s | pass（原始 +5s；脚本归一后 ≈ -5s，纯负载漂移） |
| 峰值 RSS 不劣化 | 3602MiB（3609 / 3595） | 3602MiB | pass（±0） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） | 说明 |
|------|-------:|-----:|-------:|-----------------:|------|
| script | 33.18 | 32.86 | 28.35 | +2.09 | 噪声地板；变体那轮机器明显更慢（基线后 28.35 是全轮最快） |
| layout | 0.18 | 0.20 | 0.22 | 0.00 | 噪声 |
| display | 1.62 | 1.60 | 1.48 | +0.05 | **缓存服务的目标阶段**，Δ 仅在噪声边缘 |
| backend | 0.31 | 0.28 | 0.28 | -0.02 | 噪声 |

**缓存计数器（同一 profile 头部）**

| 计数器 | 基线 | 变体 | 说明 |
|--------|-----:|-----:|------|
| `apply_segment_hit` | 28.38 | **0.00** | 命中被消融 |
| `apply_segment_miss` | 6.19 | **34.57** | = 每帧 apply 前缀指令总数 |
| `apply_util` | 26.03 | **0.00** | 消融后段表不再被填，故利用率归零 |

命中率 = 28.38 / 34.57 ≈ **82%**。

## 结论

- **判定：命中率 82%，但只值 0.05ms/frame（≈0.07s 全片）—— 死重量，透明。**
- 这是本轮最有信息量的一条"死重量"：缓存命中率很高、段表也在正常淘汰，但 import 重放一条
  apply 前缀指令并不比重新 emit 便宜，所以 82% 的命中率换不来可测量的时间。
- 墙钟原始 +5s 完全由负载解释：变体那一轮的 `script` 比两次基线高 2.09ms/frame，
  归一后变体反而比基线快 5s。这是"墙钟单独用会被误导"的又一个实例。

## 复现

```sh
R=out/ablation/r06-apply_segments   # bash /tmp/round.sh apply_segments apply_segments 06
rg "apply_segment_hit|avg ms/frame" "$R"/hyperframes-launch-*.log
python3 tools/k3diff.py "$R/base-before.mp4" "$R/apply_segments.mp4" --out "$R/diff-vs-base"
```
