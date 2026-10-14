# 消融 05 · `node_own`

## 变体

- **关闭的缓存**：`RenderCache::node_own_segments`（节点**自身**内容的 IR 段；命中即可跳过该节点
  自身内容的重复录制）。
- **关闭点**：`crates/opencat-core/src/render/dispatch.rs:606`（命中查询恒为空）与
  `:690`（不再写入）。
- **开关**：`OC_ABLATE=node_own`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r05-node_own/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `node_own` | 判定 |
|------|----------------:|-----------:|------|
| 墙钟 < 10min | 149.5s（151 / 148） | 146s | pass（Δ -3.5s，即更快；脚本归一后 ≈ -1.5s，噪声） |
| 峰值 RSS 不劣化 | 3598MiB（3602 / 3594） | 3601MiB | pass（+3MiB，噪声内） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） | 说明 |
|------|-------:|-----:|-------:|-----------------:|------|
| script | 33.86 | 32.38 | 31.80 | -0.45 | 噪声地板；变体那轮机器略快 |
| layout | 0.21 | 0.29 | 0.20 | +0.09 | 噪声级（≈0.13s 全片） |
| display | 1.54 | 1.52 | 1.59 | -0.05 | 噪声 |
| backend | 0.30 | 0.32 | 0.30 | +0.02 | 噪声 |

**缓存计数器与录制开销（同一 profile 头部）**

| 指标 | 基线 | 变体 | 说明 |
|------|-----:|-----:|------|
| `node_own_hit` | 12.45 | **0.00** | 命中被消融（设计如此） |
| `node_own_record` | 0.08 | **12.53** | 命中转为每帧重录 |
| `node_own_repeat` | 12.46 | 0.00 | 命中复用计数归零 |
| `node_own_util` | 21.57 | 0.00 | — |
| span `node_own_segment_record` | incl 0.00 / calls 0.07 | incl **0.02** / calls **12.01** | 重录 12 次/帧合计仅 0.02ms/frame |

## 结论

- **判定：命中率高但收益≈0 —— 本片的"忙而无功"缓存；仍然透明。**
- 该缓存每帧命中 12.45 次，命中率看似很好；但把它关掉后，被省下的工作只值
  **0.02ms/frame**（≈0.03s 全片），在四舍五入之外 —— 也就是说它缓存的那些节点自身段
  （几乎全是 `draw_item_rect`，1.7µs/次）本来就便宜。
- 相位 Δ 全在噪声内，墙钟甚至更快；像素逐位相同、RSS 不变。
- 判定边界：只说明 **hyperframes-launch 的节点自身段都很小**。对该片的收益是 0，
  但它是 `RenderCache` 统一路径的一部分，删它换不来任何东西，保留成本也为 0。

## 复现

```sh
R=out/ablation/r05-node_own   # bash /tmp/round.sh node_own node_own 05
rg "node_own_(hit|record|repeat|util)|node_own_segment_record: incl" "$R"/hyperframes-launch-*.log
python3 tools/k3diff.py "$R/base-before.mp4" "$R/node_own.mp4" --out "$R/diff-vs-base"
```
