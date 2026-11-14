# hyperframes-launches 语料对齐计划

> 2026-11-14 重写。旧版过程记录（vfx 直渲计划、Vulkan 批次、五项修复等）
> 见 git 历史（4d3ba8ac 及此前）。本文件只保留：口径、现状、缺陷清单、
> 流程纪律。以当前工作树与本轮实测为准，不引用旧 avg_p8 基线。

## 一、目标与验收口径

目标：opencat（硬件 Vulkan，`render_one.sh` 配置）渲染
`~/Projects/hyperframes-launches` 全部 23 个带 `*swgl*.mp4` 参考
的 example，与参考视觉一致。

判定优先级（从高到低）：

1. **有 vs 无**：整段场景、文字、图标、贴纸、按钮缺失或多余——必须修。
2. **形态与位置**：卡片几何、版式结构、竖排/弧形文字、堆叠布局——必须修。
3. **内容正确性**：文案、字标、标签色、列表标记、空格——必须修。
4. **动画与相位**：打字/回放序列相位、平移相位差、次要图标形态——酌情修。
5. **噪点不优先**：噪声种子、颗粒、AA、字体栅格、混沌/渐变背景造成的
   指标劣化（如 vfx 0.164 / inspector 0.146 / figma 0.119 的高 avg_p8）
   不是对齐问题，**不为压指标改渲染路径**。

指标（`avg_p8`/`mae`/ffmpeg `ssim`/结构 `d4`/`ncc`/`shift`）只作定位辅助，
**验收以 `walk_sheet.py` 并排目视为准**。

## 二、现状（2026-11-14，run4）

23 例全部 `rc=0`。聚合榜 `out/vulkan-sweep/summary.json`、结构采样
`structure_probe.json`、并排图 `contact/`（`scripts/walk_sheet.py` 重建）。

**A 级——达标 13 例**（并排目视一致，残余为噪声/AA/微偏移，不再投入）：
`vfx-heygen-combined`、`inspector-launch`、`figma-launch`、
`hyperframes-launch`、`texture-launch-video`（纹理字已恢复，剩微偏移）、
`website-to-hyperframes`、`sfx-music-launch`、`variables-launch`、
`liquid-brand-refraction`、`heygen-apple-motion--hero/01/03/04`。

**B 级——有真实缺陷 10 例**，见下节。

已落地不可回退的修复（run4 验证）：绝对定位 definite-width 换行、
interleaveKids 真 DOM 序、svg 内部绘制注入（resolve 期）、svg face 的
style 写穿、@font-face 作用域分类去注释、markup 大小写保留、
canvas2d 文本一律字形 Path + DrawOp（绘制阶段禁止 surface 位图）。

## 三、B 级缺陷清单与修复顺序

每项：症状（帧号）→ 疑似方向 → 验收帧。修完一例即在「现状」销号，
并重跑该例 `compare_one.sh` + `walk_sheet.py` 目视确认。

### 1. frame-md-launch-storyboard（最大遗留）
- f258：黑卡几何偏移；多出 `## Style Prompt` 大标题；缺左侧竖排
  `Written for` 文字。
- f1231：文本块整体偏高；右上泄漏 `Frame 542 (composition id:
  storyboard)` 调试小字（参考无——查是渲染端 overlay 还是语料 DOM）。
- f1033/f1549：卡片略小（比例/边距）。
- 注：f1231 的 `## Hero Band` 参考同样有，是场景内容，非缺陷。
- 验收帧：258 / 516 / 1033 / 1231 / 1549。

### 2. codex-five-hour-limit-replica
- f0/f87：`ChatGPT`/`Codex` 字标缺失（只剩图形 logo）——查文字加载
  时序与字体。
- f261/f347：输入框打字文本缺失（`SPeak`、`…restriction for plUs`）。
- f521/f608：`End the five hour limit…` 贴纸缺失。
- f434 起：发送箭头（黑圆↑）缺失。
- 人物与卡片位置已一致。
- 验收帧：0 / 87 / 261 / 347 / 434 / 521 / 608。

### 3. spacex-launch
- f1015：`$1.0T` 估值卡空卡——先查相位（数字是否晚出现）再查缺失。
- f507：聊天列表项目符号圆点缺失（文字在，`list-style` 标记没画）。
- f338：Thinking 动画点图标缺失（次要）。
- 验收帧：338 / 507 / 577 / 848 / 1015。

### 4. HF-heygen-stripe
- f829：叠字动画——参考 `pays/discovers/provisions` 竖排三行渐隐，
  我们挤成一行互相叠盖成乱码（查行的换行/定位与 stagger 时序）。
- f497：终端命令打字相位（`stripe projects add heygen/api` 只打到
  `stripe`）——相位类，排后。
- 验收帧：166 / 331 / 497 / 829 / 1160。

### 5. k3-promo
- f70：选择框内 `Every token of context` 文字缺失。
- f493：片尾 `K3` 字标只剩图标（同类字标缺失，与 codex 合并排查）。
- 平移动画横向相位差（shift=43）为相位非形态，不单独立项。
- 验收帧：70 / 141 / 211 / 282 / 493。

### 6. cloud-render-launch
- f163：我们提前进入像素怪物白底场景（参考仍为暗色聊天窗）——场景
  切换时序。
- f814：w00 进度条参考青色已填充，我们空条（查进度宽度动画/颜色）。
- 细线与字重差为既有记录，酌情。
- 验收帧：163 / 325 / 496 / 814 / 1136。

### 7. heygen-apple-motion--02-bouncy-ui
- f156/f259/f311：通知/聊天气泡的头像圆缺失。
- f311：`+ Opportunity` 参考粉色、我们黑色（查颜色继承/变量）。
- f156：弧形文字间距偏宽。
- 验收帧：52 / 104 / 156 / 259 / 311。

### 8. pr-to-video-launch
- f899：片尾 `Create your PR video today` 中彩色 span 前后空格丢失
  （渲染成 `yourPR videotoday`）——疑似文本节点间空白合并，可能是
  通用缺陷，修在解析/序列化层并全语料回归。
- 验收帧：128 / 514 / 642 / 899。

### 9. claude-design-send-hyperframes-launch（遗留已降级为动画细节）
- f1287：均衡器条参考高低起伏动画，我们一排等高平条。
- f1056：连接器行 hover 高亮缺失。
- 曲线渐画细节、弹窗微位差。
- 验收帧：211 / 422 / 633 / 1056 / 1287。

### 10. timeline-launch（观察项，不立项）
- f431：聊天回放瞬态差（参考数值 pill vs 我们文本气泡）。若修 2/8 时
  顺手可查，否则搁置。

## 四、流程与工具

```bash
# 单例渲染（硬件 Vulkan 配置见脚本头注释）
out/vulkan-sweep/scripts/render_one.sh <slug>
# 单例对比（k3diff + SSIM，含 trim/shortest 修正）
out/vulkan-sweep/scripts/compare_one.sh <slug>
# 全量并排目视（23 例 × 8 帧，覆盖 contact/）
python3 out/vulkan-sweep/scripts/walk_sheet.py
# 批次完成后全量回归
out/vulkan-sweep/scripts/run_sweep.sh && out/vulkan-sweep/scripts/summarize.py
```

- 清单与参考路径：`out/vulkan-sweep/manifest.txt`。
- 单帧验证产物放 `out/vulkan-sweep/fix<N>/`，一例一目录，含并排 compare 图。
- 每轮修复结束把结论更新进「现状」小节，不重开过程记录。

## 五、纪律

1. 视觉一致即好；随机数、噪点、采样、AA 差异不投入。
2. 修「有 vs 无」优先于修「颜色与噪点」；先修共因（字标缺失、span
   空格、列表标记可能同源），再修单例。
3. 无效补丁立即撤销（如 `:not()` 探针），不保留无收益代码。
4. 2D 文本保持字形 Path 纪律；不在绘制阶段光栅化。
5. 不引用旧 avg_p8 基线；对比只用本轮产物。
6. 中文交流；`rg` 优先；不写不必要的代码。

## 六、完成定义

1. B 级 10 例按上表逐项销号，`walk_sheet.py` 目视达标；
2. 全量 `run_sweep.sh` 回归 `rc=0` 且 A 级 13 例指标无回归
   （±0.001 量级）；`summary.json` 重建；
3. 本文件「现状」更新为 23/23 达标。

## 七、@omp 标注语料分支（2026-11-14）

fork `ZhouXiaolin/hyperframes-launches`（upstream=heygen-com），本地
remote 已切：origin=自己 fork、upstream=公司仓；分支
`feat/omp-annotations`。

扫描 76 个候选循环后只标 1 处：`inspector-launch/index.html:2319` 半调
点阵网格（`// @omp parallel for`，每帧 ~225 行 × 77,836 点，isolate
垫片腿）。其余候选全是 GSAP 时间轴构建循环（一次性 DOM 操作）、GLSL
字符串或超 ctx 垫片面（canvas-close 的 fillText 逐格+lastFill 跨行残
留）——不标。

**ABAB 全片实测**（同机交替，vulkan 硬件管线；判别必须显式 `[ "$x" = off ]`
——`[ -n "$x" ]` 对 on/off 标签都为真曾导致四轮全 off 的假数据）：

| 例 | omp off | omp on | 加速 |
|---|---|---|---|
| inspector-launch | 159.3s（均值） | 52.9s（均值） | **3.01×** |
| k3-promo（试标后撤销） | 18.9s（均值） | 19.15s（均值） | ~1.0×（持平） |

k3 on 值是 copy-in/copy-out 三步优化后的数字。优化前（isolate 首版）
k3 on 为 36.3s（0.55× 负收益），根因是 dissolve 逐像素循环 ~45 活跃帧
每帧 copy-in `DIS.mask`/`dist`（各 2.6MB Float64Array）+ copy-out 60 万
次数组写。三步优化（引擎侧，对协议面通用）：

1. **行缓冲常驻**：bundle 里 Set/Acc 行缓冲按 (worker, 槽) 复用，行末
   只按 dirty list 清零（O(脏) 非 O(全长)）；行产出缓冲 grow-only +
   `subarray` 视图。k3 take wall 193ms → 105ms。
2. **TypedArray 二进制旁路**：plain capture 树里的 TypedArray 不再走
   JSON 文本（`{"0":…}` 逐元素串化），host `copy_contents` 一次 memcpy
   进 `FrameEnv.shared`（`SharedBuf`，raw 字节 + 元素类型；**不引入
   postcard/serde 编解码——载荷的最优编解码就是不解码**），args_json
   只留 `{"__ocOmpShared": {slot, len}}` 描述符；worker 装载时按描述符
   重建真 TypedArray（每 worker 字节克隆 + 独立 backing store，v1 快
   照语义）。walker 按 JSON.stringify 语义复刻，toJSON/Date/环回落
   JSON 老路。take wall 105ms → 6.5ms。
3. **写回直写**：Set/Acc 应用面按槽预解析——f64/U8Clamped TypedArray
   直接写 backing store（U8Clamped 复刻 ES ToUint8Clamp
   round-half-to-even），JS Array 与其余类型保留 V8 API 路径。apply
   21ms → 1.7ms。

优化后 k3 活跃帧开销 ~8ms/帧，与 8 线程并行收益相抵（~1.0×）。教训
维持：**短活跃窗 + 大 TypedArray 进出面的循环不标注**（标注无害但无
益）；k3 语料标注已撤销。inspector（ctx 垫片 ovals 通道，无 writes）
不受三步影响，3.01× 完整保持。

验证链（全部 step3 后二进制）：k3 f420:470 双腿 50/50 PNG 逐字节一致；
step3 直写面 f424:434 双腿 10/10 一致；quickjs feature 腿 `cargo check`
干净；inspector 无毒化、oval 数正常。

**k3 负收益教训**（首版实测，已由三步优化消解大半）：短活跃窗 +
copy-in/out 重负载的循环标注前必须 ABAB。

inspector 验证链全过：trace 无毒化、f620 双腿 md5 一致、seg 600:630
双腿逐字节一致、全片 k3diff avg_p8 0.1459（run4 0.1460，零回归）。
