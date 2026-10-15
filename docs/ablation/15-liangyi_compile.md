# 消融 15 · `liangyi_compile`

## 变体

- **关闭的缓存**：`COMPILE_CACHE`（thread-local，两仪 lambda 编译 memo，
  key = `ahash(source + uniform 名/类型 + backend + kind)`）。
  消融后每次 `compile()` 都走 `compile_uncached` —— 重新解析 DSL 并重生成 SKSL。
- **关闭点**：`crates/opencat-liangyi/src/lib.rs:184`（`compile` 的查表与插入两处）。
- **开关**：`OC_ABLATE=liangyi_compile`。日志头自描述行：`ablate: liangyi_compile`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r15-liangyi_compile/`。

## 门禁

| 门禁 | 基线（夹逼均值） | `liangyi_compile` | 判定 |
|------|----------------:|------------------:|------|
| 墙钟 < 10min | 120s（120 / 120） | 118s | pass（Δ -2s，-1.7%） |
| 峰值 RSS 不劣化 | 3601.5MiB（3602 / 3601） | 3601MiB | pass（-0.5MiB，噪声） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

## 归因（profile，ms/frame，1494 帧平均）

**本轮是本批相位地板最干净的一轮**：两次基线的 `script` 只差 0.13ms/frame。

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| script | 26.33 | 26.34 | 26.46 | **-0.055（地板 0.13 之内）** |
| layout | 0.16 | 0.16 | 0.16 | 0.00 |
| display | 1.31 | 1.31 | 1.33 | -0.01（噪声） |
| backend | 0.26 | 0.25 | 0.26 | -0.005 |

由此得到一个**紧的上界**：|Δ| ≤ 0.065ms/frame × 1494 ≈ **0.1s ≈ 0.08% 墙钟**。
即这条缓存整片省下的时间不到 0.1 秒。墙钟那 -2s 与之矛盾 → 记为负载/顺序噪声
（同一批次里墙钟漂移最大出现过 11s，见 [消融 12](12-runtime_effects.md)）。

## 机制 / 为什么收益这么小

全片只有 **3 个** `ly.pixel(...)` 调用点，都在 `script` 相位（脚本每帧重跑）：

| 调用点 | 每帧次数 | 生效帧数 | 说明 |
|--------|--------:|---------:|------|
| `drawConic`（`examples/hyperframes-launch/hyperframes-launch.jsx:623`） | 2 | 52 | flex-css 场景 `CSS_START=17.30 / CSS_DUR=1.71` → 帧 519–570 |
| `ccIdent`（`:2156`） | 1 | 113 | canvas-close 场景 → 帧 ~1381–1493 |
| `FlexShader._kern`（`:726`） | 0 | — | 被 `if (!FlexShader._kern)` 守卫，**全片只编译一次** |

所以整片 `compile()` 调用总数 ≈ 2×52 + 113 + 1 ≈ **218 次**，其余 1300 多帧为零。
而且 `drawConic` 两次调用的 lambda 源码完全相同（颜色走 uniforms），
**同一帧内第 2 次必然是缓存命中**。

把上界摊到 218 次调用：单次解析+代码生成 ≤ **~0.5ms**，落在噪声里。
结论是本片的 lambda 都短（conic 斜坡、恒等采样），两仪的解析不算瓶颈。

## 结论

- **判定：无信号（透明）—— 调用点太少、场景太窄、单次解析太便宜。**
- 像素逐位相同；RSS 与基线一致。
- 与 `runtime_effects`（12）互为姊妹（都是 shader 编译缓存），结论也一致：
  **本片的 shader 编译面很窄，编译类缓存在此读不出收益**。
  区别在车道：12 的编译在 engine raster（相位盲区），15 的解析在 `script`（相位可见）。
  15 能给出"< 0.1s"这个紧上界，正因为它落在可见相位里。
- 判定边界：只对 hyperframes-launch 成立。`ly.pixel` 里写长循环/大噪声体的合成
  （如把 fbm 放进逐帧重建的 lambda）会让这条缓存变重要 —— 本片把 fbm 放到
  `FlexShader._kern` 一次性编译，正是这条缓存的理想用法。
- 对项目三门禁的意义：这条缓存**零内存压力、零可见成本**，无需调整预算。

## 复现

```sh
R=out/ablation/r15-liangyi_compile   # bash /tmp/round.sh liangyi_compile liangyi_compile 15
grep -o "avg ms/frame: script.*" "$R"/hyperframes-launch-*.log
for f in base-before liangyi_compile base-after; do
  printf "%-18s " $f; sort -t, -k2 -n "$R/hyperframes-launch-$f.rss.csv" | tail -1
done
python3 tools/k3diff.py "$R/base-before.mp4" "$R/liangyi_compile.mp4" --out "$R/diff-vs-base"
python3 tools/k3diff.py /home/solaren/Projects/hyperframes-launches/hyperframes-launch/renders/hyperframes-launch_2026-10-06_swgl.mp4 \
  "$R/liangyi_compile.mp4" --out "$R/ref-variant"
```
