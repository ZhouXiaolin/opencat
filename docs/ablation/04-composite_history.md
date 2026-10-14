# 消融 04 · `composite_history`

## 变体

- **关闭的缓存**：`CompositeHistory` 的 apply 变更检测历史（display 树的 apply 输入变更集）。
- **关闭点**：`crates/opencat-core/src/analyze/invalidation.rs:33`（`previous` 取空表，
  等价于 `apply_changed_nodes` 恒为空集）。
- **开关**：`OC_ABLATE=composite_history`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r04-composite_history/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `composite_history` | 判定 |
|------|----------------:|--------------------:|------|
| 墙钟 < 10min | 143s（134 / 152） | 148s | pass（原始 Δ +5s，脚本归一后 ≈ +2.2s） |
| 峰值 RSS 不劣化 | 3598MiB（3594 / 3601） | 3607MiB | pass（+10MiB，+0.3%，噪声内） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） | 说明 |
|------|-------:|-----:|-------:|-----------------:|------|
| script | 33.95 | 35.83 | 36.36 | +0.67 | 噪声地板；变体那轮机器慢 ~1.9% |
| layout | 0.17 | 0.18 | 0.17 | +0.01 | 噪声 |
| display | 1.42 | 1.44 | 1.51 | -0.03 | 噪声（且方向相反） |
| backend | 0.28 | 0.32 | 0.30 | +0.03 | 噪声 |

**缓存计数器（同一 profile 头部）**

| 计数器 | 基线 | 变体 | 说明 |
|--------|-----:|-----:|------|
| `apply_change` | 0.71 | **0.00** | 消融的唯一可见效果（变更集被清零，符合设计） |
| `apply_segment_hit` | 28.38 | 28.38 | 不变 |
| `apply_segment_miss` | 6.19 | 6.19 | 不变 |
| `apply_util` | 26.03 | 26.03 | 不变 |

**负载归一**：变体墙钟 148s × 35.16/35.83 ≈ 145s，对基线 143s 的 Δ ≈ **+2s**，
仍在基线自身 18s 的漂移范围内。

## 结论

- **判定：本片无信号（死重量），且透明。**
- 变更检测历史被清零后：`apply_segment_hit/miss`、`apply_util` 全部不变，相位 Δ 全在噪声内，
  像素逐位相同 —— 说明本片的 apply 前缀段命中与「哪些节点变了」这一集合**解耦**，
  该历史在这部片子里只被记录、没有被用来省下任何工作。
- 反面参照（**待复核，勿引用**）：预备期的 `k3-promo` 扫描里 `composite_history` 读得
  `mae 0.5013 / p8 0.00544 / maxd 244`，看似非透明。但同一次扫描里 `display_merkle`
  与 `all` 的 mae/p8 **完全相同**（0.07399 / 0.000545），而 `all` 的 mae 反而小于
  `composite_history` —— 该批次内部自相矛盾，判定为不可靠数据，另找机会重测 k3-promo。

## 复现

```sh
R=out/ablation/r04-composite_history
# 等价于: bash /tmp/round.sh composite_history composite_history 04
for n in "base-before:" "composite_history:composite_history" "base-after:"; do
  name=${n%%:*}; tok=${n#*:}
  OC_ABLATE="$tok" OC_RENDER_GUARD_FOREGROUND=1 OC_RENDER_LOG_DIR="$R" \
    OC_RENDER_GUARD_STAMP="$name" timeout -k 5 600 \
    tools/render-guard.sh examples/hyperframes-launch/hyperframes-launch.jsx "$R/$name.mp4" 20
done
rg "apply_change" "$R"/hyperframes-launch-*.log
python3 tools/k3diff.py "$R/base-before.mp4" "$R/composite_history.mp4" --out "$R/diff-vs-base"
```
