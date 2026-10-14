# 消融 08 · `scene_snapshot`

## 变体

- **关闭的缓存**：`RenderCache::last_scene_snapshot`（整帧 DrawOp 复用；消融时每帧直接判
  `Miss(PlanBlocked)`，不再做视口/根指纹比对）。
- **关闭点**：`crates/opencat-core/src/pipeline/frame.rs:230`。
- **开关**：`OC_ABLATE=scene_snapshot`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r08-scene_snapshot/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `scene_snapshot` | 判定 |
|------|----------------:|-----------------:|------|
| 墙钟 < 10min | 118.5s（117 / 120） | 120s | pass（Δ +1.5s，基线自身差 3s，噪声内） |
| 峰值 RSS 不劣化 | 3598MiB（3594 / 3602） | 3591MiB | pass（-7MiB） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

本轮基线两次自身极稳（`script` 差 0.28ms/frame），是本批最干净的一轮。

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| script | 26.71 | 26.55 | 26.43 | 0.00（噪声地板，±0.14） |
| layout | 0.16 | 0.15 | 0.15 | -0.005 |
| display | 1.33 | 1.32 | 1.33 | -0.01 |
| backend | 0.26 | 0.26 | 0.26 | 0.00 |

**缓存计数器（基线 / 变体完全相同）**

| 计数器 | 值 | 说明 |
|--------|----:|------|
| `scene_snapshot_hit` | 0.00 | 本片从未命中 |
| `scene_snapshot_miss` | 1.00 | 每帧都 miss |
| `scene_snapshot_plan_blocked_by_apply_change` | 0.71 | 主要 miss 原因 |
| `scene_snapshot_plan_blocked_by_layout / by_raster` | 0.19 / 0.19 | 次因 |
| `scene_snapshot_miss_root_fingerprint_changed` | 0.21 | 其余 0.21 |

## 结论

- **判定：本片结构上永不命中（死重量），透明。**
- 基线的决策本来就是"每帧 miss"（`plan_blocked 0.79` + `root_fingerprint_changed 0.21` = 1.00），
  消融只是把这条路径短路掉，判定结果完全不变 → 四个阶段 Δ 全在 0.01ms/frame 以内，
  像素逐位相同、RSS 略降。
- 也就是说：**整帧复用在本片的检查开销本身也可忽略**，这条缓存既没收益也没成本。
- 判定边界：只对 hyperframes-launch 成立。该片每帧都有布局/栅格/apply 变更，所以整帧
  复用永远不该命中 —— 这是设计使然，不是缺陷。长静止段的合成才有可能命中。

## 复现

```sh
R=out/ablation/r08-scene_snapshot   # bash /tmp/round.sh scene_snapshot scene_snapshot 08
rg "scene_snapshot|avg ms/frame" "$R"/hyperframes-launch-*.log
python3 tools/k3diff.py "$R/base-before.mp4" "$R/scene_snapshot.mp4" --out "$R/diff-vs-base"
```
