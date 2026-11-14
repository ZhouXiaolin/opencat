# @omp：OpenMP 式循环标注指南（正式子集）

> 目标读者：手上有一个渲染慢的 html 语料，想在不改动画面行为的前提下加速。
> 一个标注 + 一次验证。inspector 实测全片 6.1min → 3min04s（SoA 时代读数；
> 现行 isolate 团队后端读数见 §8）。标注语法 = **正式 OpenMP 子集**
> （规范对齐 OpenMP TR15 6.1 draft，见 docs/js-openmp.md）；实现不必
> 完整，但写法严格对齐规范、语义精确映射，零发明。

## 1. 原理

页面热循环（如逐像素 halftone）是 V8 单线程数学墙：每帧几十万次超越函数，
JIT 也只能串行跑。@omp 把这个循环搬出单线程：

1. 提取期：引擎读脚本时用 oxc AST 认出标注行，做 capture 分析，把循环
   嵌套打包成行函数 bundle，原循环包进降级守卫。
2. 渲染期：每帧把行派发给后端并行执行，产出按行序应用回主实例（ctx 命
   令流直推、数组写回按行序落、reduction 固定序折叠）。
3. 任何一步不成功（语法超面/运行失败/环境关闭），守卫短路，**原循环照跑**
   ——与「pragma 未被编译器处理」同语义。Chrome 直跑该 html 时标注是零
   语义注释，画面行为不变。

后端三栈（分流在 §3.3/§3.5）：**isolate 团队**（通用 JS 语义，主后端）、
**std::simd lane kernel**（simd 构造的正统实现面，feature 门控）、
**opcode/SoA**（ctx 循环的兜底批算面，保留自旧架构）。

逐位对齐保证：f64 运算 + libm（musl fdlibm 血统，与 V8 的 ieee754.cc
同源）超越函数 + RGBA8 量化吸收 1-2 ulp 连续差。验收即「与不标注时逐位
一致」（§6）。

## 2. 快速开始

在热点 `for` 的**上一行**加标注。OpenMP 缺省 `default(shared)`：未列出的
外层变量隐式共享，最小标注就是 bare 一行：

```js
// @omp parallel for
for (let y = -spacing; y < h + spacing; y += spacing) {
  for (let x = -spacing; x < w + spacing; x += spacing) {
    const noise = random();                                  // 零参裸调用 → PRNG 采集
    ctx.fillStyle = `rgba(${r | 0}, ${g | 0}, ${b | 0}, ${a.toFixed(3)})`;
    ctx.beginPath();
    ctx.arc(x, y, size, 0, Math.PI * 2);                     // 满圈 arc + fill → 批量 oval
    ctx.fill();
  }
}
```

同构于 smallpt 原文 `#pragma omp parallel for schedule(dynamic, 1)
private(r)` 的完整形态（inspector 语料同款）：

```js
// @omp parallel for schedule(dynamic, 1) shared(s, mass, w, h, random, ctx)
for (let y = 0; y < h; y += 1) { ... }
```

确认走了原生腿：

```
OC_OMP_TRACE=1 opencat render index.html out.mp4
# 期望 stderr 出现任一后端的接管行：
#   [omp] isolate take wall: ... N rows × M writes   （isolate 团队，默认主后端）
#   [omp] simd take wall: ... N rows (B batches × 8) × M writes（simd 构造 + feature on）
#   [omp] took over: N rows, M ovals                 （SoA 兜底）
# 且没有 skipped / failed / degraded / poisoned 行
```

## 3. 标注语法（正式 OpenMP 子集）

形态：`// @omp parallel for [clause[ [,] clause] ...]`，独立一行，紧贴
`for` 上一行。子句间空白或可选逗号分隔。标注哪个 `for`，就旁路以它为
最外层的**整个循环嵌套**（OpenMP combined parallel for 语义）。

### 3.1 已实现子句

| 子句 | 语义 | 实现映射 |
|---|---|---|
| `shared(list)` | 外层共享变量 | capture：逐帧解码传原生腿（ctx/prng 同样写在这里，自动分类） |
| `firstprivate(list)` | 私有副本，初值取外层 | 同 shared（原生腿逐行 `globals.clone()`，天然 firstprivate） |
| `private(list)` | 未初始化私有副本 | 主帧局部槽（读未写 = undefined）；行并行下每行一份 |
| `default(shared)` | 未列出者隐式共享（OpenMP 缺省） | 自由变量分析自动补全 capture 名单 |
| `default(none)` | 必须显式列出每个引用 | 自由标识符 ⊆ shared ∪ firstprivate ∪ private，否则整条不处理 |
| `schedule(static)` | 等分禁窃取 | rayon `with_max_len(rows/threads)`——均匀行负载最优 |
| `schedule(dynamic[, chunk])` | 共享队列按 chunk 拉取 | chunk>1 → rayon `with_min_len(chunk)`；chunk=1 → rayon 缺省（work-stealing 即 dynamic,1 形态） |
| `schedule(guided/auto/runtime)` | 自适应/运行时决定 | rayon 缺省派发 |
| `collapse(n)` | 合并 n 层循环为并行空间 | 校验 n ≤ 巢深；并行维度仍为最外层行（调度差异，结果无关） |
| `num_threads(n)` | 线程数请求 | 记入 ir；OpenMP 语义为实现可少给（rayon 全局池决定） |
| `reduction(op: list)` | 私有副本按 combiner 合并 | op ∈ `+` `*`（f64 面）：行函数收 `let acc = <initializer>`（`+`→0、`*`→1），行末收 acc，host 按行序左折叠。语义与收紧声明见 docs/js-openmp.md §3.4。仅 isolate 后端支持（simd 栈拒绝，回落 isolate） |

调度不影响字节（indexed 保序）。选型参考：inspector（每行点数不均）
实测 static 比 dynamic,1 慢约 1.5%（omp 段 87.0 vs 85.7 ms/帧）——行负载
不均时保持 dynamic,1 或缺省。

### 3.2 认识但不支持（整条标注不处理 = 原循环照跑）

`lastprivate`、`linear`、`ordered`、`if(...)`、`nowait`、`allocate`、
`order`、`proc_bind`、`copyin`、schedule 修饰符（`monotonic:` 等）、
`default(private|firstprivate)`（5.1+）。未知子句同样整条不处理——
「pragma 未被编译器处理」降级语义，绝不猜。

### 3.3 数据共享规则（OpenMP 缺省语义）

- **未列出的外层变量隐式 shared**（`default(shared)`）：提取期对循环体
  + 页面函数传递闭包做自由变量分析，自动并入 capture 名单。
- **循环迭代变量预定 private**：各循环头变量自动私有。
- **名字自动分类，无需声明类别**：
  - **零参裸调用名**（`random()`）→ PRNG capture：骨架按原序预采每次
    调用的返回值，原生腿按序消费——页面 PRNG 原样跑，不重实现。
  - **ctx**（canvas 上下文：循环体内成员调用接收者 `ctx.arc(...)`，或
    `ctx.fillStyle =` 赋值目标）→ ctx capture：op 流的落点。成员**读**
    （`s.phase`）不算 ctx。
  - 其余 → plain 数值/对象/数组，逐帧解码。
- 无 ctx capture 的标注不再在变换期拒绝：守卫照发、omp_prep 分流
  **isolate 团队后端**（见 §3.4）——完整 JS 语义（递归、任意控制流、
  对象、行内状态数组）直接加速。
- **isolate 团队对所有构造优先**（S5 起含 ctx 垫片面）：有 ctx capture
  的循环同样先试 isolate（worker 垫片记录 ctx 调用、行产出直推命令流，
  V8 JIT 行函数快于 SoA op 解释）；bundle 结构超面才回 SoA 兜底（§4.3）。
- 带 `simd` 构造且引擎以 `simd` feature 构建时，**lane kernel 栈最先
  分流**（§3.5），拒绝则落 isolate——语义等价，只慢不少错。

### 3.4 isolate 团队后端（parallel for 的通用主后端）

循环由常驻 worker 团队执行：每线程一个独立 V8 isolate，bundle（循环体
+页面函数闭包）按 ir key 缓存、多帧复用；数据=每帧 copy-in（plain 值
JSON）+ 行末 copy-out（Set/Acc 四元组 + ctx op 缓冲 + reduction 终值）。
语义与 JS 本尊逐位一致：

- **递归/完整 JS 可用**（smallpt 的递归 radiance 是验收案，不再是
  §4.2 拒绝案——该拒绝只属于 SoA 编译面）。
- **共享数组写回（Set/Acc）**：`c[i]=v`/`c[i]+=v` 文本级重写为记录
  调用，行私有缓冲、行末按槽终值 flush，host 按行序应用。协议细节
  （帧初值 0 约束、浮点 copy-in 逐位无损合同）见 docs/js-openmp.md
  §6.2/§6.4——`serde_json` 必须 `float_roundtrip`（默认解析可错
  1 ulp，smallpt 实锤）。
- **ctx 垫片**：worker 侧 recorder 记录 fillStyle/beginPath/arc/fill
  为数值段（`|0`/`toFixed` 走 JS 原生语义），行产出直推
  `FillOvalBatch`；垫片实例每 worker 各自一份，**fillStyle 不跨行
  保持**（循环体不得依赖上一行设置的样式残留）。
- **reduction**：行函数首注入 `let acc = <initializer>`，行末产出终
  值，host 固定行序左折叠（docs/js-openmp.md §3.4）。
- **行内 PRNG 状态**（erand48 风格）是普通行私有变量，无需预采。
- schedule 映射：static=连续区间、dynamic(chunk)=atomic 领行；产出
  按行号槽位收集，调度不改变字节。

### 3.5 std::simd lane kernel 栈（simd 构造的正统实现面）

标注带 `simd` 构造（`parallel for simd` / `for simd`）且引擎以
`simd` feature 构建（本机 nightly；构建：`cargo build --release -p
opencat --features opencat-engine/simd`）时，omp_prep 最先试 lane
kernel：外层迭代 8 个一组打包 `f64x8`（尾批 NaN 补位 + active 面抑制），
内层循环所有 lane 同步走、per-lane break/continue 面，批间 rayon 保序。

- **白名单**（超面即拒绝回落 isolate，不毒化主缓存）：`let/const`、
  `if/else`、同形嵌套 `for`、`break/continue`；算术 `+ - * / %`、比较
  `< <= > >= == !=`、`&&`/`||`/`!`/`?:`、赋值 `= += -= *= /= %=`；
  `Math.` 的 `abs min max floor ceil round sqrt hypot sin cos exp log
  atan atan2 pow`；`__ocOmpSet/__ocOmpAcc` 数组写。**拒绝**：位运算、
  `===`/`!==`、页面函数调用、PRNG 采集、数组读、reduction、ctx capture。
- **布尔面约定**：比较与 truthy 产出统一归一为 {0.0, 1.0}
  （`mask.select(splat(1), splat(0))`——`Mask::cast()` 的 true=-1.0
  会污染 `1-x`、乘积链等布尔算术，对拍实锤）。
- **已知近似**（docs/js-openmp.md §4.1）：`&&`/`||` 双臂都算（无短
  路）、`Math.min/max` 不分辨 ±0。超越函数逐 lane 调 Rust libm：lane
  域与标量域逐位一致；与 JS 腿（V8 自己的实现）是否逐位一致**未经
  语料验证**（omp-simd 语料只含加减乘除与比较）。
- **验证**：`OC_OMP_VERIFY=1` 时每批与标量逐点重算对拍（按
  `(channel, idx, acc, bits)` 排序逐位比）；失败即 poison 本循环、
  后续帧落 isolate。
- 数组写通道面向 TypedArray；下标负/非有限/越界 = 该写静默丢弃
  （TypedArray 语义）。

## 4. 循环体写法约束

三层白名单，超面即降级（不会错，只会慢）。**目标是满足第三层（SoA）**。

### 4.1 解析面（不满足 = 完全不加速，原循环照跑）

- 语句：`let/const` 声明、表达式、`if/else`、同形 `for` 头
  `for (let v = e; v < bound; v += step)`（step 亦收 `++`/`--`/`-=`）。
- 表达式：数值/字符串/布尔/null、算术、比较、位运算、`&&`/`||`、
  `?:`、一元 `-`/`!`/`~`、数组字面量、对象字面量、模板串、成员/下标读、
  赋值（`= += -= *= /= %=`）、`Math.<白名单>` 调用、`x.toFixed(d)`
  （d 必须是字面量）。
- `Math` 白名单：`sin cos exp hypot floor max min abs sqrt round`，另收
  `Math.imul(a, b)`。
- 超面即解析失败：`while`/`break`/`continue`、箭头函数、`new`、
  `try/catch`、解构、展开、标签语句等。

### 4.2 编译面（结构性保证）

- 循环体引用的一切外层名字必须在 capture 名单里（显式子句或隐式分析
  命中）；头部只认 capture + 循环变量。
- **内层 `for` 头不得引用外层循环变量或 capture**（保证每行采集数一致）。
- 条件分支嵌套（`if` 臂 / 三元臂 / `&&`/`||` 右侧）里不得有 PRNG 采集
  （短路求值会让行间采集数不齐）。
- 被循环体调用的辅助函数必须是本脚本内可见的 **`function name(` 声明**
  （箭头函数赋值不抽取）；它们引用的函数同样要声明式可见（传递闭包）。
- 递归调用拒绝（**仅 SoA 兜底编译面**；isolate 团队是主后端，递归可
  跑——smallpt 即验收案）。

### 4.3 SoA 向量面（满足 = 快腿；不满足 → 标量 VM 兜底）

SoA 把内层循环体当 per-point shader 批算。要求：

- 内层体是直线代码（无 `if`/无嵌套循环/无 `!`/无赋值进数组或对象成员）。
  页面**函数内**允许 `for`/`if`，条件须是行常量（对所有点同值，如
  fbm 的固定 octave 循环）；条件含向量值 = JS 会逐点分支 = 拒绝。
- canvas 五连**恰好一组**（内层体只许一个 `fillStyle` 赋值）：
  `fillStyle = \`rgba(...)\`` → `beginPath()` → `arc(5 参)` → `fill()`；
  rgba 模板每段是数值/常量/`toFixed` 产物；arc 必须满圈
  （`a1 - a0 ≈ 2π`）。
- 数组只读：行常量数组按下标读（含 `colors[colorIndex]` 每点选行、
  `mix(...)` 返回数组再 `[0]` 取分量这类两段读）；不写数组/对象。

标量 VM 兜底仍行并行，但逐点解释的开销通常**高于** V8 本尊（inspector
实测 28min vs 6.1min）——所以 prepare 提示行出现时要回头改写循环体，
不要停在 VM 兜底。

## 5. 改造手法：把现有循环搬进白名单

按命中率排序：

1. **只标最外层热点 for**。先用 `OC_FRAME_TIMING=1` 确认 script 段是
   大头、找到那个函数；标注嵌套的最外层。
2. **把 `if` 提出内层体**。`if (cond) { draw(x) }` 改成数学参与：透明度
   乘 0（`a * cond`），半径乘 0（画成不可见的点）——默认 source-over
   合成下 alpha 0 不留痕，画面等价且保持直线体。若分支只影响少数点且
   不便改写，接受 VM 兜底或不标注该循环。
3. **非满圈 arc 不在批量面**。`arc(x, y, r, 0, Math.PI)`（半圆）会被
   SoA 拒绝（`non-full-circle arc unsupported in facet`）。这类循环
   不标注。
4. **数组只写不读的局部累积**（`arr[i] = v`）搬出内层体，或改成函数
   返回值。
5. **DOM/DOM 读/`Date`/`Math.random`（全局）出循环**。循环体必须纯：
   只碰 capture 名单里的名字、局部变量、Math 白名单。帧间随机走页面
   PRNG 名（零参裸调用）。
6. **`toFixed` 只用于 rgba 的 alpha 段**这类串化出口；别在数学里串化
   再 parse 回来。
7. 函数抽取失败（辅助函数是箭头函数/在别的 `<script>` 块）：改成
   `function` 声明放在同一脚本块，或直接内联进循环体。

改造后画面必须逐位不变——上面的手法都是恒等改写，用 §6 的对拍确认。

## 6. 验证流程（改完必跑）

```bash
# ① 原生腿确认：有 took over 行，无 soa prepare skipped / take failed
OC_OMP_TRACE=1 opencat render index.html out.mp4

# ② 帧级对拍：native 腿 vs JS 腿（OC_OMP_DISABLE=1），抽**可见动画帧**
OC_OMP_DISABLE=1 opencat render index.html ref.png 300
opencat render index.html nat.png 300          # md5 应一致
```

注意：**不要拿 f0 做验证帧**——首帧常被开场遮罩盖住，坏掉的加速腿也能
「对上」。选循环可见的动画中段帧。

```bash
# ③ seg 对拍：两段 30 帧窗口（一段静止附近 + 一段动画中段）PNG 直出 cmp
opencat render index.html segA/ --frames 0:30        # native 腿
OC_OMP_DISABLE=1 opencat render index.html segB/ --frames 0:30   # JS 腿
diff -rq segA segB                                    # 无输出 = 逐字节一致

# ④ 全片回归：改造前后全片 mp4 互 diff（k3diff），p8 应在编码底噪量级
```

## 7. 排查表

| 日志行 | 含义 | 动作 |
|---|---|---|
| `[omp] took over: N rows, M ovals` | SoA 腿接手 | 无 |
| `[omp] soa prepare skipped, scalar vm backend: <原因>` | 形状超 SoA 面，走标量 VM（通常更慢） | 按 `<原因>` 回到 §4.3 改写 |
| `[omp] take failed, poisoned: <原因>` | 运行期失败，**永久**降级 JS 腿 | 按 `<原因>` 修；如 `non-full-circle arc`、`rgba alpha parse failed`、`divergent control flow: vector condition` |
| `[omp] compile failed, degraded: <原因>` | 解析/编译超面 | 按 `<原因>` 回到 §4.1/4.2 改写；无 ctx 循环超面时改走 isolate 后端（§3.4），不必改写 |
| `[omp] isolate bundle build failed, degraded: <原因>` | 循环形态超 isolate 子集（docs/js-openmp.md §5） | 按 `<原因>` 改写 |
| `[omp] isolate take wall: ... N rows × M writes` | isolate 后端接手（性能读数） | 无 |
| `[omp] simd take wall: ... N rows (M batches × 8) × K writes` | simd lane 栈接手（性能读数） | 无 |
| `[omp] simd kernel skipped, isolate backend: <原因>` | simd 构造超 lane 白名单，落 isolate（语义等价，只慢不错） | 无需动作；追求 simd 提速时按 `<原因>` 改写（docs/js-openmp.md §4.1） |
| `[omp] simd take failed, isolate fallback next frames: <原因>` | lane 执行期失败（含 VERIFY 不符），poison 本循环落 isolate | 按 `<原因>` 查；`OC_OMP_VERIFY=1` 可复现逐位点 |
| `[omp] take failed, poisoned: <原因>`（isolate 行错误） | worker 内 JS 异常（如引用 window/DOM）→ **永久**降级 JS 腿 | 循环体内禁引用页面全局（window/document）——它们不在 worker isolate 里 |
| 完全没有 omp 行 | 标注没被认出 | 检查标注行格式/子句是否在 §3.2 不支持名单（§3.2 子句 = 整条静默不处理） |

## 8. 适用判据与预期

- **收益大**：每点重数学（几十次超越函数/位域混合）× 点数上万 × script
  段占比高。inspector：77,836 点/帧 × 48 次 hash2，omp 段 V8 ~200ms →
  isolate 垫片 ~30ms/帧（2026-11-14 全片 1355 帧均值 29.95ms，全片
  WALL 155s 单次读数；历史 SoA 栈 ~70ms/帧）。
- **收益小**：循环体轻（几次乘加）、点数少、瓶颈在 GPU/编码段——不值得
  标注。
- 上限：omp 段 ≈ 点数 × 每点运算量 ÷ 核数；行并行保序，行间零共享。
- 未标注语料零影响（omp 代码惰性）；标注本身在浏览器里是注释，交付的
  html 不需要条件分支。
- **通用性锚点 = examples/smallpt**：smallpt（Kevin Beason 2008）JS 直译，
  标注与 C 原文 pragma 同构（`schedule(dynamic, 1) private(r)`），递归
  radiance + 逐位精确 erand48。由 isolate 团队后端（§3.4）执行：f120
  双腿 md5 一致、seg 两窗逐位一致；同机 ABAB 循环本体 12.6s → take
  wall 均值 3.61s/帧（8 物理核），帧总墙钟 26.2s → 15.7s/帧。
- **simd 栈锚点 = examples/omp-simd**：曼德博 192×192，`// @omp parallel
  for simd`，三腿（JS / isolate / simd lane）seg 0:30 逐位一致（md5
  e1b58f6d…），`OC_OMP_VERIFY=1` 腿无 poison。循环体只含加减乘除与
  比较——超越函数的 simd/JS 双腿一致性不在此语料覆盖内。

## 9. 实现索引

- `crates/opencat-engine/src/script/omp/`：transform（oxc 前端：标注
  定位+capture 分析+守卫/骨架/RowBundle 发射）、parse/compile（子集
  解析器+opcode 面，ctx 循环兜底链）、vm_soa（SoA 执行器）、vm（标量
  兜底）、mod（prep/take 生命周期+三栈分流+行产出应用）、protocol
  （isolate 团队引擎无关协议+bundle 模板）、team（常驻线程团队+行派发
  收集）、v8_team（V8 worker 适配层；quickjs/Hermes 走 feature 桩
  js-quickjs/js-hermes）、simd_kernel（feature `simd` 门控的 lane
  kernel 编译器，`#![feature(portable_simd)]`，仅 nightly）。
- 分流优先级：simd lane 栈（simd 构造 + feature on）→ isolate 团队
  （全部构造，S5 起含 ctx 垫片）→ opcode/SoA（ctx 循环兜底）→ JS 腿。
- CI（stable）：跑 default features——simd 栈整体在 `#[cfg(feature =
  "simd")]` 内，`portable_simd` attr 也是 cfg 门控，stable 编译不触及。
- 数学函数对拍台：`opencat-omp-verify`（配合 `OC_OMP_VERIFY` 采样）。
