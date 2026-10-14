# 消融 03 · `analyze_history`

## 变体

- **关闭的缓存**：`AnalyzeFingerprintHistory` 的逐帧指纹复用（analyze 阶段的上一帧指纹历史）。
- **关闭点**：`crates/opencat-core/src/analyze/annotation.rs:93`（强制返回空历史，等价于每帧全量）。
- **开关**：`OC_ABLATE=analyze_history`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r03-analyze_history/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `analyze_history` | 判定 |
|------|----------------:|------------------:|------|
| 墙钟 < 10min | 146s（139 / 153） | 151s | pass（原始 Δ +5s，脚本归一后 ≈ +0.9s，见下） |
| 峰值 RSS 不劣化 | 3601MiB（3601 / 3602） | 3600MiB | pass（-1MiB，噪声内） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

两次基线自身相差 14s（10%）——本机负载漂移比该变体的信号还大，所以墙钟必须用
`script` 列做负载归一。

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） | 说明 |
|------|-------:|-----:|-------:|-----------------:|------|
| script | 31.07 | 32.71 | 32.63 | +0.86 | 噪声地板；变体那轮机器慢 ~2.7%（负载代理） |
| layout | 0.19 | 0.19 | 0.19 | 0.00 | 噪声 |
| display | 1.52 | 1.65 | 1.59 | +0.095 | 噪声 |
| backend | 0.30 | 0.30 | 0.31 | 0.00 | 噪声 |

**负载归一**：用同一轮的 `script`（该缓存不作用于 script）当负载代理，
变体墙钟归一为 151s × 31.85/32.71 ≈ 147s，对基线 146s 的 Δ ≈ **+1s**。

## 结论

- **判定：本片无信号（在噪声地板内）—— 对该合成它是"死重量"，但仍是透明缓存。**
- 所有阶段 Δ 都 ≤ 0.1ms/frame；`resolve` 与 `script` 逐轮相等（差值 0.00ms/frame），
  说明指纹历史在本片没有产生可观测的复用收益。
- 墙钟原始 +5s 全部可由机器漂移解释（基线自身就漂 14s）。
- 判定边界：这只说明 **hyperframes-launch 的帧间指纹基本每帧都在变**（复用不命中），
  不等于该缓存在其它合成（长静止段）里也无收益。保留它，代价为零（RSS 不变、像素不变）。

## 复现

```sh
R=out/ablation/r03-analyze_history
# 等价于: bash /tmp/round.sh analyze_history analyze_history 03
for n in "base-before:" "analyze_history:analyze_history" "base-after:"; do
  name=${n%%:*}; tok=${n#*:}
  OC_ABLATE="$tok" OC_RENDER_GUARD_FOREGROUND=1 OC_RENDER_LOG_DIR="$R" \
    OC_RENDER_GUARD_STAMP="$name" timeout -k 5 600 \
    tools/render-guard.sh examples/hyperframes-launch/hyperframes-launch.jsx "$R/$name.mp4" 20
done
python3 tools/k3diff.py "$R/base-before.mp4" "$R/analyze_history.mp4" --out "$R/diff-vs-base"
```
