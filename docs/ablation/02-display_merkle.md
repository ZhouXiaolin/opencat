# 消融 02 · `display_merkle`

## 变体

- **关闭的缓存**：`DisplayBuildSession` 的子树 merkle 复用（display 树重建时的缓存命中）。
- **关闭点**：`crates/opencat-core/src/display/build.rs:172`（`cached_matches` 前加消融判断）。
- **开关**：`OC_ABLATE=display_merkle`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)：1494 帧 hyperframes-launch，参考 swgl mp4，
`--features profile` 构建，`render-guard.sh`（20GiB 熔断）+ `timeout 600`，前后基线夹逼。
原始件：`out/ablation/r02-display_merkle/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `display_merkle` | 判定 |
|------|----------------:|-----------------:|------|
| 墙钟 < 10min | 155s（158 / 152） | 155s | pass（Δ 0s，低于本轮机噪声 ±3s） |
| 峰值 RSS 不劣化 | 3598MiB（3601 / 3596） | 3609MiB | pass（+11MiB，+0.3%，噪声内） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） | 说明 |
|------|-------:|-----:|-------:|-----------------:|------|
| script | 34.89 | 32.98 | 31.54 | -2.96 | 噪声地板（本轮机器负载漂移较大，±1.7ms/frame） |
| layout | 0.18 | 0.17 | 0.17 | -0.005 | 噪声 |
| display | 1.73 | **3.20** | 1.57 | **+1.55** | 真实信号 |
| backend | 0.32 | 0.31 | 0.31 | -0.005 | 噪声 |

两次基线自身的 `display` 只差 0.16ms/frame，而变体高出 1.55ms/frame；首轮扫描
（单次运行）也读得 +1.65ms/frame。两条独立证据一致，判定为真实信号：
1.55ms/frame × 1494 帧 ≈ **2.3s**。

## 结论

- **判定：弱正向缓存（真实收益，但低于墙钟分辨率）。**
- 收益量级：约 2.3s / 155s ≈ **1.5%**。因为总墙钟里只有约 35ms/frame 被 profile 计入，
  其余是编码与 IO，所以 +2.3s 在 155s 的墙钟上被 ±3s 的漂移淹没 —— 墙钟读数为 0 不代表无收益。
- 归因唯一：只影响 `display` 阶段，`script`/`layout`/`backend` 都在噪声内。
- 像素逐位相同、峰值 RSS 不变 → 纯复用，不承担行为语义。
- 与消融 01 对照：同类 merkle 复用，layout 的收益（7%）是 display 的 4.7 倍 ——
  hyperframes-launch 里结构变化主要打在布局输入上，display 子树指纹的命中率更高（复用需求更少）。

## 复现

```sh
R=out/ablation/r02-display_merkle
bash /tmp/round.sh display_merkle display_merkle 02   # 见消融 01 的等价内联写法
python3 tools/k3diff.py "$R/base-before.mp4" "$R/display_merkle.mp4" --out "$R/diff-vs-base"
```
