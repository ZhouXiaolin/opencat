# @omp：给 html 语料提速的循环标注指南

> 目标读者：手上有一个渲染慢的 html 语料，想在不改动画面行为的前提下加速。
> 一个标注 + 一次验证。inspector 实测全片 6.1min → 3min04s（1.99x）。

## 1. 原理

页面热循环（如逐像素 halftone）是 V8 单线程数学墙：每帧几十万次超越函数，
JIT 也只能串行跑。@omp 把这个循环搬到原生：

1. 提取期：引擎读脚本时认出标注行，把整个循环嵌套编译成 f64 opcode 流，
   原循环包进降级守卫。
2. 渲染期：每帧把循环派发给 rayon 行并行执行（每行一次批量向量算），
   结果直推 canvas 命令流。
3. 任何一步不成功（语法超面/运行失败/环境关闭），守卫短路，**原循环照跑**
   ——与「pragma 未被编译器处理」同语义。Chrome 直跑该 html 时标注是零
   语义注释，画面行为不变。

逐位对齐保证：f64 运算 + libm（musl fdlibm 血统，与 V8 的 ieee754.cc
同源）超越函数 + RGBA8 量化吸收 1-2 ulp 连续差。验收即「与不标注时逐位
一致」（§6）。

## 2. 快速开始

在热点 `for` 的**上一行**加标注，`args()` 列出循环体引用的一切外层名字：

```js
// @omp parallel for args(w, h, spacing, radius, colors, random, ctx)
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

确认走了原生腿：

```
OC_OMP_TRACE=1 opencat render index.html out.mp4
# 期望 stderr 出现：[omp] took over: 244 rows, 77836 ovals
# 且没有 [omp] soa prepare skipped / take failed 行
```

## 3. 标注规范

- 形态：`// @omp parallel for args(名字, 名字, ...)`，独立一行，紧贴
  `for` 上一行。标注哪个 `for`，就旁路以它为最外层的**整个循环嵌套**。
- `args()` 必须列全循环体引用的外层名字（含页面函数内引用的）。漏列 =
  编译失败 = 静默降级（画面不变，只是不加速）。
- 名字自动分类，无需声明类别：
  - **零参裸调用名**（`random()`）→ PRNG capture：骨架按原序预采每次
    调用的返回值，原生腿按序消费——页面 PRNG 原样跑，不重实现。
  - **ctx**（canvas 上下文：作为成员调用接收者 `ctx.arc(...)`，或
    `ctx.fillStyle =` 赋值目标）→ ctx capture：op 流的落点。成员**读**
    （`s.phase`）不算 ctx。
  - 其余 → plain 数值/对象/数组，逐帧解码。
- 无 ctx capture 的标注直接拒绝（op 流没有落点）——纯数学循环派发暂
  不支持。

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

- 循环体引用的一切外层名字必须在 `args()` 里；头部只认 args + 循环变量。
- **内层 `for` 头不得引用外层循环变量或 capture**（保证每行采集数一致）。
- 条件分支嵌套（`if` 臂 / 三元臂 / `&&`/`||` 右侧）里不得有 PRNG 采集
  （短路求值会让行间采集数不齐）。
- 被循环体调用的辅助函数必须是本脚本内可见的 **`function name(` 声明**
  （箭头函数赋值不抽取）；它们引用的函数同样要声明式可见（传递闭包）。
- 递归调用拒绝。

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
   只碰 args 列出的名字、局部变量、Math 白名单。帧间随机走 args 里的
   页面 PRNG 名。
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
| `[omp] compile failed, degraded: <原因>` | 解析/编译超面 | 按 `<原因>` 回到 §4.1/4.2 改写 |
| `[omp] no ctx capture, degraded` | args 没有画布上下文名 | args 里补 ctx 名 |
| 完全没有 omp 行 | 标注没被认出 | 检查标注行格式/位置（§3） |

## 8. 适用判据与预期

- **收益大**：每点重数学（几十次超越函数/位域混合）× 点数上万 × script
  段占比高。inspector：77,836 点/帧 × 48 次 hash2，omp 段 ~200ms → ~70ms。
- **收益小**：循环体轻（几次乘加）、点数少、瓶颈在 GPU/编码段——不值得
  标注。
- 上限：omp 段 ≈ 点数 × 每点运算量 ÷ 核数；行并行保序，行间零共享。
- 未标注语料零影响（omp 代码惰性）；标注本身在浏览器里是注释，交付的
  html 不需要条件分支。

## 9. 实现索引

- `crates/opencat-engine/src/script/omp/`：transform（提取期文本变换）、
  parse（子集解析器）、compile（校验+opcode 编译）、vm_soa（SoA 执行器）、
  vm（标量兜底）、mod（rayon 派发）。
- 数学函数对拍台：`opencat-omp-verify`（配合 `OC_OMP_VERIFY` 采样）。
