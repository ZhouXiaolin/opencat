# 消融 09 · `font_system`（当前最大单项）

## 变体

- **关闭的缓存**：`FontSystemCache` —— 每线程按 `(db 地址, generic, emoji_first)` 复用的
  cosmic-text `FontSystem`（shape 热路径）。
- **关闭点**：`crates/opencat-core/src/text/mod.rs:429`（`with_shape_font_system` 里直接
  每次新建 `FontSystem::new_with_locale_and_db_and_fallback("en-US", db.clone(), ...)`）。
- **开关**：`OC_ABLATE=font_system`。

## 素材与构建

同 [消融 01](01-layout_merkle.md)。原始件：`out/ablation/r09-font_system/`。

## 门禁

| 门禁 | 基线（夹逼） | `font_system` | 判定 |
|------|-------------:|--------------:|------|
| 墙钟 < 10min | 121s（121 / 121） | **167s** | pass（+46s，**+38%**） |
| 峰值 RSS 不劣化 | 3597.5MiB（3601 / 3594） | 3600MiB | pass（+2.5MiB） |
| 与基线逐位一致 | — | mae 0 / p8 0 / maxd 0 | pass |
| 与参考 swgl 不劣化 | p8 0.06426145 / mae 3.146308 | p8 0.06426145 / mae 3.146308 | pass（同一数值） |

两次基线墙钟完全相同（121s / 121s）——本批最干净的一次夹逼，所以 +46s 无歧义。

## 归因（profile，ms/frame，1494 帧平均）

| 阶段 | 基线前 | 变体 | 基线后 | Δ（vs 夹逼均值） |
|------|-------:|-----:|-------:|-----------------:|
| script | 26.19 | 27.10 | 26.52 | +0.75（噪声地板） |
| layout | 0.15 | **4.07** | 0.15 | **+3.92** |
| display | 1.31 | 1.36 | 1.35 | +0.03（噪声） |
| backend | 0.26 | **24.43** | 0.27 | **+24.17** |

合计 Δ = 28.1ms/frame × 1494 帧 ≈ **42s**，与墙钟实测 +46s 同量级。

**backend 内部子归因（同一 profile 头部）**

| span | 基线 | 变体 | calls/frame |
|------|-----:|-----:|------------:|
| `draw_item_text`（顶层） | incl 0.15 | incl **19.97**（excl 19.97） | 3.02 |
| 嵌套 `draw_item → draw_item_text`（阴影/层内） | incl 0.02 | incl **2.20**（excl 2.20） | 0.40 |
| `draw_item`（总） | incl 0.26 | incl **24.43** | 13.86 |

即：**每帧只有约 3.4 次文本绘制调用**，却多花 22ms/frame —— 单次 ~6.6ms。

## 机制

- 消融后每次 shape 都要 `db.clone()`（**整库深拷贝**，cosmic-text 按值接收 `fontdb::Database`）
  再 `FontSystem::new_with_locale_and_db_and_fallback`。
- 代码注释写"构建 FontSystem 深拷贝整库约 50µs"，但实测单次代价是 **~6.6ms（≈130×）**：
  真正的开销不是那一次拷贝，而是**冷 FontSystem 丢失了 cosmic-text 内部的字体匹配 /
  fallback / shaping 缓存**，于是每次调用都要重新做字体匹配与成形。
- 缓存是线程局部的，键含 db 地址 → composition 换字体库时自动失效，语义安全。

## 结论

- **判定：全部消融里最大的性能资产（38% 墙钟），且完全透明**（像素逐位相同、RSS 不变）。
- 量级对照：`layout_merkle` 7%、`display_merkle` 1.5%，本条 **38%** —— 是第二名的 5 倍。
- 归因唯一且自洽：多出来的时间只落在 `layout`（measure_text，+3.92）与
  `backend` 的文本绘制（+24.17），其它阶段在噪声内。
- 风险提示（供后续优化参考）：任何绕过这条缓存的调用路径（新加的文本 API、非线程局部复用、
  每帧换字体库导致 db 地址变化）都会按 6.6ms/次收费。这条缓存也是"文本重活"的总闸门。
- 与历史记录的差异：`20aa12e` 当时记录为 240s→232s（-3.3%）。本次 A/B 测出 -46s/-38%。
  两者口径不同（当时是跨时间点的整片对比，且视频/两仪提速尚未落地），但差 10 倍以上，
  建议以本次带消融开关的 A/B 为准，并重新评估该提交的历史收益记录。

## 复现

```sh
R=out/ablation/r09-font_system   # bash /tmp/round.sh font_system font_system 09
rg "avg ms/frame|draw_item_text" "$R"/hyperframes-launch-*.log
python3 tools/k3diff.py "$R/base-before.mp4" "$R/font_system.mp4" --out "$R/diff-vs-base"
```
