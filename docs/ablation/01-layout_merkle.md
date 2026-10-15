# 消融 01 · `layout_merkle`

## 变体

- **关闭的缓存**：`LayoutSession` 的输入 merkle 复用 + 布局结果复用。
- **关闭点**：`crates/opencat-core/src/layout/mod.rs:113`（`same_structure` 子树复用）与
  `:128`（`layout_must_recompute` 强制重算）。
- **开关**：`OC_ABLATE=layout_merkle`（`crates/opencat-core/src/ablate.rs`）。

## 素材与构建

- 素材：`examples/hyperframes-launch/hyperframes-launch.jsx`，1494 帧，1920x1080@30。
- 参考：`hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4`。
- 构建：`cargo build --release --features profile --bin opencat`。
- 运行：`tools/render-guard.sh`（20GiB RSS 熔断 + 逐秒 RSS 采样）外层套 `timeout -k 5 600`（时间门禁）。
- 夹逼：基线前 / 变体 / 基线后，参照取相邻两次基线均值（消掉机器负载漂移）。
- 原始件：`out/ablation/r01-layout_merkle/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `layout_merkle` | 判定 |
|------|----------------:|----------------:|------|
| 墙钟 < 10min | 152s（153 / 151） | **161s** | pass（+9s，+5.9%） |
| 峰值 RSS 不劣化 | 3602MiB（3599 / 3606） | **3601MiB** | pass（±0） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） | 说明 |
|------|-------:|-----:|-------:|-----------------:|------|
| script | 33.96 | 32.43 | 32.77 | -0.94 | 噪声地板（该缓存不作用于 script） |
| layout | 0.19 | **7.34** | 0.17 | **+7.16** | 真实信号，约 10.4× 噪声地板 |
| display | 1.67 | 1.50 | 1.52 | -0.09 | 噪声 |
| backend | 0.33 | 0.26 | 0.33 | -0.07 | 噪声 |

`script` 列的跨轮散布给出噪声地板 ≈ ±0.8ms/frame。`layout` 的 +7.16ms/frame × 1494 帧
≈ 10.7s，与墙钟实测 +9s 同量级，两者互相印证。

## 结论

- **判定：正向有效缓存，且完全透明。**
- 关掉它只影响 `layout` 阶段（+7.16ms/frame），其他阶段在噪声内 → 归因唯一、无连带效应。
- 收益量级：约 10.7s / 152s ≈ **7%** 墙钟。这是当前已验证 15 个缓存里最大的单项收益。
- 像素逐位相同、峰值 RSS 不变 → 它是纯复用，不承担任何行为语义。
- 反面价值：该缓存的输入 merkle 是 `frame = f(t)` 下唯一能挡住"结构未变却整树重排"的闸门；
  消融后慢 7%，说明 hyperframes-launch 里有大量帧的结构指纹其实没变，缓存命中率高。

## 复现

```sh
cargo build --release --features profile --bin opencat
R=out/ablation/r01-layout_merkle
for n in base-before: layout_merkle:layout_merkle base-after:; do
  name=${n%%:*}; tok=${n#*:}
  OC_ABLATE="$tok" OC_RENDER_GUARD_FOREGROUND=1 OC_RENDER_LOG_DIR="$R" \
    OC_RENDER_GUARD_STAMP="$name" timeout -k 5 600 \
    tools/render-guard.sh examples/hyperframes-launch/hyperframes-launch.jsx "$R/$name.mp4" 20
done
python3 tools/k3diff.py "$R/base-before.mp4" "$R/layout_merkle.mp4" --out "$R/diff-vs-base"
python3 tools/k3diff.py "$R/layout_merkle.mp4" \
  /home/solaren/Projects/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4 \
  --out "$R/ref-variant"
```
