# 消融 12 · `runtime_effects`

## 变体

- **关闭的缓存**：`RUNTIME_EFFECT_CACHE`（thread-local，`hash -> skia::RuntimeEffect`）。
  消融后 `compile_runtime_effect_cached` 绕过查表，每次都调
  `RuntimeEffect::make_for_shader(sksl, None)` 重新编译。
- **关闭点**：`crates/opencat-engine/src/consumer.rs:391`（`compile_runtime_effect_cached`）
  与调用点 `:324` 附近的 `prepare_frame` 拷贝路径。
- **开关**：`OC_ABLATE=runtime_effects`。日志头自描述行：`ablate: runtime_effects`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r12-runtime_effects/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `runtime_effects` | 判定 |
|------|----------------:|------------------:|------|
| 墙钟 < 10min | 121.5s（127 / 116） | 118s | pass（Δ -3.5s，**在噪声内**，见下） |
| 峰值 RSS 不劣化 | 3601.5MiB（3602 / 3601） | 3603MiB | pass（+1MiB，噪声） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

**墙钟是本轮唯一的干扰源**：基线前 127s、基线后 116s，两次基线自身差 **11s（9.2%）**。
会话期间机器负载从 15min 3.59 降到 1min 1.85，运行顺序（前→变体→后）正好落在负载下降的斜坡上。
所以 118s 落在两个基线之间属于"负载顺序效应"，不是缓存收益或代价：

- 夹逼均值 121.5s → Δ -3.5s，**小于基线自身的 11s 摆幅**，读不出信号。
- RSS 三次采样 3602 / 3603 / 3601 MiB，逐位相同 → 判定 pass。

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| script | 27.35 | 26.32 | 26.39 | -0.55（**≤ 噪声地板 0.96**） |
| layout | 0.16 | 0.16 | 0.15 | +0.005 |
| display | 1.39 | 1.30 | 1.29 | -0.045（噪声，上轮同项摆 ±0.02） |
| backend | 0.28 | 0.26 | 0.25 | -0.005 |

`script` 相位同样带负载漂移（基线前 27.35 与它 127s 的墙钟一致偏高），
两基线差 0.96ms/frame，变体的 -0.55 落在其内 → 与墙钟结论一致：无信号。

## 机制 / 为什么相位与墙钟都看不见

- 该缓存在 **engine 的 raster/prepare 车道**（`consumer.rs` 的上传准备），
  core 的四个相位（script/layout/display/backend）只统计 DrawOp 录制，不含它。
  与 [消融 11](11-video_frames.md) 同类，属"相位归因盲区"。
- profile 里没有 runtime-effect 计数器；可用的密度代理是 backend 计数
  `draw_script 0.2/帧`（三次运行完全相同，0.2）。
  即整片约 **0.2 次/帧 × 1494 ≈ 300 次** runtime-effect 编译调用。
- 即使单次 `make_for_shader` 花几 ms，300 次的上限也只有 ~1s / 121.5s ≈ 1%，
  **压在噪声地板以下**，所以消融既测不出代价，也就反证了缓存的收益同样可忽略。
- 本片的 shader 面本来就窄：`ly.pixel` 只服务 conic 渐变斜坡
  （`examples/hyperframes-launch/hyperframes-launch.jsx:453/526/621`）、
  flex-shader 的 fbm（`:726`，主时间轴只开 `FS_DUR=1.20s`）与一个恒等采样核（`:2156`）。

## 结论

- **判定：无信号（透明）—— 收益与代价都低于噪声地板。**
- 像素逐位相同（纯复用，无行为语义）、RSS 不变、四相位 Δ ≤ 噪声、墙钟 Δ 被负载漂移吞掉。
- 与 `scene_snapshot`（08）、`analyze_history`（03）、`composite_history`（04）同属"本片读不出信号"一类；
  但本轮的原因是**调用密度太低**，不是结构上必然不命中。
- 判定边界：只对 hyperframes-launch 成立。shader 重（同 hash 连续多帧复用、单次编译贵）
  的合成里，这条缓存可能有真实收益；届时需在 shader 密集场景上单独复测。
- 对项目三门禁的意义：这条缓存目前**既不占内存也无可见成本**，无需为它调整预算；
  若要动它，应先在 shader 密集样本上量出收益再决定。

## 复现

```sh
R=out/ablation/r12-runtime_effects   # bash /tmp/round.sh runtime_effects runtime_effects 12
grep -o "avg ms/frame: script.*" "$R"/hyperframes-launch-*.log
grep -o "backend avg counts/frame:.*draw_script [0-9.]*" "$R"/hyperframes-launch-*.log
for f in base-before runtime_effects base-after; do
  printf "%-14s " $f; sort -t, -k2 -n "$R/hyperframes-launch-$f.rss.csv" | tail -1
done
python3 tools/k3diff.py "$R/base-before.mp4" "$R/runtime_effects.mp4" --out "$R/diff-vs-base"
python3 tools/k3diff.py /home/solaren/Projects/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4 \
  "$R/runtime_effects.mp4" --out "$R/ref-variant"
```
