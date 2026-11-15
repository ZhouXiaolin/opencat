# hyperframes-launches 硬件 Vulkan + profile 全量复跑报告

> 2026-11-15 本轮实测。口径：当前工作树、`plan.md`、本轮产物。不引用旧 avg_p8 基线作验收依据。

> 2026-11-15 收束复核：本文第 1–7 节是定位阶段的扫描记录；最终修复与全量回归见第 8 节，以第 8 节为准。

## 1. 本轮做了什么

- 编译 `target/release/opencat`，带 `--features profile`。
- 按 `out/vulkan-sweep/scripts/render_one.sh` 的硬件 Vulkan 配置串行渲染 23 例。
- 硬件设备是 AMD Radeon 610M（RADV `radeon_icd.json`）。配置用 `OC_SKIA_BACKEND=vulkan`、`OC_ANGLE_DIR`、`OC_GPU_PIPELINE=1`。
- 每例日志含完整 `Render profile`。
- 对每例 swgl 参考 MP4 做 `tools/k3diff.py`、ffmpeg `ssim` 与 `tools/align_board.py`。
- 指标含 `avg_p8`、`avg_d4`、`mae`、灰度 SSIM、ffmpeg SSIM、边缘 `shape` IoU、`ncc`、`shift0`、`shift_max`、`local_max`。
- 重建 `out/vulkan-sweep/summary.json`、`contact/`、最差帧并排图 `worst_frames/`，以及验收帧并排图 `review/`。
- 旧产物归档在 `out/vulkan-sweep-archive/2026-11-15-pre-profile/`，仅用于 delta 对照，不作验收基线。

## 2. 完整性

| 项 | 结果 |
|---|---|
| manifest 例数 | 23 |
| 渲染 MP4 | 23 |
| 渲染 `rc=0` | 23 |
| 日志含 `Render profile:` | 23 |
| k3diff `summary.json` | 23 |
| ffmpeg `ssim_stats.txt` | 23 |
| align `align.json` | 23 |
| 并排 contact sheet | 23 |
| 零字节产物 | 0 |

全量命令：`JOBS=6 /home/solaren/miniforge3/bin/python3 out/vulkan-sweep/scripts/run_sweep_parallel.py`。脚本串行渲染、并行对比，避免 23 例串行 `align_board` 耗时过长。

## 3. 指标榜

完整聚合榜：`out/vulkan-sweep/summary.json`；可读表：`out/vulkan-sweep/summary.txt`。

### 3.1 avg_p8 最差

| 排名 | slug | avg_p8 | avg_d4 | ffmpeg SSIM | shape | shift0 | local_max |
|---:|---|---:|---:|---:|---:|---:|---:|
| 1 | `vfx-heygen-combined` | 0.1654 | 0.2456 | 0.9257 | 0.9842 | 0.6849 | 170 |
| 2 | `inspector-launch` | 0.1454 | 0.4581 | 0.9660 | 0.9412 | 0.8131 | 116 |
| 3 | `claude-design-send-hyperframes-launch` | 0.1268 | 0.1970 | 0.9464 | 0.9998 | 0.6380 | 118 |
| 4 | `figma-launch` | 0.1176 | 0.2675 | 0.9502 | 1.0000 | 0.6336 | 152 |
| 5 | `frame-md-launch-storyboard` | 0.0807 | 0.1163 | 0.9527 | 1.0000 | 0.6062 | 204 |
| 6 | `texture-launch-video` | 0.0681 | 0.0983 | 0.9611 | 0.9997 | 0.6708 | 152 |
| 7 | `timeline-launch` | 0.0572 | 0.0748 | 0.9820 | 1.0000 | 0.8157 | 170 |
| 8 | `cloud-render-launch` | 0.0557 | 0.0779 | 0.9670 | 1.0000 | 0.5329 | 88 |
| 9 | `HF-heygen-stripe` | 0.0481 | 0.1898 | 0.9697 | 0.9866 | 0.7166 | 102 |
| 10 | `pr-to-video-launch` | 0.0450 | 0.0639 | 0.9607 | 1.0000 | 0.3454 | 162 |

### 3.2 其他评价标准的最差项

| 标准 | 最差样例 | 值 | 次差 |
|---|---|---:|---|
| `avg_d4` | `inspector-launch` | 0.4581 | `figma-launch` 0.2675 |
| align_board 灰度 SSIM | `claude-design-send-hyperframes-launch` | 0.9041 | `vfx-heygen-combined` 0.9131 |
| ffmpeg SSIM | `vfx-heygen-combined` | 0.9257 | `claude-design-send-hyperframes-launch` 0.9464 |
| 边缘 `shape` IoU | `inspector-launch` | 0.9412 | `hyperframes-launch` 0.9778 |
| `ncc` | `texture-launch-video` | 0.8144 | `frame-md-launch-storyboard` 0.8316 |
| `shift_max` | `texture-launch-video` | 760 | `variables-launch` 706 |
| `local_max` | `codex-five-hour-limit-replica` | 296 | `frame-md-launch-storyboard` / `hyperframes-launch` 204 |
| `shift0_ratio` | `pr-to-video-launch` | 0.3454 | `cloud-render-launch` 0.5329 |

## 4. 目视分类

判定顺序：`contact/ALL-sheets.png` → `worst_frames/<slug>.png` → `review/<slug>.png`。视觉一致即达标。随机数、噪点、采样、AA、字体栅格、渐变背景、运动模糊、相位差不投入。

### 4.1 指标最差但不是对齐问题

| slug | 指标表现 | 最差帧目视 | 结论 |
|---|---|---|---|
| `vfx-heygen-combined` | `avg_p8` 第 1，ffmpeg SSIM 最低 | f69/f93/f95：流体色带、颗粒、发光字，左右只是采样/模糊差 | 噪点/混沌背景，形态一致 |
| `inspector-launch` | `avg_p8` 第 2，`avg_d4` 与 `shape` 最差 | f544/f545：半调点阵与暗部渐变，卡片位置一致 | 点阵/半调采样差 |
| `claude-design-send-hyperframes-launch` | `avg_p8` 第 3，灰度 SSIM 最低 | f832–f836：`MERIDIAN` 蓝绿渐变背景，字标与副标题位置一致 | 渐变/噪点采样差 |
| `figma-launch` | `avg_p8` 第 4，`avg_d4` 第 2 | f425/f428：绿蓝渐变条与文字，字形位置一致 | 渐变色差 |
| `frame-md-launch-storyboard` | `avg_p8` 第 5，局部位移大 | f520 是白场过渡；f834–f861 是 `frame.md` 模糊/运动相位；其余验收帧结构一致 | 参考侧模糊/帧混合与相位差 |
| `texture-launch-video` | `avg_p8` 第 6，`ncc` 与 `shift_max` 最差 | f38/f53/f342：纹理字、色块、shader 背景，形态与位置一致，底部多出小参数标签 | 纹理采样/小标签差 |
| `hyperframes-launch` | `local_max` 204，p8 第 11 | f112/f115：蓝紫噪点背景，浏览器窗口位置与缩放一致 | 噪点背景 |
| `pr-to-video-launch` | `shift0_ratio` 最低 | f157 是窗口入场/白屏瞬态；其余验收帧一致 | 入场相位差 |
| `k3-promo` | `shift0_ratio` 0.5612 | f220 是全屏 dissolve 相位；验收帧 f70/f141/f211/f282/f493 一致 | dissolve 相位差 |
| `cloud-render-launch` | 灰度 SSIM 0.9394 | f1011 拼贴缩略图内容不同，但版式一致 | 缩略视频帧内容/相位 |
| `HF-heygen-stripe` | `avg_d4` 0.1898 | f1/f166/f267/f331/f497 结构一致；f829 叠字未淡出 | 见下节 |
| `timeline-launch` | `avg_p8` 第 7 | f371/f431 的聊天瞬态词条不同，但列表结构与时间轴一致 | 见下节 |

### 4.2 真正需要继续对齐的差异

按「形态 / 位置差异大」优先，不是按 `avg_p8` 排序。

1. **`spacex-launch`：字号尺度差，优先修。**
   - 验收帧 f985 / f1015：参考的 `$1.0T`、`SPACEX · IMPLIED IPO VALUATION` 与字幕 pill 明显小于本轮渲染。
   - 本轮 `avg_p8` 0.0290、ffmpeg SSIM 0.9781；比旧产物改善 `avg_p8 -0.0160`、SSIM `+0.0324`。
   - 根因与 `plan.md` 记录一致：嵌套 `container-type` 的 `cqw` 按合成帧视口折，未按 `.vid` 容器宽折，字号与内距约 1.33×。需要布局后二次 resolve。
   - 这是本轮唯一仍有全局尺度偏差的 example。

2. **`HF-heygen-stripe`：mask 边缘渐隐缺失。**
   - f829：参考的 3D 叠字圆鼓上下两行淡出；本轮全不透明，多出 `generates` / `provisions` 等可见词。
   - 这是「有 vs 无」和形态边缘差异，不是噪点。`shape` 0.9866 也低于多数样例。
   - 根因与 `plan.md` 记录一致：`mask-image: linear-gradient` 未实现；需要 SaveLayer + DstIn 渐变矩形并处理 3D 上下文坐标。
   - 其余验收帧 f166/f331/f497 已一致；f1 是视频首帧解码/相位差。

3. **`timeline-launch`：聊天回放瞬态内容差。**
   - f371/f431：参考是数值 pill 与 `timing is tricky` 重叠态；本轮是文本气泡态。
   - 这是短时内容形态差。`plan.md` 已列为观察项，不投入；本轮保留观察结论。

4. **`texture-launch-video`：底部参数标签多余。**
   - f38/f53：本轮多出 `TEXTURE LOOK / #0955 EACH / TEXTURE MASK-TEXT` 小标签。
   - 属于内容正确性差异，但面积极小；`avg_p8` 主要来自纹理采样。记档，不优先于 spacex / stripe。

### 4.3 局部位移指标说明

`align_board` 的 `local_max` 在平坦/高噪声背景上容易误报。例如 `codex-five-hour-limit-replica` 的 `local_max=296` 来自 f162–f174 场景过渡 blob 放大相位，不是元素错位；`texture-launch-video` 的 `shift_max=760` 来自纹理平移/缩放阶段的相关峰，形态仍一致。位置判据必须回看并排图，不能单独采信数值。

## 5. Profile 与耗时

23 例日志均含 `Render profile:` 段。代表值：

| slug | 帧数 | avg ms/frame script | resolve | layout | display | backend | elapsed_s | peak_rss_mib |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| `HF-heygen-stripe` | 1161 | 3.49 | 3.50 | 0.09 | 0.21 | 0.20 | 69 | 2560 |
| `inspector-launch` | 1350 | 18.30 | 18.30 | 1.03 | 2.78 | 0.31 | 53 | 2048 |
| `spacex-launch` | 1185 | 17.07 | 17.07 | 5.74 | 0.38 | 1.12 | 26 | 1331 |
| `vfx-heygen-combined` | 788 | — | — | — | — | — | 49 | 2867 |
| `liquid-brand-refraction` | 180 | — | — | — | — | — | 183 | 4915 |

`elapsed_s` 包含编码与进程启动；profile 段只统计渲染管线，两者不能直接相减。峰值 RSS 受 12 GiB `MemoryMax` 保护；全部样例 `rc=0`，无 OOM。

## 6. 本轮结论

- 23/23 渲染成功；23/23 k3diff、SSIM、结构指标产物完整。
- 指标最差项集中在 `vfx` / `inspector` / `claude-design-send` / `figma`，但并排目视确认它们是噪点、半调、渐变、采样或参考侧运动模糊，形态与位置一致，不投入。
- 按用户口径「找形态/位置差异大的 example」，本轮优先队列是：
  1. `spacex-launch`：cqw 容器单位导致字号/内距 1.33× 偏差；
  2. `HF-heygen-stripe`：3D 叠字 `mask-image` 边缘渐隐缺失；
  3. `timeline-launch`：聊天瞬态内容形态差，观察项；
  4. `texture-launch-video`：底部小参数标签多余，低优先。
- 不把 `avg_p8` 或 SSIM 数值当作视觉验收；验收以并排图为准。

## 7. 产物索引

- `out/vulkan-sweep/manifest.txt`：23 例输入与 swgl 参考。
- `out/vulkan-sweep/summary.json` / `summary.txt`：本轮聚合榜。
- `out/vulkan-sweep/structure_probe.json`：12 帧均匀采样的 d4/SSIM/shape/ncc/shift 探针。
- `out/vulkan-sweep/videos/*.mp4`：硬件 Vulkan 渲染视频。
- `out/vulkan-sweep/logs/*.render.log`：profile、编码、guard、RSS。
- `out/vulkan-sweep/k3diff/<slug>/`：逐帧像素指标。
- `out/vulkan-sweep/align/<slug>/`：SSIM 与 align_board 指标。
- `out/vulkan-sweep/contact/`：23 例 8 帧并排图；`ALL-sheets.png` 为总览。
- `out/vulkan-sweep/worst_frames/<slug>.png`：最差 p8 帧与均匀采样并排图。
- `out/vulkan-sweep/review/<slug>.png`：计划验收帧与本轮极值帧并排图。

## 8. 2026-11-15 收束修复与最终全量回归

### 8.1 架构收束

本轮只补 core 的标准 HTML/CSS/DOM 能力。hyperframes 的 `data-*` 与组合语义留在 engine 适配层。core 不写 example 特例。

时间模型保持不变：`frame = f(t)` 决定每帧静态结构；`animate = g(t)` 决定脚本改写。修复都是确定性纯函数。随机数、时钟、参考帧特判不进入 core。

新增或收束的标准能力：

1. `container-type` 与 `cqw`/`cqh`：尺寸查询容器按元素真盒传播；`vw`/`vh`/`vmin`/`vmax`/`rem`/`em`/`pt` 等 CSS 文本单位走统一换算。
2. `mask-image`：子树合成后按渐变 alpha 以 `DstIn` 罩回。遮罩矩形在 3-D 空间切换前压栈，避免投影矩阵把渐变挪出元素盒。
3. `getBoundingClientRect`、同步布局探针、动态文本样式：脚本隐藏测量节点按真实 CSS 字体栈与字号求值；`textContent` 在提取期写 store，测量文本注册到布局探针。
4. `transform-style: preserve-3d` 与 `backface-visibility: hidden`：3-D 子树按累计矩阵投影绘制，背向节点在 emit 任何 DrawOp 前跳过。

`backface-visibility` 的判定只用旋转链。`translateZ` 移动面但不改变正面法向，因此不能把完整 `local_m4` 的 `m[10]` 当作朝向。core 单独保存 `rotation_m4`，与当前 3-D 空间的累计矩阵相乘后取法向 z 分量。

标准属性链路：`css/to_node_style.rs` → `NodeStyle` → `ComputedVisualStyle` → `DisplayTransform` → `render/dispatch.rs`。语义、绘制与子树缓存指纹都包含该字段。

### 8.2 两例大形态差异收口

| slug | 定位阶段症状 | 修复 | 最终目视 |
|---|---|---|---|
| `spacex-launch` | f985/f1015 的 `$1.0T`、标签与字幕 pill 约 1.33× 偏大 | 嵌套 `container-type` 与 CSS 文本单位换算 | 并排图 `out/fix12/contact/spacex-launch.png`，f985/f1015 形态、位置、字号一致 |
| `HF-heygen-stripe` | f829 的 `.drum` 右偏、渐变淡出缺失、背面词条镜像叠字 | `.drum` 测量宽度、`mask-image`、`backface-visibility: hidden` | 并排图 `out/fix12/contact/HF-heygen-stripe.png`，f166/f331/f497/f829 一致；f1160 两侧片尾空帧一致 |

`HF-heygen-stripe` f829 单帧验收：

- 参考：`out/fix12/single/HF-heygen-stripe-f829-ref-crop.png`。
- 修复后：`out/fix12/single/probe829-fix19-crop.png`。
- 目视结果：中心 `discovers` 无镜像叠字；上下词条按相同渐变淡出；`The agent` 与字幕位置一致。

`spacex-launch` 残余 UI 描边/字体栅格微差不作为形态差异。

### 8.3 最终全量硬件 Vulkan + profile 复跑

归档旧产物到 `out/vulkan-sweep/archive-20261115-164051/` 后，全量重跑。二进制为 `target/release/opencat`，带 `--features profile`。

硬件环境：

```text
LD_PRELOAD=/home/solaren/Projects/opencat/tools/angle-linux/libvulkan.so.1
VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/radeon_icd.json
OC_SKIA_BACKEND=vulkan
OC_ANGLE_DIR=/home/solaren/Projects/opencat/tools/angle-linux
OC_GPU_PIPELINE=1
```

命令：`JOBS=6 /home/solaren/miniforge3/bin/python3 out/vulkan-sweep/scripts/run_sweep_parallel.py`。日志：`out/vulkan-sweep/sweep-parallel-final.log`。

| 项 | 结果 |
|---|---|
| manifest 例数 | 23 |
| 渲染 `rc=0` | 23 |
| `Render profile:` 日志 | 23 |
| k3diff `summary.json` | 23 |
| ffmpeg `ssim_stats.txt` | 23 |
| align `align.json` | 23 |
| `RENDER/COMPARE/STRUCT ALL OK` | 全部 OK |

完整聚合榜：`out/vulkan-sweep/summary.json`；可读表：`out/vulkan-sweep/summary-final.txt`。

与归档产物逐例对比后，23 例的 `avg_p8`、`ffmpeg_ssim`、`shape`、`shift_max`、`local_max` 没有形态级回归。少量噪点/混沌背景样例有 ±0.001 级指标波动，符合「不追噪点」口径。A 级 13 例无新增大位移；`HF-heygen-stripe` 与 `spacex-launch` 的指标与两例修复后的定点复跑一致。

两例最终聚合：

| slug | frames | avg_p8 | ffmpeg SSIM | shape | shift_max | local_max | verdict |
|---|---:|---:|---:|---:|---:|---:|---|
| `HF-heygen-stripe` | 1161 | 0.0413 | 0.9770 | 0.9869 | 8 | 54 | 数值仍判 FAIL，主要来自背景/视频相位；f829 目视已对齐 |
| `spacex-launch` | 1185 | 0.0232 | 0.9817 | 1.0000 | 56 | 192 | 数值仍判 FAIL，`local_max` 来自平坦背景局部相关；关键帧目视已对齐 |

指标只用于定位。并按排目视是验收依据，不为压 `avg_p8` 改渲染路径。

### 8.4 剩余队列

- `timeline-launch`：f371/f431 聊天回放瞬态内容差，继续观察，不投入。
- `vfx-heygen-combined`、`inspector-launch`、`claude-design-send-hyperframes-launch`、`figma-launch`：噪点、半调、渐变、采样或参考侧模糊，形态一致，不投入。
- `HF-heygen-stripe` f1160 与 `spacex-launch` 残余片尾/白场相位：片尾空帧或瞬态，不作为主要形态差异。

### 8.5 `display:none` 静态隐藏收束

`texture-launch-video` 的底部小标签来自 `.texture-meta` 的样式表
`display: none`。参考 HTML 把三个 `.meta-pill` 声明为隐藏；脚本没有把
`.texture-meta` 改回可见。opencat 旧版把 `display:none` 写进
`taffy::Style`，但 resolve 仍解析整棵子树并进入显示树，因此白药丸可见。

这是 core 的标准 CSS 能力缺口，不是 hyperframes 特例。修复在
`resolve_node` 收束：样式表烤出的 `css_layout` 是 `Display::None` 且没有脚本
`display:block` 解锁时，整棵子树退出布局与绘制。脚本显式
`display:block` 仍按内联优先解锁。`#domain-warp-canvas` 的
`set(display:block)` 路径保持有效。离屏 canvas 命令由脚本 store 直接
光栅化，不依赖显示树；`vfx-heygen-combined` 的 `CanvasTexture` 纹理源
不受影响。

证据：

- f38 裁剪：修复前 `out/texture-tag-probe/f38-test-crop.png` 有三枚白药丸；
  参考 `out/texture-tag-probe/f38-ref-crop.png` 没有。
- 修复后 f38 裁剪：`out/texture-tag-probe/f38-ref-fixed-crop2.png` 与
  `out/texture-tag-probe/f38-test-fixed2-crop.png` 目视一致，白药丸消失。
- 干净 release 二进制重渲 `texture-launch-video`：`rc=0`。

归档旧 sweep 到 `out/vulkan-sweep/archive-20261115-display-none/` 后，按同一
硬件 Vulkan + profile 配置再跑 23 例。日志
`out/vulkan-sweep/sweep-parallel-display-none.log` 为
`RENDER/COMPARE/STRUCT ALL OK`。

| 项 | 结果 |
|---|---|
| 渲染 `rc=0` | 23 |
| `Render profile:` 日志 | 23 |
| k3diff `summary.json` | 23 |
| ffmpeg `ssim_stats.txt` | 23 |
| align `align.json` | 23 |

与归档逐例对比：`shape`、`shift_max`、`local_max` 均不变；`texture-launch-video`
的 `avg_p8` 从 0.068098 降到 0.067439，ffmpeg SSIM 从 0.961088 升到
0.961513。`pr-to-video-launch` 与 `frame-md-launch-storyboard` 有 ±0.001 内
的编码/采样波动；其余指标不变。没有形态级回归。

### 8.6 代码验证

- `cargo check -p opencat-core --features profile`：0 error。
- `cargo build --release --bin opencat --features profile`：0 error。
- `git diff --check`：通过。
- 残留扫描无 `mask_local_space`、`let _ = mask_local_space`、`whiten_shader_colors` 或本轮新增的 `OC_DBG_BACKFACE`。
- `cargo fmt --check` 在 `crates/opencat/src/bin/opencat/render.rs`、`view.rs` 报既有格式差；本轮未改这些文件，未执行全仓格式化。
