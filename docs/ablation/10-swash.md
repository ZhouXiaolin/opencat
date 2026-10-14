# 消融 10 · `swash`

## 变体

- **关闭的缓存**：`SWASH_CACHE`（每线程 `SwashCache`，字形轮廓→位图的光栅化结果复用）。
- **关闭点**：`crates/opencat-core/src/text/mod.rs:479`（`with_swash_cache` 里每次新建 `SwashCache`）。
- **开关**：`OC_ABLATE=swash`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r10-swash/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `swash` | 判定 |
|------|----------------:|--------:|------|
| 墙钟 < 10min | 121.5s（121 / 122） | 124s | pass（+2.5s，+2.1%） |
| 峰值 RSS 不劣化 | 3601MiB（3601 / 3601） | 3592MiB | pass（-9MiB） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| script | 26.55 | 26.56 | 26.70 | -0.07（噪声地板） |
| layout | 0.15 | 0.15 | 0.16 | 0.00 |
| display | 1.33 | 1.32 | 1.35 | -0.02（噪声） |
| backend | 0.26 | **1.27** | 0.26 | **+1.01** |

`backend` +1.01ms/frame × 1494 ≈ **1.5s**，与墙钟 +2.5s 同量级（差额是编码/IO 噪声）。

## 结论

- **判定：弱正向缓存（真实但很小，约 1.2% 墙钟），透明。**
- 归因唯一：只落在 `backend` 的文本绘制上（字形光栅化），其余阶段在噪声内。
- 量级对照：约为 `display_merkle`（1.5%）的同量级，比 `font_system`（38%）小两个数量级。
  这说明本片的字形光栅结果本来就有很高的帧内/帧间复用率（同样的字形、同样的尺寸），
  缓存命中率高但每次重算也不贵。
- 像素逐位相同、RSS 略降 → 纯复用。

## 复现

```sh
R=out/ablation/r10-swash   # bash /tmp/round.sh swash swash 10
rg "avg ms/frame" "$R"/hyperframes-launch-*.log
python3 tools/k3diff.py "$R/base-before.mp4" "$R/swash.mp4" --out "$R/diff-vs-base"
```
